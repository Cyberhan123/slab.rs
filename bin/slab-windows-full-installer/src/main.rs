mod bundle;
mod cab;
mod detect;
mod fsops;
mod ggml_manifest;
mod payload;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitCode};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use uuid::Uuid;

use crate::bundle::{AssetInput, AssetKind, EmbeddedBundle, load_embedded_bundle, write_embedded_bundle};
use crate::cab::{create_cab, expand_cab};
use crate::detect::detect_best_variant;
use crate::fsops::{apply_selected_payload, remove_dir_if_exists, write_json};
use crate::payload::{
    PackagedPayloadManifest, RequestedVariant, RuntimeVariant, build_runtime_payload_plan,
};

const PAYLOAD_MANIFEST_FILE_NAME: &str = "payload-manifest.json";
const SETUP_ASSET_NAME: &str = "setup.exe";

#[derive(Debug, Parser)]
#[command(author, version, about = "Slab Windows full installer bootstrap")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Pack(PackArgs),
    Run(RunArgs),
    Apply(ApplyArgs),
    DetectGpu,
}

#[derive(Debug, Args)]
struct PackArgs {
    #[arg(long)]
    nsis_installer: Option<PathBuf>,

    #[arg(long)]
    output: Option<PathBuf>,

    #[arg(long)]
    version: Option<String>,
}

#[derive(Debug, Args, Clone)]
struct RunArgs {
    #[arg(long, value_enum, default_value_t)]
    variant: RequestedVariant,

    #[arg(long)]
    keep_staging: bool,
}

#[derive(Debug, Args)]
struct ApplyArgs {
    #[arg(long)]
    source: PathBuf,

    #[arg(long)]
    dest: PathBuf,
}

fn main() -> ExitCode {
    match run_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run_main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Pack(args)) => run_pack(args),
        Some(Commands::Run(args)) => run_bootstrap(args),
        Some(Commands::Apply(args)) => {
            apply_selected_payload(&args.source, &args.dest)?;
            Ok(())
        }
        Some(Commands::DetectGpu) => {
            println!("{}", detect_best_variant()?.as_str());
            Ok(())
        }
        None => {
            let current_exe = env::current_exe().context("failed to locate current executable")?;
            let Some(_) = load_embedded_bundle(&current_exe)? else {
                bail!("no subcommand was provided and '{}' is not a bundled bootstrap executable", current_exe.display());
            };
            run_bootstrap(RunArgs {
                variant: RequestedVariant::Auto,
                keep_staging: false,
            })
        }
    }
}

fn run_pack(args: PackArgs) -> Result<()> {
    let workspace_root = workspace_root()?;
    let version = args
        .version
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    let output_path = args.output.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("release")
            .join("bundle")
            .join("nsis")
            .join("SlabFullInstaller.exe")
    });
    let nsis_installer = resolve_nsis_installer(&workspace_root, args.nsis_installer)?;

    let payload_plan = build_runtime_payload_plan(&workspace_root, &version)?;
    let staging_root = env::temp_dir().join(format!("slab-full-installer-pack-{}", Uuid::new_v4()));
    fs::create_dir_all(&staging_root)
        .with_context(|| format!("failed to create staging directory {}", staging_root.display()))?;

    let result = (|| -> Result<()> {
        let mut asset_inputs = Vec::new();
        let setup_staging_path = staging_root.join(SETUP_ASSET_NAME);
        fs::copy(&nsis_installer, &setup_staging_path).with_context(|| {
            format!(
                "failed to stage NSIS installer {} -> {}",
                nsis_installer.display(),
                setup_staging_path.display()
            )
        })?;
        asset_inputs.push(AssetInput {
            name: SETUP_ASSET_NAME.to_string(),
            kind: AssetKind::NsisInstaller,
            source_path: setup_staging_path,
        });

        for package in &payload_plan.packages {
            let cab_path = staging_root.join(package.variant.cab_name());
            create_cab(&cab_path, &package.files)?;
            asset_inputs.push(AssetInput {
                name: package.variant.cab_name().to_string(),
                kind: AssetKind::Cab,
                source_path: cab_path,
            });
        }

        let manifest_path = staging_root.join(PAYLOAD_MANIFEST_FILE_NAME);
        write_json(&manifest_path, &payload_plan.packaged_manifest)?;
        asset_inputs.push(AssetInput {
            name: PAYLOAD_MANIFEST_FILE_NAME.to_string(),
            kind: AssetKind::PayloadManifest,
            source_path: manifest_path,
        });

        let base_executable = env::current_exe().context("failed to locate pack executable")?;
        write_embedded_bundle(&base_executable, &version, &asset_inputs, &output_path)?;
        Ok(())
    })();

    let _ = remove_dir_if_exists(&staging_root);
    result
}

fn run_bootstrap(args: RunArgs) -> Result<()> {
    let current_exe = env::current_exe().context("failed to locate current executable")?;
    let bundle = load_embedded_bundle(&current_exe)?
        .ok_or_else(|| anyhow!("'{}' does not contain an embedded installer bundle", current_exe.display()))?;

    let resolved_variant = resolve_requested_variant(args.variant)?;
    let selected_packages = selected_packages(resolved_variant);
    let staging_root = env::temp_dir()
        .join("SlabFullInstaller")
        .join(format!("{}-{}", bundle.version(), Uuid::new_v4()));
    let setup_path = staging_root.join(SETUP_ASSET_NAME);
    let payload_root = staging_root.join("payload");
    let helper_path = staging_root.join("slab-windows-full-installer-helper.exe");

    fs::create_dir_all(&payload_root)
        .with_context(|| format!("failed to create staging payload directory {}", payload_root.display()))?;

    let result = (|| -> Result<()> {
        let packaged_manifest = extract_packaged_manifest(&bundle)?;
        bundle.extract_asset_to_path(SETUP_ASSET_NAME, &setup_path)?;

        for package in &selected_packages {
            let cab_name = package.cab_name();
            let cab_path = staging_root.join(cab_name);
            bundle.extract_asset_to_path(cab_name, &cab_path)?;
            expand_cab(&cab_path, &payload_root)?;
            fs::remove_file(&cab_path)
                .with_context(|| format!("failed to remove expanded CAB {}", cab_path.display()))?;
        }

        let selected_manifest = packaged_manifest.selected_for(&selected_packages)?;
        write_json(&payload_root.join(PAYLOAD_MANIFEST_FILE_NAME), &selected_manifest)?;
        bundle.write_base_executable_to_path(&helper_path)?;

        let status = ProcessCommand::new(&setup_path)
            .env("SLAB_INSTALLER_PAYLOAD_DIR", &payload_root)
            .env("SLAB_INSTALLER_HELPER_PATH", &helper_path)
            .status()
            .with_context(|| format!("failed to launch {}", setup_path.display()))?;

        if !status.success() {
            bail!(
                "NSIS installer '{}' exited with status {}",
                setup_path.display(),
                status
            );
        }

        Ok(())
    })();

    if args.keep_staging {
        return result;
    }

    let cleanup = remove_dir_if_exists(&staging_root);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Ok(()), Err(error)) => Err(error),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup_error)) => Err(error.context(cleanup_error.to_string())),
    }
}

fn resolve_requested_variant(requested: RequestedVariant) -> Result<RuntimeVariant> {
    match requested {
        RequestedVariant::Auto => detect_best_variant().or(Ok(RuntimeVariant::Base)),
        RequestedVariant::Base => Ok(RuntimeVariant::Base),
        RequestedVariant::Cuda => Ok(RuntimeVariant::Cuda),
        RequestedVariant::Hip => Ok(RuntimeVariant::Hip),
    }
}

fn selected_packages(variant: RuntimeVariant) -> Vec<RuntimeVariant> {
    match variant {
        RuntimeVariant::Base => vec![RuntimeVariant::Base],
        RuntimeVariant::Cuda => vec![RuntimeVariant::Base, RuntimeVariant::Cuda],
        RuntimeVariant::Hip => vec![RuntimeVariant::Base, RuntimeVariant::Hip],
    }
}

fn extract_packaged_manifest(bundle: &EmbeddedBundle) -> Result<PackagedPayloadManifest> {
    let bytes = bundle.read_asset_bytes(PAYLOAD_MANIFEST_FILE_NAME)?;
    serde_json::from_slice(&bytes).context("failed to parse embedded payload manifest")
}

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .context("failed to resolve workspace root")
}

fn resolve_nsis_installer(workspace_root: &Path, explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        if !path.is_file() {
            bail!("NSIS installer '{}' does not exist", path.display());
        }
        return Ok(path);
    }

    let bundle_dir = workspace_root.join("target").join("release").join("bundle").join("nsis");
    let entries = fs::read_dir(&bundle_dir)
        .with_context(|| format!("failed to read Tauri NSIS bundle directory {}", bundle_dir.display()))?;

    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to read entry under {}", bundle_dir.display()))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if path.extension().and_then(|value| value.to_str()) != Some("exe") {
            continue;
        }
        if file_name.eq_ignore_ascii_case("SlabFullInstaller.exe") {
            continue;
        }

        let modified = entry
            .metadata()
            .with_context(|| format!("failed to read metadata for {}", path.display()))?
            .modified()
            .with_context(|| format!("failed to read modified time for {}", path.display()))?;
        let looks_like_setup = file_name.to_ascii_lowercase().contains("setup");
        candidates.push((looks_like_setup, modified, path));
    }

    candidates.sort_by(|left, right| right.cmp(left));
    candidates
        .into_iter()
        .next()
        .map(|(_, _, path)| path)
        .ok_or_else(|| anyhow!("no NSIS setup executable was found under {}", bundle_dir.display()))
}

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bootstrap_ui;
mod bundle;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitCode};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use uuid::Uuid;

use crate::bootstrap_ui::run_bootstrap_with_ui;
use crate::bundle::{
    AssetInput, AssetKind, EmbeddedBundle, load_embedded_bundle, write_embedded_bundle,
};
use slab_utils::cab::{
    PackagedPayloadManifest, RequestedVariant, RuntimeVariant, SelectedPayloadManifest,
    apply_selected_payload, detect_best_variant, expand_cab_with_progress, remove_dir_if_exists,
    selected_packages, stage_runtime_payloads, write_json,
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
    StagePayloads(StagePayloadsArgs),
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

#[derive(Debug, Args)]
struct StagePayloadsArgs {
    #[arg(long)]
    output_dir: Option<PathBuf>,

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

#[derive(Debug)]
struct PreparedBootstrap {
    staging_root: PathBuf,
    setup_path: PathBuf,
    payload_root: PathBuf,
    helper_path: PathBuf,
}

trait BootstrapProgressSink {
    fn set_total_bytes(&mut self, total_bytes: u64);
    fn set_stage(&mut self, stage: impl Into<String>, detail: impl Into<String>);
    fn advance(&mut self, bytes: u64) -> Result<()>;
}

struct NoopProgressSink;

impl BootstrapProgressSink for NoopProgressSink {
    fn set_total_bytes(&mut self, _total_bytes: u64) {}

    fn set_stage(&mut self, _stage: impl Into<String>, _detail: impl Into<String>) {}

    fn advance(&mut self, _bytes: u64) -> Result<()> {
        Ok(())
    }
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
        Some(Commands::StagePayloads(args)) => run_stage_payloads(args),
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
                bail!(
                    "no subcommand was provided and '{}' is not a bundled bootstrap executable",
                    current_exe.display()
                );
            };
            run_bootstrap_with_ui(RunArgs { variant: RequestedVariant::Auto, keep_staging: false })
        }
    }
}

fn run_pack(args: PackArgs) -> Result<()> {
    let workspace_root = workspace_root()?;
    let version = args.version.unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    let output_path = args.output.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("release")
            .join("bundle")
            .join("nsis")
            .join(full_installer_output_name(&version))
    });
    let nsis_installer = resolve_nsis_installer(&workspace_root, args.nsis_installer)?;
    let bundle_dir = output_path.parent().ok_or_else(|| {
        anyhow!("offline installer output path '{}' has no parent", output_path.display())
    })?;
    let staged_payloads = stage_runtime_payloads(&workspace_root, &version, bundle_dir)?;

    let staging_root = env::temp_dir().join(format!("slab-full-installer-pack-{}", Uuid::new_v4()));
    fs::create_dir_all(&staging_root).with_context(|| {
        format!("failed to create staging directory {}", staging_root.display())
    })?;

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

        for package in &staged_payloads.packages {
            asset_inputs.push(AssetInput {
                name: package.variant.cab_name().to_string(),
                kind: AssetKind::Cab,
                source_path: package.cab_path.clone(),
            });
        }

        asset_inputs.push(AssetInput {
            name: PAYLOAD_MANIFEST_FILE_NAME.to_string(),
            kind: AssetKind::PayloadManifest,
            source_path: staged_payloads.manifest_path.clone(),
        });

        let base_executable = env::current_exe().context("failed to locate pack executable")?;
        write_embedded_bundle(&base_executable, &version, &asset_inputs, &output_path)?;
        Ok(())
    })();

    let _ = remove_dir_if_exists(&staging_root);
    result
}

fn run_stage_payloads(args: StagePayloadsArgs) -> Result<()> {
    let workspace_root = workspace_root()?;
    let version = args.version.unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    let output_dir = args.output_dir.unwrap_or_else(|| {
        workspace_root.join("target").join("release").join("bundle").join("nsis")
    });
    let staged = stage_runtime_payloads(&workspace_root, &version, &output_dir)?;

    for package in &staged.packages {
        println!("{}", package.cab_path.display());
    }
    Ok(())
}

fn run_bootstrap(args: RunArgs) -> Result<()> {
    let mut progress = NoopProgressSink;
    let prepared = prepare_bootstrap(args.clone(), &mut progress)?;
    let result = launch_prepared_bootstrap(&prepared);
    finalize_bootstrap(prepared, args.keep_staging, result)
}

fn prepare_bootstrap(
    args: RunArgs,
    progress: &mut impl BootstrapProgressSink,
) -> Result<PreparedBootstrap> {
    let current_exe = env::current_exe().context("failed to locate current executable")?;
    let bundle = load_embedded_bundle(&current_exe)?.ok_or_else(|| {
        anyhow!("'{}' does not contain an embedded installer bundle", current_exe.display())
    })?;

    progress.set_stage("Detecting hardware", "Selecting the best runtime package");
    let resolved_variant = resolve_requested_variant(args.variant)?;
    let selected_packages = selected_packages(resolved_variant);
    let packaged_manifest = extract_packaged_manifest(&bundle)?;
    let selected_manifest = packaged_manifest.selected_for(&selected_packages)?;
    progress.set_total_bytes(total_bootstrap_bytes(
        &bundle,
        &selected_packages,
        &selected_manifest,
    )?);
    progress.set_stage(
        "Preparing installer",
        format!("Using '{}' runtime package", resolved_variant.as_str()),
    );

    let staging_root = env::temp_dir().join("SlabFullInstaller").join(format!(
        "{}-{}",
        bundle.version(),
        Uuid::new_v4()
    ));
    let cleanup_root = staging_root.clone();
    let setup_path = staging_root.join(SETUP_ASSET_NAME);
    let payload_root = staging_root.join("payload");
    let helper_path = staging_root.join("slab-windows-full-installer-helper.exe");

    let result = (|| -> Result<PreparedBootstrap> {
        fs::create_dir_all(&payload_root).with_context(|| {
            format!("failed to create staging payload directory {}", payload_root.display())
        })?;

        progress.set_stage("Preparing installer", "Extracting the NSIS installer");
        bundle.extract_asset_to_path_with_progress(SETUP_ASSET_NAME, &setup_path, |bytes| {
            progress.advance(bytes)
        })?;

        for package in &selected_packages {
            let cab_name = package.cab_name();
            let cab_path = staging_root.join(cab_name);

            progress.set_stage(
                "Preparing runtime files",
                format!("Extracting {cab_name} from the bundled installer"),
            );
            bundle.extract_asset_to_path_with_progress(cab_name, &cab_path, |bytes| {
                progress.advance(bytes)
            })?;

            progress.set_stage(
                "Preparing runtime files",
                format!("Expanding {cab_name} into the payload staging area"),
            );
            expand_cab_with_progress(&cab_path, &payload_root, |bytes| progress.advance(bytes))?;
            fs::remove_file(&cab_path)
                .with_context(|| format!("failed to remove expanded CAB {}", cab_path.display()))?;
        }

        write_json(&payload_root.join(PAYLOAD_MANIFEST_FILE_NAME), &selected_manifest)?;

        progress.set_stage("Preparing installer", "Writing the helper executable");
        bundle.write_base_executable_to_path_with_progress(&helper_path, |bytes| {
            progress.advance(bytes)
        })?;
        progress.set_stage("Launching installer", "Opening the NSIS setup wizard");

        Ok(PreparedBootstrap { staging_root, setup_path, payload_root, helper_path })
    })();

    match result {
        Ok(prepared) => Ok(prepared),
        Err(error) if args.keep_staging => Err(error),
        Err(error) => {
            let _ = remove_dir_if_exists(&cleanup_root);
            Err(error)
        }
    }
}

fn total_bootstrap_bytes(
    bundle: &EmbeddedBundle,
    selected_packages: &[RuntimeVariant],
    selected_manifest: &SelectedPayloadManifest,
) -> Result<u64> {
    let mut total_bytes = bundle.asset_len(SETUP_ASSET_NAME)? + bundle.base_executable_len();
    for package in selected_packages {
        total_bytes += bundle.asset_len(package.cab_name())?;
    }
    total_bytes += selected_manifest.files.iter().map(|file| file.size).sum::<u64>();
    Ok(total_bytes.max(1))
}

fn launch_prepared_bootstrap(prepared: &PreparedBootstrap) -> Result<()> {
    let status = ProcessCommand::new(&prepared.setup_path)
        .env("SLAB_INSTALLER_PAYLOAD_DIR", &prepared.payload_root)
        .env("SLAB_INSTALLER_HELPER_PATH", &prepared.helper_path)
        .status()
        .with_context(|| format!("failed to launch {}", prepared.setup_path.display()))?;

    if !status.success() {
        bail!("NSIS installer '{}' exited with status {}", prepared.setup_path.display(), status);
    }

    Ok(())
}

fn finalize_bootstrap(
    prepared: PreparedBootstrap,
    keep_staging: bool,
    result: Result<()>,
) -> Result<()> {
    if keep_staging {
        return result;
    }

    let cleanup = remove_dir_if_exists(&prepared.staging_root);
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

fn full_installer_output_name(version: &str) -> String {
    format!("Slab_{version}_x64-offline-setup.exe")
}

fn resolve_nsis_installer(workspace_root: &Path, explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        if !path.is_file() {
            bail!("NSIS installer '{}' does not exist", path.display());
        }
        return Ok(path);
    }

    let bundle_dir = workspace_root.join("target").join("release").join("bundle").join("nsis");
    let entries = fs::read_dir(&bundle_dir).with_context(|| {
        format!("failed to read Tauri NSIS bundle directory {}", bundle_dir.display())
    })?;

    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry
            .with_context(|| format!("failed to read entry under {}", bundle_dir.display()))?;
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
        if is_offline_setup_executable(file_name) {
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

fn is_offline_setup_executable(file_name: &str) -> bool {
    let lowered = file_name.to_ascii_lowercase();
    lowered.starts_with("slab_") && lowered.ends_with("_x64-offline-setup.exe")
}

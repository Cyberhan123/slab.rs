use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use uuid::Uuid;

use crate::payload::ResolvedPayloadFile;

pub fn create_cab(output_path: &Path, files: &[ResolvedPayloadFile]) -> Result<()> {
    if files.is_empty() {
        bail!("cannot create CAB '{}' with no files", output_path.display());
    }

    let output_dir = output_path
        .parent()
        .ok_or_else(|| anyhow!("CAB path '{}' has no parent directory", output_path.display()))?;
    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create directory {}", output_dir.display()))?;

    let ddf_dir = std::env::temp_dir().join(format!("slab-full-installer-ddf-{}", Uuid::new_v4()));
    fs::create_dir_all(&ddf_dir)
        .with_context(|| format!("failed to create DDF directory {}", ddf_dir.display()))?;
    let ddf_path = ddf_dir.join("payload.ddf");

    let ddf = build_ddf(output_path, files)?;
    let result = fs::write(&ddf_path, ddf)
        .with_context(|| format!("failed to write {}", ddf_path.display()))
        .and_then(|_| run_makecab(&ddf_path));

    let _ = fs::remove_dir_all(&ddf_dir);
    result
}

pub fn expand_cab(cab_path: &Path, destination_root: &Path) -> Result<()> {
    fs::create_dir_all(destination_root)
        .with_context(|| format!("failed to create directory {}", destination_root.display()))?;

    let output = Command::new("expand.exe")
        .arg(cab_path)
        .arg("-F:*")
        .arg(destination_root)
        .output()
        .with_context(|| format!("failed to execute expand.exe for {}", cab_path.display()))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    bail!(
        "expand.exe failed for '{}': {}\n{}",
        cab_path.display(),
        stdout.trim(),
        stderr.trim()
    );
}

fn build_ddf(output_path: &Path, files: &[ResolvedPayloadFile]) -> Result<String> {
    let output_dir = output_path
        .parent()
        .ok_or_else(|| anyhow!("CAB path '{}' has no parent directory", output_path.display()))?;
    let output_name = output_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("CAB path '{}' is invalid", output_path.display()))?;

    let mut lines = Vec::with_capacity(files.len() + 8);
    lines.push(".OPTION EXPLICIT".to_string());
    lines.push(format!(
        ".Set CabinetNameTemplate=\"{}\"",
        escape_ddf_string(output_name)
    ));
    lines.push(format!(
        ".Set DiskDirectoryTemplate=\"{}\"",
        escape_ddf_string(&ddf_path_string(output_dir))
    ));
    lines.push(".Set MaxDiskSize=0".to_string());
    lines.push(".Set Cabinet=on".to_string());
    lines.push(".Set Compress=on".to_string());
    lines.push(".Set CompressionType=LZX".to_string());
    lines.push(String::new());

    for file in files {
        lines.push(format!(
            "\"{}\" \"{}\"",
            escape_ddf_string(&ddf_path_string(&file.source_path)),
            escape_ddf_string(&cab_relative_path(&file.source_relative_path))
        ));
    }

    Ok(lines.join("\r\n"))
}

fn cab_relative_path(path: &str) -> String {
    path.replace('/', "\\")
}

fn escape_ddf_string(value: &str) -> String {
    value.replace('"', "\"\"")
}

fn ddf_path_string(path: &Path) -> String {
    let value = path.display().to_string();
    if let Some(stripped) = value.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{stripped}");
    }
    if let Some(stripped) = value.strip_prefix(r"\\?\") {
        return stripped.to_string();
    }
    value
}

fn run_makecab(ddf_path: &Path) -> Result<()> {
    let output = Command::new("makecab.exe")
        .arg("/F")
        .arg(PathBuf::from(ddf_path))
        .output()
        .with_context(|| format!("failed to execute makecab.exe for {}", ddf_path.display()))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    bail!(
        "makecab.exe failed for '{}': {}\n{}",
        ddf_path.display(),
        stdout.trim(),
        stderr.trim()
    );
}

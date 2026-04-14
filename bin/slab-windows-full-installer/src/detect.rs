use std::process::Command;

use anyhow::{Context, Result};

use crate::payload::RuntimeVariant;

pub fn detect_best_variant() -> Result<RuntimeVariant> {
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name",
        ])
        .output()
        .context("failed to execute PowerShell GPU detection")?;

    if !output.status.success() {
        return Ok(RuntimeVariant::Base);
    }

    let names = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    if names.contains("nvidia") {
        return Ok(RuntimeVariant::Cuda);
    }

    if names.contains("advanced micro devices")
        || names.contains(" amd")
        || names.starts_with("amd")
        || names.contains("radeon")
    {
        return Ok(RuntimeVariant::Hip);
    }

    Ok(RuntimeVariant::Base)
}

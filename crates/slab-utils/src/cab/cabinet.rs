use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use uuid::Uuid;

use super::payload::ResolvedPayloadFile;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
use std::collections::HashMap;
#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    FILE_IN_CABINET_INFO_W, FILEOP_ABORT, FILEOP_DOIT, FILEPATHS_W, SPFILENOTIFY_FILEEXTRACTED,
    SPFILENOTIFY_FILEINCABINET, SetupIterateCabinetW,
};
#[cfg(windows)]
use windows::core::PCWSTR;

pub fn create_cab(output_path: &Path, files: &[ResolvedPayloadFile]) -> Result<()> {
    if files.is_empty() {
        bail!("cannot create CAB '{}' with no files", output_path.display());
    }

    let output_dir = output_path
        .parent()
        .ok_or_else(|| anyhow!("CAB path '{}' has no parent directory", output_path.display()))?;
    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create directory {}", output_dir.display()))?;

    let ddf_dir = std::env::temp_dir().join(format!("slab-installer-ddf-{}", Uuid::new_v4()));
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

pub fn expand_cab_with_progress<F>(
    cab_path: &Path,
    destination_root: &Path,
    on_file_extracted: F,
) -> Result<()>
where
    F: FnMut(u64) -> Result<()>,
{
    #[cfg(windows)]
    {
        expand_cab_windows(cab_path, destination_root, on_file_extracted)
    }

    #[cfg(not(windows))]
    {
        let _ = on_file_extracted;
        let _ = (cab_path, destination_root);
        bail!("CAB expansion is only supported on Windows")
    }
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
    lines.push(format!(".Set CabinetNameTemplate=\"{}\"", escape_ddf_string(output_name)));
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
    let mut command = Command::new("makecab.exe");
    command.arg("/F").arg(PathBuf::from(ddf_path));

    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    let output = command
        .output()
        .with_context(|| format!("failed to execute makecab.exe for {}", ddf_path.display()))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    bail!("makecab.exe failed for '{}': {}\n{}", ddf_path.display(), stdout.trim(), stderr.trim());
}

#[cfg(windows)]
fn expand_cab_windows<F>(
    cab_path: &Path,
    destination_root: &Path,
    mut on_file_extracted: F,
) -> Result<()>
where
    F: FnMut(u64) -> Result<()>,
{
    fs::create_dir_all(destination_root)
        .with_context(|| format!("failed to create directory {}", destination_root.display()))?;

    let cabinet_path = encode_wide_null(cab_path.as_os_str());
    let mut state = ExpandCabState {
        destination_root: destination_root.to_path_buf(),
        extracted_sizes: HashMap::new(),
        on_file_extracted: &mut on_file_extracted,
        error: None,
    };

    let setup_result = unsafe {
        SetupIterateCabinetW(
            PCWSTR(cabinet_path.as_ptr()),
            None,
            Some(setup_iterate_callback),
            (&mut state as *mut ExpandCabState<'_>).cast::<c_void>(),
        )
    };

    if let Some(error) = state.error {
        return Err(error);
    }

    setup_result.with_context(|| format!("failed to expand CAB {}", cab_path.display()))
}

#[cfg(windows)]
struct ExpandCabState<'a> {
    destination_root: PathBuf,
    extracted_sizes: HashMap<PathBuf, u64>,
    on_file_extracted: &'a mut dyn FnMut(u64) -> Result<()>,
    error: Option<anyhow::Error>,
}

#[cfg(windows)]
unsafe extern "system" fn setup_iterate_callback(
    context: *const c_void,
    notification: u32,
    param1: usize,
    _param2: usize,
) -> u32 {
    let state = unsafe { &mut *(context as *mut ExpandCabState<'_>) };
    match notification {
        SPFILENOTIFY_FILEINCABINET => unsafe { on_file_in_cabinet(state, param1) },
        SPFILENOTIFY_FILEEXTRACTED => unsafe { on_file_extracted(state, param1) },
        _ => 0,
    }
}

#[cfg(windows)]
unsafe fn on_file_in_cabinet(state: &mut ExpandCabState<'_>, param1: usize) -> u32 {
    if state.error.is_some() {
        return FILEOP_ABORT;
    }

    let info = unsafe { &mut *(param1 as *mut FILE_IN_CABINET_INFO_W) };
    let relative_name = match unsafe { pcwstr_to_string(info.NameInCabinet) } {
        Ok(value) => value,
        Err(error) => {
            state.error = Some(error);
            return FILEOP_ABORT;
        }
    };

    let relative_path = Path::new(&relative_name);
    if let Err(error) = super::fsops::validate_relative_path(relative_path) {
        state.error = Some(error.context(format!("invalid path inside CAB: {relative_name}")));
        return FILEOP_ABORT;
    }

    let target_path = state.destination_root.join(relative_path);
    if let Err(error) = super::fsops::ensure_parent_dir(&target_path) {
        state.error = Some(error);
        return FILEOP_ABORT;
    }

    if let Err(error) = write_target_path(&mut info.FullTargetName, &target_path) {
        state.error = Some(error);
        return FILEOP_ABORT;
    }

    state.extracted_sizes.insert(target_path, u64::from(info.FileSize));
    FILEOP_DOIT
}

#[cfg(windows)]
unsafe fn on_file_extracted(state: &mut ExpandCabState<'_>, param1: usize) -> u32 {
    if state.error.is_some() {
        return FILEOP_ABORT;
    }

    let paths = unsafe { &*(param1 as *const FILEPATHS_W) };
    if paths.Win32Error != 0 {
        let target =
            unsafe { pcwstr_to_string(paths.Target) }.unwrap_or_else(|_| "<unknown>".to_string());
        state.error = Some(anyhow!(
            "failed to extract '{}' from CAB: Win32 error {}",
            target,
            paths.Win32Error
        ));
        return FILEOP_ABORT;
    }

    let target = match unsafe { pcwstr_to_string(paths.Target) } {
        Ok(value) => value,
        Err(error) => {
            state.error = Some(error);
            return FILEOP_ABORT;
        }
    };
    let target_path = PathBuf::from(target);
    let Some(size) = state.extracted_sizes.remove(&target_path) else {
        return 0;
    };

    if let Err(error) = (state.on_file_extracted)(size) {
        state.error = Some(error);
        return FILEOP_ABORT;
    }

    0
}

#[cfg(windows)]
fn write_target_path(buffer: &mut [u16; 260], target_path: &Path) -> Result<()> {
    let encoded = encode_wide_null(target_path.as_os_str());
    if encoded.len() > buffer.len() {
        bail!("expanded CAB target path is too long: {}", target_path.display());
    }

    buffer.fill(0);
    buffer[..encoded.len()].copy_from_slice(&encoded);
    Ok(())
}

#[cfg(windows)]
fn encode_wide_null(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
unsafe fn pcwstr_to_string(value: PCWSTR) -> Result<String> {
    if value.is_null() {
        bail!("received null UTF-16 pointer from SetupAPI");
    }

    let mut len = 0usize;
    unsafe {
        while *value.0.add(len) != 0 {
            len += 1;
        }
        Ok(String::from_utf16_lossy(std::slice::from_raw_parts(value.0, len)))
    }
}

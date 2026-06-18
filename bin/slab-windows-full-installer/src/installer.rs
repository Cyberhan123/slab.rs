use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct NsisInstallerCandidate {
    pub looks_like_setup: bool,
    pub modified: SystemTime,
    pub path: PathBuf,
}

pub fn full_installer_output_name(version: &str) -> String {
    format!("Slab_{version}_x64-offline-setup.exe")
}

pub fn is_offline_setup_executable(file_name: &str) -> bool {
    let lowered = file_name.to_ascii_lowercase();
    lowered.starts_with("slab_") && lowered.ends_with("_x64-offline-setup.exe")
}

pub fn nsis_installer_candidate(
    path: PathBuf,
    modified: SystemTime,
) -> Option<NsisInstallerCandidate> {
    let file_name = path.file_name()?.to_str()?;
    if path.extension().and_then(|value| value.to_str()) != Some("exe") {
        return None;
    }
    if is_offline_setup_executable(file_name) {
        return None;
    }

    Some(NsisInstallerCandidate {
        looks_like_setup: file_name.to_ascii_lowercase().contains("setup"),
        modified,
        path,
    })
}

pub fn select_nsis_installer_candidate(
    candidates: impl IntoIterator<Item = NsisInstallerCandidate>,
) -> Option<PathBuf> {
    let mut candidates = candidates.into_iter().collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.cmp(left));
    candidates.into_iter().next().map(|candidate| candidate.path)
}

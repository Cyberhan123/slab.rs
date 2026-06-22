use std::path::PathBuf;

pub(crate) fn settings_path_from_env_or_default() -> PathBuf {
    std::env::var("SLAB_SETTINGS_PATH")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(slab_utils::app_home::settings_path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn settings_path_returns_a_path() {
        let path = super::settings_path_from_env_or_default();

        assert!(!path.as_os_str().is_empty());
    }
}

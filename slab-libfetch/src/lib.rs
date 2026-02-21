pub mod api;
pub mod downloader;
pub mod error;
pub mod install;

pub use api::{Api, RepoApi, VersionApi};
pub use error::FetchError;
pub use install::VersionInfo;

use std::path::Path;

/// Download header files for a GitHub repository and extract them to
/// `target_include_path`.
///
/// This is a convenience wrapper around the builder API:
/// ```rust,ignore
/// Api::new().repo("owner/repo").version("vX.Y.Z").fetch_header(path).await
/// ```
///
/// When `tag` is `None` the latest release is used.  The download is skipped
/// if `target_include_path/version.json` already records the same version.
pub async fn fetch_header(
    owner: &str,
    repo: &str,
    tag: Option<&str>,
    target_include_path: &Path,
) -> Result<(), FetchError> {
    let repo_full = format!("{}/{}", owner, repo);
    let install_dir = target_include_path
        .to_str()
        .ok_or_else(|| FetchError::InvalidPath {
            message: format!("target_include_path contains invalid UTF-8: {:?}", target_include_path),
        })?
        .to_string();

    let version_api = match tag {
        Some(t) => Api::new()
            .set_install_dir(install_dir)
            .repo(repo_full)
            .version(t),
        None => Api::new()
            .set_install_dir(install_dir)
            .repo(repo_full)
            .latest(),
    };

    version_api.fetch_header(target_include_path).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[tokio::test]
    async fn test_extract_include_ggml() {
        fetch_header(
            "ggml-org",
            "ggml",
            Some("v0.9.7"),
            Path::new("target/ggml"),
        )
        .await
        .unwrap();
    }

     #[tokio::test]
    async fn test_extract_include_whisper() {
        fetch_header(
            "ggml-org",
            "whisper.cpp",
            Some("v1.8.3"),
            Path::new("target/whisper"),
        )
        .await
        .unwrap();
    }
}

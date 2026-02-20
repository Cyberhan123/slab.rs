use std::path::{Path, PathBuf};

use slab_libfetch::Api;

use anyhow::Result;

pub struct DylibService {
    prefix_path: Option<PathBuf>,
}

impl DylibService {
    pub fn new() -> Self {
        Self { prefix_path: None }
    }

    pub fn with_prefix_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.prefix_path = Some(path.as_ref().to_path_buf());
        self
    }

    pub async fn download_diffusion(&self) -> Result<PathBuf> {
        let install_dir = self
            .prefix_path
            .as_ref()
            .map(|p| p.join("diffusion"))
            .unwrap_or_else(|| PathBuf::from("./diffusion"));

        let result = Api::new()
            .set_install_dir(&install_dir.to_str().unwrap())
            .repo("leejet/stable-diffusion.cpp")
            .version("master-504-636d3cb")
            .install(|v| format!("stable-diffusion-{v}-bin-win-cpu-x64.zip"))
            .await?;
        Ok(result)
    }

    pub async fn download_whisper(&self) -> Result<PathBuf> {
        let install_dir = self
            .prefix_path
            .as_ref()
            .map(|p| p.join("whisper"))
            .unwrap_or_else(|| PathBuf::from("./whisper"));

        let result = Api::new()
            .set_install_dir(&install_dir)
            .repo("ggml-org/whisper.cpp")
            .version("v1.8.3")
            .install(|v| format!("whisper-cublas-12.4.0-bin-x64.zip"))
            .await?;
        Ok(result)
    }

    pub async fn download_llama(&self) -> Result<PathBuf> {
        let install_dir = self
            .prefix_path
            .as_ref()
            .map(|p| p.join("llama"))
            .unwrap_or_else(|| PathBuf::from("./llama"));

        let result = Api::new()
            .set_install_dir(&install_dir)
            .repo("ggml-org/llama.cpp")
            .version("b8069")
            .install(|v| format!("llama-{v}-bin-win-cpu-x64.zip"))
            .await?;
        Ok(result)
    }
}

impl Default for DylibService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_download_diffusion() {
        DylibService::new()
            .with_prefix_path("../../../testdata".to_string())
            .download_diffusion()
            .await
            .expect("Failed to download diffusion");
    }

    #[tokio::test]
    async fn test_download_whisper() {
        let mut path = env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        path.pop();
        path.push("../../../testdata");
        print!("Current executable path: {:?}", path);
        DylibService::new()
            .with_prefix_path(path)
            .download_whisper()
            .await
            .expect("Failed to download whisper");
    }

    #[tokio::test]
    async fn test_download_llama() {
        let mut path = env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        path.pop();
        path.push("../../../testdata");
        print!("Current executable path: {:?}", path);  
        DylibService::new()
            .with_prefix_path(path)
            .download_llama()
            .await
            .expect("Failed to download llama");
    }
}

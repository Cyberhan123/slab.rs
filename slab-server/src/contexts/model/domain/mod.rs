#[derive(Debug, Clone)]
pub struct ModelLoadCommand {
    pub backend_id: String,
    pub model_path: String,
    pub num_workers: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ModelStatus {
    pub backend: String,
    pub status: String,
}

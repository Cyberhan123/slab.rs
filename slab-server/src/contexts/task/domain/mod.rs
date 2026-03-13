#[derive(Debug, Clone)]
pub struct TaskResult {
    pub image: Option<String>,
    pub images: Option<Vec<String>>,
    pub video_path: Option<String>,
    pub text: Option<String>,
}

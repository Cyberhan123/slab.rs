#[derive(Debug, Clone)]
pub struct FfmpegConvertCommand {
    pub source_path: String,
    pub output_format: String,
    pub output_path: Option<String>,
}

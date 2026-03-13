use crate::api::v1::ffmpeg::schema::ConvertRequest;

#[derive(Debug, Clone)]
pub struct FfmpegConvertCommand {
    pub source_path: String,
    pub output_format: String,
    pub output_path: Option<String>,
}

impl From<ConvertRequest> for FfmpegConvertCommand {
    fn from(request: ConvertRequest) -> Self {
        Self {
            source_path: request.source_path,
            output_format: request.output_format,
            output_path: request.output_path,
        }
    }
}

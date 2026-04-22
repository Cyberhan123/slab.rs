#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtitleVariant {
    Source,
    Translated,
}

impl SubtitleVariant {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Translated => "translated",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderSubtitleEntry {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct RenderSubtitleCommand {
    pub source_path: String,
    pub variant: SubtitleVariant,
    pub entries: Vec<RenderSubtitleEntry>,
    pub output_path: Option<String>,
    pub overwrite: bool,
}

#[derive(Debug, Clone)]
pub struct RenderSubtitleResult {
    pub output_path: String,
    pub format: String,
    pub entry_count: usize,
}

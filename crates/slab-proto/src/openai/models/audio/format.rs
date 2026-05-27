use super::misc::Rate;
use crate::models;
use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct PcmaAudioFormat {
    /// The audio format. Always `audio/pcma`.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<PcmaAudioFormatType>,
}

impl PcmaAudioFormat {
    /// The G.711 A-law format.
    pub fn new() -> PcmaAudioFormat {
        PcmaAudioFormat { r#type: None }
    }
}
/// The audio format. Always `audio/pcma`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum PcmaAudioFormatType {
    #[serde(rename = "audio/pcma")]
    #[default]
    AudioSlashPcma,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct PcmuAudioFormat {
    /// The audio format. Always `audio/pcmu`.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<PcmuAudioFormatType>,
}

impl PcmuAudioFormat {
    /// The G.711 μ-law format.
    pub fn new() -> PcmuAudioFormat {
        PcmuAudioFormat { r#type: None }
    }
}
/// The audio format. Always `audio/pcmu`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum PcmuAudioFormatType {
    #[serde(rename = "audio/pcmu")]
    #[default]
    AudioSlashPcmu,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct PcmAudioFormat {
    /// The audio format. Always `audio/pcm`.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<PcmAudioFormatType>,
    /// The sample rate of the audio. Always `24000`.
    #[serde(rename = "rate", skip_serializing_if = "Option::is_none")]
    pub rate: Option<Rate>,
}

impl PcmAudioFormat {
    /// The PCM audio format. Only a 24kHz sample rate is supported.
    pub fn new() -> PcmAudioFormat {
        PcmAudioFormat { r#type: None, rate: None }
    }
}

/// The audio format. Always `audio/pcm`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum PcmAudioFormatType {
    #[serde(rename = "audio/pcm")]
    #[default]
    AudioSlashPcm,
}


#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum Format {
    #[serde(rename = "mp3")]
    #[default]
    Mp3,
    #[serde(rename = "wav")]
    Wav,
}


/// AudioResponseFormat : The format of the output, in one of these options: `json`, `text`, `srt`, `verbose_json`, `vtt`, or `diarized_json`. For `gpt-4o-transcribe` and `gpt-4o-mini-transcribe`, the only supported format is `json`. For `gpt-4o-transcribe-diarize`, the supported formats are `json`, `text`, and `diarized_json`, with `diarized_json` required to receive speaker annotations.
/// The format of the output, in one of these options: `json`, `text`, `srt`, `verbose_json`, `vtt`, or `diarized_json`. For `gpt-4o-transcribe` and `gpt-4o-mini-transcribe`, the only supported format is `json`. For `gpt-4o-transcribe-diarize`, the supported formats are `json`, `text`, and `diarized_json`, with `diarized_json` required to receive speaker annotations.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum AudioResponseFormat {
    #[serde(rename = "json")]
    #[default]
    Json,
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "srt")]
    Srt,
    #[serde(rename = "verbose_json")]
    VerboseJson,
    #[serde(rename = "vtt")]
    Vtt,
    #[serde(rename = "diarized_json")]
    DiarizedJson,
}

impl std::fmt::Display for AudioResponseFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Json => write!(f, "json"),
            Self::Text => write!(f, "text"),
            Self::Srt => write!(f, "srt"),
            Self::VerboseJson => write!(f, "verbose_json"),
            Self::Vtt => write!(f, "vtt"),
            Self::DiarizedJson => write!(f, "diarized_json"),
        }
    }
}


#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ResponseFormat {
    #[serde(rename = "mp3")]
    #[default]
    Mp3,
    #[serde(rename = "opus")]
    Opus,
    #[serde(rename = "aac")]
    Aac,
    #[serde(rename = "flac")]
    Flac,
    #[serde(rename = "wav")]
    Wav,
    #[serde(rename = "pcm")]
    Pcm,
}

// The format to stream the audio in. Supported formats are `sse` and `audio`. `sse` is not supported for `tts-1` or `tts-1-hd`.

pub mod text_format_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "text")]
        #[default]
        Text,
    }
    
}
pub use text_format_type::Type as TextFormatType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextFormat {
    /// Unconstrained text format. Always `text`.
    #[serde(rename = "type")]
    pub r#type: TextFormatType,
}

impl TextFormat {
    /// Unconstrained free-form text.
    pub fn new(r#type: TextFormatType) -> TextFormat {
        TextFormat { r#type }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum StreamFormat {
    #[serde(rename = "sse")]
    #[default]
    Sse,
    #[serde(rename = "audio")]
    Audio,
}


pub mod custom_text_format_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "text")]
        #[default]
        Text,
    }
    
}
pub use custom_text_format_type::Type as CustomTextFormatType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomTextFormatParam {
    /// Unconstrained text format. Always `text`.
    #[serde(rename = "type")]
    pub r#type: CustomTextFormatType,
}

impl CustomTextFormatParam {
    /// Unconstrained free-form text.
    pub fn new(r#type: CustomTextFormatType) -> CustomTextFormatParam {
        CustomTextFormatParam { r#type }
    }
}

pub mod custom_grammar_format_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "grammar")]
        #[default]
        Grammar,
    }
    
}
pub use custom_grammar_format_type::Type as CustomGrammarFormatType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomGrammarFormatParam {
    /// Grammar format. Always `grammar`.
    #[serde(rename = "type")]
    pub r#type: CustomGrammarFormatType,
    /// The syntax of the grammar definition. One of `lark` or `regex`.
    #[serde(rename = "syntax")]
    pub syntax: models::GrammarSyntax1,
    /// The grammar definition.
    #[serde(rename = "definition")]
    pub definition: String,
}

impl CustomGrammarFormatParam {
    /// A grammar defined by the user.
    pub fn new(
        r#type: CustomGrammarFormatType,
        syntax: models::GrammarSyntax1,
        definition: String,
    ) -> CustomGrammarFormatParam {
        CustomGrammarFormatParam { r#type, syntax, definition }
    }
}

pub mod grammar_format_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "grammar")]
        #[default]
        Grammar,
    }
    
}
pub use grammar_format_type::Type as GrammarFormatType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct GrammarFormat {
    /// Grammar format. Always `grammar`.
    #[serde(rename = "type")]
    pub r#type: GrammarFormatType,
    #[serde(rename = "grammar")]
    pub grammar: Box<models::GrammarFormat>,
}

impl GrammarFormat {
    /// A grammar defined by the user.
    pub fn new(r#type: GrammarFormatType, grammar: models::GrammarFormat) -> GrammarFormat {
        GrammarFormat { r#type, grammar: Box::new(grammar) }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum GrammarSyntax1 {
    #[serde(rename = "lark")]
    #[default]
    Lark,
    #[serde(rename = "regex")]
    Regex,
}

impl std::fmt::Display for GrammarSyntax1 {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Lark => write!(f, "lark"),
            Self::Regex => write!(f, "regex"),
        }
    }
}


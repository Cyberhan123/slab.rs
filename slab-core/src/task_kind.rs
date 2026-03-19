pub(crate) use crate::model::{Capability, DriverHints as DispatchHints, ModelSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum TaskKind {
    TextGeneration,
    AudioTranscription,
    ImageGeneration,
    ImageEmbedding,
}

impl TaskKind {
    pub(crate) fn capability(self) -> Capability {
        match self {
            Self::TextGeneration => Capability::TextGeneration,
            Self::AudioTranscription => Capability::AudioTranscription,
            Self::ImageGeneration => Capability::ImageGeneration,
            Self::ImageEmbedding => Capability::ImageEmbedding,
        }
    }
}

impl From<Capability> for TaskKind {
    fn from(value: Capability) -> Self {
        match value {
            Capability::TextGeneration => Self::TextGeneration,
            Capability::AudioTranscription => Self::AudioTranscription,
            Capability::ImageGeneration => Self::ImageGeneration,
            Capability::ImageEmbedding => Self::ImageEmbedding,
        }
    }
}

impl ModelSpec {
    pub(crate) fn task_kind(&self) -> TaskKind {
        self.capability.into()
    }
}

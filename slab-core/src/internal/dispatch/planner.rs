use std::cmp::Ordering;

use crate::base::error::CoreError;
use crate::model::ModelSource;
use crate::task_kind::{DispatchHints, ModelSpec, TaskKind};

use super::plan::{DriverDescriptor, ModelSourceKind, ResolvedDriver};

#[derive(Debug, Clone, Default)]
pub(crate) struct DriverResolver {
    descriptors: Vec<DriverDescriptor>,
}

impl DriverResolver {
    pub(crate) fn new(descriptors: Vec<DriverDescriptor>) -> Self {
        Self { descriptors }
    }

    pub(crate) fn descriptors(&self) -> &[DriverDescriptor] {
        &self.descriptors
    }

    pub(crate) fn resolve(
        &self,
        spec: &ModelSpec,
        task_kind: TaskKind,
        streaming: bool,
    ) -> Result<ResolvedDriver, CoreError> {
        if spec.capability != task_kind.capability() {
            return Err(CoreError::UnsupportedCapability {
                family: format!("{:?}", spec.family),
                capability: format!("{:?}", task_kind.capability()),
            });
        }

        let source_kind = source_kind(&spec.source);
        let hints = &spec.driver_hints;
        let mut candidates: Vec<&DriverDescriptor> = self
            .descriptors
            .iter()
            .filter(|descriptor| descriptor.family == spec.family)
            .filter(|descriptor| descriptor.capability == spec.capability)
            .filter(|descriptor| descriptor.supported_sources.contains(&source_kind))
            .collect();

        if candidates.is_empty() {
            return Err(CoreError::NoViableDriver {
                family: format!("{:?}", spec.family),
                capability: format!("{:?}", spec.capability),
            });
        }

        if streaming {
            if !candidates
                .iter()
                .any(|descriptor| descriptor.supports_streaming)
            {
                return Err(CoreError::UnsupportedOperation {
                    backend: format!("{:?}", spec.family),
                    op: "stream".to_owned(),
                });
            }

            candidates.retain(|descriptor| descriptor.supports_streaming);
        }

        candidates.sort_by(|left, right| compare_descriptors(left, right, hints));
        let winner = candidates[0];

        Ok(ResolvedDriver {
            driver_id: winner.driver_id.clone(),
            backend_id: winner.backend_id.clone(),
            family: winner.family,
            capability: winner.capability,
            supports_streaming: winner.supports_streaming,
            load_style: winner.load_style,
        })
    }
}

fn compare_descriptors(
    left: &DriverDescriptor,
    right: &DriverDescriptor,
    hints: &DispatchHints,
) -> Ordering {
    driver_score(left, hints)
        .cmp(&driver_score(right, hints))
        .then_with(|| left.priority.cmp(&right.priority))
        .then_with(|| left.driver_id.cmp(&right.driver_id))
}

fn driver_score(descriptor: &DriverDescriptor, hints: &DispatchHints) -> i32 {
    let mut score = 0;

    if hints
        .prefer_drivers
        .iter()
        .any(|id| id == &descriptor.driver_id)
    {
        score -= 1000;
    }
    if hints
        .avoid_drivers
        .iter()
        .any(|id| id == &descriptor.driver_id)
    {
        score += 1000;
    }
    if hints.require_streaming && !descriptor.supports_streaming {
        score += 10_000;
    }

    score
}

fn source_kind(source: &ModelSource) -> ModelSourceKind {
    match source {
        ModelSource::LocalPath { .. } => ModelSourceKind::LocalPath,
        ModelSource::LocalArtifacts { .. } => ModelSourceKind::LocalArtifacts,
        ModelSource::HuggingFace { .. } => ModelSourceKind::HuggingFace,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::internal::dispatch::DriverLoadStyle;
    use crate::model::{Capability, ModelFamily, ModelSource};

    #[test]
    fn resolver_filters_by_family_capability_and_source() {
        let resolver = DriverResolver::new(vec![
            descriptor(
                "candle.llama",
                ModelFamily::Llama,
                Capability::TextGeneration,
                &[ModelSourceKind::LocalPath],
                true,
                10,
            ),
            descriptor(
                "onnx.text",
                ModelFamily::Onnx,
                Capability::TextGeneration,
                &[ModelSourceKind::LocalPath],
                false,
                30,
            ),
        ]);

        let spec = ModelSpec::new(
            ModelFamily::Llama,
            Capability::TextGeneration,
            ModelSource::LocalPath {
                path: PathBuf::from("model.gguf"),
            },
        );

        let resolved = resolver
            .resolve(&spec, TaskKind::TextGeneration, false)
            .expect("llama local path should resolve");

        assert_eq!(resolved.driver_id, "candle.llama");
        assert_eq!(resolved.backend_id, "candle.llama");
    }

    #[test]
    fn resolver_applies_driver_hints_before_priority() {
        let resolver = DriverResolver::new(vec![
            descriptor(
                "candle.llama",
                ModelFamily::Llama,
                Capability::TextGeneration,
                &[ModelSourceKind::LocalPath],
                true,
                10,
            ),
            descriptor(
                "ggml.llama",
                ModelFamily::Llama,
                Capability::TextGeneration,
                &[ModelSourceKind::LocalPath],
                true,
                20,
            ),
        ]);

        let mut spec = ModelSpec::new(
            ModelFamily::Llama,
            Capability::TextGeneration,
            ModelSource::LocalPath {
                path: PathBuf::from("model.gguf"),
            },
        );
        spec.driver_hints = DispatchHints {
            prefer_drivers: vec!["ggml.llama".to_owned()],
            avoid_drivers: vec!["candle.llama".to_owned()],
            require_streaming: false,
        };

        let resolved = resolver
            .resolve(&spec, TaskKind::TextGeneration, false)
            .expect("preferred driver should win");

        assert_eq!(resolved.driver_id, "ggml.llama");
    }

    #[test]
    fn resolver_returns_no_viable_driver_when_none_match() {
        let resolver = DriverResolver::new(vec![descriptor(
            "candle.llama",
            ModelFamily::Llama,
            Capability::TextGeneration,
            &[ModelSourceKind::LocalPath],
            true,
            10,
        )]);

        let spec = ModelSpec::new(
            ModelFamily::Whisper,
            Capability::AudioTranscription,
            ModelSource::LocalPath {
                path: PathBuf::from("model.bin"),
            },
        );

        let error = resolver
            .resolve(&spec, TaskKind::AudioTranscription, false)
            .expect_err("resolver should reject unsupported family");

        assert!(matches!(error, CoreError::NoViableDriver { .. }));
    }

    #[test]
    fn resolver_rejects_streaming_when_driver_cannot_stream() {
        let resolver = DriverResolver::new(vec![descriptor(
            "onnx.text",
            ModelFamily::Onnx,
            Capability::TextGeneration,
            &[ModelSourceKind::LocalPath],
            false,
            30,
        )]);

        let spec = ModelSpec::new(
            ModelFamily::Onnx,
            Capability::TextGeneration,
            ModelSource::LocalPath {
                path: PathBuf::from("model.onnx"),
            },
        );

        let error = resolver
            .resolve(&spec, TaskKind::TextGeneration, true)
            .expect_err("non-streaming driver should fail before dispatch");

        assert!(matches!(
            error,
            CoreError::UnsupportedOperation { ref op, .. } if op == "stream"
        ));
    }

    fn descriptor(
        driver_id: &str,
        family: ModelFamily,
        capability: Capability,
        supported_sources: &[ModelSourceKind],
        supports_streaming: bool,
        priority: i32,
    ) -> DriverDescriptor {
        DriverDescriptor {
            driver_id: driver_id.to_owned(),
            backend_id: driver_id.to_owned(),
            family,
            capability,
            supported_sources: supported_sources.to_vec(),
            supports_streaming,
            load_style: DriverLoadStyle::ModelOnly,
            priority,
        }
    }
}

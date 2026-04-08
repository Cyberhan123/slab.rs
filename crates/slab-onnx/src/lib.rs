use slab_runtime_core::api::{
    Capability, DriverDescriptor, DriverLoadStyle, ModelFamily, ModelSourceKind,
    RuntimeBackendRegistration,
};
use slab_runtime_core::backend::spawn_workers;
use slab_runtime_core::engines::onnx::OnnxWorker;

#[derive(Debug, Clone, Copy, Default)]
pub struct OnnxBackendConfig {
    pub enabled: bool,
}

pub fn runtime_registrations(config: &OnnxBackendConfig) -> Vec<RuntimeBackendRegistration> {
    if !config.enabled {
        return Vec::new();
    }

    vec![RuntimeBackendRegistration::new(
        vec![
            driver_descriptor(
                "onnx.text",
                "onnx",
                ModelFamily::Onnx,
                Capability::TextGeneration,
                false,
                DriverLoadStyle::ModelOnly,
                30,
            ),
            driver_descriptor(
                "onnx.embedding",
                "onnx",
                ModelFamily::Onnx,
                Capability::ImageEmbedding,
                false,
                DriverLoadStyle::ModelOnly,
                10,
            ),
        ],
        |resource_manager, worker_count| {
            resource_manager.register_backend("onnx", move |shared_rx, control_tx| {
                let count = worker_count.max(1);
                spawn_workers(shared_rx, control_tx, count, move |worker_id, bc_tx| {
                    OnnxWorker::new(bc_tx, worker_id)
                });
            });
            Ok(())
        },
    )]
}

fn driver_descriptor(
    driver_id: &str,
    backend_id: &str,
    family: ModelFamily,
    capability: Capability,
    supports_streaming: bool,
    load_style: DriverLoadStyle,
    priority: i32,
) -> DriverDescriptor {
    DriverDescriptor {
        driver_id: driver_id.to_owned(),
        backend_id: backend_id.to_owned(),
        family,
        capability,
        supported_sources: vec![
            ModelSourceKind::LocalPath,
            ModelSourceKind::LocalArtifacts,
            ModelSourceKind::HuggingFace,
        ],
        supports_streaming,
        load_style,
        priority,
    }
}

use slab_runtime_core::api::{
    Capability, DriverDescriptor, DriverLoadStyle, ModelFamily, ModelSourceKind,
    RuntimeBackendRegistration,
};
use slab_runtime_core::engines::candle;

#[derive(Debug, Clone, Default)]
pub struct CandleBackendConfig {
    pub enable_llama: bool,
    pub enable_whisper: bool,
    pub enable_diffusion: bool,
}

pub fn runtime_registrations(config: &CandleBackendConfig) -> Vec<RuntimeBackendRegistration> {
    let mut registrations = Vec::new();

    if config.enable_llama {
        registrations.push(RuntimeBackendRegistration::new(
            vec![driver_descriptor(
                "candle.llama",
                "candle.llama",
                ModelFamily::Llama,
                Capability::TextGeneration,
                true,
                DriverLoadStyle::ModelOnly,
                10,
            )],
            |resource_manager, _worker_count| {
                resource_manager.register_backend("candle.llama", move |shared_rx, control_tx| {
                    candle::spawn_llama_backend(shared_rx, control_tx, None);
                });
                Ok(())
            },
        ));
    }

    if config.enable_whisper {
        registrations.push(RuntimeBackendRegistration::new(
            vec![driver_descriptor(
                "candle.whisper",
                "candle.whisper",
                ModelFamily::Whisper,
                Capability::AudioTranscription,
                false,
                DriverLoadStyle::ModelOnly,
                10,
            )],
            |resource_manager, worker_count| {
                resource_manager.register_backend("candle.whisper", move |shared_rx, control_tx| {
                    candle::spawn_whisper_backend(shared_rx, control_tx, worker_count);
                });
                Ok(())
            },
        ));
    }

    if config.enable_diffusion {
        registrations.push(RuntimeBackendRegistration::new(
            vec![driver_descriptor(
                "candle.diffusion",
                "candle.diffusion",
                ModelFamily::Diffusion,
                Capability::ImageGeneration,
                false,
                DriverLoadStyle::ModelOnly,
                10,
            )],
            |resource_manager, worker_count| {
                resource_manager.register_backend(
                    "candle.diffusion",
                    move |shared_rx, control_tx| {
                        candle::spawn_diffusion_backend(shared_rx, control_tx, worker_count);
                    },
                );
                Ok(())
            },
        ));
    }

    registrations
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

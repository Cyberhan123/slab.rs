use slab_types::{Capability, DriverHints, ModelFamily, RuntimeBackendId};

use crate::manifest::ResourceFootprint;
use crate::resolve::ResolvedModelPack;

#[derive(Debug, Clone)]
pub struct ModelPackCatalogSummary {
    pub id: String,
    pub label: String,
    pub family: ModelFamily,
    pub capabilities: Vec<Capability>,
    pub backend_hints: DriverHints,
    pub backends: Vec<RuntimeBackendId>,
    pub component_ids: Vec<String>,
    pub variant_ids: Vec<String>,
    pub adapter_ids: Vec<String>,
    pub preset_ids: Vec<String>,
    pub default_preset_id: Option<String>,
    pub footprint: ResourceFootprint,
}

impl ResolvedModelPack {
    pub fn catalog_summary(&self) -> ModelPackCatalogSummary {
        let mut backends = Vec::new();
        for variant in self.variants.values() {
            push_backend(&mut backends, variant.document.backend);
            push_backend(&mut backends, variant.load_config.as_ref().map(|config| config.backend));
            push_backend(
                &mut backends,
                variant.inference_config.as_ref().map(|config| config.backend),
            );
        }
        for preset in self.presets.values() {
            push_backend(
                &mut backends,
                preset.effective_load_config.as_ref().map(|config| config.backend),
            );
            push_backend(
                &mut backends,
                preset.effective_inference_config.as_ref().map(|config| config.backend),
            );
        }

        ModelPackCatalogSummary {
            id: self.manifest.id.clone(),
            label: self.manifest.label.clone(),
            family: self.manifest.family,
            capabilities: self.manifest.capabilities.clone(),
            backend_hints: self.manifest.backend_hints.clone(),
            backends,
            component_ids: self.components.keys().cloned().collect(),
            variant_ids: self.variants.keys().cloned().collect(),
            adapter_ids: self.adapters.keys().cloned().collect(),
            preset_ids: self.presets.keys().cloned().collect(),
            default_preset_id: self.default_preset_id.clone(),
            footprint: self.manifest.footprint.clone(),
        }
    }
}

fn push_backend(backends: &mut Vec<RuntimeBackendId>, candidate: Option<RuntimeBackendId>) {
    if let Some(candidate) = candidate && !backends.contains(&candidate) {
        backends.push(candidate);
    }
}
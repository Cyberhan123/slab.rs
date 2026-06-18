use slab_types::{Capability, ModelFamily, RuntimeBackendId};

use crate::manifest::ResourceFootprint;
use crate::resolve::ResolvedModelPack;
#[derive(Debug, Clone)]
pub struct ModelPackCatalogSummary {
    pub id: String,
    pub label: String,
    pub family: ModelFamily,
    pub capabilities: Vec<Capability>,
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
        ModelPackCatalogSummary {
            id: self.manifest.id.clone(),
            label: self.manifest.label.clone(),
            family: self.manifest.family,
            capabilities: self.manifest.capabilities.clone(),
            backends: self.manifest.engines.iter().map(|engine| engine.id).collect(),
            component_ids: self.components.keys().cloned().collect(),
            variant_ids: self.variants.keys().cloned().collect(),
            adapter_ids: self.adapters.keys().cloned().collect(),
            preset_ids: self.presets.keys().cloned().collect(),
            default_preset_id: self.default_preset_id.clone(),
            footprint: self.manifest.footprint.clone(),
        }
    }
}

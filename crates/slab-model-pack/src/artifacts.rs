use std::collections::BTreeMap;

use crate::resolve::{ResolvedComponent, ResolvedPreset};

#[derive(Debug, Clone)]
pub struct ResolvedArtifact {
    pub component_id: String,
    pub file_id: String,
    pub path: String,
    pub label: Option<String>,
    pub description: Option<String>,
    pub source_kind: &'static str,
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedArtifactMap {
    pub by_component: BTreeMap<String, BTreeMap<String, ResolvedArtifact>>,
    pub flat: BTreeMap<String, ResolvedArtifact>,
}

impl ResolvedArtifactMap {
    pub fn get(&self, key: &str) -> Option<&ResolvedArtifact> {
        self.flat.get(key)
    }
}

impl ResolvedPreset {
    pub fn artifact_map(&self) -> ResolvedArtifactMap {
        let mut artifacts = ResolvedArtifactMap::default();

        for (component_id, component) in &self.variant.components {
            insert_component_artifacts(&mut artifacts, component_id, component);
        }

        for adapter in self.adapters.values() {
            for (component_id, component) in &adapter.components {
                insert_component_artifacts(&mut artifacts, component_id, component);
            }
        }

        artifacts
    }
}

fn insert_component_artifacts(
    artifacts: &mut ResolvedArtifactMap,
    component_id: &str,
    component: &ResolvedComponent,
) {
    let source = &component.document.source;
    let mut files = BTreeMap::new();

    for file in source.files() {
        let artifact = ResolvedArtifact {
            component_id: component_id.to_owned(),
            file_id: file.id.clone(),
            path: file.path.clone(),
            label: file.label.clone(),
            description: file.description.clone(),
            source_kind: source.kind(),
        };

        let flat_key = flat_artifact_key(component_id, &file.id);
        artifacts.flat.insert(flat_key, artifact.clone());
        files.insert(file.id.clone(), artifact);
    }

    artifacts.by_component.insert(component_id.to_owned(), files);
}

fn flat_artifact_key(component_id: &str, file_id: &str) -> String {
    if component_id == file_id {
        component_id.to_owned()
    } else {
        format!("{component_id}/{file_id}")
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use serde_json::json;
    use zip::CompressionMethod;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    use crate::pack::ModelPack;

    fn build_pack(entries: Vec<(&str, String)>) -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(&mut cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        for (path, content) in entries {
            writer.start_file(path, options).expect("start file");
            writer.write_all(content.as_bytes()).expect("write file");
        }

        writer.finish().expect("finish zip");
        cursor.into_inner()
    }

    #[test]
    fn builds_flat_artifact_map_from_resolved_preset() {
        let bytes = build_pack(vec![
            (
                "manifest.json",
                json!({
                    "version": 2,
                    "id": "sdxl-base-1.0",
                    "label": "SDXL Base 1.0",
                    "family": "diffusion",
                    "capabilities": ["image_generation"],
                    "components": [
                        { "id": "model", "label": "Model", "$config": "ref://models/components/model.json" },
                        { "id": "vae", "label": "VAE", "$config": "ref://models/components/vae.json" }
                    ],
                    "variants": [
                        { "id": "fp16", "label": "FP16", "$config": "ref://models/variants/fp16.json" }
                    ],
                    "presets": [
                        { "id": "default", "label": "Default", "$config": "ref://models/presets/default.json" }
                    ],
                    "default_preset": "default"
                })
                .to_string(),
            ),
            (
                "models/components/model.json",
                json!({
                    "kind": "component",
                    "id": "model",
                    "label": "Model",
                    "source": {
                        "kind": "local_files",
                        "files": [
                            { "id": "model", "path": "C:/models/sdxl/model.safetensors" }
                        ]
                    }
                })
                .to_string(),
            ),
            (
                "models/components/vae.json",
                json!({
                    "kind": "component",
                    "id": "vae",
                    "label": "VAE",
                    "source": {
                        "kind": "local_files",
                        "files": [
                            { "id": "weights", "path": "C:/models/sdxl/vae.safetensors" }
                        ]
                    }
                })
                .to_string(),
            ),
            (
                "models/variants/fp16.json",
                json!({
                    "kind": "variant",
                    "id": "fp16",
                    "label": "FP16",
                    "component_ids": ["model", "vae"]
                })
                .to_string(),
            ),
            (
                "models/presets/default.json",
                json!({
                    "kind": "preset",
                    "id": "default",
                    "label": "Default",
                    "variant_id": "fp16"
                })
                .to_string(),
            ),
        ]);

        let pack = ModelPack::from_bytes(&bytes).expect("pack should load");
        let resolved = pack.resolve().expect("pack should resolve");
        let artifacts = resolved.default_preset().expect("default preset").artifact_map();

        assert_eq!(artifacts.get("model").map(|artifact| artifact.path.as_str()), Some("C:/models/sdxl/model.safetensors"));
        assert_eq!(artifacts.get("vae/weights").map(|artifact| artifact.path.as_str()), Some("C:/models/sdxl/vae.safetensors"));
    }
}
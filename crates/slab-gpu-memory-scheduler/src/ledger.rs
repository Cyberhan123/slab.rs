//! Per-device model-memory ledger. Attribution and diagnostics only —
//! scheduling decisions always read probe-measured free bytes (ground truth),
//! never the computed sum of expected footprints.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use slab_types::RuntimeBackendId;

use crate::snapshot::DeviceMemoryGauge;

/// Key single-GPU entries land on until placement routing exists.
const PRIMARY_KEY: &str = "primary";

/// One resident model's accounting on a device.
#[derive(Debug, Clone)]
pub struct LedgerEntry {
    pub backend: RuntimeBackendId,
    pub model_id: Option<String>,
    pub model_path: String,
    pub num_workers: usize,
    /// The engine-resolved `n_ctx` (what `auto` sized to), when reported.
    pub resolved_context_length: Option<u32>,
    pub mmproj_resident: bool,
    /// Model weights file size, when stat-able.
    pub weights_bytes: Option<u64>,
    /// Multimodal projector file size, when configured and stat-able.
    pub mmproj_bytes: Option<u64>,
    /// Measured VRAM delta across the load (baseline free − post-load free),
    /// when both samples were available.
    pub measured_delta_bytes: Option<u64>,
    pub recorded_at: DateTime<Utc>,
}

/// Accounting for one GPU device.
#[derive(Debug, Default, Clone)]
pub struct DeviceLedger {
    pub uuid: String,
    pub gauge: Option<DeviceMemoryGauge>,
    pub resident: Vec<LedgerEntry>,
}

/// uuid-keyed ledger shared between the scheduler and its internal hook.
/// Clone-cheap handle (Arc inside).
#[derive(Clone, Default)]
pub struct ModelLedger {
    devices: Arc<tokio::sync::Mutex<HashMap<String, DeviceLedger>>>,
}

impl ModelLedger {
    /// Record (or replace, on reload) the resident entry for a backend on the
    /// given device. `uuid: None` lands on the primary slot.
    pub async fn note_model_loaded(&self, uuid: Option<&str>, entry: LedgerEntry) {
        let key = device_key(uuid);
        let mut devices = self.devices.lock().await;
        let device = devices.entry(key.clone()).or_default();
        device.uuid = key;
        device.resident.retain(|existing| existing.backend != entry.backend);
        device.resident.push(entry);
    }

    /// Remove a backend's entry from every device (unload).
    pub async fn note_model_unloaded(&self, backend: RuntimeBackendId) {
        let mut devices = self.devices.lock().await;
        for device in devices.values_mut() {
            device.resident.retain(|existing| existing.backend != backend);
        }
    }

    /// Toggle a backend's projector residency (and size, when known).
    pub async fn note_mmproj(&self, backend: RuntimeBackendId, resident: bool, bytes: Option<u64>) {
        let mut devices = self.devices.lock().await;
        for device in devices.values_mut() {
            for entry in device.resident.iter_mut() {
                if entry.backend == backend {
                    entry.mmproj_resident = resident;
                    if bytes.is_some() {
                        entry.mmproj_bytes = bytes;
                    }
                }
            }
        }
    }

    /// Fold the latest probe gauges in (called by the refresh loop).
    pub async fn sync_gauges(&self, gauges: &[DeviceMemoryGauge]) {
        let mut devices = self.devices.lock().await;
        // Match devices by uuid; a uuid-less gauge only maps when the probe
        // saw exactly one device (it then lands on the primary slot).
        let uuidless_single =
            (gauges.len() == 1 && gauges[0].uuid.is_none()).then(|| gauges[0].clone());
        for device in devices.values_mut() {
            let matched = gauges
                .iter()
                .find(|gauge| gauge.uuid.as_deref() == Some(device.uuid.as_str()))
                .cloned()
                .or_else(|| uuidless_single.clone());
            if let Some(gauge) = matched {
                device.gauge = Some(gauge);
            }
        }
    }

    /// Current ledger contents (display/diagnostics).
    pub async fn snapshot(&self) -> Vec<DeviceLedger> {
        let devices = self.devices.lock().await;
        let mut devices: Vec<DeviceLedger> = devices.values().cloned().collect();
        devices.sort_by(|left, right| left.uuid.cmp(&right.uuid));
        devices
    }

    /// Probe-measured free bytes for a device — the number decisions use.
    pub async fn effective_free_bytes(&self, uuid: &str) -> Option<u64> {
        let devices = self.devices.lock().await;
        devices.get(uuid).and_then(|device| device.gauge.as_ref()).map(|gauge| gauge.free_bytes())
    }

    /// The engine-resolved `n_ctx` for a backend, when the ledger has one —
    /// feeds the compaction context budget.
    pub async fn resolved_context_for(&self, backend: RuntimeBackendId) -> Option<u32> {
        let devices = self.devices.lock().await;
        devices
            .values()
            .flat_map(|device| device.resident.iter())
            .find(|entry| entry.backend == backend)
            .and_then(|entry| entry.resolved_context_length)
            .filter(|value| *value > 0)
    }

    /// The engine-resolved `n_ctx` for a resident model (by model id). Takes
    /// the largest when several backends hold entries for the model.
    pub async fn resolved_context_for_model(&self, model_id: &str) -> Option<u32> {
        let model_id = model_id.trim();
        if model_id.is_empty() {
            return None;
        }
        let devices = self.devices.lock().await;
        devices
            .values()
            .flat_map(|device| device.resident.iter())
            .filter(|entry| entry.model_id.as_deref() == Some(model_id))
            .filter_map(|entry| entry.resolved_context_length)
            .filter(|value| *value > 0)
            .max()
    }
}

fn device_key(uuid: Option<&str>) -> String {
    uuid.filter(|uuid| !uuid.trim().is_empty()).unwrap_or(PRIMARY_KEY).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(backend: RuntimeBackendId) -> LedgerEntry {
        LedgerEntry {
            backend,
            model_id: Some("model-a".to_owned()),
            model_path: "model.gguf".to_owned(),
            num_workers: 1,
            resolved_context_length: Some(8192),
            mmproj_resident: false,
            weights_bytes: Some(1024),
            mmproj_bytes: None,
            measured_delta_bytes: Some(2048),
            recorded_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn load_replaces_same_backend_and_unload_removes() {
        let ledger = ModelLedger::default();
        ledger.note_model_loaded(Some("gpu-0"), entry(RuntimeBackendId::GgmlLlama)).await;
        ledger.note_model_loaded(Some("gpu-0"), entry(RuntimeBackendId::GgmlLlama)).await;
        let snapshot = ledger.snapshot().await;
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].resident.len(), 1, "reload replaces, not appends");

        ledger.note_model_loaded(None, entry(RuntimeBackendId::GgmlWhisper)).await;
        assert_eq!(ledger.snapshot().await.len(), 2, "uuid-less lands on primary slot");

        ledger.note_model_unloaded(RuntimeBackendId::GgmlLlama).await;
        let snapshot = ledger.snapshot().await;
        assert!(snapshot.iter().all(|device| {
            device.resident.iter().all(|entry| entry.backend != RuntimeBackendId::GgmlLlama)
        }));
    }

    #[tokio::test]
    async fn mmproj_toggle_updates_entry() {
        let ledger = ModelLedger::default();
        ledger.note_model_loaded(Some("gpu-0"), entry(RuntimeBackendId::GgmlLlama)).await;

        ledger.note_mmproj(RuntimeBackendId::GgmlLlama, true, Some(4096)).await;
        let snapshot = ledger.snapshot().await;
        let entry = &snapshot[0].resident[0];
        assert!(entry.mmproj_resident);
        assert_eq!(entry.mmproj_bytes, Some(4096));
    }

    #[tokio::test]
    async fn gauges_sync_by_uuid_and_single_uuidless() {
        let ledger = ModelLedger::default();
        ledger.note_model_loaded(Some("gpu-0"), entry(RuntimeBackendId::GgmlLlama)).await;
        ledger.note_model_loaded(None, entry(RuntimeBackendId::GgmlWhisper)).await;

        // A uuid gauge maps to its device; an ambiguous uuid-less gauge
        // alongside another device maps nowhere.
        ledger
            .sync_gauges(&[
                DeviceMemoryGauge {
                    uuid: Some("gpu-0".to_owned()),
                    used_bytes: 4,
                    total_bytes: 10,
                },
                DeviceMemoryGauge { uuid: None, used_bytes: 2, total_bytes: 6 },
            ])
            .await;
        assert_eq!(ledger.effective_free_bytes("gpu-0").await, Some(6));
        assert_eq!(ledger.effective_free_bytes("primary").await, None);

        // A lone uuid-less gauge maps to the primary slot.
        ledger
            .sync_gauges(&[DeviceMemoryGauge { uuid: None, used_bytes: 2, total_bytes: 6 }])
            .await;
        assert_eq!(ledger.effective_free_bytes("primary").await, Some(4));
    }

    #[tokio::test]
    async fn resolved_context_lookup_filters_zero_and_missing() {
        let ledger = ModelLedger::default();
        let mut zeroed = entry(RuntimeBackendId::GgmlLlama);
        zeroed.resolved_context_length = Some(0);
        ledger.note_model_loaded(Some("gpu-0"), zeroed).await;
        assert_eq!(ledger.resolved_context_for(RuntimeBackendId::GgmlLlama).await, None);
        assert_eq!(ledger.resolved_context_for(RuntimeBackendId::GgmlDiffusion).await, None);
    }

    #[tokio::test]
    async fn resolved_context_for_model_matches_and_takes_max() {
        let ledger = ModelLedger::default();
        ledger.note_model_loaded(Some("gpu-0"), entry(RuntimeBackendId::GgmlLlama)).await;

        let mut larger = entry(RuntimeBackendId::GgmlWhisper);
        larger.resolved_context_length = Some(16384);
        ledger.note_model_loaded(Some("gpu-1"), larger).await;

        assert_eq!(ledger.resolved_context_for_model("model-a").await, Some(16384));
        assert_eq!(ledger.resolved_context_for_model("  ").await, None);
        assert_eq!(ledger.resolved_context_for_model("other").await, None);
    }
}

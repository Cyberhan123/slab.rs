//! Pure scheduling policy: memory-pressure predicates, eviction ordering, and
//! OOM message classification. No I/O, no state — hosts execute these over
//! their own snapshots.

use slab_types::RuntimeBackendId;

/// System-or-GPU memory gauge (free = total − used).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryGauge {
    pub used_bytes: u64,
    pub total_bytes: u64,
}

impl MemoryGauge {
    pub fn free_bytes(self) -> u64 {
        self.total_bytes.saturating_sub(self.used_bytes)
    }
}

/// Measured memory state fed into [`is_under_memory_pressure`]. `None` gauges
/// are ignored — a missing signal must not fabricate pressure.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryPressureInput {
    pub system: Option<MemoryGauge>,
    pub gpu: Option<MemoryGauge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressureThresholds {
    pub min_free_system_memory_bytes: u64,
    pub min_free_gpu_memory_bytes: u64,
}

/// True when either gauge's free bytes fall below its threshold.
pub fn is_under_memory_pressure(
    input: MemoryPressureInput,
    thresholds: PressureThresholds,
) -> bool {
    let system_pressure = input
        .system
        .is_some_and(|system| system.free_bytes() < thresholds.min_free_system_memory_bytes);
    let gpu_pressure =
        input.gpu.is_some_and(|gpu| gpu.free_bytes() < thresholds.min_free_gpu_memory_bytes);
    system_pressure || gpu_pressure
}

/// One backend's scheduling state for eviction ordering.
#[derive(Debug, Clone)]
pub struct EvictionCandidate {
    pub backend: RuntimeBackendId,
    pub resident: bool,
    pub active_refs: u64,
    pub last_access_seq: u64,
}

/// Pick the pressure-eviction victim: the oldest-accessed idle, resident
/// runtime-worker backend other than the one being loaded. Active backends
/// are never chosen.
pub fn choose_pressure_eviction_candidate(
    candidates: &[EvictionCandidate],
    target_backend: RuntimeBackendId,
) -> Option<RuntimeBackendId> {
    candidates
        .iter()
        .filter(|candidate| {
            candidate.backend != target_backend
                && candidate.backend.is_runtime_worker_backend()
                && candidate.resident
                && candidate.active_refs == 0
        })
        .min_by(|left, right| {
            left.last_access_seq
                .cmp(&right.last_access_seq)
                .then_with(|| left.backend.canonical_id().cmp(right.backend.canonical_id()))
        })
        .map(|candidate| candidate.backend)
}

/// Needle-list OOM classification over a runtime error message (already
/// lowercased by the caller or not — both work). Hosts combine this with
/// their transport-level error-code checks.
pub fn is_oom_message(message: &str) -> bool {
    let message = message.trim().to_ascii_lowercase();
    [
        "out of memory",
        "not enough memory",
        "insufficient memory",
        "memory allocation",
        "memory",
        "oom",
        "vram",
        "cudaerrormemoryallocation",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_pressure_checks_respect_free_memory_thresholds() {
        let thresholds = PressureThresholds {
            min_free_system_memory_bytes: 1_024,
            min_free_gpu_memory_bytes: 512,
        };
        let input = MemoryPressureInput {
            system: Some(MemoryGauge { used_bytes: 9_500, total_bytes: 10_000 }),
            gpu: Some(MemoryGauge { used_bytes: 7_000, total_bytes: 8_000 }),
        };

        assert!(is_under_memory_pressure(input, thresholds));

        let relaxed = PressureThresholds {
            min_free_system_memory_bytes: 400,
            min_free_gpu_memory_bytes: 400,
        };
        assert!(!is_under_memory_pressure(input, relaxed));
    }

    #[test]
    fn memory_pressure_ignores_missing_gauges() {
        let thresholds =
            PressureThresholds { min_free_system_memory_bytes: 1, min_free_gpu_memory_bytes: 1 };
        assert!(!is_under_memory_pressure(MemoryPressureInput::default(), thresholds));
    }

    #[test]
    fn pressure_eviction_candidate_uses_oldest_idle_resident_backend() {
        let candidates = vec![
            EvictionCandidate {
                backend: RuntimeBackendId::GgmlLlama,
                resident: true,
                active_refs: 0,
                last_access_seq: 10,
            },
            EvictionCandidate {
                backend: RuntimeBackendId::GgmlWhisper,
                resident: true,
                active_refs: 0,
                last_access_seq: 4,
            },
            EvictionCandidate {
                backend: RuntimeBackendId::GgmlDiffusion,
                resident: true,
                active_refs: 1,
                last_access_seq: 1,
            },
        ];

        let candidate =
            choose_pressure_eviction_candidate(&candidates, RuntimeBackendId::GgmlLlama);
        assert_eq!(candidate, Some(RuntimeBackendId::GgmlWhisper));
    }

    #[test]
    fn pressure_eviction_candidate_skips_active_backends() {
        let candidates = vec![EvictionCandidate {
            backend: RuntimeBackendId::GgmlWhisper,
            resident: true,
            active_refs: 1,
            last_access_seq: 1,
        }];

        let candidate =
            choose_pressure_eviction_candidate(&candidates, RuntimeBackendId::GgmlLlama);
        assert_eq!(candidate, None);
    }

    #[test]
    fn oom_message_matches_memory_needles() {
        assert!(is_oom_message("CUDA error: out of memory"));
        assert!(is_oom_message("cudaerrormemoryallocation during alloc"));
        assert!(is_oom_message("  insufficient MEMORY  "));
        assert!(!is_oom_message("queue full: ggml.llama"));
    }
}

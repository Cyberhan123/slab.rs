//! VRAM-aware context sizing — the single implementation of `auto` `n_ctx`
//! math, consumed by both `bin/slab-runtime` (at engine load) and
//! `slab-app-core` (estimates).

/// KV cache element size (f16 k + f16 v are llama.cpp's defaults).
const F16_BYTES: u64 = 2;

/// Everything known at model-load time to size an `auto` context.
#[derive(Debug, Clone)]
pub struct AutoContextInput {
    pub n_ctx_train: Option<u32>,
    pub model_size_bytes: u64,
    pub n_layer: u32,
    pub n_head_kv: u32,
    pub n_embd: u32,
    pub n_head: u32,
    /// Each worker context allocates its own full KV cache, so the per-token
    /// cost scales with the worker count.
    pub num_workers: u32,
    /// Multimodal projector bytes reserved before KV sizing (`use_gpu`
    /// defaults to on, so a resident projector eats the same VRAM budget).
    pub mmproj_bytes: Option<u64>,
    pub free_vram_bytes: Option<u64>,
    /// Headroom reserved above weights + KV (compute buffers, CUDA context,
    /// driver release lag).
    pub vram_buffer_bytes: u64,
    /// Result floored to a multiple of this many tokens.
    pub quantum: u32,
    /// Conservative fallback when no VRAM signal or degenerate dims.
    pub fallback: u32,
}

/// KV bytes per token = 2 (k+v) · n_layer · n_head_kv · head_dim · sizeof(f16).
/// GQA is modeled via `n_head_kv`; degenerate dims yield 0.
pub fn kv_bytes_per_token(n_layer: u32, n_head_kv: u32, n_embd: u32, n_head: u32) -> u64 {
    let head_dim = n_embd.checked_div(n_head).unwrap_or(0);
    n_layer as u64 * 2 * n_head_kv as u64 * head_dim as u64 * F16_BYTES
}

/// Resolve an `auto` context length: the largest `n_ctx` whose KV cache fits
/// in `free_vram_bytes` (minus model weights, the headroom buffer, the
/// projector, and divided across all worker contexts), floored to the token
/// quantum and capped at the model's native training context.
///
/// With no VRAM signal (or degenerate dimensions) it falls back to
/// `min(n_ctx_train, fallback)` to stay OOM-safe.
pub fn resolve_auto_context(input: &AutoContextInput) -> u32 {
    let cap = input.n_ctx_train.unwrap_or(input.fallback);
    let fallback = cap.min(input.fallback);

    let Some(free_vram) = input.free_vram_bytes else {
        return fallback;
    };

    let workers = input.num_workers.max(1) as u64;
    // checked ops also cover the degenerate case (bytes_per_token == 0).
    let per_slot = kv_bytes_per_token(input.n_layer, input.n_head_kv, input.n_embd, input.n_head)
        .checked_mul(workers);
    let budget = free_vram
        .saturating_sub(input.model_size_bytes)
        .saturating_sub(input.vram_buffer_bytes)
        .saturating_sub(input.mmproj_bytes.unwrap_or(0));
    let Some(max_for_vram) =
        per_slot.and_then(|divisor| budget.checked_div(divisor)).map(|value| value as u32)
    else {
        return fallback;
    };
    let quantized = (max_for_vram / input.quantum) * input.quantum;
    quantized.clamp(input.quantum, cap)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    /// head_dim = 4096/32 = 128 → bytes_per_token = 32·2·8·128·2 = 131072.
    fn input(free_vram: Option<u64>) -> AutoContextInput {
        AutoContextInput {
            n_ctx_train: Some(32768),
            model_size_bytes: 5 * GIB,
            n_layer: 32,
            n_head_kv: 8,
            n_embd: 4096,
            n_head: 32,
            num_workers: 1,
            mmproj_bytes: None,
            free_vram_bytes: free_vram,
            vram_buffer_bytes: 2 * GIB,
            quantum: 512,
            fallback: 8192,
        }
    }

    #[test]
    fn auto_context_falls_back_when_no_vram() {
        // No VRAM signal → min(n_ctx_train, 8192).
        assert_eq!(resolve_auto_context(&input(None)), 8192);

        let small_train = AutoContextInput { n_ctx_train: Some(4096), ..input(None) };
        assert_eq!(resolve_auto_context(&small_train), 4096);

        // Unknown training context + no VRAM → fallback default.
        let unknown_train = AutoContextInput { n_ctx_train: None, ..input(None) };
        assert_eq!(resolve_auto_context(&unknown_train), 8192);
    }

    #[test]
    fn auto_context_sizes_to_vram_and_caps_at_training_context() {
        // free 10 GiB − model 5 GiB − buffer 2 GiB = 3 GiB; ÷131072 = 24576.
        let sized = resolve_auto_context(&input(Some(10 * GIB)));
        assert_eq!(sized, 24576);

        // Capped at n_ctx_train when VRAM would allow more.
        let capped = resolve_auto_context(&AutoContextInput {
            n_ctx_train: Some(8192),
            free_vram_bytes: Some(64 * GIB),
            ..input(None)
        });
        assert_eq!(capped, 8192);
    }

    #[test]
    fn auto_context_floors_to_quantum_with_a_minimum() {
        // Budget too small for even one full KV slot → clamped to the 512 minimum.
        let n_ctx = resolve_auto_context(&input(Some(5_500_000_000)));
        assert!(n_ctx >= 512);
        assert_eq!(n_ctx % 512, 0);
    }

    #[test]
    fn auto_context_divides_budget_across_workers() {
        // Same 3 GiB budget split over two worker contexts → 12288.
        let two_workers = AutoContextInput { num_workers: 2, ..input(Some(10 * GIB)) };
        assert_eq!(resolve_auto_context(&two_workers), 12288);
    }

    #[test]
    fn auto_context_reserves_mmproj_before_kv() {
        // A 1 GiB projector shrinks the budget to 2 GiB → 16384.
        let multimodal = AutoContextInput { mmproj_bytes: Some(GIB), ..input(Some(10 * GIB)) };
        assert_eq!(resolve_auto_context(&multimodal), 16384);
    }

    #[test]
    fn auto_context_degenerate_dims_fall_back() {
        // n_head = 0 → head_dim 0 → bytes_per_token 0 → fallback.
        let degenerate = AutoContextInput { n_head: 0, ..input(Some(10 * GIB)) };
        assert_eq!(resolve_auto_context(&degenerate), 8192);
    }
}

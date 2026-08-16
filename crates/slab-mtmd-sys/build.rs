#![allow(clippy::uninlined_format_args)]

use slab_build_utils::generate_vendor_sys_bindings;

fn main() {
    // mtmd ships inside the llama SDK (vendor/llama) as a separate shared
    // library. Reuse the "llama" primary artifact (+ ggml dep) so the vendor
    // download, include dirs and runtime DLL sync are identical to
    // slab-llama-sys. The allowlist restricts emission to mtmd_* / MTMD_*
    // symbols; the llama/ggml types referenced by the mtmd API (llama_model,
    // llama_context, llama_flash_attn_type, …) surface as opaque types local to
    // this crate — no symbol clash with slab-llama-sys (separate crates).
    generate_vendor_sys_bindings(
        "llama",
        &["ggml"],
        "MtmdLib",
        &[],
        Some(&["mtmd_.*", "MTMD_.*"]),
        None,
        Some(&["mtmd_context_params"]),
    )
    .expect("failed to prepare mtmd bindings");
}

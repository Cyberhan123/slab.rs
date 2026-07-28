#![allow(clippy::uninlined_format_args)]

use slab_build_utils::generate_vendor_sys_bindings;

fn main() {
    // parakeet ships inside the whisper SDK (vendor/whisper) as a separate
    // shared library. Reuse the "whisper" primary artifact (+ ggml dep) so the
    // vendor download, include dirs and runtime DLL sync are identical to
    // slab-whisper-sys. The allowlist restricts emission to parakeet_* /
    // PARAKEET_* symbols; the ggml types referenced by the parakeet API
    // (ggml_abort_callback, ggml_log_callback, …) surface as opaque types
    // local to this crate — no symbol clash with slab-whisper-sys (separate
    // crates).
    generate_vendor_sys_bindings(
        "whisper",
        &["ggml"],
        "ParakeetLib",
        &["../../vendor/whisper/include/parakeet.h"],
        Some(&["parakeet_.*", "PARAKEET_.*"]),
        None,
        Some(&["parakeet_token_data"]),
    )
    .expect("failed to prepare parakeet bindings");
}

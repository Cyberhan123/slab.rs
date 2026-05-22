#![allow(clippy::uninlined_format_args)]

use slab_build_utils::generate_vendor_sys_bindings;

fn main() {
    generate_vendor_sys_bindings(
        "whisper",
        &["ggml"],
        "WhisperLib",
        &["../../vendor/whisper/include/whisper.h"],
    )
    .expect("failed to prepare whisper bindings");
}

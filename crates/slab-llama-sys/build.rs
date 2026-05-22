#![allow(clippy::uninlined_format_args)]

use slab_build_utils::generate_vendor_sys_bindings;

fn main() {
    generate_vendor_sys_bindings("llama", &["ggml"], "LlamaLib", &[])
        .expect("failed to prepare llama bindings");
}

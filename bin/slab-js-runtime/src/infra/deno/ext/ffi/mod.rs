use deno_core::{extension, Extension};

use super::ExtensionTrait;

extension!(
    init_ffi,
    deps = [rustyscript],
    esm_entry_point = "ext:init_ffi/init_ffi.js",
    esm = [ dir "src/ext/ffi", "init_ffi.js" ],
);

impl ExtensionTrait<()> for init_ffi {
    fn init((): ()) -> Extension {
        init_ffi::init()
    }
}

impl ExtensionTrait<()> for deno_ffi::deno_ffi {
    fn init((): ()) -> Extension {
        deno_ffi::deno_ffi::init(None)
    }
}

pub fn extensions(is_snapshot: bool) -> Vec<Extension> {
    vec![
        deno_ffi::deno_ffi::build((), is_snapshot),
        init_ffi::build((), is_snapshot),
    ]
}

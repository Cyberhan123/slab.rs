use deno_core::{extension, Extension};

use super::ExtensionTrait;

extension!(
    init_napi,
    deps = [rustyscript],
    esm_entry_point = "ext:init_napi/init_napi.js",
    esm = [ dir "src/ext/napi", "init_napi.js" ],
);

impl ExtensionTrait<()> for init_napi {
    fn init((): ()) -> Extension {
        init_napi::init()
    }
}

impl ExtensionTrait<()> for deno_napi::deno_napi {
    fn init((): ()) -> Extension {
        deno_napi::deno_napi::init(None)
    }
}

pub fn extensions(is_snapshot: bool) -> Vec<Extension> {
    vec![
        deno_napi::deno_napi::build((), is_snapshot),
        init_napi::build((), is_snapshot),
    ]
}

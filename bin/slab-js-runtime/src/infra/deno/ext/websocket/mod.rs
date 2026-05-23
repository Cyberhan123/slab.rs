use deno_core::{extension, Extension};

use super::ExtensionTrait;

extension!(
    init_websocket,
    deps = [rustyscript],
    esm_entry_point = "ext:init_websocket/init_websocket.js",
    esm = [ dir "src/ext/websocket", "init_websocket.js" ],
);

impl ExtensionTrait<()> for init_websocket {
    fn init((): ()) -> Extension {
        init_websocket::init()
    }
}

impl ExtensionTrait<()> for deno_websocket::deno_websocket {
    fn init((): ()) -> Extension {
        deno_websocket::deno_websocket::init()
    }
}

pub fn extensions(is_snapshot: bool) -> Vec<Extension> {
    vec![
        deno_websocket::deno_websocket::build((), is_snapshot),
        init_websocket::build((), is_snapshot),
    ]
}

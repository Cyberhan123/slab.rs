use deno_core::{extension, Extension};

use super::ExtensionTrait;

extension!(
    init_url,
    deps = [rustyscript, deno_web],
    esm_entry_point = "ext:init_url/init_url.js",
    esm = [ dir "src/ext/url", "init_url.js" ],
);
impl ExtensionTrait<()> for init_url {
    fn init((): ()) -> Extension {
        init_url::init()
    }
}

pub fn extensions(is_snapshot: bool) -> Vec<Extension> {
    vec![init_url::build((), is_snapshot)]
}

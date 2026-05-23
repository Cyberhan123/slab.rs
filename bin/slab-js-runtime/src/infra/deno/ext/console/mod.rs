use deno_core::{extension, Extension};

use super::ExtensionTrait;

extension!(
    init_console,
    deps = [rustyscript, deno_web],
    esm_entry_point = "ext:init_console/init_console.js",
    esm = [ dir "src/ext/console", "init_console.js" ],
);
impl ExtensionTrait<()> for init_console {
    fn init((): ()) -> Extension {
        deno_terminal::colors::set_use_color(true);
        init_console::init()
    }
}

pub fn extensions(is_snapshot: bool) -> Vec<Extension> {
    vec![<init_console as ExtensionTrait<()>>::build((), is_snapshot)]
}

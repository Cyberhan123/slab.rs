use std::sync::Arc;

use deno_core::{Extension, extension};
use deno_process::deno_process;
use deno_resolver::npm::DenoInNpmPackageChecker;
use resolvers::{RustyNpmPackageFolderResolver, RustyResolver};
use sys_traits::impls::RealSys;

use super::ExtensionTrait;

mod cjs_translator;
pub mod resolvers;
pub use cjs_translator::NodeCodeTranslator;

#[cfg(not(feature = "deno_runtime"))]
#[deno_core::op2(fast)]
fn op_bootstrap_color_depth() -> i32 {
    24
}

#[cfg(not(feature = "deno_runtime"))]
#[deno_core::op2(fast)]
fn op_current_thread_cpu_usage(#[buffer] out: &mut [f64]) {
    if out.len() >= 2 {
        out[0] = 0.0;
        out[1] = 0.0;
    }
}

#[cfg(not(feature = "deno_runtime"))]
extension!(
    runtime,
    ops = [
        op_bootstrap_color_depth,
        op_current_thread_cpu_usage,
    ],
    esm = [ dir "src/infra/deno/ext/node/runtime_stub", "98_global_scope_shared.js" ],
);
#[cfg(not(feature = "deno_runtime"))]
impl ExtensionTrait<()> for runtime {
    fn init((): ()) -> Extension {
        runtime::init()
    }
}

extension!(
    init_node,
    deps = [rustyscript, deno_web],
    esm_entry_point = "ext:init_node/init_node.js",
    esm = [ dir "src/infra/deno/ext/node", "init_node.js" ],
);
impl ExtensionTrait<()> for init_node {
    fn init((): ()) -> Extension {
        init_node::init()
    }
}
impl ExtensionTrait<Arc<RustyResolver>> for deno_node::deno_node {
    fn init(resolver: Arc<RustyResolver>) -> Extension {
        deno_node::deno_node::init::<DenoInNpmPackageChecker, RustyNpmPackageFolderResolver, RealSys>(
            Some(resolver.init_services()),
            resolver.filesystem(),
        )
    }
}

impl ExtensionTrait<Option<deno_os::ExitCode>> for deno_os::deno_os {
    fn init(exit_code: Option<deno_os::ExitCode>) -> Extension {
        deno_os::deno_os::init(exit_code)
    }
}

impl ExtensionTrait<()> for deno_node_sqlite::deno_node_sqlite {
    fn init((): ()) -> Extension {
        deno_node_sqlite::deno_node_sqlite::init()
    }
}

impl ExtensionTrait<Arc<RustyResolver>> for deno_process {
    fn init(resolver: Arc<RustyResolver>) -> Extension {
        deno_process::init(Some(resolver))
    }
}

pub fn extensions(resolver: Arc<RustyResolver>, is_snapshot: bool) -> Vec<Extension> {
    let mut extensions = Vec::new();

    #[cfg(not(feature = "deno_runtime"))]
    extensions.push(runtime::build((), is_snapshot));

    extensions.extend([
        deno_os::deno_os::build(None, is_snapshot),
        deno_process::build(resolver.clone(), is_snapshot),
        deno_node_sqlite::deno_node_sqlite::build((), is_snapshot),
        deno_node::deno_node::build(resolver, is_snapshot),
        init_node::build((), is_snapshot),
    ]);
    extensions
}

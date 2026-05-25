//！This file originally from the `rustyscript` crate, and has been modified for use in this project.
//! ![Rustyscript - Effortless JS Integration for Rust](https://raw.githubusercontent.com/rscarson/rustyscript/refs/heads/master/.github/rustyscript-logo-wide.png)
//!
//! [![Crates.io](https://img.shields.io/crates/v/rustyscript.svg)](https://crates.io/crates/rustyscript/)
//! [![Build Status](https://github.com/rscarson/rustyscript/actions/workflows/tests.yml/badge.svg?branch=master)](https://github.com/rscarson/rustyscript/actions?query=branch%3Amaster)
//! [![docs.rs](https://img.shields.io/docsrs/rustyscript)](https://docs.rs/rustyscript/latest/rustyscript/)
//! [![Static Badge](https://img.shields.io/badge/mdbook-user%20guide-blue)](https://rscarson.github.io/rustyscript-book/)
//! [![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://raw.githubusercontent.com/rscarson/rustyscript/master/LICENSE)
//!
//! ## Rustyscript - Effortless JS Integration for Rust
//!
//! rustyscript provides a quick and simple way to integrate a runtime javascript or typescript component from within Rust.
//!
//! It uses the v8 engine through the `deno_core` crate, and aims to be as simple as possible to use without sacrificing flexibility or performance.  
//! I also have attempted to abstract away the v8 engine details so you can for the most part operate directly on rust types.
//!
//!
//! **Sandboxed**  
//! By default, the code being run is entirely sandboxed from the host, having no filesystem or network access.  
//! [extensions](https://rscarson.github.io/rustyscript-book/extensions) can be added to grant additional capabilities that may violate sandboxing
//!
//! **Flexible**  
//! The runtime is designed to be as flexible as possible, allowing you to modify capabilities, the module loader, and more.  
//! - Asynchronous JS is fully supported, and the runtime can be configured to run in a multithreaded environment.  
//! - Typescript is supported, and will be transpired into JS for execution.
//! - Node JS is supported experimentally, but is not yet fully compatible ([See the `NodeJS` Compatibility section](https://rscarson.github.io/rustyscript-book/advanced/nodejs_compatibility.md))
//!
//! **Unopinionated**  
//! Rustyscript is designed to be a thin wrapper over the Deno runtime, to remove potential pitfalls and simplify the API without sacrificing flexibility or performance.
//!
//! -----
//!
//! Here is a very basic use of this crate to execute a JS module. It will:
//! - Create a basic runtime
//! - Load a javascript module,
//! - Call a function registered as the entrypoint
//! - Return the resulting value
//! ```ignore
//! use rustyscript::{json_args, Runtime, Module, Error};
//!
//! # fn main() -> Result<(), Error> {
//! let module = Module::new(
//!     "test.js",
//!     "
//!     export default (string, integer) => {
//!         console.log(`Hello world: string=${string}, integer=${integer}`);
//!         return 2;
//!     }
//!     "
//! );
//!
//! let value: usize = Runtime::execute_module(
//!     &module, vec![],
//!     Default::default(),
//!     json_args!("test", 5)
//! )?;
//!
//! assert_eq!(value, 2);
//! # Ok(())
//! # }
//! ```
//!
//! Modules can also be loaded from the filesystem with [`Module::load`] or [`Module::load_dir`] if you want to collect all modules in a given directory.
//!
//! ----
//!
//! If all you need is the result of a single javascript expression, you can use:
//! ```ignore
//! let result: i64 = rustyscript::evaluate("5 + 5").expect("The expression was invalid!");
//! ```
//!
//! Or to just import a single module for use:
//! ```ignore
//! use rustyscript::{json_args, import};
//! let mut module = import("js/my_module.js").expect("Something went wrong!");
//! let value: String = module.call("exported_function_name", json_args!()).expect("Could not get a value!");
//! ```
//!
//! There are a few other utilities included, such as [`validate`] and [`resolve_path`]
//!
//! ----
//!
//! A more detailed version of the crate's usage can be seen below, which breaks down the steps instead of using the one-liner [`Runtime::execute_module`]:
//! ```ignore
//! use rustyscript::{json_args, Runtime, RuntimeOptions, Module, Error, Undefined};
//! use std::time::Duration;
//!
//! # fn main() -> Result<(), Error> {
//! let module = Module::new(
//!     "test.js",
//!     "
//!     let internalValue = 0;
//!     export const load = (value) => internalValue = value;
//!     export const getValue = () => internalValue;
//!     "
//! );
//!
//! // Create a new runtime
//! let mut runtime = Runtime::new(RuntimeOptions {
//!     timeout: Duration::from_millis(50), // Stop execution by force after 50ms
//!     default_entrypoint: Some("load".to_string()), // Run this as the entrypoint function if none is registered
//!     ..Default::default()
//! })?;
//!
//! // The handle returned is used to get exported functions and values from that module.
//! // We then call the entrypoint function, but do not need a return value.
//! //Load can be called multiple times, and modules can import other loaded modules
//! // Using `import './filename.js'`
//! let module_handle = runtime.load_module(&module)?;
//! runtime.call_entrypoint::<Undefined>(&module_handle, json_args!(2))?;
//!
//! // Functions don't need to be the entrypoint to be callable!
//! let internal_value: i64 = runtime.call_function(Some(&module_handle), "getValue", json_args!())?;
//! # Ok(())
//! # }
//! ```
//!
//! There are also '_async' and 'immediate' versions of most runtime functions;
//! '_async' functions return a future that resolves to the result of the operation, while
//! '_immediate' functions will make no attempt to wait for the event loop, making them suitable
//! for using [`crate::js_value::Promise`]
//!
//! Rust functions can also be registered to be called from javascript:
//! ```ignore
//! use rustyscript::{ Runtime, Module, serde_json::Value };
//!
//! # fn main() -> Result<(), rustyscript::Error> {
//! let module = Module::new("test.js", " rustyscript.functions.foo(); ");
//! let mut runtime = Runtime::new(Default::default())?;
//! runtime.register_function("foo", |args| {
//!     if let Some(value) = args.get(0) {
//!         println!("called with: {}", value);
//!     }
//!     Ok(Value::Null)
//! })?;
//! runtime.load_module(&module)?;
//! # Ok(())
//! # }
//! ```
//!
//! ----
//!
//! Asynchronous JS can be called in 2 ways;
//!
//! The first is to use the 'async' keyword in JS, and then call the function using [`Runtime::call_function_async`]
//! ```ignore
//! use rustyscript::{ Runtime, Module, json_args };
//!
//! # fn main() -> Result<(), rustyscript::Error> {
//! let module = Module::new("test.js", "export async function foo() { return 5; }");
//! let mut runtime = Runtime::new(Default::default())?;
//!
//! // The runtime has its own tokio runtime; you can get a handle to it with [Runtime::tokio_runtime]
//! // You can also build the runtime with your own tokio runtime, see [Runtime::with_tokio_runtime]
//! let tokio_runtime = runtime.tokio_runtime();
//!
//! let result: i32 = tokio_runtime.block_on(async {
//!     // Top-level await is supported - we can load modules asynchronously
//!     let handle = runtime.load_module_async(&module).await?;
//!
//!     // Call the function asynchronously
//!     runtime.call_function_async(Some(&handle), "foo", json_args!()).await
//! })?;
//!
//! assert_eq!(result, 5);
//! # Ok(())
//! # }
//! ```
//!
//! The second is to use [`crate::js_value::Promise`]
//! ```ignore
//! use rustyscript::{ Runtime, Module, js_value::Promise, json_args };
//!
//! # fn main() -> Result<(), rustyscript::Error> {
//! let module = Module::new("test.js", "export async function foo() { return 5; }");
//!
//! let mut runtime = Runtime::new(Default::default())?;
//! let handle = runtime.load_module(&module)?;
//!
//! // We call the function without waiting for the event loop to run, or for the promise to resolve
//! // This way we can store it and wait for it later, without blocking the event loop or borrowing the runtime
//! let result: Promise<i32> = runtime.call_function_immediate(Some(&handle), "foo", json_args!())?;
//!
//! // We can then wait for the promise to resolve
//! // We can do so asynchronously, using [crate::js_value::Promise::into_future]
//! // But we can also block the current thread:
//! let result = result.into_value(&mut runtime)?;
//! assert_eq!(result, 5);
//! # Ok(())
//! # }
//! ```
//!
//! - See [`Runtime::register_async_function`] for registering and calling async rust from JS
//! - See `examples/async_javascript.rs` for a more detailed example of using async JS
//!
//! ----
//!
//! For better performance calling rust code, consider using an extension instead of a module - see the `runtime_extensions` example for details
//!
//! ----
//!
//! A threaded worker can be used to run code in a separate thread, or to allow multiple concurrent runtimes.
//!
//! the [`worker`] module provides a simple interface to create and interact with workers.
//! The [`worker::InnerWorker`] trait can be implemented to provide custom worker behavior.
//!
//! It also provides a default worker implementation that can be used without any additional setup:
//! ```ignore
//! use rustyscript::{Error, worker::{Worker, DefaultWorker, DefaultWorkerOptions}};
//! use std::time::Duration;
//!
//! fn main() -> Result<(), Error> {
//!     let worker = DefaultWorker::new(DefaultWorkerOptions {
//!         default_entrypoint: None,
//!         timeout: Duration::from_secs(5),
//!     })?;
//!
//!     let result: i32 = worker.eval("5 + 5".to_string())?;
//!     assert_eq!(result, 10);
//!     Ok(())
//! }
//! ```
//!
//! ----
//!
//! ## Utility Functions
//! These functions provide simple one-liner access to common features of this crate:
//! - `evaluate`; Evaluate a single JS expression and return the resulting value
//! - `import`; Get a handle to a JS module from which you can get exported values and functions
//! - `resolve_path`; Resolve a relative path to the current working dir
//! - `validate`; Validate the syntax of a JS expression
//! - `init_platform`; Initialize the V8 platform for multi-threaded applications
//!
//! Commonly used features have been grouped into the following feature-sets:
//! - **`safe_extensions`** - On by default, these extensions are safe to use in a sandboxed environment
//! - **`network_extensions`** - These extensions break sandboxing by allowing network connectivity
//! - **`io_extensions`** - These extensions break sandboxing by allowing filesystem access (WARNING: Also allows some network access)
//! - **`all_extensions`** - All 3 above groups are included
//! - **`extra_features`** - Enables the `worker` feature (enabled by default), and the `snapshot_builder` feature
//! - **`node_experimental`** - HIGHLY EXPERIMENTAL nodeJS support that enables all available Deno extensions
//!
//! ## Crate features
//! The table below lists the available features for this crate. Features marked at `Preserves Sandbox: NO` break isolation between loaded JS modules and the host system.
//! Use with caution.
//!
//! More details on the features can be found in `Cargo.toml`
//!
//! Please note that the `web` feature will also enable `fs_import` and `url_import`, allowing arbitrary filesystem and network access for import statements
//! - This is because the `deno_web` crate allows both fetch and FS reads already
//!
//! | Feature           | Description                                                                                               | Preserves Sandbox| Dependencies                                                                                  |  
//! |-------------------|-----------------------------------------------------------------------------------------------------------|------------------|-----------------------------------------------------------------------------------------------|
//! |`broadcast_channel`|Implements the web-messaging API for Deno                                                                  |**NO**            |`deno_broadcast_channel`, `deno_web`, `deno_webidl`                                            |
//! |`cache`            |Implements the Cache API for Deno                                                                          |**NO**            |`deno_cache`, `deno_webidl`, `deno_web`, `deno_crypto`, `deno_fetch`, `deno_url`, `deno_net`   |
//! |`console`          |Provides `console.*` functionality from JS                                                                 |yes               |`deno_console`, `deno_terminal`                                                                |
//! |`cron`             |Implements scheduled tasks (crons) API                                                                     |**NO**            |`deno_cron`, `deno_console`                                                                    |
//! |`crypto`           |Provides `crypto.*` functionality from JS                                                                  |yes               |`deno_crypto`, `deno_webidl`                                                                   |
//! |`ffi`              |Dynamic library ffi features                                                                               |**NO**            |`deno_ffi`                                                                                     |
//! |`fs`               |Provides ops for interacting with the file system.                                                         |**NO**            |`deno_fs`, `web`,  `io`                                                                        |
//! |`http`             |Implements the fetch standard                                                                              |**NO**            |`deno_http`, `web`, `websocket`                                                                |
//! |`kv`               |Implements the Deno KV Connect protocol                                                                    |**NO**            |`deno_kv`, `web`, `console`                                                                    |
//! |`url`              |Provides the `URL`, and `URLPattern` APIs from within JS                                                   |yes               |`deno_webidl`, `deno_url`                                                                      |
//! |`io`               |Provides IO primitives such as stdio streams and abstraction over File System files.                       |**NO**            |`deno_io`, `rustyline`, `winapi`, `nix`, `libc`, `once_cell`                                   |
//! |`web`              |Provides the `Event`, `TextEncoder`, `TextDecoder`, `File`, Web Cryptography, and fetch APIs from within JS|**NO**            |`deno_webidl`, `deno_web`, `deno_crypto`, `deno_fetch`, `deno_url`, `deno_net`                 |
//! |`webgpu`           |Implements the WebGPU API                                                                                  |**NO**            |`deno_webgpu`, `web`                                                                           |
//! |`webstorage`       |Provides the `WebStorage` API                                                                              |**NO**            |`deno_webidl`, `deno_webstorage`                                                               |
//! |`websocket`        |Provides the `WebSocket` API                                                                               |**NO**            |`deno_web`, `deno_websocket`                                                                   |
//! |`webidl`           |Provides the `webidl` API                                                                                  |yes               |`deno_webidl`                                                                                  |
//! |                   |                                                                                                           |                  |                                                                                               |
//! |`default`          |Provides only those extensions that preserve sandboxing                                                    |yes               |`deno_console`, `deno_crypto`, `deno_webidl`, `deno_url`                                       |
//! |`fs_import`        |Enables importing arbitrary code from the filesystem through JS                                            |**NO**            |None                                                                                           |
//! |`url_import`       |Enables importing arbitrary code from network locations through JS                                         |**NO**            |`reqwest`                                                                                      |
//! |                   |                                                                                                           |                  |                                                                                               |
//! |`node_experimental`|HIGHLY EXPERIMENTAL nodeJS support backed by deno_runtime extensions                                       |**NO**            |For complete list, see Cargo.toml                                                              |
//! |                   |                                                                                                           |                  |                                                                                               |
//! |`worker`           |Enables access to the threaded worker API [`worker`]                                                       |yes               |None                                                                                           |
//! |`snapshot_builder` |Enables access to [`SnapshotBuilder`], a runtime for creating snapshots that can improve start-times       |yes               |None                                                                                           |
//! |`web_stub`         |Enables a subset of `web` features that do not break sandboxing                                            |yes               |`deno_webidl`                                                                                  |
//!
//! ----
//!
//! For an example of this crate in use, see [Lavendeux](https://github.com/rscarson/lavendeux)
//!
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)] //   Does not account for crate-level re-exports
#![allow(clippy::inline_always)] //             Does not account for deno_core's use of inline(always) on op2
#![allow(clippy::needless_pass_by_value)] //    Disabling some features can trigger this
#![allow(clippy::result_large_err)] //          Some Deno types trigger this
#![allow(clippy::doc_comment_double_space_linebreaks)]
#![allow(clippy::doc_overindented_list_items)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "snapshot_builder")]
mod snapshot_builder;

#[cfg(feature = "snapshot_builder")]
#[cfg_attr(docsrs, doc(cfg(feature = "snapshot_builder")))]
pub use snapshot_builder::SnapshotBuilder;

mod runtime_builder;
pub use runtime_builder::RuntimeBuilder;

pub mod error;
pub mod js_value;
pub mod module_loader;
pub mod static_runtime;

pub(crate) mod async_bridge;
pub(crate) mod ext;
pub(crate) mod inner_runtime;
mod module;
mod module_handle;
mod module_wrapper;
mod runtime;
pub(crate) mod traits;
pub(crate) mod transpiler;
pub(crate) mod utilities;

/// Re-exports of the deno extension crates used by this library
pub mod extensions {
    #[cfg(feature = "broadcast_channel")]
    #[cfg_attr(docsrs, doc(cfg(feature = "broadcast_channel")))]
    pub use deno_broadcast_channel;

    #[cfg(feature = "cache")]
    #[cfg_attr(docsrs, doc(cfg(feature = "cache")))]
    pub use deno_cache;

    #[cfg(feature = "console")]
    #[cfg_attr(docsrs, doc(cfg(feature = "console")))]
    pub use deno_console;

    #[cfg(feature = "cron")]
    #[cfg_attr(docsrs, doc(cfg(feature = "cron")))]
    pub use deno_cron;

    #[cfg(feature = "crypto")]
    #[cfg_attr(docsrs, doc(cfg(feature = "crypto")))]
    pub use deno_crypto;

    #[cfg(feature = "ffi")]
    #[cfg_attr(docsrs, doc(cfg(feature = "ffi")))]
    pub use deno_ffi;

    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    pub use deno_fs;

    #[cfg(feature = "http")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http")))]
    pub use deno_http;

    #[cfg(feature = "io")]
    #[cfg_attr(docsrs, doc(cfg(feature = "io")))]
    pub use deno_io;

    #[cfg(feature = "kv")]
    #[cfg_attr(docsrs, doc(cfg(feature = "kv")))]
    pub use deno_kv;

    #[cfg(feature = "url")]
    #[cfg_attr(docsrs, doc(cfg(feature = "url")))]
    pub use deno_url;

    #[cfg(feature = "webgpu")]
    #[cfg_attr(docsrs, doc(cfg(feature = "webgpu")))]
    pub use deno_webgpu;

    #[cfg(feature = "websocket")]
    #[cfg_attr(docsrs, doc(cfg(feature = "websocket")))]
    pub use deno_websocket;

    #[cfg(feature = "webstorage")]
    #[cfg_attr(docsrs, doc(cfg(feature = "webstorage")))]
    pub use deno_webstorage;

    #[cfg(feature = "web")]
    #[cfg_attr(docsrs, doc(cfg(feature = "webstorage")))]
    pub use deno_tls;
}

#[cfg(feature = "kv")]
#[cfg_attr(docsrs, doc(cfg(feature = "kv")))]
pub use ext::kv::{KvConfig, KvStore};

//#[cfg(feature = "cache")]
//#[cfg_attr(docsrs, doc(cfg(feature = "cache")))]
//pub use ext::cache::CacheBackend;

#[cfg(feature = "node_experimental")]
#[cfg_attr(docsrs, doc(cfg(feature = "node_experimental")))]
pub use ext::node::resolvers::RustyResolver;

pub use ext::ExtensionOptions;
#[cfg(feature = "web")]
#[cfg_attr(docsrs, doc(cfg(feature = "web")))]
pub use ext::web::{
    AllowlistWebPermissions, CheckedPath, DefaultWebPermissions, PermissionCheckError,
    PermissionDeniedError, PermissionsOptions, SystemsPermissionKind, WebOptions, WebPermissions,
    to_permissions_options,
};

// Expose some important stuff from us
pub use async_bridge::TokioRuntime;
pub use error::Error;
pub use inner_runtime::{RsAsyncFunction, RsFunction};
pub use module::Module;
pub use module_handle::ModuleHandle;
pub use module_wrapper::ModuleWrapper;
pub use runtime::{Runtime, RuntimeOptions, Undefined};
pub use utilities::{evaluate, import, init_platform, resolve_path, validate};

// Deprecated traits for backward compatibility
#[allow(deprecated)]
#[deprecated(since = "0.8.0", note = "Use v8::String::new() directly")]
pub use traits::ToV8String;

#[cfg(feature = "broadcast_channel")]
#[cfg_attr(docsrs, doc(cfg(feature = "broadcast_channel")))]
pub use ext::broadcast_channel::{
    BroadcastChannelWrapper, IsolatedBroadcastChannel, IsolatedBroadcastChannelWrapper,
};

#[cfg(feature = "web")]
#[cfg_attr(docsrs, doc(cfg(feature = "web")))]
pub use hyper_util;

#[cfg(feature = "op_whitelist")]
pub mod op_whitelist;

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, anyhow, bail};
use deno_ast::{
    DecoratorsTranspileOption, EmitOptions, ImportsNotUsedAsValues, MediaType, ParseParams,
    SourceMapOption, TranspileModuleOptions, TranspileOptions,
};
use deno_core::{
    JsRuntime, ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader,
    ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType, OpState, ResolutionKind,
    RuntimeOptions as DenoRuntimeOptions, error::ModuleLoaderError, op2, resolve_import,
};
use deno_error::JsErrorBox;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slab_types::{
    DESKTOP_API_HOST, DESKTOP_API_PORT, PluginApiRequest, PluginNetworkMode,
    PluginRuntimeApiHostRequest, PluginRuntimeCallRequest, PluginRuntimeCallResponse,
    PluginRuntimeFileAccess, PluginRuntimeFileGrant, PluginRuntimeUiEmitRequest,
};

use crate::application::PluginExecutor;
use crate::domain::RuntimeHost;

const EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_FETCH_RESPONSE_BYTES: usize = 1024 * 1024;
const BOOTSTRAP_JS: &str = include_str!("bootstrap.js");

/// Executes JavaScript plugins inside the embedded Deno runtime.
pub struct DenoPluginExecutor {
    host: Arc<dyn RuntimeHost>,
    http: reqwest::Client,
}

impl DenoPluginExecutor {
    /// Creates a new executor bound to the provided runtime host.
    pub fn new(host: Arc<dyn RuntimeHost>) -> Self {
        let http = reqwest::Client::builder().timeout(FETCH_TIMEOUT).build().unwrap_or_default();
        Self { host, http }
    }
}

#[async_trait::async_trait]
impl PluginExecutor for DenoPluginExecutor {
    async fn execute(
        &self,
        request: PluginRuntimeCallRequest,
    ) -> Result<PluginRuntimeCallResponse, anyhow::Error> {
        let host = self.host.clone();
        let http = self.http.clone();
        let result = tokio::task::spawn_blocking(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .context("failed to create JS plugin runtime thread")?;
            runtime.block_on(async move {
                tokio::time::timeout(EXECUTION_TIMEOUT, execute_call(request, host, http))
                    .await
                    .map_err(|_| {
                        anyhow!("JS plugin execution timed out after {EXECUTION_TIMEOUT:?}")
                    })?
            })
        })
        .await
        .context("JS plugin runtime worker failed")??;
        Ok(PluginRuntimeCallResponse { result })
    }
}

#[derive(Clone)]
struct ExecutionContext {
    call_id: String,
    plugin_id: String,
    permissions: slab_types::PluginPermissionsManifest,
    file_grants: Vec<PluginRuntimeFileGrant>,
    blocked_fetch_origins: Vec<String>,
}

#[derive(Clone)]
struct RuntimeState {
    context: ExecutionContext,
    host: Arc<dyn RuntimeHost>,
    http: reqwest::Client,
    result: Arc<Mutex<Option<Value>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchRequest {
    url: String,
    #[serde(default = "default_fetch_method")]
    method: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FetchResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UiEmitRequest {
    topic: String,
    #[serde(default)]
    data: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteFileRequest {
    path: String,
    bytes: Vec<u8>,
}

async fn execute_call(
    request: PluginRuntimeCallRequest,
    host: Arc<dyn RuntimeHost>,
    http: reqwest::Client,
) -> Result<Value, anyhow::Error> {
    validate_entry_extension(&request.entry)?;
    let root_dir = PathBuf::from(&request.root_dir);
    let entry_path = root_dir.join(&request.entry);
    ensure_path_within_root(&root_dir, &entry_path)?;
    if !entry_path.is_file() {
        bail!("plugin entry does not exist at {}", entry_path.display());
    }

    let result = Arc::new(Mutex::new(None));
    let context = ExecutionContext {
        call_id: request.call_id.clone(),
        plugin_id: request.plugin_id.clone(),
        permissions: request.permissions.clone(),
        file_grants: request.file_grants.clone(),
        blocked_fetch_origins: request.blocked_fetch_origins.clone(),
    };
    let state = RuntimeState { context, host, http, result: result.clone() };

    let source_maps = Rc::new(RefCell::new(HashMap::new()));
    let mut runtime = JsRuntime::new(DenoRuntimeOptions {
        module_loader: Some(Rc::new(SlabModuleLoader {
            root_dir: root_dir.clone(),
            source_maps: source_maps.clone(),
        })),
        extensions: vec![slab_extension::init()],
        ..Default::default()
    });
    runtime.op_state().borrow_mut().put(state);
    runtime.execute_script("slab:bootstrap", BOOTSTRAP_JS)?;

    let main_module = ModuleSpecifier::from_file_path(root_dir.join("__slab_plugin_call__.mjs"))
        .map_err(|()| anyhow!("failed to build plugin wrapper module URL"))?;
    let entry_specifier = ModuleSpecifier::from_file_path(&entry_path).map_err(|()| {
        anyhow!("failed to convert entry path to file URL: {}", entry_path.display())
    })?;
    let wrapper = build_wrapper_module(
        entry_specifier.as_str(),
        request.export_name.as_str(),
        &request.params,
    )?;

    let mod_id = runtime.load_main_es_module_from_code(&main_module, wrapper).await?;
    let evaluation = runtime.mod_evaluate(mod_id);
    runtime.run_event_loop(deno_core::PollEventLoopOptions::default()).await?;
    evaluation.await?;

    let result = result
        .lock()
        .map_err(|_| anyhow!("failed to lock JS plugin result"))?
        .take()
        .unwrap_or(Value::Null);
    Ok(result)
}

deno_core::extension!(
    slab_extension,
    ops = [
        op_slab_plugin_id,
        op_slab_set_result,
        op_slab_api_request,
        op_slab_ui_emit,
        op_slab_fetch,
        op_slab_read_file,
        op_slab_write_file,
        op_slab_decode_utf8,
        op_slab_encode_utf8,
    ],
);

fn build_wrapper_module(
    entry_specifier: &str,
    export_name: &str,
    params: &Value,
) -> Result<String, anyhow::Error> {
    let entry_json = serde_json::to_string(entry_specifier)?;
    let export_json = serde_json::to_string(export_name)?;
    let params_json = serde_json::to_string(params)?;
    Ok(format!(
        r#"
const module = await import({entry_json});
const exportName = {export_json};
const target = module[exportName];
if (typeof target !== "function") {{
  throw new Error(`Plugin does not export function: ${{exportName}}`);
}}
const result = await target({params_json});
Deno.core.ops.op_slab_set_result(result === undefined ? null : result);
"#
    ))
}

struct SlabModuleLoader {
    root_dir: PathBuf,
    source_maps: Rc<RefCell<HashMap<String, Vec<u8>>>>,
}

impl ModuleLoader for SlabModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
        let resolved = resolve_import(specifier, referrer).map_err(JsErrorBox::from_err)?;
        if resolved.scheme() != "file" {
            return Err(JsErrorBox::generic("Only file:// module imports are supported."));
        }
        let path = resolved
            .to_file_path()
            .map_err(|()| JsErrorBox::generic("Invalid file module specifier."))?;
        ensure_path_within_root(&self.root_dir, &path)
            .map_err(|error| JsErrorBox::generic(error.to_string()))?;
        let _ = kind;
        Ok(resolved)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleLoadReferrer>,
        _options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
        let source_maps = self.source_maps.clone();
        let root_dir = self.root_dir.clone();
        let specifier = module_specifier.clone();
        ModuleLoadResponse::Sync(load_module(root_dir, source_maps, &specifier))
    }

    fn get_source_map(&self, specifier: &str) -> Option<Cow<'_, [u8]>> {
        self.source_maps.borrow().get(specifier).map(|value| value.clone().into())
    }
}

fn load_module(
    root_dir: PathBuf,
    source_maps: Rc<RefCell<HashMap<String, Vec<u8>>>>,
    module_specifier: &ModuleSpecifier,
) -> Result<ModuleSource, ModuleLoaderError> {
    let path = module_specifier
        .to_file_path()
        .map_err(|()| JsErrorBox::generic("Only file:// module imports are supported."))?;
    ensure_path_within_root(&root_dir, &path)
        .map_err(|error| JsErrorBox::generic(error.to_string()))?;

    let media_type = MediaType::from_path(&path);
    let (module_type, should_transpile) = match media_type {
        MediaType::JavaScript | MediaType::Mjs => (ModuleType::JavaScript, false),
        MediaType::Jsx | MediaType::TypeScript | MediaType::Tsx | MediaType::Mts => {
            (ModuleType::JavaScript, true)
        }
        MediaType::Json => (ModuleType::Json, false),
        _ => {
            return Err(JsErrorBox::generic(format!(
                "Unsupported plugin module extension {:?}",
                path.extension()
            )));
        }
    };

    let code = std::fs::read_to_string(&path).map_err(JsErrorBox::from_err)?;
    let code = if should_transpile {
        let parsed = deno_ast::parse_module(ParseParams {
            specifier: module_specifier.clone(),
            text: code.into(),
            media_type,
            capture_tokens: false,
            scope_analysis: false,
            maybe_syntax: None,
        })
        .map_err(JsErrorBox::from_err)?;
        let emitted = parsed
            .transpile(
                &TranspileOptions {
                    imports_not_used_as_values: ImportsNotUsedAsValues::Remove,
                    decorators: DecoratorsTranspileOption::Ecma,
                    ..Default::default()
                },
                &TranspileModuleOptions { module_kind: None },
                &EmitOptions {
                    source_map: SourceMapOption::Separate,
                    inline_sources: true,
                    ..Default::default()
                },
            )
            .map_err(JsErrorBox::from_err)?
            .into_source();
        if let Some(source_map) = emitted.source_map {
            source_maps.borrow_mut().insert(module_specifier.to_string(), source_map.into_bytes());
        }
        emitted.text
    } else {
        code
    };

    Ok(ModuleSource::new(
        module_type,
        ModuleSourceCode::String(code.into()),
        module_specifier,
        None,
    ))
}

#[op2]
#[string]
fn op_slab_plugin_id(state: &mut OpState) -> String {
    state.borrow::<RuntimeState>().context.plugin_id.clone()
}

#[op2]
fn op_slab_set_result(
    state: &mut OpState,
    #[serde] value: serde_json::Value,
) -> Result<(), JsErrorBox> {
    let result = state.borrow::<RuntimeState>().result.clone();
    *result.lock().map_err(|_| JsErrorBox::generic("failed to lock result"))? = Some(value);
    Ok(())
}

#[op2]
#[serde]
async fn op_slab_api_request(
    state: Rc<RefCell<OpState>>,
    #[serde] request: PluginApiRequest,
) -> Result<serde_json::Value, JsErrorBox> {
    let state = {
        let state = state.borrow();
        state.borrow::<RuntimeState>().clone()
    };
    let payload = PluginRuntimeApiHostRequest {
        call_id: state.context.call_id,
        plugin_id: state.context.plugin_id,
        request,
    };
    let params = serde_json::to_value(payload).map_err(JsErrorBox::from_err)?;
    state.host.request("slab.api.request", params).await.map_err(JsErrorBox::generic)
}

#[op2]
#[serde]
async fn op_slab_ui_emit(
    state: Rc<RefCell<OpState>>,
    #[serde] request: UiEmitRequest,
) -> Result<serde_json::Value, JsErrorBox> {
    let state = {
        let state = state.borrow();
        state.borrow::<RuntimeState>().clone()
    };
    let payload = PluginRuntimeUiEmitRequest {
        call_id: state.context.call_id,
        plugin_id: state.context.plugin_id,
        topic: request.topic,
        data: request.data,
    };
    let params = serde_json::to_value(payload).map_err(JsErrorBox::from_err)?;
    state.host.request("slab.ui.emit", params).await.map_err(JsErrorBox::generic)
}

#[op2]
#[serde]
async fn op_slab_fetch(
    state: Rc<RefCell<OpState>>,
    #[serde] request: FetchRequest,
) -> Result<FetchResponse, JsErrorBox> {
    let state = {
        let state = state.borrow();
        state.borrow::<RuntimeState>().clone()
    };
    authorize_fetch(&state.context, &request.url).map_err(JsErrorBox::generic)?;

    let method = reqwest::Method::from_bytes(request.method.as_bytes())
        .map_err(|error| JsErrorBox::generic(format!("invalid fetch method: {error}")))?;
    let mut builder = state.http.request(method, request.url);
    for (name, value) in request.headers {
        if is_blocked_header(name.as_str()) {
            continue;
        }
        builder = builder.header(name, value);
    }
    if let Some(timeout_ms) = request.timeout_ms {
        builder = builder.timeout(Duration::from_millis(timeout_ms.min(60_000)));
    }
    if let Some(body) = request.body {
        builder = builder.body(body);
    }

    let response = builder
        .send()
        .await
        .map_err(|error| JsErrorBox::generic(format!("fetch failed: {error}")))?;
    let status = response.status().as_u16();
    let headers = collect_headers(response.headers());
    let bytes = response
        .bytes()
        .await
        .map_err(|error| JsErrorBox::generic(format!("failed to read fetch body: {error}")))?;
    if bytes.len() > MAX_FETCH_RESPONSE_BYTES {
        return Err(JsErrorBox::generic(format!(
            "fetch response exceeds {MAX_FETCH_RESPONSE_BYTES} byte limit"
        )));
    }

    Ok(FetchResponse { status, headers, body: String::from_utf8_lossy(&bytes).to_string() })
}

#[op2]
#[serde]
async fn op_slab_read_file(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
) -> Result<Vec<u8>, JsErrorBox> {
    let context = {
        let state = state.borrow();
        state.borrow::<RuntimeState>().context.clone()
    };
    authorize_file_access(&context, &path, PluginRuntimeFileAccess::Read)
        .map_err(JsErrorBox::generic)?;
    tokio::fs::read(path)
        .await
        .map_err(|error| JsErrorBox::generic(format!("failed to read file: {error}")))
}

#[op2]
async fn op_slab_write_file(
    state: Rc<RefCell<OpState>>,
    #[serde] request: WriteFileRequest,
) -> Result<(), JsErrorBox> {
    let context = {
        let state = state.borrow();
        state.borrow::<RuntimeState>().context.clone()
    };
    authorize_file_access(&context, &request.path, PluginRuntimeFileAccess::Write)
        .map_err(JsErrorBox::generic)?;
    tokio::fs::write(request.path, request.bytes)
        .await
        .map_err(|error| JsErrorBox::generic(format!("failed to write file: {error}")))
}

#[op2]
#[string]
fn op_slab_decode_utf8(#[serde] bytes: Vec<u8>) -> Result<String, JsErrorBox> {
    String::from_utf8(bytes).map_err(|error| JsErrorBox::generic(format!("invalid UTF-8: {error}")))
}

#[op2]
#[serde]
fn op_slab_encode_utf8(#[string] value: String) -> Vec<u8> {
    value.into_bytes()
}

fn authorize_fetch(context: &ExecutionContext, raw_url: &str) -> Result<(), String> {
    let url = Url::parse(raw_url).map_err(|error| format!("invalid fetch URL: {error}"))?;
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err("fetch only supports http:// and https:// URLs".to_owned());
    }
    let host = url.host_str().ok_or_else(|| "fetch URL is missing a host".to_owned())?;
    if is_blocked_slab_api_origin(context, &url) {
        return Err("local Slab API origins are blocked; use Slab.api.request instead".to_owned());
    }
    if context.permissions.network.mode != PluginNetworkMode::Allowlist {
        return Err("plugin network permission mode blocks fetch".to_owned());
    }
    if context.permissions.network.allow_hosts.iter().any(|allowed| host_matches(allowed, &url)) {
        return Ok(());
    }
    Err(format!("fetch host `{host}` is not declared in permissions.network.allowHosts"))
}

fn authorize_file_access(
    context: &ExecutionContext,
    raw_path: &str,
    access: PluginRuntimeFileAccess,
) -> Result<(), String> {
    let target = canonical_for_access(raw_path, access)?;
    for grant in &context.file_grants {
        if grant.access != access {
            continue;
        }
        let manifest_labels = match access {
            PluginRuntimeFileAccess::Read => &context.permissions.files.read,
            PluginRuntimeFileAccess::Write => &context.permissions.files.write,
        };
        if !manifest_labels.iter().any(|label| label == &grant.label) {
            continue;
        }
        let grant_path = canonical_for_access(&grant.path, access)?;
        if grant_path == target {
            return Ok(());
        }
    }

    Err(format!(
        "file {} access to `{raw_path}` requires a matching host-issued grant and manifest permission",
        match access {
            PluginRuntimeFileAccess::Read => "read",
            PluginRuntimeFileAccess::Write => "write",
        }
    ))
}

fn canonical_for_access(
    raw_path: &str,
    access: PluginRuntimeFileAccess,
) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw_path);
    match access {
        PluginRuntimeFileAccess::Read => path
            .canonicalize()
            .map_err(|error| format!("failed to resolve `{}`: {error}", path.display())),
        PluginRuntimeFileAccess::Write => {
            if path.exists() {
                return path
                    .canonicalize()
                    .map_err(|error| format!("failed to resolve `{}`: {error}", path.display()));
            }
            let parent = path.parent().ok_or_else(|| {
                format!("write target `{}` does not have a parent directory", path.display())
            })?;
            let parent = parent.canonicalize().map_err(|error| {
                format!("failed to resolve parent `{}`: {error}", parent.display())
            })?;
            let file_name = path.file_name().ok_or_else(|| {
                format!("write target `{}` does not have a file name", path.display())
            })?;
            Ok(parent.join(file_name))
        }
    }
}

fn ensure_path_within_root(root: &Path, path: &Path) -> Result<(), anyhow::Error> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to resolve plugin root {}", root.display()))?;
    let path = if path.exists() {
        path.canonicalize()
            .with_context(|| format!("failed to resolve plugin path {}", path.display()))?
    } else {
        let parent = path.parent().ok_or_else(|| {
            anyhow!("plugin path {} does not have a parent directory", path.display())
        })?;
        let parent = parent
            .canonicalize()
            .with_context(|| format!("failed to resolve plugin parent {}", parent.display()))?;
        let file_name = path
            .file_name()
            .ok_or_else(|| anyhow!("plugin path {} does not have a file name", path.display()))?;
        parent.join(file_name)
    };
    if path.starts_with(root) {
        Ok(())
    } else {
        bail!("plugin path {} escapes plugin root", path.display())
    }
}

fn validate_entry_extension(entry: &str) -> Result<(), anyhow::Error> {
    let extension = Path::new(entry)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(extension.as_str(), "ts" | "tsx" | "js" | "mjs") {
        return Ok(());
    }
    bail!("runtime.js.entry must use .ts, .tsx, .js, or .mjs")
}

fn host_matches(allowed: &str, url: &Url) -> bool {
    let allowed = allowed.trim();
    if allowed.is_empty() {
        return false;
    }
    if let Ok(allowed_url) = Url::parse(allowed) {
        return allowed_url.host_str() == url.host_str()
            && allowed_url.port_or_known_default() == url.port_or_known_default();
    }
    let host = url.host_str().unwrap_or_default();
    let host_port = url.port().map_or_else(|| host.to_owned(), |port| format!("{host}:{port}"));
    allowed == host || allowed == host_port
}

fn is_blocked_slab_api_origin(context: &ExecutionContext, url: &Url) -> bool {
    is_default_slab_api_origin(url)
        || context
            .blocked_fetch_origins
            .iter()
            .filter_map(|origin| Url::parse(origin).ok())
            .any(|origin| same_origin(&origin, url))
}

fn is_default_slab_api_origin(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    matches!(host, DESKTOP_API_HOST | "localhost")
        && url.port_or_known_default() == Some(DESKTOP_API_PORT)
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn collect_headers(headers: &reqwest::header::HeaderMap) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for (name, value) in headers {
        if is_blocked_header(name.as_str()) {
            continue;
        }
        if let Ok(value) = value.to_str() {
            result.insert(name.to_string(), value.to_string());
        }
    }
    result
}

fn is_blocked_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "host" | "connection" | "content-length" | "transfer-encoding"
    )
}

fn default_fetch_method() -> String {
    "GET".to_owned()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use serde_json::json;
    use slab_types::{
        PluginNetworkManifest, PluginPermissionsManifest, PluginRuntimeCallRequest,
        PluginRuntimeFileAccess, PluginRuntimeFileGrant,
    };
    use tempfile::tempdir;
    use tokio::net::TcpListener;

    use super::{DenoPluginExecutor, PluginExecutor, RuntimeHost};

    struct TestHost;

    #[async_trait]
    impl RuntimeHost for TestHost {
        async fn request(
            &self,
            method: &str,
            params: serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            match method {
                "slab.api.request" => {
                    Ok(json!({ "status": 200, "headers": {}, "body": "{\"ok\":true}" }))
                }
                "slab.ui.emit" => Ok(params),
                _ => Err(format!("unexpected host method `{method}`")),
            }
        }
    }

    fn call_request(
        root_dir: &std::path::Path,
        entry: &str,
        export_name: &str,
    ) -> PluginRuntimeCallRequest {
        PluginRuntimeCallRequest {
            call_id: "call-1".to_owned(),
            plugin_id: "test-plugin".to_owned(),
            root_dir: root_dir.to_string_lossy().into_owned(),
            entry: entry.to_owned(),
            bundle: None,
            export_name: export_name.to_owned(),
            params: json!({ "name": "Slab" }),
            permissions: PluginPermissionsManifest::default(),
            file_grants: Vec::new(),
            blocked_fetch_origins: Vec::new(),
        }
    }

    #[tokio::test]
    async fn runs_ts_esm_named_export_and_awaits_result() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("plugin.ts"),
            "export async function greet(input: { name: string }) { return { message: `hello ${input.name}` }; }",
        )
        .unwrap();

        let executor = DenoPluginExecutor::new(Arc::new(TestHost));
        let response =
            executor.execute(call_request(dir.path(), "plugin.ts", "greet")).await.unwrap();

        assert_eq!(response.result, json!({ "message": "hello Slab" }));
    }

    #[tokio::test]
    async fn blocks_file_read_without_grant() {
        let dir = tempdir().unwrap();
        let secret = dir.path().join("secret.txt");
        std::fs::write(&secret, "secret").unwrap();
        std::fs::write(
            dir.path().join("plugin.ts"),
            format!(
                "export async function run() {{ await Deno.readTextFile({}); }}",
                serde_json::to_string(&secret.to_string_lossy()).unwrap()
            ),
        )
        .unwrap();

        let executor = DenoPluginExecutor::new(Arc::new(TestHost));
        let error =
            executor.execute(call_request(dir.path(), "plugin.ts", "run")).await.unwrap_err();

        assert!(error.to_string().contains("requires a matching host-issued grant"));
    }

    #[tokio::test]
    async fn allows_granted_file_read_with_matching_manifest_label() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("allowed.txt");
        std::fs::write(&file, "ok").unwrap();
        std::fs::write(
            dir.path().join("plugin.ts"),
            format!(
                "export async function run() {{ return await Deno.readTextFile({}); }}",
                serde_json::to_string(&file.to_string_lossy()).unwrap()
            ),
        )
        .unwrap();

        let mut request = call_request(dir.path(), "plugin.ts", "run");
        request.permissions.files.read.push("fixture".to_owned());
        request.file_grants.push(PluginRuntimeFileGrant {
            label: "fixture".to_owned(),
            path: file.to_string_lossy().into_owned(),
            access: PluginRuntimeFileAccess::Read,
        });
        let executor = DenoPluginExecutor::new(Arc::new(TestHost));
        let response = executor.execute(request).await.unwrap();

        assert_eq!(response.result, json!("ok"));
    }

    #[tokio::test]
    async fn blocks_fetch_without_allowlist() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("plugin.ts"),
            "export async function run() { await fetch('https://example.com'); }",
        )
        .unwrap();

        let executor = DenoPluginExecutor::new(Arc::new(TestHost));
        let error =
            executor.execute(call_request(dir.path(), "plugin.ts", "run")).await.unwrap_err();

        assert!(error.to_string().contains("blocks fetch"));
    }

    #[tokio::test]
    async fn allows_allowlisted_fetch() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};

            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0u8; 1024];
            let _ = socket.read(&mut buffer).await.unwrap();
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").await.unwrap();
        });

        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("plugin.ts"),
            format!(
                "export async function run() {{ const r = await fetch('http://{addr}'); return await r.text(); }}"
            ),
        )
        .unwrap();

        let mut request = call_request(dir.path(), "plugin.ts", "run");
        request.permissions.network = PluginNetworkManifest {
            mode: slab_types::PluginNetworkMode::Allowlist,
            allow_hosts: vec![addr.to_string()],
        };
        let executor = DenoPluginExecutor::new(Arc::new(TestHost));
        let response = executor.execute(request).await.unwrap();

        assert_eq!(response.result, json!("ok"));
    }

    #[tokio::test]
    async fn blocks_fetch_to_configured_slab_api_origin() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("plugin.ts"),
            format!("export async function run() {{ await fetch('http://{addr}'); }}"),
        )
        .unwrap();

        let mut request = call_request(dir.path(), "plugin.ts", "run");
        request.permissions.network = PluginNetworkManifest {
            mode: slab_types::PluginNetworkMode::Allowlist,
            allow_hosts: vec![addr.to_string()],
        };
        request.blocked_fetch_origins.push(format!("http://{addr}"));

        let executor = DenoPluginExecutor::new(Arc::new(TestHost));
        let error = executor.execute(request).await.unwrap_err();

        assert!(error.to_string().contains("use Slab.api.request"));
    }

    #[tokio::test]
    async fn slab_api_request_goes_through_host() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("plugin.ts"),
            "export async function run() { return await Slab.api.request({ method: 'GET', path: '/v1/models' }); }",
        )
        .unwrap();

        let executor = DenoPluginExecutor::new(Arc::new(TestHost));
        let response =
            executor.execute(call_request(dir.path(), "plugin.ts", "run")).await.unwrap();

        assert_eq!(response.result["status"], json!(200));
    }
}

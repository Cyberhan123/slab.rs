use std::path::PathBuf;
#[cfg(feature = "lsp_runtime")]
use std::time::Duration;

use anyhow::bail;
#[cfg(feature = "lsp_runtime")]
use anyhow::{Context, anyhow};

#[cfg(feature = "lsp_runtime")]
use crate::{ExtensionOptions, Module, Runtime, RuntimeOptions, RustyResolver, deno_core};

pub async fn run(entry: PathBuf, args: Vec<String>) -> anyhow::Result<()> {
    run_inner(entry, args).await
}

#[cfg(feature = "lsp_runtime")]
async fn run_inner(entry: PathBuf, args: Vec<String>) -> anyhow::Result<()> {
    let entry = entry
        .canonicalize()
        .with_context(|| format!("failed to resolve LSP entry {}", entry.display()))?;
    if !entry.is_file() {
        bail!("LSP entry does not exist at {}", entry.display());
    }
    let entry_dir =
        entry.parent().ok_or_else(|| anyhow!("failed to resolve LSP entry parent directory"))?;
    let entry_specifier = deno_core::ModuleSpecifier::from_file_path(&entry)
        .map_err(|()| anyhow!("failed to convert LSP entry to file URL: {}", entry.display()))?;
    let runtime_exe = std::env::current_exe().context("failed to resolve slab-js-runtime path")?;
    let entry_json = serde_json::to_string(entry_specifier.as_str())?;
    let runtime_exe_json = serde_json::to_string(&runtime_exe)?;
    let args_json = serde_json::to_string(&args)?;
    let bootstrap = format!(
        r#"
import {{ Buffer as __SlabBuffer }} from "node:buffer";
import process from "node:process";
import childProcess from "node:child_process";
globalThis.Buffer ??= __SlabBuffer;
if (!("setImmediate" in globalThis)) {{
  Object.defineProperty(globalThis, "setImmediate", {{
    configurable: true,
    value: (callback, ...args) => setTimeout(callback, 0, ...args),
  }});
}}
if (!("clearImmediate" in globalThis)) {{
  Object.defineProperty(globalThis, "clearImmediate", {{
    configurable: true,
    value: (handle) => clearTimeout(handle),
  }});
}}
globalThis.__SLAB_LSP_ARGS__ = {args_json};
globalThis.__SLAB_LSP_RUNTIME_EXE__ = {runtime_exe_json};
if (globalThis.Deno) {{
  try {{
    globalThis.Deno.args = [...globalThis.__SLAB_LSP_ARGS__];
  }} catch {{
    // Deno.args may be read-only in some runtime builds; process.argv below is the fallback.
  }}
}}
process.argv = ["slab-js-runtime", "language-server", ...globalThis.__SLAB_LSP_ARGS__];
function __slabCreateEmitter() {{
  const listeners = new Map();
  return {{
    on(event, listener) {{
      let eventListeners = listeners.get(event);
      if (!eventListeners) {{
        eventListeners = new Set();
        listeners.set(event, eventListeners);
      }}
      eventListeners.add(listener);
      return this;
    }},
    addListener(event, listener) {{
      return this.on(event, listener);
    }},
    off(event, listener) {{
      listeners.get(event)?.delete(listener);
      return this;
    }},
    removeListener(event, listener) {{
      return this.off(event, listener);
    }},
    emit(event, ...args) {{
      for (const listener of [...(listeners.get(event) ?? [])]) {{
        listener(...args);
      }}
      return true;
    }},
  }};
}}
function __slabReadableStdin() {{
  const stdin = __slabCreateEmitter();
  let started = false;
  const start = () => {{
    if (started) {{
      return;
    }}
    started = true;
    queueMicrotask(async () => {{
      try {{
        while (true) {{
          const buffer = new Uint8Array(65536);
          const length = await globalThis.Deno.stdin.read(buffer);
          if (length === null) {{
            stdin.emit("end");
            stdin.emit("close");
            return;
          }}
          if (length > 0) {{
            stdin.emit("data", __SlabBuffer.from(buffer.subarray(0, length)));
          }}
        }}
      }} catch (error) {{
        stdin.emit("error", error);
      }}
    }});
  }};
  const on = stdin.on.bind(stdin);
  stdin.on = (event, listener) => {{
    on(event, listener);
    if (event === "data") {{
      start();
    }}
    return stdin;
  }};
  stdin.addListener = stdin.on;
  stdin.fd = 0;
  stdin._isStdio = true;
  stdin.isTTY = false;
  stdin.read = () => null;
  stdin.resume = () => {{
    start();
    return stdin;
  }};
  stdin.pause = () => stdin;
  stdin.setEncoding = () => stdin;
  return stdin;
}}
function __slabWritableStdio(writer, fd) {{
  const output = __slabCreateEmitter();
  output.fd = fd;
  output._isStdio = true;
  output.isTTY = false;
  output.columns = 80;
  output.write = (chunk, encoding, callback) => {{
    if (typeof encoding === "function") {{
      callback = encoding;
      encoding = undefined;
    }}
    const data = typeof chunk === "string"
      ? __SlabBuffer.from(chunk, encoding)
      : __SlabBuffer.from(chunk);
    Promise.resolve(writer.write(data)).then(() => {{
      callback?.();
    }}, (error) => {{
      output.emit("error", error);
      callback?.(error);
    }});
    return true;
  }};
  output.end = (chunk, encoding, callback) => {{
    if (typeof chunk === "function") {{
      callback = chunk;
      chunk = undefined;
      encoding = undefined;
    }} else if (typeof encoding === "function") {{
      callback = encoding;
      encoding = undefined;
    }}
    const finish = () => {{
      output.emit("finish");
      output.emit("end");
      output.emit("close");
      callback?.();
    }};
    if (chunk !== undefined) {{
      output.write(chunk, encoding, finish);
    }} else {{
      finish();
    }}
    return output;
  }};
  return output;
}}
if (globalThis.Deno?.stdin) {{
  process.stdin = __slabReadableStdin();
}}
if (globalThis.Deno?.stdout) {{
  process.stdout = __slabWritableStdio(globalThis.Deno.stdout, 1);
}}
if (globalThis.Deno?.stderr) {{
  process.stderr = __slabWritableStdio(globalThis.Deno.stderr, 2);
}}
process.execPath ||= globalThis.__SLAB_LSP_RUNTIME_EXE__;
globalThis.process = process;
function __slabReadableFromWebStream(stream) {{
  const readable = __slabCreateEmitter();
  let started = false;
  const start = () => {{
    if (started) {{
      return;
    }}
    started = true;
    queueMicrotask(async () => {{
      const reader = stream.getReader();
      try {{
        while (true) {{
          const result = await reader.read();
          if (result.done) {{
            readable.emit("end");
            readable.emit("close");
            return;
          }}
          readable.emit("data", __SlabBuffer.from(result.value));
        }}
      }} catch (error) {{
        readable.emit("error", error);
      }} finally {{
        reader.releaseLock();
      }}
    }});
  }};
  const on = readable.on.bind(readable);
  readable.on = (event, listener) => {{
    on(event, listener);
    if (event === "data") {{
      start();
    }}
    return readable;
  }};
  readable.addListener = readable.on;
  readable.read = () => null;
  readable.resume = () => {{
    start();
    return readable;
  }};
  readable.pause = () => readable;
  readable.setEncoding = () => readable;
  return readable;
}}
function __slabWritableFromWebStream(stream) {{
  const writable = __slabCreateEmitter();
  const writer = stream.getWriter();
  writable.write = (chunk, encoding, callback) => {{
    if (typeof encoding === "function") {{
      callback = encoding;
      encoding = undefined;
    }}
    const data = typeof chunk === "string"
      ? __SlabBuffer.from(chunk, encoding)
      : __SlabBuffer.from(chunk);
    writer.write(data).then(() => {{
      callback?.();
    }}, (error) => {{
      writable.emit("error", error);
      callback?.(error);
    }});
    return true;
  }};
  writable.end = (chunk, encoding, callback) => {{
    if (typeof chunk === "function") {{
      callback = chunk;
      chunk = undefined;
      encoding = undefined;
    }} else if (typeof encoding === "function") {{
      callback = encoding;
      encoding = undefined;
    }}
    const finish = () => writer.close().then(() => {{
      writable.emit("finish");
      writable.emit("end");
      writable.emit("close");
      callback?.();
    }}, (error) => {{
      writable.emit("error", error);
      callback?.(error);
    }});
    if (chunk !== undefined) {{
      writable.write(chunk, encoding, finish);
    }} else {{
      finish();
    }}
    return writable;
  }};
  return writable;
}}
childProcess.fork = (modulePath, forkArgs = [], options = {{}}) => {{
  const child = __slabCreateEmitter();
  const entryPath = modulePath.endsWith("tsserver.js")
    ? modulePath.replace(/tsserver\.js$/, "_tsserver.js")
    : modulePath;
  const runtimeArgs = forkArgs.filter((arg) => arg !== "--useNodeIpc");
  if (entryPath.endsWith("_tsserver.js") && !runtimeArgs.includes("--disableAutomaticTypingAcquisition")) {{
    runtimeArgs.push("--disableAutomaticTypingAcquisition");
  }}
  const command = new globalThis.Deno.Command(globalThis.__SLAB_LSP_RUNTIME_EXE__, {{
    args: [
      "lsp",
      "--entry",
      entryPath,
      "--",
      ...runtimeArgs,
    ],
    cwd: options.cwd,
    env: options.env,
    stdin: "piped",
    stdout: "piped",
    stderr: "piped",
  }});
  const childProcessHandle = command.spawn();
  child.pid = childProcessHandle.pid;
  child.stdin = __slabWritableFromWebStream(childProcessHandle.stdin);
  child.stdout = __slabReadableFromWebStream(childProcessHandle.stdout);
  child.stderr = __slabReadableFromWebStream(childProcessHandle.stderr);
  child.stderr.on("data", (chunk) => {{
    process.stderr.write(chunk);
  }});
  child.send = (message, callback) => {{
    child.stdin.write(`${{JSON.stringify(message)}}\r\n`, "utf8", callback);
    return true;
  }};
  child.kill = () => {{
    try {{
      childProcessHandle.kill();
      return true;
    }} catch {{
      return false;
    }}
  }};
  childProcessHandle.status.then((status) => {{
    child.emit("exit", status.code, status.signal ?? null);
    child.emit("close", status.code, status.signal ?? null);
  }}, (error) => {{
    child.emit("error", error);
  }});
  return child;
}};
await import({entry_json});
"#
    );

    let bootstrap_path =
        std::env::temp_dir().join(format!("slab-lsp-bootstrap-{}.mjs", std::process::id()));
    std::fs::write(&bootstrap_path, bootstrap).with_context(|| {
        format!("failed to write LSP bootstrap module {}", bootstrap_path.display())
    })?;
    let module = Module::load(&bootstrap_path).with_context(|| {
        format!("failed to load LSP bootstrap module {}", bootstrap_path.display())
    })?;
    let extension_options = ExtensionOptions {
        node_resolver: std::sync::Arc::new(RustyResolver::new(
            Some(entry_dir.to_path_buf()),
            std::sync::Arc::new(deno_fs::RealFs),
        )),
        ..Default::default()
    };
    let mut runtime = Runtime::with_tokio_runtime_handle(
        RuntimeOptions { timeout: Duration::MAX, extension_options, ..Default::default() },
        tokio::runtime::Handle::current(),
    )?;
    runtime.set_current_dir(entry_dir)?;
    let result = runtime.load_module_async(&module).await;
    let _ = std::fs::remove_file(&bootstrap_path);
    result?;
    runtime.await_event_loop(deno_core::PollEventLoopOptions::default(), None).await?;
    Ok(())
}

#[cfg(not(feature = "lsp_runtime"))]
async fn run_inner(_entry: PathBuf, _args: Vec<String>) -> anyhow::Result<()> {
    bail!("slab-js-runtime was built without lsp_runtime support")
}

use std::path::PathBuf;
use std::sync::Arc;

use slab_python_runtime::api::jsonrpc::{JsonRpcRuntimeHost, serve_stdio, serve_uds};
use slab_python_runtime::{PythonRuntime, PythonRuntimeConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    slab_utils::tracing::init_stderr_tracing("info");

    let command = RuntimeCommand::parse(std::env::args().skip(1))?;
    let (host, outbound) = JsonRpcRuntimeHost::new();
    let host = Arc::new(host);
    let runtime = Arc::new(PythonRuntime::with_config(PythonRuntimeConfig {
        host: host.clone(),
        ..PythonRuntimeConfig::default()
    }));
    runtime.initialize()?;
    match command {
        RuntimeCommand::PluginJsonRpc { socket: Some(socket) } => {
            serve_uds(host, runtime, outbound, &socket).await
        }
        RuntimeCommand::PluginJsonRpc { socket: None } => {
            serve_stdio(host, runtime, outbound).await
        }
    }
}

enum RuntimeCommand {
    PluginJsonRpc { socket: Option<PathBuf> },
}

impl RuntimeCommand {
    fn parse<I>(args: I) -> anyhow::Result<Self>
    where
        I: Iterator<Item = String>,
    {
        Self::parse_plugin_args(args)
    }

    fn parse_plugin_args<I>(mut args: I) -> anyhow::Result<Self>
    where
        I: Iterator<Item = String>,
    {
        let mut socket = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--socket" => {
                    let Some(value) = args.next() else {
                        anyhow::bail!("--socket requires a value");
                    };
                    socket = Some(PathBuf::from(value));
                }
                _ => anyhow::bail!("unknown slab-python-runtime plugin argument `{arg}`"),
            }
        }

        Ok(Self::PluginJsonRpc { socket })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::RuntimeCommand;

    #[test]
    fn parses_default_plugin_json_rpc_mode() {
        let command = RuntimeCommand::parse([].into_iter()).unwrap();

        assert!(matches!(command, RuntimeCommand::PluginJsonRpc { socket: None }));
    }

    #[test]
    fn parses_plugin_mode_socket() {
        let command =
            RuntimeCommand::parse(["--socket", "runtime.sock"].into_iter().map(str::to_owned))
                .unwrap();

        let RuntimeCommand::PluginJsonRpc { socket } = command;
        assert_eq!(socket, Some(PathBuf::from("runtime.sock")));
    }
}

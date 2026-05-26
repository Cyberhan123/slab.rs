use std::path::PathBuf;
use std::sync::Arc;

use crate::api::jsonrpc::{JsonRpcRuntimeHost, serve_stdio, serve_uds};
use crate::application::PluginRuntimeServer;
use crate::infra::deno::DenoPluginExecutor;
use crate::lsp;

pub fn run() -> anyhow::Result<()> {
    let command = RuntimeCommand::parse(std::env::args().skip(1))?;
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(async move {
        match command {
            RuntimeCommand::PluginJsonRpc { socket } => {
                let (host, outbound) = JsonRpcRuntimeHost::new();
                let host = Arc::new(host);
                let executor = Arc::new(DenoPluginExecutor::new(host.clone()));
                let server = Arc::new(PluginRuntimeServer::new(executor));
                match socket {
                    Some(socket) => serve_uds(host, server, outbound, &socket).await,
                    None => serve_stdio(host, server, outbound).await,
                }
            }
            RuntimeCommand::Lsp { entry, args } => lsp::run(entry, args).await,
        }
    })
}

enum RuntimeCommand {
    PluginJsonRpc { socket: Option<PathBuf> },
    Lsp { entry: PathBuf, args: Vec<String> },
}

impl RuntimeCommand {
    fn parse<I>(mut args: I) -> anyhow::Result<Self>
    where
        I: Iterator<Item = String>,
    {
        let Some(command) = args.next() else {
            return Ok(Self::PluginJsonRpc { socket: None });
        };

        if command == "lsp" {
            return Self::parse_lsp_args(args);
        }

        let plugin_args = std::iter::once(command).chain(args);
        Self::parse_plugin_args(plugin_args)
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
                _ => anyhow::bail!("unknown slab-js-runtime plugin argument `{arg}`"),
            }
        }

        Ok(Self::PluginJsonRpc { socket })
    }

    fn parse_lsp_args<I>(mut args: I) -> anyhow::Result<Self>
    where
        I: Iterator<Item = String>,
    {
        let mut entry = None;
        let mut server_args = Vec::new();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--entry" => {
                    let Some(value) = args.next() else {
                        anyhow::bail!("lsp --entry requires a value");
                    };
                    entry = Some(PathBuf::from(value));
                }
                "--" => {
                    server_args.extend(args);
                    break;
                }
                _ => anyhow::bail!("unknown lsp argument `{arg}`"),
            }
        }

        let entry = entry.ok_or_else(|| anyhow::anyhow!("lsp --entry is required"))?;
        Ok(Self::Lsp { entry, args: server_args })
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

        let RuntimeCommand::PluginJsonRpc { socket } = command else {
            panic!("expected plugin mode");
        };
        assert_eq!(socket, Some(PathBuf::from("runtime.sock")));
    }

    #[test]
    fn parses_lsp_mode_entry_and_server_args() {
        let command = RuntimeCommand::parse(
            ["lsp", "--entry", "server.mjs", "--", "--stdio"].into_iter().map(str::to_owned),
        )
        .unwrap();

        let RuntimeCommand::Lsp { entry, args } = command else {
            panic!("expected lsp mode");
        };
        assert_eq!(entry, PathBuf::from("server.mjs"));
        assert_eq!(args, vec!["--stdio"]);
    }
}

use std::path::PathBuf;
use std::sync::Arc;

use crate::api::jsonrpc::{JsonRpcRuntimeHost, serve_stdio};
use crate::infra::deno::DenoPluginExecutor;
use crate::lsp;

pub fn run() -> anyhow::Result<()> {
    let command = RuntimeCommand::parse(std::env::args().skip(1))?;
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(async move {
        match command {
            RuntimeCommand::PluginJsonRpc => {
                let host = Arc::new(JsonRpcRuntimeHost::new());
                let executor = Arc::new(DenoPluginExecutor::new(host.clone()));
                serve_stdio(host, executor).await
            }
            RuntimeCommand::Lsp { entry, args } => lsp::run(entry, args).await,
        }
    })
}

enum RuntimeCommand {
    PluginJsonRpc,
    Lsp { entry: PathBuf, args: Vec<String> },
}

impl RuntimeCommand {
    fn parse<I>(mut args: I) -> anyhow::Result<Self>
    where
        I: Iterator<Item = String>,
    {
        let Some(command) = args.next() else {
            return Ok(Self::PluginJsonRpc);
        };

        if command != "lsp" {
            anyhow::bail!("unknown slab-js-runtime mode `{command}`");
        }

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

        assert!(matches!(command, RuntimeCommand::PluginJsonRpc));
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

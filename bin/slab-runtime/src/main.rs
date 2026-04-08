use clap::Parser;

use slab_runtime::infra::config::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    slab_runtime::api::server::run(Cli::parse()).await
}

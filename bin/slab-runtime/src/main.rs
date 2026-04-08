use clap::Parser;

use slab_runtime::config::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    slab_runtime::launch::run(Cli::parse()).await
}

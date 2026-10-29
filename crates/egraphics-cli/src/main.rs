mod cli;
mod commands;

use anyhow::Result;
use tracing::info;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Hello, world!");

    Ok(())
}

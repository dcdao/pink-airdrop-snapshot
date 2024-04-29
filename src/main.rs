mod cli;
mod format;
mod snapshot;

// crates.io
use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
	color_eyre::install().unwrap();
	tracing_subscriber::fmt::init();
	cli::Cli::parse().run().await?;

	Ok(())
}

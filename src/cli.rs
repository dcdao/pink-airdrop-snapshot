// crates.io
use anyhow::Result;
use clap::Parser;
// pas
use crate::{format, snapshot::Pas};

#[derive(Debug, Parser)]
#[command(
	version = concat!(
		env!("CARGO_PKG_VERSION"),
		"-",
		env!("VERGEN_GIT_SHA"),
		"-",
		env!("VERGEN_CARGO_TARGET_TRIPLE"),
	),
	about,
	rename_all = "kebab",
)]
pub struct Cli {
	#[command(subcommand)]
	pub subcmd: SubCmd,
}
impl Cli {
	pub async fn run(&self) -> Result<()> {
		match &self.subcmd {
			SubCmd::Snap { ws } => Pas::new(ws).await?.snap().await?.merge().save()?,
			SubCmd::Format { path } => format::format(path)?,
		}

		Ok(())
	}
}

#[derive(Debug, Parser)]
pub enum SubCmd {
	/// Create the snapshot from a live chain.
	Snap {
		/// Websocket endpoint.
		#[clap(value_name = "URI")]
		ws: String,
	},
	/// Reformat the CSV airdrop list to adhere to the specifications required by safe-airdrop.
	Format {
		/// Path to the CSV file.
		#[clap(value_name = "PATH")]
		path: String,
	},
}

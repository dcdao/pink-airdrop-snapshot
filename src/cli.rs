// crates.io
use anyhow::Result;
use clap::Parser;
// pas
use crate::snapshot::Pas;

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
	/// Websocket endpoint.
	#[arg(long, value_name = "URI")]
	pub ws: String,
}
impl Cli {
	pub async fn run(&self) -> Result<()> {
		Pas::new(&self.ws).await?.snap().await?.merge().save()?;

		Ok(())
	}
}

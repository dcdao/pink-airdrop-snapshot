// crates.io
use anyhow::Result;
use clap::Parser;
use futures::future;
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
		let pas = Pas::new(&self.ws).await?;

		for f in future::join_all([
			// RING and deposit.
			// 2023-10-11 00:01:06 (+UTC)
			// https://darwinia.subscan.io/block/1186226
			pas.snap(
				"0x2562247315cc789376da2049c6e792f689387781fd1249374ce238760547b578",
				true,
				true,
			),
			// KTON.
			// 2023-09-26 00:00:06 (+UTC)
			// https://darwinia.subscan.io/block/1080104
			pas.snap(
				"0x6768a1fe876999de62a0120f50190f50b126fdcf0d0bee8a83b3b692f5e47a48",
				false,
				true,
			),
			// RING and deposit.
			// 2024-01-23 16:51:00 (+UTC)
			// https://darwinia.subscan.io/block/1910929
			pas.snap(
				"0x7bcab5906d0ceceec70874f7eb12093ee42122b5e4d6c8dfc613084186d5f282",
				true,
				false,
			),
			// KTON.
			// 2024-01-23 16:51:00 (+UTC)
			// https://darwinia.subscan.io/block/1910929
			pas.snap(
				"0x7bcab5906d0ceceec70874f7eb12093ee42122b5e4d6c8dfc613084186d5f282",
				false,
				false,
			),
		])
		.await
		{
			f?;
		}

		pas.save()?;

		Ok(())
	}
}

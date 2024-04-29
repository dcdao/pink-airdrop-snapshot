// crates.io
use anyhow::Result;
use chrono::Utc;
use csv::{Reader, Writer};

pub fn format(path: &str) -> Result<()> {
	let mut rdr = Reader::from_path(path)?;
	let mut wtr =
		Writer::from_path(format!("airdrop-list-{}.csv", Utc::now().format("%F-%H-%M-%S")))?;
	let mut sum = 0.;

	wtr.write_record(&["token_address", "receiver", "amount"])?;

	for result in rdr.records() {
		let record = result?;
		let receiver = &record[0];
		let amount = record[3].parse::<f64>()? / 1_000_000_000.;

		sum += amount;

		wtr.write_record([
			"0x0000000000000000000000000000000000000404",
			receiver,
			&amount.to_string(),
		])?;
	}

	dbg!(sum);

	wtr.flush()?;

	Ok(())
}

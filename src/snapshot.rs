// std
use std::{collections::HashMap, fs::File, sync::Arc};
// crates.io
use anyhow::Result;
use chrono::Utc;
use csv::Writer;
use futures::{future, FutureExt};
use parity_scale_codec::Decode;
use serde::Serialize;
use subxt::{
	config::polkadot::H256,
	dynamic::{self, DecodedValueThunk},
	storage::{address::Yes, StorageAddress},
	OnlineClient, PolkadotConfig,
};
use tokio::sync::{Mutex, MutexGuard};

type Map = HashMap<[u8; 20], Data>;

const RING_THRESHOLD: u128 = 1_000_000_000_000_000_000;
const KTON_THRESHOLD: u128 = 10_000_000_000_000_000;

pub struct Pas {
	api: OnlineClient<PolkadotConfig>,
	map: Arc<Mutex<Map>>,
}
impl Pas {
	pub async fn new(ws: &str) -> Result<Self> {
		let api = <OnlineClient<PolkadotConfig>>::from_url(ws).await?;

		Ok(Self { api, map: Arc::new(Mutex::new(Map::new())) })
	}

	async fn iter_storage_with<F, A, S>(
		&self,
		map: Arc<Mutex<Map>>,
		at: H256,
		address: A,
		f: F,
	) -> Result<()>
	where
		F: Fn(MutexGuard<Map>, [u8; 20], S),
		A: 'static + StorageAddress<IsIterable = Yes, Target = DecodedValueThunk>,
		S: Decode,
	{
		let mut storage = self.api.storage().at(at).iter(address).await?;

		while let Some(r) = storage.next().await {
			match r {
				Ok((k, v)) => {
					let who = array_bytes::slice2array_unchecked::<_, 20>(&k[k.len() - 20..]);
					let s = S::decode(&mut &*v.into_encoded()).unwrap();
					let map = map.lock().await;

					f(map, who, s);
				},
				Err(e) => println!("Error: {e:?}"),
			}
		}

		Ok(())
	}

	pub async fn snap(
		&self,
		at: &str,
		// Snapshot target.
		// `true` for RING and `false` for KTON.
		ring: bool,
		// Check eligibility won't calculate the staked amount.
		check_eligibility: bool,
	) -> Result<()> {
		let at = array_bytes::hex_n_into_unchecked::<_, H256, 32>(at);
		let f1 = self
			.iter_storage_with(
				self.map.clone(),
				at,
				dynamic::storage("DarwiniaStaking", "Ledgers", <Vec<()>>::new()),
				|mut map, who, ledger: Ledger| {
					if ring {
						// Check if eligibility needs to be checked and if it passes the threshold,
						if (check_eligibility && ledger.staked_ring >= RING_THRESHOLD)
							// or if it just needs to be greater than 0.
							|| (!check_eligibility && ledger.staked_ring > 0)
						{
							// If checking eligibility, simply insert a record; otherwise, add the
							// staked amount.
							let ring = if check_eligibility { 0 } else { ledger.staked_ring };

							map.entry(who).and_modify(|d| d.ring += ring).or_insert_with(|| Data {
								who: array_bytes::bytes2hex("0x", who),
								ring,
								kton: 0,
							});
						}
					} else {
						// Check if eligibility needs to be checked and if it passes the threshold,
						if (check_eligibility && ledger.staked_kton >= KTON_THRESHOLD)
							// or if it just needs to be greater than 0.
							|| (!check_eligibility && ledger.staked_kton > 0)
						{
							// If checking eligibility, simply insert a record; otherwise, add the
							// staked amount.
							let kton = if check_eligibility { 0 } else { ledger.staked_kton };

							map.entry(who).and_modify(|d| d.kton += kton).or_insert_with(|| Data {
								who: array_bytes::bytes2hex("0x", who),
								ring: 0,
								kton,
							});
						}
					}
				},
			)
			.boxed();
		let f2 = if ring {
			self.iter_storage_with(
				self.map.clone(),
				at,
				dynamic::storage("Deposit", "Deposits", <Vec<()>>::new()),
				|mut map, who, deposits: Vec<Deposit>| {
					// If checking eligibility, simply insert a record; otherwise, add the
					// staked amount.
					// No threshold requirement for deposit.
					let ring = if check_eligibility {
						0
					} else {
						deposits.iter().fold(0, |acc, d| acc + d.value)
					};

					map.entry(who).and_modify(|d| d.ring += ring).or_insert_with(|| Data {
						who: array_bytes::bytes2hex("0x", who),
						ring,
						kton: 0,
					});
				},
			)
			.boxed()
		} else {
			future::ready(Ok(())).boxed()
		};

		for r in future::join_all([f1, f2]).await {
			r?;
		}

		Ok(())
	}

	pub fn save(self) -> Result<()> {
		let f = File::create(format!("eligible-list-{}.csv", Utc::now().format("%F-%H-%M-%S")))?;
		let mut wtr = Writer::from_writer(f);

		for (_, d) in Arc::try_unwrap(self.map).unwrap().into_inner() {
			if d.ring > 0 || d.kton > 0 {
				wtr.serialize(d)?;
			}
		}

		wtr.flush()?;

		Ok(())
	}
}

#[derive(Debug, Decode, Serialize)]
struct Data {
	who: String,
	ring: u128,
	kton: u128,
}

#[derive(Debug, Decode)]
struct Ledger {
	staked_ring: u128,
	staked_kton: u128,
	_0: Vec<u16>,
	_1: Vec<(u128, u32)>,
	_2: Vec<(u128, u32)>,
	_3: Vec<(u16, u32)>,
}

#[derive(Debug, Decode)]
struct Deposit {
	_0: u16,
	value: u128,
	_1: u128,
	_2: u128,
	_3: bool,
}

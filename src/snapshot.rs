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
					let who = array_bytes::dyn2array!(&k[k.len() - 20..], 20);
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
		check_amount: bool,
	) -> Result<()> {
		let at = array_bytes::hex_into_unchecked::<H256, 32>(at);
		let f1 = self
			.iter_storage_with(
				self.map.clone(),
				at,
				dynamic::storage("DarwiniaStaking", "Ledgers", <Vec<()>>::new()),
				|mut map, who, ledger: Ledger| {
					if ring {
						if ledger.staked_ring > 0
							&& (!check_amount || ledger.staked_ring >= RING_THRESHOLD)
						{
							map.entry(who)
								.and_modify(|d| d.ring += ledger.staked_ring)
								.or_insert_with(|| Data {
									who: array_bytes::bytes2hex("0x", &who),
									ring: ledger.staked_ring,
									kton: 0,
								});
						}
					} else if ledger.staked_kton > 0
						&& (!check_amount || ledger.staked_kton >= KTON_THRESHOLD)
					{
						map.entry(who).and_modify(|d| d.kton += ledger.staked_kton).or_insert_with(
							|| Data {
								who: array_bytes::bytes2hex("0x", &who),
								ring: 0,
								kton: ledger.staked_kton,
							},
						);
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
					let staked = deposits.iter().fold(0, |acc, d| acc + d.value);

					map.entry(who).and_modify(|d| d.ring += staked).or_insert_with(|| Data {
						who: array_bytes::bytes2hex("0x", &who),
						ring: staked,
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
		let f = File::create(format!("eligible list {}.csv", Utc::now()))?;
		let mut wtr = Writer::from_writer(f);

		for (_, d) in Arc::try_unwrap(self.map).unwrap().into_inner() {
			wtr.serialize(d)?;
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

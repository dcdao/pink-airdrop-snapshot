// std
use std::collections::{HashMap, HashSet};
// crates.io
use anyhow::Result;
use chrono::Utc;
use csv::Writer;
use parity_scale_codec::Decode;
use serde::Serialize;
use subxt::{
	config::polkadot::H256,
	dynamic::{self, DecodedValueThunk},
	storage::{address::Yes, StorageAddress},
	OnlineClient, PolkadotConfig,
};

const RING_THRESHOLD: u128 = 1_000_000_000_000_000_000;
const KTON_THRESHOLD: u128 = 10_000_000_000_000_000;

pub struct Pas {
	api: OnlineClient<PolkadotConfig>,
	ring_eligible_list: HashSet<String>,
	kton_eligible_list: HashSet<String>,
	deposit_eligible_list: HashSet<String>,
	ring_kton_holders_detail: Vec<(String, u128, u128)>,
	deposit_holders_detail: Vec<(String, u128)>,
	data: HashMap<String, FinalData>,
}
impl Pas {
	pub async fn new(ws: &str) -> Result<Self> {
		let api = <OnlineClient<PolkadotConfig>>::from_url(ws).await?;

		Ok(Self {
			api,
			ring_eligible_list: HashSet::new(),
			kton_eligible_list: HashSet::new(),
			deposit_eligible_list: HashSet::new(),
			ring_kton_holders_detail: Vec::new(),
			deposit_holders_detail: Vec::new(),
			data: HashMap::new(),
		})
	}

	async fn iter_storage_with<A, C, F, S>(
		&self,
		at: &str,
		address: A,
		collection: &mut C,
		f: F,
	) -> Result<()>
	where
		A: 'static + StorageAddress<IsIterable = Yes, Target = DecodedValueThunk>,
		F: Fn(&mut C, String, S),
		S: Decode,
	{
		let at = array_bytes::hex_n_into_unchecked::<_, H256, 32>(at);
		let mut storage = self.api.storage().at(at).iter(address).await?;

		while let Some(r) = storage.next().await {
			match r {
				Ok((k, v)) => {
					let k = array_bytes::bytes2hex(
						"0x",
						array_bytes::slice2array_unchecked::<_, 20>(&k[k.len() - 20..]),
					);
					let v = S::decode(&mut &*v.into_encoded()).unwrap();

					f(collection, k, v);
				},
				Err(e) => println!("Error: {e:?}"),
			}
		}

		Ok(())
	}

	pub async fn snap(mut self) -> Result<Self> {
		let mut ring_eligible_list = HashSet::new();
		let mut kton_eligible_list = HashSet::new();
		let mut deposit_eligible_list = HashSet::new();
		let mut ring_kton_holders_detail = Vec::new();
		let mut deposit_holders_detail = Vec::new();
		let (f1, f2, f3, f4, f5) = tokio::join!(
			// ---
			// 2023-10-11 00:01:06 (+UTC)
			// https://darwinia.subscan.io/block/1186226
			// ---
			//
			// A list for RING at this moment.
			self.iter_storage_with(
				"0x2562247315cc789376da2049c6e792f689387781fd1249374ce238760547b578",
				dynamic::storage("DarwiniaStaking", "Ledgers", <Vec<()>>::new()),
				&mut ring_eligible_list,
				Box::new(|c: &mut HashSet<_>, k, v: Ledger| {
					if v.staked_ring >= RING_THRESHOLD {
						c.insert(k);
					}
				}),
			),
			// A list for Deposit at this moment.
			self.iter_storage_with(
				"0x2562247315cc789376da2049c6e792f689387781fd1249374ce238760547b578",
				dynamic::storage("Deposit", "Deposits", <Vec<()>>::new()),
				&mut deposit_eligible_list,
				Box::new(|c: &mut HashSet<_>, k, v: Vec<Deposit>| {
					let v = v.iter().map(|d| d.value).sum::<u128>();

					if v > 0 {
						c.insert(k);
					}
				}),
			),
			// ---
			// 2023-09-26 00:00:06 (+UTC)
			// https://darwinia.subscan.io/block/1080104
			// ---
			//
			// A list for KTON at this moment.
			self.iter_storage_with(
				"0x6768a1fe876999de62a0120f50190f50b126fdcf0d0bee8a83b3b692f5e47a48",
				dynamic::storage("DarwiniaStaking", "Ledgers", <Vec<()>>::new()),
				&mut kton_eligible_list,
				Box::new(|c: &mut HashSet<_>, k, v: Ledger| {
					if v.staked_kton >= KTON_THRESHOLD {
						c.insert(k);
					}
				}),
			),
			// ---
			// 2024-01-23 16:51:00 (+UTC)
			// https://darwinia.subscan.io/block/1910929
			// ---
			//
			// A list for RING and KTON at this moment.
			self.iter_storage_with(
				"0x7bcab5906d0ceceec70874f7eb12093ee42122b5e4d6c8dfc613084186d5f282",
				dynamic::storage("DarwiniaStaking", "Ledgers", <Vec<()>>::new()),
				&mut ring_kton_holders_detail,
				Box::new(|c: &mut Vec<_>, k, v: Ledger| {
					c.push((k, v.staked_ring, v.staked_kton));
				}),
			),
			// A list for Deposit at this moment.
			self.iter_storage_with(
				"0x7bcab5906d0ceceec70874f7eb12093ee42122b5e4d6c8dfc613084186d5f282",
				dynamic::storage("Deposit", "Deposits", <Vec<()>>::new()),
				&mut deposit_holders_detail,
				Box::new(|c: &mut Vec<_>, k, v: Vec<Deposit>| {
					let v = v.iter().map(|d| d.value).sum::<u128>();

					c.push((k, v));
				}),
			),
		);

		f1?;
		f2?;
		f3?;
		f4?;
		f5?;

		self.ring_eligible_list = ring_eligible_list;
		self.kton_eligible_list = kton_eligible_list;
		self.deposit_eligible_list = deposit_eligible_list;
		self.ring_kton_holders_detail = ring_kton_holders_detail;
		self.deposit_holders_detail = deposit_holders_detail;

		Ok(self)
	}

	pub fn merge(mut self) -> Self {
		for &(ref who, ring, kton) in &self.ring_kton_holders_detail {
			if self.ring_eligible_list.contains(who) {
				if let Some(d) = self.data.get_mut(who) {
					d.ring += ring;
				} else {
					self.data.insert(who.into(), FinalData { who: who.into(), ring, kton: 0 });
				}
			}
			if self.kton_eligible_list.contains(who) {
				if let Some(d) = self.data.get_mut(who) {
					d.kton += kton;
				} else {
					self.data.insert(who.into(), FinalData { who: who.into(), ring: 0, kton });
				}
			}
		}
		for &(ref who, ring) in &self.deposit_holders_detail {
			if self.deposit_eligible_list.contains(who) {
				if let Some(d) = self.data.get_mut(who) {
					d.ring += ring;
				} else {
					self.data.insert(who.into(), FinalData { who: who.into(), ring, kton: 0 });
				}
			}
		}

		self
	}

	pub fn save(self) -> Result<()> {
		let now = Utc::now().format("%F-%H-%M-%S");

		{
			let mut wtr = Writer::from_path(format!("snapshot-ring-eligible-list-{now}.csv"))?;

			for who in self.ring_eligible_list {
				wtr.serialize(UserData { who })?;
			}

			wtr.flush()?;
		}

		{
			let mut wtr = Writer::from_path(format!("snapshot-kton-eligible-list{now}.csv"))?;

			for who in self.kton_eligible_list {
				wtr.serialize(UserData { who })?;
			}

			wtr.flush()?;
		}

		{
			let mut wtr = Writer::from_path(format!("snapshot-deposit-eligible-list-{now}.csv"))?;

			for who in self.deposit_eligible_list {
				wtr.serialize(UserData { who })?;
			}

			wtr.flush()?;
		}

		{
			let mut wtr1 = Writer::from_path(format!("snapshot-ring-holders-detail-{now}.csv"))?;
			let mut wtr2 = Writer::from_path(format!("snapshot-kton-holders-detail-{now}.csv"))?;

			for (who, ring, kton) in self.ring_kton_holders_detail {
				wtr1.serialize(BasicData { who: who.clone(), amount: ring })?;
				wtr2.serialize(BasicData { who, amount: kton })?;
			}

			wtr1.flush()?;
			wtr2.flush()?;
		}

		{
			let mut wtr = Writer::from_path(format!("snapshot-deposit-holders-detail-{now}.csv"))?;

			for (who, ring) in self.deposit_holders_detail {
				wtr.serialize(BasicData { who, amount: ring })?;
			}

			wtr.flush()?;
		}

		{
			let mut wtr = Writer::from_path(format!("eligible-list-{now}.csv"))?;

			for (_, d) in self.data {
				if d.ring > 0 || d.kton > 0 {
					wtr.serialize(d)?;
				}
			}

			wtr.flush()?;
		}

		Ok(())
	}
}

#[derive(Debug, Default, Decode, Serialize)]
struct UserData {
	who: String,
}

#[derive(Debug, Default, Decode, Serialize)]
struct BasicData {
	who: String,
	amount: u128,
}

#[derive(Debug, Default, Decode, Serialize)]
struct FinalData {
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

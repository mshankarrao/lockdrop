#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

impl crate::WeightInfo for () {
	fn create_campaign() -> Weight {
		(30_100_000 as Weight)
			.saturating_add(DbWeight::get().reads(2 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn conclude_campaign() -> Weight {
		(77_800_000 as Weight)
			.saturating_add(DbWeight::get().reads(2 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn remove_expired_child_storage() -> Weight {
		(9_500_000 as Weight).saturating_add(DbWeight::get().reads(1 as Weight))
	}
	fn lock() -> Weight {
		(44_100_000 as Weight)
			.saturating_add(DbWeight::get().reads(2 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn unlock() -> Weight {
		(7_200_000 as Weight).saturating_add(DbWeight::get().reads(1 as Weight))
	}
}

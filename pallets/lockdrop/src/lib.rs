#![cfg_attr(not(feature = "std"), no_std)]
// #[cfg(feature = "runtime-benchmarks")]
// mod benchmarking;
// mod default_weights;

pub use pallet::*;

use codec::{Decode, Encode};
use scale_info::TypeInfo;
use frame_support::{
	ensure,
	storage::child,
	traits::{Currency, Get, LockIdentifier, LockableCurrency, WithdrawReasons},
	weights::Weight,
};
use frame_support::traits::IsType;
use frame_system::{ensure_root, ensure_signed};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{traits::Hash, RuntimeDebug};
use sp_std::{cmp,convert::TryInto, prelude::*};
pub type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
pub type AccountId<T> = <T as frame_system::Config>::AccountId;

#[frame_support::pallet]
pub mod pallet {
	use codec::{EncodeLike};
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	pub type CampaignIdentifier = [u8; 4];

	#[derive(Encode, Decode, TypeInfo, Clone, Eq, PartialEq, RuntimeDebug)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub struct CampaignInfo<BlockNumber> {
		end_block: BlockNumber,
		min_lock_end_block: BlockNumber,
		child_root: Option<Vec<u8>>,
	}

	   impl<T: Parameter + Encode + Decode> EncodeLike<Option<CampaignInfo<T>>> for CampaignInfo<T>
	   {
	   }

	#[derive(Encode, Decode, TypeInfo, Clone, Eq, PartialEq, RuntimeDebug)]
    #[scale_info(skip_type_params(T))]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub struct LockInfo<T: Config> {
		balance: BalanceOf<T>,
		end_block: BlockNumberFor<T>,
	}

	use frame_support::traits::Currency;

	impl<T: Config> EncodeLike<Option<LockInfo<T>>> for LockInfo<T>
		{
			
		}

	#[derive(Encode, Decode, TypeInfo, Clone, Eq, PartialEq, RuntimeDebug)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub struct ChildLockData<BlockNumber, Balance> {
		balance: Balance,
		end_block: BlockNumber,
		payload: Option<Vec<u8>>,
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// An implementation of on-chain currency.
		type Currency: LockableCurrency<Self::AccountId>;

		/// Payload length limit.
		type PayloadLenLimit: Get<u32>;
		/// Max number of storage keys to remove per extrinsic call.
		type RemoveKeysLimit: Get<u32>;

	}

	#[pallet::error]
	pub enum Error<T> {
		/// The given campaign name was used in the past.
		CampaignIdentifierUsedInPast,
		/// The given campaign trying to create has already existed.
		CampaignAlreadyExists,
		/// Campaign end block must be in the future.
		CampaignEndInThePast,
		/// Campaign lock block must be after campaign end block.
		CampaignLockEndBeforeCampaignEnd,
		/// Not enough balance.
		NotEnoughBalance,
		/// Payload over length limit.
		PayloadOverLenLimit,
		/// Campaign does not exist.
		CampaignNotExists,
		/// Campaign has already expired.
		CampaignAlreadyExpired,
		/// Attempt to lock less than what is already locked.
		AttemptedToLockLess,
		/// Invalid lock end block.
		InvalidLockEndBlock,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config>  {
		CampaignCreated(CampaignIdentifier),
		CampaignConcluded(CampaignIdentifier, Vec<u8>),
		ChildStorageRemoved(CampaignIdentifier),
		ChildStoragePartiallyRemoved(CampaignIdentifier),
		Locked(CampaignIdentifier, AccountId<T>),
		Unlocked(CampaignIdentifier, AccountId<T>),
	}

	#[pallet::storage]
	#[pallet::getter(fn campaigns)]
	pub(super) type Campaigns<T: Config> = StorageMap<_, Blake2_128Concat, CampaignIdentifier, Option<CampaignInfo<T::BlockNumber>>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn locks)]
	pub(super) type Locks<T: Config> = StorageDoubleMap<_, Blake2_128Concat, CampaignIdentifier, Blake2_128Concat, T::AccountId, Option<LockInfo<T>>, ValueQuery>;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(10_000)]
		pub fn create_campaign(origin: OriginFor<T>, identifier: CampaignIdentifier, end_block: T::BlockNumber, min_lock_end_block: T::BlockNumber) -> DispatchResultWithPostInfo {

			ensure_root(origin)?;

			let campaign_name_used_in_past = Locks::<T>::iter_prefix_values(identifier).next().is_some();
			ensure!(!campaign_name_used_in_past, Error::<T>::CampaignIdentifierUsedInPast);

			ensure!(!Campaigns::<T>::contains_key(&identifier), Error::<T>::CampaignAlreadyExists);

			let current_number = frame_system::Pallet::<T>::block_number();
			ensure!(end_block > current_number, Error::<T>::CampaignEndInThePast);
			ensure!(min_lock_end_block > end_block, Error::<T>::CampaignLockEndBeforeCampaignEnd);

			Campaigns::<T>::insert(identifier, CampaignInfo { end_block, min_lock_end_block, child_root: None });
			Self::deposit_event(Event::<T>::CampaignCreated(identifier));

			Ok(().into())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(10_000)]
		pub fn conclude_campaign(origin: OriginFor<T>, identifier: CampaignIdentifier) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			Campaigns::<T>::mutate(&identifier, |info| {
				if let Some(ref mut info) = info {
					if info.child_root.is_none() {
						let current_number = frame_system::Pallet::<T>::block_number();
						if current_number > info.end_block {
							let child_root = Self::child_root(&identifier);
							info.child_root = Some(child_root.clone());
							Self::deposit_event(Event::<T>::CampaignConcluded(identifier, child_root));
						}
					}
				}
			});

			Ok(().into())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(10_000)]
		pub fn remove_expired_child_storage(origin: OriginFor<T>, identifier: CampaignIdentifier) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			let info = Campaigns::<T>::get(&identifier);
			if let Some(info) = info {
				let current_number = frame_system::Pallet::<T>::block_number();
				if current_number > info.end_block && info.child_root.is_some() {
					match Self::child_kill(&identifier) {
						child::KillStorageResult::AllRemoved(_) => {
							Self::deposit_event(Event::<T>::ChildStorageRemoved(identifier));
						},
						child::KillStorageResult::SomeRemaining(_) => {
							Self::deposit_event(Event::<T>::ChildStoragePartiallyRemoved(identifier));
						}
					}
				}
			}
			Ok(().into())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(10_000)]
		pub fn lock(origin: OriginFor<T>, identifier: CampaignIdentifier, amount: BalanceOf<T>, lock_end_block: T::BlockNumber, payload: Option<Vec<u8>>) -> DispatchResultWithPostInfo {
			let account_id = ensure_signed(origin)?;

			ensure!(T::Currency::free_balance(&account_id) >= amount, Error::<T>::NotEnoughBalance);

			if let Some(ref payload) = payload {
				ensure!(payload.len() <= T::PayloadLenLimit::get() as usize, Error::<T>::PayloadOverLenLimit);
			}
			let campaign_info = Campaigns::<T>::get(&identifier).ok_or(Error::<T>::CampaignNotExists)?;

			let current_number = frame_system::Pallet::<T>::block_number();
			ensure!(current_number <= campaign_info.end_block, Error::<T>::CampaignAlreadyExpired);
			ensure!(lock_end_block > campaign_info.min_lock_end_block, Error::<T>::InvalidLockEndBlock);

			let lock_info = match Locks::<T>::get(&identifier, &account_id) {
				Some(mut lock_info) => {
					ensure!(amount >= lock_info.balance, Error::<T>::AttemptedToLockLess);
					ensure!(lock_end_block >= lock_info.end_block, Error::<T>::AttemptedToLockLess);

					lock_info.balance = cmp::max(amount, lock_info.balance);
					lock_info.end_block = cmp::max(lock_end_block, lock_info.end_block);
					lock_info
				},
				None => LockInfo { balance: amount, end_block: lock_end_block },
			};

			let lock_identifier = Self::lock_identifier(identifier);
			T::Currency::extend_lock(lock_identifier, &account_id, amount, WithdrawReasons::all());

			let child_lock_data = ChildLockData { balance: lock_info.balance, end_block: lock_info.end_block, payload };
			Self::child_data_put(&identifier, &account_id, &child_lock_data);

			Locks::<T>::insert(identifier, account_id.clone(), lock_info);
			Self::deposit_event(Event::<T>::Locked(identifier, account_id));
			Ok(().into())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(10_000)]
		pub fn unlock(origin: OriginFor<T>, identifier: CampaignIdentifier, amount: BalanceOf<T>) -> DispatchResultWithPostInfo {
			let account_id = ensure_signed(origin)?;

			let info = Locks::<T>::get(&identifier, &account_id);
			if let Some(info) = info {
				let current_number = frame_system::Pallet::<T>::block_number();
				if current_number > info.end_block {
					Locks::<T>::remove(identifier, account_id.clone());

					let lock_identifier = Self::lock_identifier(identifier);
					T::Currency::remove_lock(lock_identifier, &account_id);

					Self::deposit_event(Event::<T>::Unlocked(identifier, account_id));
				}
			}
			Ok(().into())
		}
	}


	impl<T: Config> Pallet<T> {
		pub fn lock_identifier(identifier: CampaignIdentifier) -> LockIdentifier {
			[
				b'd',
				b'r',
				b'o',
				b'p',
				identifier[0],
				identifier[1],
				identifier[2],
				identifier[3],
			]
		}

		pub fn child_info(identifier: &CampaignIdentifier) -> child::ChildInfo {
			let mut buf = Vec::new();
			buf.extend_from_slice(b"lockdrop:");
			buf.extend_from_slice(identifier);
			child::ChildInfo::new_default(T::Hashing::hash(&buf[..]).as_ref())
		}

		fn child_data_put(
			identifier: &CampaignIdentifier,
			account_id: &T::AccountId,
			data: &ChildLockData<T::BlockNumber, BalanceOf<T>>,
		) {
			account_id.using_encoded(|account_id| {
				child::put(&Self::child_info(identifier), &account_id, &data)
			})
		}

		pub fn child_data_get(
			identifier: &CampaignIdentifier,
			account_id: &T::AccountId,
		) -> Option<ChildLockData<T::BlockNumber, BalanceOf<T>>> {
			account_id
				.using_encoded(|account_id| child::get(&Self::child_info(identifier), &account_id))
		}

		pub fn child_root(identifier: &CampaignIdentifier) -> Vec<u8> {
			child::root(&Self::child_info(identifier), Default::default())
		}

		fn child_kill(identifier: &CampaignIdentifier) -> child::KillStorageResult {
			child::kill_storage(
				&Self::child_info(identifier),
				Some(T::RemoveKeysLimit::get()),
			)
		}
	}
}
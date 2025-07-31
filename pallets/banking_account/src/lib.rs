use frame_support::{
    pallet_prelude::*,
    traits::{Currency, ExistenceRequirement, ReservableCurrency},
    weights::Weight,
};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::{AtLeast32BitUnsigned};
use sp_std::vec::Vec;
use codec::{Encode, Decode};
use scale_info::TypeInfo;

#[derive(Clone, Encode, Decode, Eq, PartialEq, Debug, TypeInfo, MaxEncodedLen)]
pub enum Status {
    Operative,
    Dormant,
    Closed,
    Frozen,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, Debug, TypeInfo, MaxEncodedLen)]
pub struct BankingAccount<AccountId, Balance, Moment> {
    pub account_number: Vec<u8>,
    pub ifsc_code: Vec<u8>,
    pub micr_code: Option<Vec<u8>>,
    pub bank_name: Vec<u8>,
    pub branch_name: Vec<u8>,
    pub branch_address: Vec<u8>,
    pub account_holder: AccountId,
    pub holder_dob: Option<Moment>,
    pub holder_pan: Option<Vec<u8>>,
    pub holder_aadhaar: Option<Vec<u8>>,
    pub holder_category: Option<Vec<u8>>,
    pub account_type: Vec<u8>,
    pub opening_date: Moment,
    pub status: Status,
    pub current_balance: Balance,
    pub overdraft_limit: Option<Balance>,
    pub has_cheque_book: bool,
    pub has_atm_debit_card: bool,
    pub has_internet_banking: bool,
    pub has_mobile_banking: bool,
    pub last_txn: Option<Moment>,

    // Hierarchy
    pub parent_account: Option<AccountId>,
    pub child_accounts: Vec<AccountId>,
}

type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: ReservableCurrency<Self::AccountId>;
        type Moment: AtLeast32BitUnsigned + Parameter + Default + Copy + MaybeSerializeDeserialize + MaxEncodedLen;
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        AccountCreated(T::AccountId, BalanceOf<T>),
        SubAccountAdded(T::AccountId, T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        AccountAlreadyExists,
        AccountNotFound,
        CannotAddSelfAsChild,
    }

    #[pallet::storage]
    #[pallet::getter(fn bank_accounts)]
    pub type BankAccounts<T: Config> = StorageMap<
        _, Blake2_128Concat, T::AccountId, BankingAccount<T::AccountId, BalanceOf<T>, T::Moment>
    >;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(T::WeightInfo::create_account())]
        pub fn create_account(
            origin: OriginFor<T>,
            account_number: Vec<u8>,
            ifsc_code: Vec<u8>,
            bank_name: Vec<u8>,
            branch_name: Vec<u8>,
            branch_address: Vec<u8>,
            holder_dob: Option<T::Moment>,
            holder_pan: Option<Vec<u8>>,
            holder_aadhaar: Option<Vec<u8>>,
            holder_category: Option<Vec<u8>>,
            account_type: Vec<u8>,
            initial_balance: BalanceOf<T>,
        ) -> DispatchResult {
            let account_holder = ensure_signed(origin)?;

            ensure!(
                !BankAccounts::<T>::contains_key(&account_holder),
                Error::<T>::AccountAlreadyExists
            );

            let now = <frame_system::Pallet<T>>::block_number();

            let new_account = BankingAccount {
                account_number,
                ifsc_code,
                micr_code: None,
                bank_name,
                branch_name,
                branch_address,
                account_holder: account_holder.clone(),
                holder_dob,
                holder_pan,
                holder_aadhaar,
                holder_category,
                account_type,
                opening_date: now,
                status: Status::Operative,
                current_balance: initial_balance,
                overdraft_limit: None,
                has_cheque_book: false,
                has_atm_debit_card: false,
                has_internet_banking: false,
                has_mobile_banking: false,
                last_txn: None,
                parent_account: None,
                child_accounts: Vec::new(),
            };

            BankAccounts::<T>::insert(&account_holder, new_account);

            T::Currency::transfer(
                &account_holder,
                &Self::account_id(),
                initial_balance,
                ExistenceRequirement::KeepAlive,
            )?;

            Self::deposit_event(Event::AccountCreated(account_holder, initial_balance));
            Ok(())
        }

        #[pallet::weight(T::WeightInfo::add_sub_account())]
        pub fn add_sub_account(
            origin: OriginFor<T>,
            parent: T::AccountId,
            sub_account_id: T::AccountId,
        ) -> DispatchResult {
            let _caller = ensure_signed(origin)?;

            ensure!(parent != sub_account_id, Error::<T>::CannotAddSelfAsChild);
            ensure!(BankAccounts::<T>::contains_key(&parent), Error::<T>::AccountNotFound);
            ensure!(BankAccounts::<T>::contains_key(&sub_account_id), Error::<T>::AccountNotFound);

            BankAccounts::<T>::try_mutate(&parent, |maybe_parent| {
                let parent_account = maybe_parent.as_mut().ok_or(Error::<T>::AccountNotFound)?;
                if !parent_account.child_accounts.contains(&sub_account_id) {
                    parent_account.child_accounts.push(sub_account_id.clone());
                }
                Ok(())
            })?;

            BankAccounts::<T>::try_mutate(&sub_account_id, |maybe_sub| {
                let sub = maybe_sub.as_mut().ok_or(Error::<T>::AccountNotFound)?;
                sub.parent_account = Some(parent.clone());
                Ok(())
            })?;

            Self::deposit_event(Event::SubAccountAdded(parent, sub_account_id));
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        fn account_id() -> T::AccountId {
            // You can use PalletId if needed
            T::PalletInfo::name::<Self>()
                .as_bytes()
                .using_encoded(|b| T::AccountId::decode(&mut &blake2_256(b)[..]).unwrap_or_default())
        }
    }
}

pub trait WeightInfo {
    fn create_account() -> Weight;
    fn add_sub_account() -> Weight;
}

// 🧪 Default weights (mock); replace with benchmarked weights in production
// pub struct DefaultWeight;
// impl WeightInfo for DefaultWeight {
//     fn create_account() -> Weight {
//         10_000 + 10_000 // basic weight + write cost
//     }
//     fn add_sub_account() -> Weight {
//         15_000 + 20_000 // read + write cost
//     }
// }
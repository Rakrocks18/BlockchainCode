use frame_support::pallet;

#[derive(Clone, Encode, Decode, Eq, PartialEq, Debug, TypeInfo)]
pub struct BankingAccount<AccountId, Balance, Moment> {
    pub account_number: Vec<u8>,    // e.g. b"123456789012345"
    pub ifsc_code: Vec<u8>,         // 11-char IFSC
    pub micr_code: Option<Vec<u8>>,
    pub bank_name: Vec<u8>,
    pub branch_name: Vec<u8>,
    pub branch_address: Vec<u8>,
    pub account_holder: AccountId,
    pub holder_dob: Option<Moment>,
    pub holder_pan: Option<Vec<u8>>,
    pub holder_aadhaar: Option<Vec<u8>>,
    pub holder_category: Option<Vec<u8>>,
    pub account_type: Vec<u8>,      // e.g. b"Savings"
    pub opening_date: Moment,
    pub status: Status,            // e.g. b"Operative"
    pub current_balance: Balance,
    pub overdraft_limit: Option<Balance>,
    pub has_cheque_book: bool,
    pub has_atm_debit_card: bool,
    pub has_internet_banking: bool,
    pub has_mobile_banking: bool,
    pub last_txn: Option<Moment>,

    
}

pub enum Status {
    Operative,
    Dormant,
    Closed,
    Frozen,
}

type BalanceOf<T> =  <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
#[pallet::config]
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    type Currency: ReservableCurrency<Self::AccountId>;
    type Moment: AtLeast32BitUnsigned + Parameter + Default + Copy + MaybeSerializeDeserialize + MaxEncodedLen;
}

#[pallet::pallet]
#[pallet::generate_store(pub(super) trait Store)]
pub struct Pallet<T>(_);


#[pallet::event]
pub enum Event<T: Config> {
    AccountCreated(T::AccountId, BalanceOf<T>),
    Deposited(T::AccountId, BalanceOf<T>),
    Withdrawn(T::AccountId, BalanceOf<T>),
    Transferred(T::AccountId, T::AccountId, BalanceOf<T>),
    AccountClosed(T::AccountId),
}

#[pallet::storage]
#[pallet::getter(fn bank_accounts)]
/// Stores the banking account details for each account ID.
pub type BankAccounts<T: Config> = StorageMap<
    _, Blake2_128Concat, T::AccountId, BankingAccount<T::AccountId, BalanceOf<T>, T::Moment>
>;

#[pallet::call]
impl<T: Config> Pallet<T> {
    #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
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

        // Create a new banking account
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

        };

        //ensure!(new_account.opening_balance >= initial_balance, Error::<T>::BalanceLow);



        // Store the account in the BankAccounts map
        <BankAccounts<T>>::insert(&account_holder, new_account);

        // Deposit the initial balance into the account
         <T::Currency>::transfer(
            &account_holder,
            & Self::account_id(),
            initial_balance,
            ExistenceRequirement::KeepAlive,
        )?;

        // Emit an event
        Self::deposit_event(Event::AccountCreated(account_holder,initial_balance));
        Ok(())
    }
}

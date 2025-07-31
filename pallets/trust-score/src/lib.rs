#![cfg_attr(not(feature = "std"), no_std)]

use core::f32::consts::E;

use frame_support::{
    decl_module, decl_storage, decl_event, decl_error, 
    traits::{Get, Randomness},
    weights::Weight,
    codec::{Encode, Decode},
};

use frame_system::ensure_signed;
use sp_std::vec::Vec;
use sp_runtime::traits::{Zero, Saturating};

pub trait Config: frame_system::Config {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
    
    /// Maximum trust score a node can have
    type MaxTrustScore: Get<f32>;
    
    /// Minimum trust score before penalties
    type MinTrustScore: Get<f32>;
    
    /// Trust score adjustment for successful validation
    type SuccessReward: Get<f32>;
    
    /// Trust score penalty for failed validation
    type FailurePenalty: Get<f32>;
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug)]
pub struct NodeTrustData<AccountId> {
    pub validator: AccountId,
    pub trust_score: f32,
    pub successful_validations: u32,
    pub failed_validations: u32,
    pub last_updated: u32,
    pub flagged_for_removal: bool, 
}

decl_storage! {
    trait Store for Module<T: Config> as TrustScore {
        /// Trust scores for validator nodes
        TrustScores get(fn trust_scores): 
            map hasher(blake2_128_concat) T::AccountId => Option<NodeTrustData<T::AccountId>>;
        
        /// List of all validators with trust scores
        ValidatorList get(fn validator_list): Vec<T::AccountId>;
        
        /// Global trust score statistics
        AverageTrustScore get(fn average_trust_score): f32 = 0.5;
        
        /// Minimum trust score required for validation
        MinValidationTrust get(fn min_validation_trust): f32 = 0.4;
    }
}

decl_event!(
    pub enum Event<T> where AccountId = <T as frame_system::Config>::AccountId {
        /// Trust score updated for a validator
        TrustScoreUpdated(AccountId, u32),
        
        /// Validator added to trust system
        ValidatorAdded(AccountId),
        
        /// Validator removed due to low trust score
        ValidatorRemoved(AccountId),
        
        /// Validation successful, trust score increased
        ValidationSuccessful(AccountId, u32),
        
        /// Validation failed, trust score decreased
        ValidationFailed(AccountId, u32),
    }
);

decl_error! {
    pub enum Error for Module<T: Config> {
        /// Validator not found in trust system
        ValidatorNotFound,
        /// Trust score too low for validation
        TrustScoreTooLow,
        /// Invalid trust score value
        InvalidTrustScore,
    }
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        type Error = Error<T>;
        fn deposit_event() = default;
        
        const MaxTrustScore: f32 = T::MaxTrustScore::get();
        const MinTrustScore: f32 = T::MinTrustScore::get();
        
        /// Initialize a validator in the trust system
        #[weight = 10_000]
        pub fn initialize_validator(
            origin,
            validator: T::AccountId,
        ) -> Result<(), Error<T>> {
            let _who = ensure_signed(origin)?;
            
            let initial_trust_data = NodeTrustData {
                validator: validator.clone(),
                trust_score: 0.5, // Start with neutral score
                successful_validations: 0,
                failed_validations: 0,
                last_updated: <frame_system::Module<T>>::block_number().saturated_into::<u32>(),
                flagged_for_removal: false,
            };
            
            TrustScores::<T>::insert(&validator, &initial_trust_data);
            ValidatorList::<T>::mutate(|list| list.push(validator.clone()));
            
            Self::deposit_event(RawEvent::ValidatorAdded(validator));
            Ok(())
        }
        
        /// Record successful validation
        #[weight = 5_000]
        pub fn update_trust_score(
            origin,
            validator: T::AccountId,
            vote_matched: bool,  // True if node's vote matched network consensus
        ) -> Result<(), Error<T>> {
            let _who = ensure_signed(origin)?;
            
            TrustScores::<T>::try_mutate(&validator, |trust_data_opt| {
                let trust_data = trust_data_opt.as_mut().ok_or(Error::<T>::ValidatorNotFound)?;
                
                // Skip update if node is already flagged for removal
                if trust_data.flagged_for_removal {
                    return Ok(());
                }
                
                // Calculate new trust score based on vote match
                if vote_matched {
                    let increase = increase_fn(trust_data.trust_score);
                    trust_data.trust_score = (trust_data.trust_score + increase).min(1.0);
                    trust_data.successful_validations += 1;
                } else {
                    let decrease = decrease_fn(trust_data.trust_score);
                    trust_data.trust_score = (trust_data.trust_score - decrease).max(0.0);
                    trust_data.failed_validations += 1;
                    
                    // Flag for removal if trust score falls below 0.1
                    if trust_data.trust_score < 0.1 {
                        trust_data.flagged_for_removal = true;
                        Self::deposit_event(RawEvent::ValidatorRemoved(validator.clone()));
                    }
                }
                
                trust_data.last_updated = <frame_system::Module<T>>::block_number().saturated_into::<u32>();
                
                // Emit appropriate events
                if vote_matched {
                    Self::deposit_event(RawEvent::ValidationSuccessful(validator.clone(), trust_data.trust_score));
                } else {
                    Self::deposit_event(RawEvent::ValidationFailed(validator.clone(), trust_data.trust_score));
                }
                Self::deposit_event(RawEvent::TrustScoreUpdated(validator.clone(), trust_data.trust_score));
                
                Ok(())
            })
        }
    }

    #[weight = 10_000]
    pub fn cleanup_validators(origin) -> Result<(), Error<T>> {
        let _who = ensure_signed(origin)?;
        
        let validators_to_remove: Vec<T::AccountId> = ValidatorList::<T>::get()
            .into_iter()
            .filter(|validator| {
                Self::trust_scores(validator)
                    .map(|data| data.flagged_for_removal)
                    .unwrap_or(false)
            })
            .collect();
        
        for validator in validators_to_remove {
            Self::remove_validator(&validator);
        }
        
        Ok(())
    }
}

#[inline(always)]
fn increase_fn(trust_score: f32) -> f32 {
    0.001_f32 * (0.5_f32 * E.powf(2.5_f32 * trust_score))
}

#[inline(always)]
fn decrease_fn(trust_score: f32) -> f32 {
    0.001_f32 * (1.0_f32 - (1.0_f32 / (1.0_f32 - 2.5_f32 * trust_score)))
}

impl<T: Config> Module<T> {
    /// Check if validator can participate in validation
    fn remove_validator(validator: &T::AccountId) {
        if let Some(trust_data) = Self::trust_scores(validator) {
            if trust_data.flagged_for_removal {
                TrustScores::<T>::remove(validator);
                ValidatorList::<T>::mutate(|list| list.retain(|v| v != validator));
                Self::deposit_event(RawEvent::ValidatorRemoved(validator.clone()));
            }
        }
    }
    
    /// Get trust score for a validator
    pub fn get_trust_score(validator: &T::AccountId) -> Option<f32> {
        Self::trust_scores(validator).map(|data| data.trust_score)
    }
    
    /// Remove validator from the system
    fn remove_validator(validator: &T::AccountId) {
        TrustScores::<T>::remove(validator);
        ValidatorList::<T>::mutate(|list| list.retain(|v| v != validator));
        Self::deposit_event(RawEvent::ValidatorRemoved(validator.clone()));
    }
    
    /// Get validators sorted by trust score
    pub fn get_validators_by_trust() -> Vec<(T::AccountId, u32)> {
        let mut validators: Vec<(T::AccountId, u32)> = Self::validator_list()
            .into_iter()
            .filter_map(|validator| {
                Self::get_trust_score(&validator).map(|score| (validator, score))
            })
            .collect();
        
        validators.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by trust score descending
        validators
    }

    
}
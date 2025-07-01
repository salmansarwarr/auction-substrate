#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        dispatch::DispatchResult,
        pallet_prelude::*,
        traits::{Get, UnixTime},
    };
    use frame_system::pallet_prelude::*;
    use sp_std::vec::Vec;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_identity::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type TimeProvider: UnixTime;
        #[pallet::constant]
        type MaxUsernameLength: Get<u32>;
    }

    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
    pub struct UserProfile<AccountId, Moment> {
        pub username: Vec<u8>,
        pub wallet_address: AccountId,
        pub created_at: Moment,
    }

    #[pallet::storage]
    #[pallet::getter(fn profiles)]
    pub type Profiles<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, UserProfile<T::AccountId, u64>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn username_to_account)]
    pub type UsernameToAccount<T: Config> =
        StorageMap<_, Blake2_128Concat, Vec<u8>, T::AccountId, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Profile created [who, username]
        ProfileCreated {
            who: T::AccountId,
            username: Vec<u8>,
        },
        /// Profile updated [who, username]
        ProfileUpdated {
            who: T::AccountId,
            username: Vec<u8>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Profile already exists
        ProfileAlreadyExists,
        /// Profile not found
        ProfileNotFound,
        /// Username already taken
        UsernameTaken,
        /// Username too long
        UsernameTooLong,
        /// Invalid username
        InvalidUsername,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(10_000)]
        pub fn create_profile(origin: OriginFor<T>, username: Vec<u8>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Validate username
            ensure!(
                username.len() <= <T as Config>::MaxUsernameLength::get() as usize,
                Error::<T>::UsernameTooLong
            );
            ensure!(!username.is_empty(), Error::<T>::InvalidUsername);

            // Check if profile already exists
            ensure!(
                !Profiles::<T>::contains_key(&who),
                Error::<T>::ProfileAlreadyExists
            );

            // Check if username is taken
            ensure!(
                !UsernameToAccount::<T>::contains_key(&username),
                Error::<T>::UsernameTaken
            );

            let created_at = T::TimeProvider::now().as_secs();

            let profile = UserProfile {
                username: username.clone(),
                wallet_address: who.clone(),
                created_at,
            };

            // Store profile
            Profiles::<T>::insert(&who, &profile);
            UsernameToAccount::<T>::insert(&username, &who);

            Self::deposit_event(Event::ProfileCreated { who, username });

            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(10_000)]
        pub fn update_profile(origin: OriginFor<T>, username: Vec<u8>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Validate username
            ensure!(
                username.len() <= <T as Config>::MaxUsernameLength::get() as usize,
                Error::<T>::UsernameTooLong
            );
            ensure!(!username.is_empty(), Error::<T>::InvalidUsername);

            // Check if profile exists
            let mut profile = Profiles::<T>::get(&who).ok_or(Error::<T>::ProfileNotFound)?;

            // If username changed, check availability
            if profile.username != username {
                ensure!(
                    !UsernameToAccount::<T>::contains_key(&username),
                    Error::<T>::UsernameTaken
                );

                // Remove old username mapping
                UsernameToAccount::<T>::remove(&profile.username);
                // Add new username mapping
                UsernameToAccount::<T>::insert(&username, &who);
            }

            profile.username = username.clone();
            Profiles::<T>::insert(&who, &profile);

            Self::deposit_event(Event::ProfileUpdated { who, username });

            Ok(())
        }
    }

    // Helper functions
    impl<T: Config> Pallet<T> {
        pub fn get_profile_by_username(username: &[u8]) -> Option<UserProfile<T::AccountId, u64>> {
            let account = UsernameToAccount::<T>::get(username)?;
            Profiles::<T>::get(&account)
        }

        pub fn get_account_by_username(username: &[u8]) -> Option<T::AccountId> {
            UsernameToAccount::<T>::get(username)
        }
    }
}

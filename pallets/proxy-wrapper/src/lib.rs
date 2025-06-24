#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::sp_runtime::traits::{BlockNumberProvider, StaticLookup};
    use frame_support::sp_runtime::Saturating;
    use frame_support::{dispatch::DispatchResult, pallet_prelude::*, traits::Currency};
    use frame_system::pallet_prelude::*;
    use pallet_balances as balances;
    use pallet_proxy as proxy;

    #[pallet::config]
    pub trait Config: frame_system::Config + proxy::Config + balances::Config {}

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(10_000)]
        #[pallet::call_index(0)]
        pub fn add_proxy_and_create_account(
            origin: OriginFor<T>,
            delegate: T::AccountId,
            proxy_type: T::ProxyType,
            delay: <T::BlockNumberProvider as BlockNumberProvider>::BlockNumber,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Convert delegate AccountId to Lookup::Source type
            let delegate_lookup = T::Lookup::unlookup(delegate.clone());

            // Ensure the delegate account exists and is properly initialized
            // This will set the provider reference correctly
            frame_system::Pallet::<T>::inc_providers(&delegate);

            let min_balance = balances::Pallet::<T>::minimum_balance();

            // Deposit zero to create account storage entry
            let _ = balances::Pallet::<T>::deposit_creating(&delegate, min_balance);

            // Call proxy add_proxy
            proxy::Pallet::<T>::add_proxy(
                frame_system::RawOrigin::Signed(who.clone()).into(),
                delegate_lookup,
                proxy_type,
                delay,
            )?;

            Ok(())
        }
    }
}

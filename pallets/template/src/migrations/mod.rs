
use super::*;
use frame_support::{pallet_prelude::*, traits::OnRuntimeUpgrade, weights::Weight};
use sp_std::marker::PhantomData;

pub mod v1;
/// The current storage version.
pub const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

/// Migration from u32 to u64 in Prices storage
pub struct MigrateToV2<T>(PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for MigrateToV2<T> {
    fn on_runtime_upgrade() -> Weight {
        let current_version = Pallet::<T>::in_code_storage_version();
        let onchain_version = Pallet::<T>::on_chain_storage_version();
        
        if current_version == STORAGE_VERSION && onchain_version < STORAGE_VERSION {
            return v1::migrate::<T>();
        }
        
        T::DbWeight::get().reads(1)
    }
}
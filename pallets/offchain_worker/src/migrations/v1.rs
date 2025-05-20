//! # V1 Migration
//! 
//! Migration from V0 to V1
//! This migration converts the Prices storage from u32 to u64.

use crate::*;
use frame_support::{
    weights::Weight,
    BoundedVec,
};
use sp_std::vec::Vec;
use frame_support::pallet_prelude::ValueQuery;

/// Perform the V0 -> V1 migration (u32 to u64).
pub fn migrate<T: crate::Config>() -> Weight {
    let mut reads = 0;
    let mut writes = 0;

    // Define the old storage type
    #[frame_support::storage_alias]
    type OldPrices<T: Config> = StorageValue<Pallet<T>, BoundedVec<u32, <T as pallet::Config>::MaxPrices>, ValueQuery>;

    // Read the old prices
    let old_prices = OldPrices::<T>::get();
    reads += 1;

    // Convert u32 prices to u64
    let new_prices: BoundedVec<u64, T::MaxPrices> = old_prices
        .into_iter()
        .map(|price| price as u64)
        .collect::<Vec<u64>>()
        .try_into()
        .expect("Same number of elements as the original BoundedVec; qed");

    // Write the new prices
    Prices::<T>::put(new_prices);
    writes += 1;

    log::info!("✅ Migration to v2 complete: Prices storage migrated from u32 to u64");
    
    T::DbWeight::get().reads_writes(reads, writes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{new_test_ext, Test};

    #[test]
    fn test_migration_u32_to_u64() {
        new_test_ext().execute_with(|| {
            // Setup old storage with test values
            let old_prices: BoundedVec<u32, <Test as Config>::MaxPrices> = 
                vec![100, 200, 300].try_into().unwrap();
            
            #[frame_support::storage_alias]
            type OldPrices<T: Config> = StorageValue<Pallet<T>, BoundedVec<u32, T::MaxPrices>, ValueQuery>;
            
            OldPrices::<Test>::put(old_prices);
            
            // Run migration
            let weight = migrate::<Test>();
            
            // Assert new storage has correct values
            let new_prices = Prices::<Test>::get();
            assert_eq!(new_prices.len(), 3);
            assert_eq!(new_prices[0], 100u64);
            assert_eq!(new_prices[1], 200u64);
            assert_eq!(new_prices[2], 300u64);
        });
    }
}
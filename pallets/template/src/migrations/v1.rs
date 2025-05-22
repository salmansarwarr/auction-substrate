use super::*;
use frame_support::{traits::Get, weights::Weight};
use frame_support::migration::{have_storage_value, get_storage_value, take_storage_value};

pub fn migrate<T: Config>() -> Weight {
    let mut weight = T::DbWeight::get().reads_writes(1, 1);
    log::info!("🔄 Running migration from v1 to v2 to remove DummyStorage");

    // Get the pallet name bytes for storage operations
    let pallet_name = <Pallet<T>>::name().as_bytes();
    
    // Check if DummyStorage exists before proceeding
    let exists = have_storage_value(
        pallet_name,
        "DummyStorage".as_bytes(),
        &[]
    );

    if exists {
        // Read the value for logging purposes
        let old_value = get_storage_value::<u64>(
            pallet_name,
            "DummyStorage".as_bytes(),
            &[]
        ).unwrap_or_default();
        
        log::info!("📝 Found DummyStorage with value: {:?}", old_value);
        
        // Remove the storage item and retrieve its value
        take_storage_value::<u64>(
            pallet_name,
            "DummyStorage".as_bytes(),
            &[]
        );
        
        weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
        log::info!("✅ DummyStorage has been removed successfully");
    } else {
        log::info!("ℹ️ DummyStorage doesn't exist, nothing to migrate");
    }

    // Update storage version
    StorageVersion::new(2).put::<Pallet<T>>();
    weight = weight.saturating_add(T::DbWeight::get().writes(1));
    
    log::info!("✅ Migration to v2 completed successfully");
    weight
}
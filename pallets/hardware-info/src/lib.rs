#![cfg_attr(not(feature = "std"), no_std)]

pub mod weights;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo;
    use frame_support::{
        pallet_prelude::*,
        traits::Get,
        PalletId,
    };
    use frame_system::pallet_prelude::*;
    use sp_std::prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The interval of blocks after which hardware info should be collected
        #[pallet::constant]
        type HardwareInfoInterval: Get<u32>;

        /// Maximum number of hardware info entries to keep in history
        #[pallet::constant]
        type MaxHardwareHistoryEntries: Get<u32>;

        type PalletId: Get<PalletId>;

        type WeightInfo: WeightInfo;
    }

    /// Current hardware information
    #[pallet::storage]
    #[pallet::getter(fn current_hardware_info)]
    pub type CurrentHardwareInfo<T: Config> = StorageValue<_, HardwareInfo, OptionQuery>;

    /// Historical hardware information with bounded size
    #[pallet::storage]
    #[pallet::getter(fn hardware_history)]
    pub type HardwareHistory<T: Config> = StorageValue<
        _,
        BoundedVec<HardwareInfo, T::MaxHardwareHistoryEntries>,
        ValueQuery,
    >;

    /// Last block number when hardware info was collected
    #[pallet::storage]
    #[pallet::getter(fn last_collection_block)]
    pub type LastCollectionBlock<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Hardware information structure
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
    pub struct HardwareInfo {
        pub cpu_cores: u32,
        pub total_memory: u64,     // in bytes
        pub available_memory: u64, // in bytes
        pub cpu_usage: u32,        // percentage
        pub disk_usage: u32,       // percentage
        pub timestamp: u64,
        pub block_number: u32,
    }

    impl Default for HardwareInfo {
        fn default() -> Self {
            Self {
                cpu_cores: 0,
                total_memory: 0,
                available_memory: 0,
                cpu_usage: 0,
                disk_usage: 0,
                timestamp: 0,
                block_number: 0,
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Hardware information collected [block_number]
        HardwareInfoCollected(u32),
        /// Hardware info collection failed [block_number, error_message]
        HardwareInfoCollectionFailed(u32, Vec<u8>),
        /// Hardware history cleared [cleared_entries_count]
        HardwareHistoryCleared(u32),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Failed to collect hardware information
        HardwareCollectionFailed,
        /// Hardware info collection not due yet
        CollectionNotDue,
        /// Hardware history is full
        HardwareHistoryFull,
        /// No hardware info available
        NoHardwareInfoAvailable,
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(block_number: BlockNumberFor<T>) -> Weight {
            let current_block = TryInto::<u32>::try_into(block_number).unwrap_or(0);
            let last_collection = Self::last_collection_block();
            let interval = T::HardwareInfoInterval::get();

            // Check if it's time to collect hardware info
            if current_block.saturating_sub(last_collection) >= interval {
                let _ = Self::collect_and_store_hardware_info(current_block);
                return T::DbWeight::get().reads_writes(2, 3);
            }

            T::DbWeight::get().reads(1)
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Manual trigger for hardware info collection
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::force_collect_hardware_info())]
        pub fn force_collect_hardware_info(origin: OriginFor<T>) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            let current_block = <frame_system::Pallet<T>>::block_number();
            let block_num = TryInto::<u32>::try_into(current_block)
                .map_err(|_| Error::<T>::HardwareCollectionFailed)?;

            Self::collect_and_store_hardware_info(block_num)?;

            Ok(())
        }

        /// Clear hardware history (root only)
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::clear_hardware_history())]
        pub fn clear_hardware_history(origin: OriginFor<T>) -> DispatchResult {
            ensure_root(origin)?;

            let history = Self::hardware_history();
            let count = history.len() as u32;

            HardwareHistory::<T>::kill();

            Self::deposit_event(Event::HardwareHistoryCleared(count));

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Collect hardware information and store it
        fn collect_and_store_hardware_info(block_number: u32) -> DispatchResult {
            match Self::get_hardware_info(block_number) {
                Ok(hardware_info) => {
                    // Store current hardware info
                    CurrentHardwareInfo::<T>::put(&hardware_info);

                    // Add to history with bounded size
                    HardwareHistory::<T>::try_mutate(|history| -> DispatchResult {
                        // Remove oldest entry if at capacity
                        if history.len() >= T::MaxHardwareHistoryEntries::get() as usize {
                            history.remove(0);
                        }

                        history
                            .try_push(hardware_info.clone())
                            .map_err(|_| Error::<T>::HardwareHistoryFull)?;

                        Ok(())
                    })?;

                    LastCollectionBlock::<T>::put(block_number);

                    Self::deposit_event(Event::HardwareInfoCollected(block_number));

                    Ok(())
                }
                Err(e) => {
                    Self::deposit_event(Event::HardwareInfoCollectionFailed(
                        block_number,
                        e.as_bytes().to_vec(),
                    ));
                    Err(Error::<T>::HardwareCollectionFailed.into())
                }
            }
        }

        /// Get current hardware information
        fn get_hardware_info(block_number: u32) -> Result<HardwareInfo, &'static str> {
            #[cfg(feature = "std")]
            {
                Self::get_hardware_info_std(block_number)
            }

            #[cfg(not(feature = "std"))]
            {
                // In no_std environment, return mock data
                Ok(HardwareInfo {
                    cpu_cores: 4,
                    total_memory: 8_000_000_000,
                    available_memory: 4_000_000_000,
                    cpu_usage: 50,
                    disk_usage: 30,
                    timestamp: 0, // Would need a timestamp source in no_std
                    block_number,
                })
            }
        }

        #[cfg(feature = "std")]
        fn get_hardware_info_std(block_number: u32) -> Result<HardwareInfo, &'static str> {
            use std::time::{SystemTime, UNIX_EPOCH};

            // Get timestamp
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "Failed to get timestamp")?
                .as_secs();

            // Get CPU cores
            let cpu_cores = num_cpus::get() as u32;

            // Get memory information using sysinfo
            let mut system = sysinfo::System::new_all();
            system.refresh_all();

            let total_memory = system.total_memory();
            let available_memory = system.available_memory();

            // Calculate CPU usage (simplified)
            let cpu_usage = system.global_cpu_usage() as u32;

            let disks = sysinfo::Disks::new_with_refreshed_list();
            let disk_usage = if !disks.is_empty() {
                let total_usage: u32 = disks
                    .iter()
                    .map(|disk| {
                        let total = disk.total_space();
                        let available = disk.available_space();
                        if total > 0 {
                            ((total - available) * 100 / total) as u32
                        } else {
                            0
                        }
                    })
                    .sum();
                
                total_usage / disks.len() as u32 // Average usage across all disks
            } else {
                0 // No disks found
            };

            Ok(HardwareInfo {
                cpu_cores,
                total_memory,
                available_memory,
                cpu_usage,
                disk_usage,
                timestamp,
                block_number,
            })
        }

        /// Get hardware info by block number from history
        pub fn get_hardware_info_at_block(block_number: u32) -> Option<HardwareInfo> {
            Self::hardware_history()
                .iter()
                .find(|info| info.block_number == block_number)
                .cloned()
        }

        /// Get latest N hardware info entries
        pub fn get_latest_hardware_info(count: u32) -> Vec<HardwareInfo> {
            let history = Self::hardware_history();
            let start_idx = if history.len() > count as usize {
                history.len() - count as usize
            } else {
                0
            };

            history.iter().skip(start_idx).cloned().collect()
        }
    }
}
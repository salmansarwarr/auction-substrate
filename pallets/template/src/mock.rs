use crate as pallet_template;
use frame_support::derive_impl;
use frame_support::{
    parameter_types,
    traits::{ConstU128, ConstU32, ConstU64, ConstU8},
};
use frame_system::{self as system, EnsureRoot};
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

pub const MILLI_UNIT: u128 = 1_000_000_000;

#[frame_support::runtime]
mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask
    )]
    pub struct Test;

    #[runtime::pallet_index(0)]
    pub type System = frame_system::Pallet<Test>;

    #[runtime::pallet_index(1)]
    pub type Balances = pallet_balances::Pallet<Test>;

    #[runtime::pallet_index(2)]
    pub type Template = pallet_template::Pallet<Test>;

    #[runtime::pallet_index(3)]
    pub type Uniques = pallet_uniques::Pallet<Test>;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

impl pallet_balances::Config for Test {
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    /// The type for recording an account's balance.
    type Balance = u128;
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<1>;
    type AccountStore = System;
    type WeightInfo = ();
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxFreezes = ();
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type DoneSlashHandler = ();
}

impl pallet_template::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type MaxBidsPerAuction = ConstU32<10>;
    type AuctionTimeoutBlocks = ConstU64<100>;
    type RoyaltyPercentage = ConstU8<10>;
}

impl pallet_uniques::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = u32;
    type ItemId = u32;
    type Currency = Balances;
    type ForceOrigin = frame_system::EnsureSigned<u64>;
    type CreateOrigin = frame_system::EnsureSigned<u64>;
    type Locker = ();
    
    // Lower deposit values
    type CollectionDeposit = ConstU128<1>;  // Minimum value
    type ItemDeposit = ConstU128<1>;        // Minimum value
    type MetadataDepositBase = ConstU128<1>;
    type AttributeDepositBase = ConstU128<1>;
    type DepositPerByte = ConstU128<1>;  

    type StringLimit = ConstU32<128>;
    type KeyLimit = ConstU32<32>;
    type ValueLimit = ConstU32<64>;
    type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1, 1000 * 1_000_000_000), // Asset owner
            (2, 2000 * 1_000_000_000), // Bidder 1
            (3, 3000 * 1_000_000_000), // Bidder 2
            (4, 4000 * 1_000_000_000), // Bidder 3
            (5, 5000 * 1_000_000_000), // Bidder 4
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();
    
    // Initialize the accounts explicitly
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        // Ensure reference counting is properly initialized for each account
        for account_id in 1..=5 {
            frame_system::Pallet::<Test>::inc_providers(&account_id);
        }
        // Set block number to 1 for event emission
        frame_system::Pallet::<Test>::set_block_number(1);
    });
    ext
}
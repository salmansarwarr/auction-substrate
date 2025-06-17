// This is free and unencumbered software released into the public domain.
//
// Anyone is free to copy, modify, publish, use, compile, sell, or
// distribute this software, either in source code form or as a compiled
// binary, for any purpose, commercial or non-commercial, and by any
// means.
//
// In jurisdictions that recognize copyright laws, the author or authors
// of this software dedicate any and all copyright interest in the
// software to the public domain. We make this dedication for the benefit
// of the public at large and to the detriment of our heirs and
// successors. We intend this dedication to be an overt act of
// relinquishment in perpetuity of all present and future rights to this
// software under copyright law.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
// OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
// OTHER DEALINGS IN THE SOFTWARE.
//
// For more information, please refer to <http://unlicense.org>

// Substrate and Polkadot dependencies
use frame_support::{
	derive_impl, parameter_types,
	traits::{AsEnsureOriginWithArg, ConstBool, ConstU128, ConstU32, ConstU64, ConstU8, VariantCountOf},
	weights::{
		constants::{RocksDbWeight, WEIGHT_REF_TIME_PER_SECOND},
		IdentityFee, Weight,
	},
};
use frame_system::{limits::{BlockLength, BlockWeights}, EnsureRoot, EnsureSigned};
use pallet_transaction_payment::{ConstFeeMultiplier, FungibleAdapter, Multiplier};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_runtime::{generic, traits::{One}, Perbill, SaturatedConversion };
use sp_version::RuntimeVersion;
use sp_core::sr25519::Signature;
use frame_support::PalletId;
use codec::Encode;

use crate::UncheckedExtrinsic;

use super::{
	AccountId, Aura, Balance, Balances, Block, BlockNumber, Hash, Nonce, PalletInfo, Runtime,
	RuntimeCall, RuntimeEvent, RuntimeFreezeReason, RuntimeHoldReason, RuntimeOrigin, RuntimeTask,
	System, EXISTENTIAL_DEPOSIT, SLOT_DURATION, VERSION, MILLI_UNIT
};

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

parameter_types! {
	pub const BlockHashCount: BlockNumber = 2400;
	pub const Version: RuntimeVersion = VERSION;

	/// We allow for 2 seconds of compute with a 6 second average block time.
	pub RuntimeBlockWeights: BlockWeights = BlockWeights::with_sensible_defaults(
		Weight::from_parts(2u64 * WEIGHT_REF_TIME_PER_SECOND, u64::MAX),
		NORMAL_DISPATCH_RATIO,
	);
	pub RuntimeBlockLength: BlockLength = BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub const SS58Prefix: u8 = 42;
}

/// The default types are being injected by [`derive_impl`](`frame_support::derive_impl`) from
/// [`SoloChainDefaultConfig`](`struct@frame_system::config_preludes::SolochainDefaultConfig`),
/// but overridden as needed.
#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig)]
impl frame_system::Config for Runtime {
	/// The block type for the runtime.
	type Block = Block;
	/// Block & extrinsics weights: base values and limits.
	type BlockWeights = RuntimeBlockWeights;
	/// The maximum length of a block (in bytes).
	type BlockLength = RuntimeBlockLength;
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The type for storing how many extrinsics an account has signed.
	type Nonce = Nonce;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = RocksDbWeight;
	/// Version of the runtime.
	type Version = Version;
	/// The data to be stored in an account.
	type AccountData = pallet_balances::AccountData<Balance>;
	/// This is used as an identifier of the chain. 42 is the generic substrate prefix.
	type SS58Prefix = SS58Prefix;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type DisabledValidators = ();
	type MaxAuthorities = ConstU32<32>;
	type AllowMultipleBlocksPerSlot = ConstBool<false>;
	type SlotDuration = pallet_aura::MinimumPeriodTimesTwo<Runtime>;
}

impl pallet_grandpa::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;

	type WeightInfo = ();
	type MaxAuthorities = ConstU32<32>;
	type MaxNominators = ConstU32<0>;
	type MaxSetIdSessionEntries = ConstU64<0>;

	type KeyOwnerProof = sp_core::Void;
	type EquivocationReportSystem = ();
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = Aura;
	type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
	type WeightInfo = ();
}

impl pallet_balances::Config for Runtime {
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<EXISTENTIAL_DEPOSIT>;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
	type FreezeIdentifier = RuntimeFreezeReason;
	type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type DoneSlashHandler = ();
}

parameter_types! {
	pub FeeMultiplier: Multiplier = Multiplier::one();
}

impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = FungibleAdapter<Balances, ()>;
	type OperationalFeeMultiplier = ConstU8<5>;
	type WeightToFee = IdentityFee<Balance>;
	type LengthToFee = IdentityFee<Balance>;
	type FeeMultiplierUpdate = ConstFeeMultiplier<FeeMultiplier>;
	type WeightInfo = pallet_transaction_payment::weights::SubstrateWeight<Runtime>;
}

impl pallet_assets::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;

    /// Balance type for assets (same as used elsewhere).
    type Balance = u128;

    /// Asset identifier (commonly u32).
    type AssetId = u32;

    /// Who can create new assets.
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;

    /// Which currency is used to pay deposits (usually `Balances` pallet).
    type Currency = Balances;

    /// Who can forcibly manage assets.
    type ForceOrigin = frame_system::EnsureRoot<AccountId>;

    /// Deposit required to create a new asset.
    type AssetDeposit = ConstU128<1_000_000_000_000>;

    /// Deposit required to register a new account for an asset.
    type AssetAccountDeposit = ConstU128<100_000_000>;

    /// Base deposit for asset metadata.
    type MetadataDepositBase = ConstU128<10_000_000>;

    /// Per-byte deposit for metadata.
    type MetadataDepositPerByte = ConstU128<1_000_000>;

    /// Approval deposit (optional, set to 0 if approvals are not used).
    type ApprovalDeposit = ConstU128<0>;

    /// Limit for string lengths (like asset name/symbol).
    type StringLimit = ConstU32<50>;

    /// Max number of items to remove during forced asset destruction.
    type RemoveItemsLimit = ConstU32<1000>;

    /// Extra type (can be `()` if unused).
    type Extra = ();

    /// Optional callback hooks (can be `()` if unused).
    type CallbackHandle = ();

    /// Optional freezing logic (can be `()` if unused).
    type Freezer = ();

    /// Weight info for benchmarking (you should generate this using benchmarking).
    type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;

	type AssetIdParameter = codec::Compact<u32>; // or just `u32` if no Compact encoding is needed

	type Holder = ();
}

impl pallet_sudo::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const CollectionDeposit: u128 = 1_000 * MILLI_UNIT;
    pub const ItemDeposit: u128 = 500 * MILLI_UNIT;
    pub const MetadataDepositBase: u128 = 100 * MILLI_UNIT;
    pub const AttributeDepositBase: u128 = 10 * MILLI_UNIT;
    pub const DepositPerByte: u128 = 1 * MILLI_UNIT;

    pub const StringLimit: u32 = 128;
    pub const KeyLimit: u32 = 32;
    pub const ValueLimit: u32 = 64;
}

impl pallet_uniques::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;

    type CollectionId = u32;
    type ItemId = u32;

    type Currency = Balances;
    type ForceOrigin = EnsureRoot<AccountId>;
    
    type CreateOrigin = frame_system::EnsureSigned<AccountId>; // or EnsureRootWithArg if using custom collection access
    
    type Locker = ();
    type CollectionDeposit = CollectionDeposit;
    type ItemDeposit = ItemDeposit;
    type MetadataDepositBase = MetadataDepositBase;
    type AttributeDepositBase = AttributeDepositBase;
    type DepositPerByte = DepositPerByte;

    type StringLimit = StringLimit;
    type KeyLimit = KeyLimit;
    type ValueLimit = ValueLimit;

    type WeightInfo = pallet_uniques::weights::SubstrateWeight<Runtime>;

    type Helper = ();
}

parameter_types! {
    pub const RoyaltyPercentage: u8 = 10; // 10% royalty
    pub const TemplatePalletId: PalletId = PalletId(*b"ex/auctn");
}

/// Configure the pallet-template in pallets/template.
impl pallet_template::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	
	// Use the Balances pallet as the Currency implementation
	type Currency = Balances;
	
	// Set maximum number of bids per auction
	type MaxBidsPerAuction = ConstU32<100>;
	
	// Set number of blocks after which auction auto-resolves
	type AuctionTimeoutBlocks = ConstU32<100>; // 100 blocks as per your requirement

	type RoyaltyPercentage = RoyaltyPercentage;

    type PalletId = TemplatePalletId;

    type WeightInfo = pallet_template::weights::SubstrateWeight<Runtime>;    
}




pub type SignedExtra = (
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
    frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
    frame_system::WeightReclaim<Runtime>,
);

pub type SignedPayload = generic::SignedPayload<RuntimeCall, SignedExtra>;

parameter_types! {
	pub const UnsignedPriority: u64 = 1 << 20;
}

impl frame_system::offchain::SigningTypes for Runtime {
	type Public = sp_runtime::MultiSigner;
	type Signature = Signature;
}

impl<LocalCall> frame_system::offchain::CreateTransactionBase<LocalCall> for Runtime
where
    RuntimeCall: From<LocalCall>,
{
    type RuntimeCall = RuntimeCall;
    // Use your actual UncheckedExtrinsic type, not the trait
    type Extrinsic = UncheckedExtrinsic;
}

impl frame_system::offchain::CreateInherent<pallet_example_offchain_worker::Call<Runtime>> for Runtime {
    fn create_inherent(call: RuntimeCall) -> UncheckedExtrinsic {
        UncheckedExtrinsic::new_bare(call)
    }
}

impl frame_system::offchain::CreateSignedTransaction<pallet_example_offchain_worker::Call<Runtime>> for Runtime
{
    fn create_signed_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
        call: RuntimeCall,
        public: Self::Public,
        account: AccountId,
        nonce: Nonce,
    ) -> Option<UncheckedExtrinsic> {
        // Create a signed transaction for the call
        let period = BlockHashCount::get() as u64;
        let current_block = System::block_number()
            .saturated_into::<u64>()
            .saturating_sub(1);
        let tip = 0;
        let extra: SignedExtra = (
            frame_system::CheckNonZeroSender::<Runtime>::new(),
            frame_system::CheckSpecVersion::<Runtime>::new(),
            frame_system::CheckTxVersion::<Runtime>::new(),
            frame_system::CheckGenesis::<Runtime>::new(),
            frame_system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
            frame_system::CheckNonce::<Runtime>::from(nonce),
            frame_system::CheckWeight::<Runtime>::new(),
            pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
            frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
            frame_system::WeightReclaim::<Runtime>::new(),
        );

        let raw_payload = SignedPayload::new(call, extra)
            .map_err(|_e| {
                // log::warn!("Unable to create signed payload: {:?}", e);
             })
            .ok()?;
        let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
        let address = account;
        let (call, extra, _) = raw_payload.deconstruct();
        
        Some(UncheckedExtrinsic::new_signed(
            call,
            address.into(),
            signature.into(),
            extra,
        ))
    }
}

pub mod crypto {
    use pallet_example_offchain_worker::KEY_TYPE;
    use sp_runtime::{
        app_crypto::{app_crypto, sr25519},
        MultiSignature, MultiSigner,
    };
    
    app_crypto!(sr25519, KEY_TYPE);

    pub struct OffchainAuthId;
    
    // Implementation for MultiSignature setup
    impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for OffchainAuthId {
        type RuntimeAppPublic = Public;
        type GenericSignature = sp_core::sr25519::Signature;
        type GenericPublic = sp_core::sr25519::Public;
    }
}

impl pallet_example_offchain_worker::Config for Runtime {
	type AuthorityId = pallet_example_offchain_worker::crypto::TestAuthId;
	type RuntimeEvent = RuntimeEvent;
	type GracePeriod = ConstU32<5>;
	type UnsignedInterval = ConstU32<128>;
	type UnsignedPriority = UnsignedPriority;
	type MaxPrices = ConstU32<64>;
}


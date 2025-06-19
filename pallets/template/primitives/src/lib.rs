#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode, MaxEncodedLen, DecodeWithMemTracking};
use frame_support::pallet_prelude::RuntimeDebug;
use sp_runtime::{BoundedVec, traits::ConstU32};
use scale_info::TypeInfo;

#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[derive(
    Clone,
    Encode,
    Decode,
    DecodeWithMemTracking,
    Eq,
    PartialEq,
    RuntimeDebug,
    MaxEncodedLen,
    TypeInfo,
)]
pub struct BatchListingInfo<CollectionId, ItemId, Balance, BlockNumber> {
    pub nfts: BoundedVec<(CollectionId, ItemId), ConstU32<10>>,
    pub min_bid: Option<Balance>,
    pub custom_timeout: Option<BlockNumber>,
}

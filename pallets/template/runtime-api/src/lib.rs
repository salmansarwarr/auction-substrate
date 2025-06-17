#![cfg_attr(not(feature = "std"), no_std)]

use codec::Codec;
use sp_std::vec::Vec;
use sp_runtime::scale_info::TypeInfo;

sp_api::decl_runtime_apis! {
    pub trait AuctionApi<CollectionId, ItemId, AccountId, Balance, BlockNumber> where
        CollectionId: Codec,
        ItemId: Codec,
        AccountId: Codec,
        Balance: Codec,
        BlockNumber: Codec,
    {
        /// Get auction information for a specific NFT
        fn get_auction_info(
            collection_id: CollectionId,
            item_id: ItemId,
        ) -> Option<AuctionInfo<AccountId, Balance, BlockNumber>>;

        /// Get all bids for a specific NFT auction
        fn get_bids(
            collection_id: CollectionId,
            item_id: ItemId,
        ) -> Vec<(AccountId, Balance)>;

        /// Check if an NFT is currently in auction
        fn is_in_auction(
            collection_id: CollectionId,
            item_id: ItemId,
        ) -> bool;

        /// Get current fee percentage
        fn get_fee_percentage() -> u8;

        /// Get accumulated fees
        fn get_accumulated_fees() -> Balance;

        /// Get all active auctions
        fn get_active_auctions() -> Vec<((CollectionId, ItemId), AuctionInfo<AccountId, Balance, BlockNumber>)>;
    }
}

/// Auction info structure for runtime API
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[derive(codec::Encode, codec::Decode, Clone, PartialEq, Eq, sp_runtime::RuntimeDebug, TypeInfo)]
pub struct AuctionInfo<AccountId, Balance, BlockNumber> {
    pub owner: AccountId,
    pub start_block: BlockNumber,
    pub highest_bid: Balance,
    pub highest_bidder: Option<AccountId>,
    pub ended: bool,
}
use jsonrpsee::{
    core::{RpcResult},
    proc_macros::rpc,
    types::ErrorObjectOwned
};  
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::{Block as BlockT, Hash}};
use std::sync::Arc;
use codec::Codec;
use sp_rpc::number::NumberOrHex;

pub use pallet_template_runtime_api::AuctionInfo;
pub use pallet_template_runtime_api::AuctionApi as AuctionRuntimeApi;

fn to_rpc_error<E: std::fmt::Display>(e: E) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(1, format!("Unable to query auction info: {}", e), None::<()>)
}

#[rpc(client, server)]
pub trait AuctionApi<BlockHash, CollectionId, ItemId, AccountId, Balance, BlockNumber> {
    /// Get auction information for a specific NFT
    #[method(name = "auction_getAuctionInfo")]
    fn get_auction_info(
        &self,
        collection_id: CollectionId,
        item_id: ItemId,
        at: Option<BlockHash>,
    ) -> RpcResult<Option<AuctionInfo<AccountId, Balance, BlockNumber>>>;

    /// Get all bids for a specific NFT auction
    #[method(name = "auction_getBids")]
    fn get_bids(
        &self,
        collection_id: CollectionId,
        item_id: ItemId,
        at: Option<BlockHash>,
    ) -> RpcResult<Vec<(AccountId, Balance)>>;

    /// Check if an NFT is currently in auction
    #[method(name = "auction_isInAuction")]
    fn is_in_auction(
        &self,
        collection_id: CollectionId,
        item_id: ItemId,
        at: Option<BlockHash>,
    ) -> RpcResult<bool>;

    /// Get current fee percentage
    #[method(name = "auction_getFeePercentage")]
    fn get_fee_percentage(&self, at: Option<BlockHash>) -> RpcResult<u8>;

    /// Get accumulated fees
    #[method(name = "auction_getAccumulatedFees")]
    fn get_accumulated_fees(&self, at: Option<BlockHash>) -> RpcResult<NumberOrHex>;

    /// Get all active auctions
    #[method(name = "auction_getActiveAuctions")]
    fn get_active_auctions(
        &self,
        at: Option<BlockHash>,
    ) -> RpcResult<Vec<((CollectionId, ItemId), AuctionInfo<AccountId, Balance, BlockNumber>)>>;
}

/// A struct that implements the `AuctionApi`.
pub struct AuctionRpc<C, M> {
    /// Shared reference to the client.
    client: Arc<C>,
    /// Shared reference to the block import context.
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> AuctionRpc<C, M> {
    /// Create new `AuctionRpc` instance with the given reference to the client.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, BlockHash, CollectionId, ItemId, AccountId, Balance, BlockNumber>
    AuctionApiServer<
        BlockHash,
        CollectionId,
        ItemId,
        AccountId,
        Balance,
        BlockNumber,
    > for AuctionRpc<C, Block>
where
    Block: BlockT<Hash = BlockHash>,
    AccountId: Clone + std::fmt::Display + Codec,
    Balance: Clone + std::fmt::Display + Codec + Into<NumberOrHex>,
    BlockNumber: Clone + std::fmt::Display + Codec,
    CollectionId: Clone + std::fmt::Display + Codec,
    ItemId: Clone + std::fmt::Display + Codec,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: AuctionRuntimeApi<Block, CollectionId, ItemId, AccountId, Balance, BlockNumber>,
{
    fn get_auction_info(
        &self,
        collection_id: CollectionId,
        item_id: ItemId,
        at: Option<BlockHash>,
    ) -> RpcResult<Option<AuctionInfo<AccountId, Balance, BlockNumber>>> {
        let api = self.client.runtime_api();
        // let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result = api.get_auction_info(at_hash, collection_id, item_id);
        runtime_api_result.map_err(to_rpc_error)
            .map(|info| info.map(|i| i.into()))
    }

    fn get_bids(
        &self,
        collection_id: CollectionId,
        item_id: ItemId,
        at: Option<BlockHash>,
    ) -> RpcResult<Vec<(AccountId, Balance)>> {
        let api = self.client.runtime_api();
        // let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result = api.get_bids(at_hash, collection_id, item_id);
        runtime_api_result.map_err(|e| {
            ErrorObjectOwned::owned(1, format!("Unable to query auction bids: {}", e), None::<()>)
        })
    }

    fn is_in_auction(
        &self,
        collection_id: CollectionId,
        item_id: ItemId,
        at: Option<BlockHash>,
    ) -> RpcResult<bool> {
        let api = self.client.runtime_api();
        // let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result = api.is_in_auction(at_hash, collection_id, item_id);
        runtime_api_result.map_err(|e| {
            ErrorObjectOwned::owned(1, format!("Unable to query auction bids: {}", e), None::<()>)
        })
    }

    fn get_fee_percentage(&self, at: Option<BlockHash>) -> RpcResult<u8> {
        let api = self.client.runtime_api();
        // let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result = api.get_fee_percentage(at_hash);
        runtime_api_result.map_err(|e| {
            ErrorObjectOwned::owned(1, format!("Unable to query auction bids: {}", e), None::<()>)
        })
    }

    fn get_accumulated_fees(&self, at: Option<BlockHash>) -> RpcResult<NumberOrHex> {
        let api = self.client.runtime_api();
        // let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result = api.get_accumulated_fees(at_hash);
        runtime_api_result
            .map(|balance| balance.into())
            .map_err(|e| {
                ErrorObjectOwned::owned(1, format!("Unable to query auction bids: {}", e), None::<()>)
            })
    }

    fn get_active_auctions(
        &self,
        at: Option<BlockHash>,
    ) -> RpcResult<Vec<((CollectionId, ItemId), AuctionInfo<AccountId, Balance, BlockNumber>)>> {
        let api = self.client.runtime_api();
        // let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result = api.get_active_auctions(at_hash);
        runtime_api_result.map_err(|e| {
            ErrorObjectOwned::owned(1, format!("Unable to query auction bids: {}", e), None::<()>)
        })
    }
}
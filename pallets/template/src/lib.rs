#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ReservableCurrency, ExistenceRequirement},
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::Zero;
    use sp_std::prelude::*;

    type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type BalanceOf<T> = <<T as Config>::Currency as Currency<AccountIdOf<T>>>::Balance;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        
        /// The currency mechanism for handling bids
        type Currency: ReservableCurrency<Self::AccountId>;
        
        /// Asset identifier type
        type AssetId: Member + Parameter + MaxEncodedLen + Copy + PartialEq;
        
        /// The maximum number of bids per auction
        #[pallet::constant]
        type MaxBidsPerAuction: Get<u32>;
        
        /// Number of blocks after which the auction auto-resolves
        #[pallet::constant]
        type AuctionTimeoutBlocks: Get<BlockNumberFor<Self>>;
    }

    /// Asset information
    #[pallet::storage]
    #[pallet::getter(fn assets)]
    pub type Assets<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AssetId,
        AssetInfo<T::AccountId>,
        OptionQuery,
    >;

    /// Auctions information
    #[pallet::storage]
    #[pallet::getter(fn auctions)]
    pub type Auctions<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AssetId,
        AuctionInfo<T::AccountId, BalanceOf<T>, BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// Mapping from asset to bidders and their bids, ordered by bid amount
    #[pallet::storage]
    #[pallet::getter(fn bids)]
    pub type Bids<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AssetId,
        BoundedVec<(T::AccountId, BalanceOf<T>), T::MaxBidsPerAuction>,
        ValueQuery,
    >;

    /// Structure for asset information
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
    pub struct AssetInfo<AccountId> {
        /// The owner of the asset
        pub owner: AccountId,
        /// Whether the asset has been sold
        pub is_bought: bool,
        /// The buyer of the asset, if any
        pub buyer: Option<AccountId>,
    }

    /// Structure for auction information
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
    pub struct AuctionInfo<AccountId, Balance, BlockNumber> {
        /// The owner of the auction
        pub owner: AccountId,
        /// The block number when the auction started
        pub start_block: BlockNumber,
        /// The highest bid amount
        pub highest_bid: Balance,
        /// The highest bidder
        pub highest_bidder: Option<AccountId>,
        /// Whether the auction has ended
        pub ended: bool,
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// An asset was listed for auction. [asset_id, owner]
        AssetListed(T::AssetId, T::AccountId),
        /// A bid was placed. [asset_id, bidder, bid_amount]
        BidPlaced(T::AssetId, T::AccountId, BalanceOf<T>),
        /// An auction was resolved with a winner. [asset_id, winner, bid_amount]
        AuctionResolved(T::AssetId, T::AccountId, BalanceOf<T>),
        /// An auction failed to find a valid buyer. [asset_id]
        AuctionFailed(T::AssetId),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Asset already exists
        AssetAlreadyExists,
        /// Asset does not exist
        AssetNotFound,
        /// Not the asset owner
        NotAssetOwner,
        /// Auction already exists
        AuctionAlreadyExists,
        /// Auction does not exist
        AuctionNotFound,
        /// Auction has ended
        AuctionEnded,
        /// Bid is too low
        BidTooLow,
        /// Cannot bid on own auction
        CannotBidOnOwnAuction,
        /// Too many bids
        TooManyBids,
        /// Cannot find a valid buyer with sufficient funds
        NoValidBuyer,
        /// Asset is already sold
        AssetAlreadySold,
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            let mut weight = Weight::zero();

            // Check for auctions that need to be auto-resolved
            let mut auctions_to_resolve = Vec::new();

            for (asset_id, auction_info) in Auctions::<T>::iter() {
                if !auction_info.ended && 
                   now >= auction_info.start_block + T::AuctionTimeoutBlocks::get() {
                    auctions_to_resolve.push(asset_id);
                }
                weight = weight.saturating_add(T::DbWeight::get().reads(1));
            }

            // Resolve expired auctions
            for asset_id in auctions_to_resolve {
                let _ = Self::auto_resolve_auction(&asset_id);
                weight = weight.saturating_add(T::DbWeight::get().reads_writes(3, 2));
            }

            weight
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// List an asset for auction
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(2, 2))]
        pub fn list_asset(
            origin: OriginFor<T>,
            asset_id: T::AssetId,
        ) -> DispatchResult {
            let owner = ensure_signed(origin)?;

            // Ensure asset doesn't exist already
            ensure!(!Assets::<T>::contains_key(&asset_id), Error::<T>::AssetAlreadyExists);
            // Ensure auction doesn't exist
            ensure!(!Auctions::<T>::contains_key(&asset_id), Error::<T>::AuctionAlreadyExists);

            // Create and store asset info
            let asset_info = AssetInfo {
                owner: owner.clone(),
                is_bought: false,
                buyer: None,
            };
            Assets::<T>::insert(&asset_id, asset_info);

            // Create and store auction info
            let auction_info = AuctionInfo {
                owner: owner.clone(),
                start_block: <frame_system::Pallet<T>>::block_number(),
                highest_bid: Zero::zero(),
                highest_bidder: None,
                ended: false,
            };
            Auctions::<T>::insert(&asset_id, auction_info);

            // Emit event
            Self::deposit_event(Event::AssetListed(asset_id, owner));

            Ok(())
        }

        /// Place a bid on an asset
        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().reads_writes(4, 2))]
        pub fn place_bid(
            origin: OriginFor<T>,
            asset_id: T::AssetId,
            bid_amount: BalanceOf<T>,
        ) -> DispatchResult {
            let bidder = ensure_signed(origin)?;

            // Check if asset exists
            let asset_info = Assets::<T>::get(&asset_id).ok_or(Error::<T>::AssetNotFound)?;
            
            // Check if asset is not already sold
            ensure!(!asset_info.is_bought, Error::<T>::AssetAlreadySold);

            // Check if auction exists and is active
            let auction_info = Auctions::<T>::get(&asset_id).ok_or(Error::<T>::AuctionNotFound)?;
            ensure!(!auction_info.ended, Error::<T>::AuctionEnded);

            // Ensure bidder is not the owner
            ensure!(bidder != auction_info.owner, Error::<T>::CannotBidOnOwnAuction);

            // Ensure bid is higher than current highest bid
            ensure!(bid_amount > auction_info.highest_bid, Error::<T>::BidTooLow);

            // Check if bidder has enough funds and reserve them
            T::Currency::reserve(&bidder, bid_amount)?;

            // If there's a previous highest bidder, unreserve their funds
            if let Some(highest_bidder) = auction_info.highest_bidder {
                if highest_bidder != bidder {
                    let _ = T::Currency::unreserve(&highest_bidder, auction_info.highest_bid);
                } else {
                    // If same bidder is increasing their bid, unreserve previous amount
                    let _ = T::Currency::unreserve(&bidder, auction_info.highest_bid);
                }
            }

            // Update auction with new highest bid
            let new_auction_info = AuctionInfo {
                highest_bid: bid_amount,
                highest_bidder: Some(bidder.clone()),
                ..auction_info
            };
            Auctions::<T>::insert(&asset_id, new_auction_info);

            // Update bids collection
            let mut bids = Bids::<T>::get(&asset_id);
            
            // Remove previous bid by this bidder if exists
            bids.retain(|(b, _)| b != &bidder);
            
            // Add new bid, ensuring it's sorted (highest first)
            let new_bid = (bidder.clone(), bid_amount);
            match bids.binary_search_by(|(_, b)| b.cmp(&bid_amount).reverse()) {
                Ok(pos) | Err(pos) => {
                    if bids.len() == T::MaxBidsPerAuction::get() as usize && pos >= bids.len() {
                        // New bid is too low to be included in max bids
                        return Err(Error::<T>::BidTooLow.into());
                    }
                    
                    if bids.len() == T::MaxBidsPerAuction::get() as usize {
                        // Remove lowest bid if at capacity
                        bids.pop();
                    }
                    
                    if let Err(_e) = bids.try_insert(pos, new_bid) {
                        return Err(Error::<T>::TooManyBids.into());
                    }
                }
            }
            Bids::<T>::insert(&asset_id, bids);

            // Emit event
            Self::deposit_event(Event::BidPlaced(asset_id, bidder, bid_amount));

            Ok(())
        }

        /// Owner chooses a buyer for the auction
        #[pallet::call_index(2)]
        #[pallet::weight(T::DbWeight::get().reads_writes(4, 3))]
        pub fn choose_buyer(
            origin: OriginFor<T>,
            asset_id: T::AssetId,
            buyer: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Check if asset exists
            let asset_info = Assets::<T>::get(&asset_id).ok_or(Error::<T>::AssetNotFound)?;
            
            // Check if asset is not already sold
            ensure!(!asset_info.is_bought, Error::<T>::AssetAlreadySold);
            
            // Check if auction exists
            let auction_info = Auctions::<T>::get(&asset_id).ok_or(Error::<T>::AuctionNotFound)?;
            
            // Check if auction is still active
            ensure!(!auction_info.ended, Error::<T>::AuctionEnded);
            
            // Ensure caller is the asset owner
            ensure!(who == asset_info.owner, Error::<T>::NotAssetOwner);

            // Find the chosen buyer's bid
            let bids = Bids::<T>::get(&asset_id);
            let buyer_bid = bids.iter()
                .find(|(bidder, _)| bidder == &buyer)
                .ok_or(Error::<T>::NoValidBuyer)?;

            // Try to transfer funds from buyer to owner
            Self::finalize_auction(&asset_id, buyer.clone(), buyer_bid.1)?;

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Auto-resolve auction after timeout
        fn auto_resolve_auction(asset_id: &T::AssetId) -> DispatchResult {
            // Get auction info
            let mut auction_info = Auctions::<T>::get(asset_id).ok_or(Error::<T>::AuctionNotFound)?;
            
            // Check if auction is still active
            if auction_info.ended {
                return Err(Error::<T>::AuctionEnded.into());
            }

            // Mark auction as ended
            auction_info.ended = true;
            Auctions::<T>::insert(asset_id, &auction_info);

            // Try to finalize auction with the highest bidder
            if let Some(highest_bidder) = &auction_info.highest_bidder {
                // Try to transfer funds
                if Self::finalize_auction(asset_id, highest_bidder.clone(), auction_info.highest_bid).is_err() {
                    // If transfer fails, try next highest bidders
                    let bids = Bids::<T>::get(asset_id);
                    for (bidder, bid_amount) in bids.iter() {
                        if bidder != highest_bidder && 
                           Self::finalize_auction(asset_id, bidder.clone(), *bid_amount).is_ok() {
                            return Ok(());
                        }
                    }
                    // If all transfers fail, emit auction failed event
                    Self::deposit_event(Event::AuctionFailed(*asset_id));
                }
            } else {
                // No bids, auction failed
                Self::deposit_event(Event::AuctionFailed(*asset_id));
            }

            Ok(())
        }

        /// Finalize auction by transferring funds and updating asset status
        fn finalize_auction(
            asset_id: &T::AssetId,
            buyer: T::AccountId,
            bid_amount: BalanceOf<T>,
        ) -> DispatchResult {
            // Get asset info
            let mut asset_info = Assets::<T>::get(asset_id).ok_or(Error::<T>::AssetNotFound)?;
            
            // Check if asset is not already sold
            ensure!(!asset_info.is_bought, Error::<T>::AssetAlreadySold);

            // Unreserve and transfer funds from buyer to owner
            let _ = T::Currency::unreserve(&buyer, bid_amount);
            T::Currency::transfer(
                &buyer,
                &asset_info.owner,
                bid_amount,
                ExistenceRequirement::KeepAlive,
            )?;

            // Update asset info
            asset_info.is_bought = true;
            asset_info.buyer = Some(buyer.clone());
            Assets::<T>::insert(asset_id, &asset_info);

            // Mark auction as ended
            if let Some(mut auction_info) = Auctions::<T>::get(asset_id) {
                auction_info.ended = true;
                Auctions::<T>::insert(asset_id, &auction_info);
            }

            // Unreserve funds for all other bidders
            let bids = Bids::<T>::get(asset_id);
            for (bidder, bid_amount) in bids.iter() {
                if bidder != &buyer {
                    let _ = T::Currency::unreserve(bidder, *bid_amount);
                }
            }

            // Emit event
            Self::deposit_event(Event::AuctionResolved(*asset_id, buyer, bid_amount));

            Ok(())
        }
    }
}
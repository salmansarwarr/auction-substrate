#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
pub mod mock;

#[cfg(test)]
pub mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

pub mod migrations;

pub mod weights;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use crate::migrations;
    use crate::weights::WeightInfo;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, ReservableCurrency, WithdrawReasons},
        transactional, PalletId,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::AccountIdConversion;
    use sp_runtime::traits::{CheckedDiv, Zero};
    use sp_runtime::Saturating;
    use sp_std::prelude::*;

    type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type BalanceOf<T> = <<T as Config>::Currency as Currency<AccountIdOf<T>>>::Balance;

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_uniques::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The currency mechanism for handling bids
        type Currency: ReservableCurrency<Self::AccountId>;

        /// The maximum number of bids per auction
        #[pallet::constant]
        type MaxBidsPerAuction: Get<u32>;

        /// Number of blocks after which the auction auto-resolves
        #[pallet::constant]
        type AuctionTimeoutBlocks: Get<BlockNumberFor<Self>>;

        /// Royalty percentage for original creators (0-100)
        #[pallet::constant]
        type RoyaltyPercentage: Get<u8>;

        type PalletId: Get<PalletId>;

        type WeightInfo: WeightInfo;
    }

    /// Auctions information
    #[pallet::storage]
    #[pallet::getter(fn auctions)]
    pub type Auctions<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        (T::CollectionId, T::ItemId),
        AuctionInfo<T::AccountId, BalanceOf<T>, BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// Mapping from NFT to bidders and their bids, ordered by bid amount
    #[pallet::storage]
    #[pallet::getter(fn bids)]
    pub type Bids<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        (T::CollectionId, T::ItemId),
        BoundedVec<(T::AccountId, BalanceOf<T>), T::MaxBidsPerAuction>,
        ValueQuery,
    >;

    /// Tracks whether an NFT is currently in an auction
    #[pallet::storage]
    #[pallet::getter(fn is_in_auction)]
    pub type InAuction<T: Config> =
        StorageMap<_, Blake2_128Concat, (T::CollectionId, T::ItemId), bool, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn fee_percentage)]
    pub(super) type FeePercentage<T> = StorageValue<_, u8, ValueQuery>; // e.g., 5 for 5%

    #[pallet::storage]
    #[pallet::getter(fn accumulated_fees)]
    pub(super) type AccumulatedFees<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

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
        /// An NFT was listed for auction. [collection_id, item_id, owner]
        NftListed(T::CollectionId, T::ItemId, T::AccountId),
        /// A bid was placed. [collection_id, item_id, bidder, bid_amount]
        BidPlaced(T::CollectionId, T::ItemId, T::AccountId, BalanceOf<T>),
        /// An auction was resolved with a winner. [collection_id, item_id, winner, bid_amount]
        AuctionResolved(T::CollectionId, T::ItemId, T::AccountId, BalanceOf<T>),
        /// An auction failed to find a valid buyer. [collection_id, item_id]
        AuctionFailed(T::CollectionId, T::ItemId),
        FeePercentageSet(u8),
        FeesWithdrawn(T::AccountId, BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// NFT is already in an auction
        NftAlreadyInAuction,
        /// Auction does not exist
        AuctionNotFound,
        /// Not the NFT owner
        NotNftOwner,
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
        /// Collection or item does not exist
        NftNotFound,
        InvalidFee,
        NoFeesAvailable,
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
    #[pallet::storage_version(migrations::STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            let mut weight = Weight::zero();

            // Check for auctions that need to be auto-resolved
            let mut auctions_to_resolve = Vec::new();

            for ((collection_id, item_id), auction_info) in Auctions::<T>::iter() {
                if !auction_info.ended
                    && now >= auction_info.start_block + T::AuctionTimeoutBlocks::get()
                {
                    auctions_to_resolve.push((collection_id, item_id));
                }
                weight = weight.saturating_add(T::DbWeight::get().reads(1));
            }

            // Resolve expired auctions
            for (collection_id, item_id) in auctions_to_resolve {
                let _ = Self::auto_resolve_auction(&collection_id, &item_id);
                weight = weight.saturating_add(T::DbWeight::get().reads_writes(3, 2));
            }

            weight
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // List an NFT for auction
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(3, 2))]
        pub fn list_nft_for_auction(
            origin: OriginFor<T>,
            collection_id: T::CollectionId,
            item_id: T::ItemId,
        ) -> DispatchResult {
            let owner = ensure_signed(origin)?;

            // Ensure collection and item exist
            ensure!(
                pallet_uniques::Pallet::<T>::owner(collection_id.clone(), item_id.clone())
                    .is_some(),
                Error::<T>::NftNotFound
            );

            // Ensure caller is the NFT owner
            let nft_owner =
                pallet_uniques::Pallet::<T>::owner(collection_id.clone(), item_id.clone())
                    .ok_or(Error::<T>::NftNotFound)?;
            ensure!(owner == nft_owner, Error::<T>::NotNftOwner);

            // Ensure NFT is not already in an auction
            ensure!(
                !InAuction::<T>::get((collection_id.clone(), item_id.clone())),
                Error::<T>::NftAlreadyInAuction
            );

            // Freeze the Nft
            pallet_uniques::Pallet::<T>::freeze(
                frame_system::RawOrigin::Signed(owner.clone()).into(),
                collection_id.clone(),
                item_id.clone(),
            )?;

            // Create auction info
            let auction_info = AuctionInfo {
                owner: owner.clone(),
                start_block: <frame_system::Pallet<T>>::block_number(),
                highest_bid: Zero::zero(),
                highest_bidder: None,
                ended: false,
            };
            Auctions::<T>::insert((collection_id.clone(), item_id.clone()), auction_info);

            // Mark NFT as in auction
            InAuction::<T>::insert((collection_id.clone(), item_id.clone()), true);

            // Emit event
            Self::deposit_event(Event::NftListed(collection_id, item_id, owner));

            Ok(())
        }

        // Place a bid on an NFT
        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().reads_writes(4, 2))]
        pub fn place_bid(
            origin: OriginFor<T>,
            collection_id: T::CollectionId,
            item_id: T::ItemId,
            bid_amount: BalanceOf<T>,
        ) -> DispatchResult {
            let bidder = ensure_signed(origin)?;

            // Ensure auction exists and is active
            let auction_info = Auctions::<T>::get((collection_id.clone(), item_id.clone()))
                .ok_or(Error::<T>::AuctionNotFound)?;
            ensure!(!auction_info.ended, Error::<T>::AuctionEnded);

            // Ensure bidder is not the auction owner
            ensure!(
                bidder != auction_info.owner,
                Error::<T>::CannotBidOnOwnAuction
            );

            // Ensure bid is higher than current highest bid
            ensure!(bid_amount > auction_info.highest_bid, Error::<T>::BidTooLow);

            // Check if bidder has enough funds and reserve them
            <T as Config>::Currency::reserve(&bidder, bid_amount)?;

            // If there's a previous highest bidder, unreserve their funds
            if let Some(highest_bidder) = auction_info.highest_bidder {
                if highest_bidder != bidder {
                    let _ = <T as Config>::Currency::unreserve(
                        &highest_bidder,
                        auction_info.highest_bid,
                    );
                } else {
                    // If same bidder is increasing their bid, unreserve previous amount
                    let _ = <T as Config>::Currency::unreserve(&bidder, auction_info.highest_bid);
                }
            }

            // Update auction with new highest bid
            let new_auction_info = AuctionInfo {
                highest_bid: bid_amount,
                highest_bidder: Some(bidder.clone()),
                ..auction_info
            };
            Auctions::<T>::insert((collection_id.clone(), item_id.clone()), new_auction_info);

            // Update bids collection
            let mut bids = Bids::<T>::get((collection_id.clone(), item_id.clone()));

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
            Bids::<T>::insert((collection_id.clone(), item_id.clone()), bids);

            // Emit event
            Self::deposit_event(Event::BidPlaced(collection_id, item_id, bidder, bid_amount));

            Ok(())
        }

        // Resolve auction by choosing a buyer
        #[pallet::call_index(2)]
        #[pallet::weight(T::DbWeight::get().reads_writes(4, 3))]
        pub fn resolve_auction(
            origin: OriginFor<T>,
            collection_id: T::CollectionId,
            item_id: T::ItemId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Get auction info
            let auction_info = Auctions::<T>::get((collection_id.clone(), item_id.clone()))
                .ok_or(Error::<T>::AuctionNotFound)?;

            // Check if auction is still active
            ensure!(!auction_info.ended, Error::<T>::AuctionEnded);

            // Ensure caller is the auction owner
            ensure!(who == auction_info.owner, Error::<T>::NotNftOwner);

            // Require at least one bid
            let highest_bidder = auction_info
                .highest_bidder
                .ok_or(Error::<T>::NoValidBuyer)?;

            // Finalize the auction
            Self::finalize_auction(
                &collection_id,
                &item_id,
                &highest_bidder,
                auction_info.highest_bid,
            )?;

            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(10_000)]
        pub fn set_fee_percentage(origin: OriginFor<T>, fee: u8) -> DispatchResult {
            ensure_root(origin)?; // Only Sudo (Root) can call
            ensure!(fee <= 100, Error::<T>::InvalidFee);
            FeePercentage::<T>::put(fee);
            Self::deposit_event(Event::FeePercentageSet(fee));
            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(10_000)]
        pub fn withdraw_fees(origin: OriginFor<T>, to: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;

            let total_fees = AccumulatedFees::<T>::take();
            if total_fees.is_zero() {
                Err(Error::<T>::NoFeesAvailable)?
            }

            log::info!(
                "Pallet account balance: {:?}",
                <T as Config>::Currency::free_balance(&Self::account_id())
            );

            <T as Config>::Currency::transfer(
                &Self::account_id(),
                &to,
                total_fees,
                ExistenceRequirement::AllowDeath,
            )?;
            Self::deposit_event(Event::FeesWithdrawn(to, total_fees));
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn account_id() -> T::AccountId {
            T::PalletId::get().into_account_truncating()
        }

        // Auto-resolve auction after timeout
        fn auto_resolve_auction(
            collection_id: &T::CollectionId,
            item_id: &T::ItemId,
        ) -> DispatchResult {
            // Get auction info
            let mut auction_info =
                Auctions::<T>::get((collection_id, item_id)).ok_or(Error::<T>::AuctionNotFound)?;

            // Check if auction is still active
            if auction_info.ended {
                return Err(Error::<T>::AuctionEnded.into());
            }

            // Try to finalize auction with the highest bidder
            if let Some(highest_bidder) = &auction_info.highest_bidder {
                // Try to transfer NFT and funds
                if Self::finalize_auction(
                    collection_id,
                    item_id,
                    highest_bidder,
                    auction_info.highest_bid,
                )
                .is_err()
                {
                    // If transfer fails, try next highest bidders
                    let bids = Bids::<T>::get((collection_id, item_id));
                    for (bidder, bid_amount) in bids.iter() {
                        if bidder != highest_bidder
                            && Self::finalize_auction(collection_id, item_id, bidder, *bid_amount)
                                .is_ok()
                        {
                            return Ok(());
                        }
                    }
                    // If all transfers fail, emit auction failed event
                    auction_info.ended = true;
                    Auctions::<T>::insert((collection_id, item_id), &auction_info);
                    Self::deposit_event(Event::AuctionFailed(collection_id.clone(), *item_id));
                }
            } else {
                // No bids, auction failed
                auction_info.ended = true;
                Auctions::<T>::insert((collection_id, item_id), &auction_info);
                Self::deposit_event(Event::AuctionFailed(collection_id.clone(), *item_id));
            }

            Ok(())
        }

        // Finalize auction by transferring NFT and handling funds
        #[transactional]
        fn finalize_auction(
            collection_id: &T::CollectionId,
            item_id: &T::ItemId,
            buyer: &T::AccountId,
            bid_amount: BalanceOf<T>,
        ) -> DispatchResult {
            // Retrieve auction information
            let auction_info =
                Auctions::<T>::get((collection_id, item_id)).ok_or(Error::<T>::AuctionNotFound)?;

            // Ensure auction hasn't already ended
            ensure!(!auction_info.ended, Error::<T>::AuctionEnded);

            // Verify current NFT ownership
            let current_owner =
                pallet_uniques::Pallet::<T>::owner(collection_id.clone(), item_id.clone())
                    .ok_or(Error::<T>::NftNotFound)?;
            ensure!(current_owner == auction_info.owner, Error::<T>::NotNftOwner);

            // Validate buyer's funds
            ensure!(
                <T as Config>::Currency::can_slash(buyer, bid_amount),
                Error::<T>::NoValidBuyer
            );

            // Calculate royalty (if applicable)
            let royalty_percentage = T::RoyaltyPercentage::get();
            let royalty_amount = bid_amount
                .checked_mul(&BalanceOf::<T>::from(royalty_percentage as u32))
                .and_then(|royalty| royalty.checked_div(&BalanceOf::<T>::from(100u32)))
                .unwrap_or_else(|| Zero::zero());

            // Calculate seller's amount (total bid minus royalty)
            let _seller_amount = bid_amount.saturating_sub(royalty_amount);

            // Perform atomic transactions
            let _ = <T as Config>::Currency::unreserve(buyer, bid_amount.into());

            // 1. Transfer funds from buyer
            let _ = <T as Config>::Currency::withdraw(
                buyer,
                bid_amount,
                WithdrawReasons::TRANSFER,
                ExistenceRequirement::KeepAlive,
            )?;

            // 2. Pay royalty to collection creator (if applicable)
            if royalty_amount > Zero::zero() {
                if let Some(collection_admin) =
                    pallet_uniques::Pallet::<T>::collection_owner(collection_id.clone())
                {
                    let _ = <T as Config>::Currency::deposit_creating(
                        &collection_admin,
                        royalty_amount,
                    );
                }
            }

            let fee_percent = FeePercentage::<T>::get(); // e.g., 5
            let fee_amount = bid_amount * fee_percent.into() / 100u32.into();
            let payout = bid_amount.saturating_sub(fee_amount);

            // 3. Pay remaining funds to auction owner
            let _ = <T as Config>::Currency::deposit_creating(&auction_info.owner, payout);

            // Transfer fees to pallet account
            let _ = <T as Config>::Currency::deposit_creating(&Self::account_id(), fee_amount);

            // Add fee to pallet storage
            AccumulatedFees::<T>::mutate(|f| *f += fee_amount);

            // 4. Unfreeze the NFT before transferring
            pallet_uniques::Pallet::<T>::thaw(
                frame_system::RawOrigin::Signed(auction_info.owner.clone()).into(),
                collection_id.clone(),
                *item_id,
            )?;

            // 5. Transfer NFT to the buyer
            pallet_uniques::Pallet::<T>::do_transfer(
                collection_id.clone(),
                *item_id,
                buyer.clone(),
                |_, _| Ok(()),
            )?;

            // Update auction status
            Auctions::<T>::mutate((collection_id, item_id), |auction| {
                if let Some(auction_info) = auction {
                    auction_info.ended = true;
                    auction_info.highest_bidder = Some(buyer.clone());
                }
            });

            // Remove from in-auction tracking
            InAuction::<T>::remove((collection_id, item_id));

            // Clear bids
            Bids::<T>::remove((collection_id, item_id));

            // Emit auction resolved event
            Self::deposit_event(Event::AuctionResolved(
                collection_id.clone(),
                *item_id,
                buyer.clone(),
                bid_amount,
            ));

            Ok(())
        }
    }
}

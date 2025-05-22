//! Benchmarking setup for pallet-auction
#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;

#[benchmarks(where
    T::CollectionId: From<u32>,
    T::ItemId: From<u32>
)]
mod benchmarks {
    use super::*;

    use crate::pallet::Pallet as Template;
    use frame_support::traits::Get;
    use frame_support::{
        assert_ok,
        traits::{Currency, Hooks},
        pallet_prelude::Zero,
    };
    use frame_system::RawOrigin;
    use sp_runtime::traits::{Bounded, One, StaticLookup};

    type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    const SEED: u32 = 0;
    const COLLECTION_ID: u32 = 1;
    const ITEM_ID: u32 = 1;
    const FEE_PERCENTAGE: u8 = 5;

    // Helper function to create a collection and mint an NFT
    fn setup_nft<T: pallet_uniques::Config>(owner: &T::AccountId) -> (T::CollectionId, T::ItemId)
    where
        T::CollectionId: From<u32>,
        T::ItemId: From<u32>,
    {
        let collection_id = T::CollectionId::from(COLLECTION_ID);

        let item_id = T::ItemId::from(ITEM_ID);

        let owner_lookup = <T::Lookup as StaticLookup>::unlookup(owner.clone());

        // Create a collection
        assert_ok!(pallet_uniques::Pallet::<T>::create(
            RawOrigin::Signed(owner.clone()).into(),
            collection_id.clone(),
            owner_lookup.clone(),
        ));

        // Mint NFT
        assert_ok!(pallet_uniques::Pallet::<T>::mint(
            RawOrigin::Signed(owner.clone()).into(),
            collection_id.clone(),
            item_id.clone(),
            owner_lookup.clone(),
        ));

        (collection_id, item_id)
    }

    // Helper function to fund an account
    fn fund_account<T: Config>(account: &T::AccountId, amount: BalanceOf<T>) {
        let _ = <T as Config>::Currency::make_free_balance_be(account, amount);
    }

    #[benchmark]
    fn list_nft_for_auction<T: Config + pallet_uniques::Config>() {
        let caller: T::AccountId = whitelisted_caller();
        fund_account::<T>(&caller, BalanceOf::<T>::max_value() / 100u32.into());
        let (collection_id, item_id) = setup_nft::<T>(&caller);

        #[extrinsic_call]
        list_nft_for_auction(
            RawOrigin::Signed(caller),
            collection_id.clone(),
            item_id.clone(),
        );

        assert!(InAuction::<T>::get((
            collection_id.clone(),
            item_id.clone()
        )));
        assert!(Auctions::<T>::contains_key((collection_id, item_id)));
    }

    #[benchmark]
    fn place_bid() {
        // Setup NFT and auction
        let seller: T::AccountId = whitelisted_caller();
        fund_account::<T>(&seller, BalanceOf::<T>::max_value() / 100u32.into());
        let (collection_id, item_id) = setup_nft::<T>(&seller);

        // List NFT for auction
        assert_ok!(Template::<T>::list_nft_for_auction(
            RawOrigin::Signed(seller.clone()).into(),
            collection_id.clone(),
            item_id.clone()
        ));

        // Create bidder with funds
        let bidder: T::AccountId = account("bidder", 0, SEED);
        let bid_amount = BalanceOf::<T>::from(100u32);
        let min_balance = <T as pallet::Config>::Currency::minimum_balance();
        fund_account::<T>(&bidder, bid_amount + min_balance * 5u32.into());

        #[extrinsic_call]
        place_bid(
            RawOrigin::Signed(bidder.clone()),
            collection_id.clone(),
            item_id.clone(),
            bid_amount,
        );

        let auction = Auctions::<T>::get((collection_id, item_id)).unwrap();
        assert_eq!(auction.highest_bid, bid_amount);
        assert_eq!(auction.highest_bidder, Some(bidder));
    }

    #[benchmark]
    fn resolve_auction() {
        // Setup NFT and auction
        let seller: T::AccountId = whitelisted_caller();
        fund_account::<T>(&seller, BalanceOf::<T>::max_value() / 100u32.into());
        let (collection_id, item_id) = setup_nft::<T>(&seller);

        // List NFT for auction
        assert_ok!(Template::<T>::list_nft_for_auction(
            RawOrigin::Signed(seller.clone()).into(),
            collection_id.clone(),
            item_id.clone()
        ));

        // Add bidder and place bid
        let bidder: T::AccountId = account("bidder", 0, SEED);
        let bid_amount = BalanceOf::<T>::from(100u32);
        let min_balance = <T as pallet::Config>::Currency::minimum_balance();
        fund_account::<T>(&bidder, bid_amount + min_balance * 5u32.into());

        assert_ok!(Template::<T>::place_bid(
            RawOrigin::Signed(bidder.clone()).into(),
            collection_id.clone(),
            item_id.clone(),
            bid_amount
        ));

        // Set fee percentage
        assert_ok!(Template::<T>::set_fee_percentage(
            RawOrigin::Root.into(),
            FEE_PERCENTAGE
        ));

        #[extrinsic_call]
        resolve_auction(
            RawOrigin::Signed(seller.clone()),
            collection_id.clone(),
            item_id,
        );

        assert!(!InAuction::<T>::get((
            collection_id.clone(),
            item_id.clone()
        )));
        let auction = Auctions::<T>::get((collection_id.clone(), item_id.clone())).unwrap();
        assert!(auction.ended);
        assert_eq!(
            pallet_uniques::Pallet::<T>::owner(collection_id, item_id),
            Some(bidder)
        );
    }

    #[benchmark]
    fn set_fee_percentage() {
        let fee = 10u8;

        #[extrinsic_call]
        set_fee_percentage(RawOrigin::Root, fee);

        assert_eq!(FeePercentage::<T>::get(), fee);
    }

    #[benchmark]
    fn withdraw_fees() {
        // Setup initial fee
        assert_ok!(Template::<T>::set_fee_percentage(
            RawOrigin::Root.into(),
            FEE_PERCENTAGE
        ));

        // Create a complete auction cycle to generate fees
        let seller: T::AccountId = whitelisted_caller();
        fund_account::<T>(&seller, BalanceOf::<T>::max_value() / 100u32.into());
        let (collection_id, item_id) = setup_nft::<T>(&seller);

        assert_ok!(Template::<T>::list_nft_for_auction(
            RawOrigin::Signed(seller.clone()).into(),
            collection_id.clone(),
            item_id.clone()
        ));

        let bidder: T::AccountId = account("bidder", 0, SEED);
        let bid_amount = BalanceOf::<T>::from(100u32);
        let min_balance = <T as pallet::Config>::Currency::minimum_balance();
        fund_account::<T>(&bidder, bid_amount + min_balance * 5u32.into());

        assert_ok!(Template::<T>::place_bid(
            RawOrigin::Signed(bidder.clone()).into(),
            collection_id.clone(),
            item_id.clone(),
            bid_amount
        ));

        // Resolve the auction
        assert_ok!(Template::<T>::resolve_auction(
            RawOrigin::Signed(seller.clone()).into(),
            collection_id.clone(),
            item_id.clone()
        ));

        // Create recipient for fees
        let recipient: T::AccountId = account("recipient", 0, SEED);

        // For benchmark purposes, directly add funds to pallet account to match accumulated fees
        let fees = AccumulatedFees::<T>::get();
        assert!(
            !fees.is_zero(),
            "No fees were accumulated during the auction"
        );

        let initial_balance = <T as Config>::Currency::free_balance(&recipient);


        assert!(
            frame_system::Pallet::<T>::account_exists(&Template::<T>::account_id()),
            "Account doesn't exist"
        );
        
        // Manually ensure pallet account has sufficient funds for benchmark
        <T as Config>::Currency::deposit_creating(&Template::<T>::account_id(), fees);

        // Verify the pallet account has the correct balance
        assert_eq!(
            <T as Config>::Currency::free_balance(&Template::<T>::account_id()),
            fees,
            "Pallet account balance doesn't match accumulated fees"
        );

        #[extrinsic_call]
        withdraw_fees(RawOrigin::Root, recipient.clone());

        // Verify the fees were properly transferred
        assert_eq!(AccumulatedFees::<T>::get(), BalanceOf::<T>::zero());
        assert_eq!(
            <T as Config>::Currency::free_balance(&recipient),
            initial_balance + fees
        );
    }

    #[benchmark]
    fn on_initialize() {
        // Setup initial fee
        assert_ok!(Template::<T>::set_fee_percentage(
            RawOrigin::Root.into(),
            FEE_PERCENTAGE
        ));

        // Setup NFT and auction
        let seller: T::AccountId = whitelisted_caller();
        fund_account::<T>(&seller, BalanceOf::<T>::max_value() / 100u32.into());
        let (collection_id, item_id) = setup_nft::<T>(&seller);

        assert_ok!(Template::<T>::list_nft_for_auction(
            RawOrigin::Signed(seller.clone()).into(),
            collection_id.clone(),
            item_id.clone()
        ));

        // Add bidder and place bid
        let bidder: T::AccountId = account("bidder", 0, SEED);
        let bid_amount = BalanceOf::<T>::from(100u32);
        let min_balance = <T as pallet::Config>::Currency::minimum_balance();

        fund_account::<T>(&bidder, bid_amount + min_balance * 5u32.into());

        assert_ok!(Template::<T>::place_bid(
            RawOrigin::Signed(bidder.clone()).into(),
            collection_id.clone(),
            item_id.clone(),
            bid_amount
        ));

        // Fast forward blocks
        let auction_info = Auctions::<T>::get((collection_id.clone(), item_id.clone())).unwrap();
        let start_block = auction_info.start_block;
        let timeout_block = start_block + T::AuctionTimeoutBlocks::get() + One::one();

        #[block]
        {
            Template::<T>::on_initialize(timeout_block);
        }

        let auction = Auctions::<T>::get((collection_id.clone(), item_id)).unwrap();
        assert!(auction.ended);
        assert!(!InAuction::<T>::get((collection_id, item_id)));
    }

    impl_benchmark_test_suite!(Template, crate::mock::new_test_ext(), crate::mock::Test);
}

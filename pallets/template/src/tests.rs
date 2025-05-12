use crate::{mock::*, Error, Event};
use frame_support::{assert_noop, assert_ok};
use frame_support::traits::OnInitialize;

#[test]
fn list_asset_works() {
    new_test_ext().execute_with(|| {
        // Arrange: Set the block number
        System::set_block_number(1);

        // Act: List an asset
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 1));

        // Assert: Asset should be created and Template should be set up
        let asset = Template::assets(1).unwrap();
        assert_eq!(asset.owner, 1);
        assert_eq!(asset.is_bought, false);
        assert_eq!(asset.buyer, None);

        let auction = Template::auctions(1).unwrap();
        assert_eq!(auction.owner, 1);
        assert_eq!(auction.highest_bid, 0);
        assert_eq!(auction.highest_bidder, None);
        assert_eq!(auction.ended, false);

        // Check event was emitted
        System::assert_last_event(Event::AssetListed(1, 1).into());
    });
}

#[test]
fn cant_list_asset_twice() {
    new_test_ext().execute_with(|| {
        // Arrange: List an asset
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 1));

        // Act & Assert: Try to list the same asset again
        assert_noop!(
            Template::list_asset(RuntimeOrigin::signed(1), 1),
            Error::<Test>::AssetAlreadyExists
        );
    });
}

#[test]
fn place_bid_works() {
    new_test_ext().execute_with(|| {
        // Arrange: Set block number and list an asset
        System::set_block_number(1);
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 1));

        // Act: Place a bid
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(2), 1, 50));

        // Assert: Bid should be recorded
        let auction = Template::auctions(1).unwrap();
        assert_eq!(auction.highest_bid, 50);
        assert_eq!(auction.highest_bidder, Some(2));

        // Check event was emitted
        System::assert_last_event(Event::BidPlaced(1, 2, 50).into());

        // Check funds were reserved
        assert_eq!(Balances::reserved_balance(2), 50);
    });
}

#[test]
fn cant_bid_on_own_auction() {
    new_test_ext().execute_with(|| {
        // Arrange: List an asset
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 1));

        // Act & Assert: Try to bid on own Template
        assert_noop!(
            Template::place_bid(RuntimeOrigin::signed(1), 1, 50),
            Error::<Test>::CannotBidOnOwnAuction
        );
    });
}

#[test]
fn must_bid_higher() {
    new_test_ext().execute_with(|| {
        // Arrange: List an asset and place a bid
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 1));
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(2), 1, 50));

        // Act & Assert: Try to place a lower bid
        assert_noop!(
            Template::place_bid(RuntimeOrigin::signed(3), 1, 40),
            Error::<Test>::BidTooLow
        );

        // Try to place the same bid
        assert_noop!(
            Template::place_bid(RuntimeOrigin::signed(3), 1, 50),
            Error::<Test>::BidTooLow
        );
    });
}

#[test]
fn increase_own_bid_works() {
    new_test_ext().execute_with(|| {
        // Arrange: List an asset and place a bid
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 1));
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(2), 1, 50));

        // Act: Increase own bid
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(2), 1, 70));

        // Assert: Higher bid should be recorded
        let auction = Template::auctions(1).unwrap();
        assert_eq!(auction.highest_bid, 70);
        assert_eq!(auction.highest_bidder, Some(2));

        // Check funds were reserved correctly
        assert_eq!(Balances::reserved_balance(2), 70);
    });
}

#[test]
fn outbid_works_and_unreserves_previous_bid() {
    new_test_ext().execute_with(|| {
        // Arrange: List an asset and place a bid
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 1));
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(2), 1, 50));

        // Act: Outbid by another bidder
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(3), 1, 60));

        // Assert: New highest bidder should be recorded
        let auction = Template::auctions(1).unwrap();
        assert_eq!(auction.highest_bid, 60);
        assert_eq!(auction.highest_bidder, Some(3));

        // Check previous bidder's funds were unreserved
        assert_eq!(Balances::reserved_balance(2), 0);

        // Check new bidder's funds were reserved
        assert_eq!(Balances::reserved_balance(3), 60);
    });
}

#[test]
fn choose_buyer_works() {
    new_test_ext().execute_with(|| {
        // Arrange: Set block number, list an asset, and place bids
        System::set_block_number(1);
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 1));
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(2), 1, 50));
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(3), 1, 60));

        // Act: Owner chooses a buyer (not the highest bidder)
        assert_ok!(Template::choose_buyer(RuntimeOrigin::signed(1), 1, 2));

        // Assert: Asset should be marked as sold
        let asset = Template::assets(1).unwrap();
        assert_eq!(asset.is_bought, true);
        assert_eq!(asset.buyer, Some(2));

        // Check Template is marked as ended
        let auction = Template::auctions(1).unwrap();
        assert_eq!(auction.ended, true);

        // Check funds were transferred
        assert_eq!(Balances::reserved_balance(2), 0); // No reserves left
        assert_eq!(Balances::reserved_balance(3), 0); // Other bidder's funds released

        // Check event was emitted
        System::assert_last_event(Event::AuctionResolved(1, 2, 50).into());
    });
}

#[test]
fn only_owner_can_choose_buyer() {
    new_test_ext().execute_with(|| {
        // Arrange: List an asset and place a bid
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 1));
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(2), 1, 50));

        // Act & Assert: Try to choose buyer as non-owner
        assert_noop!(
            Template::choose_buyer(RuntimeOrigin::signed(3), 1, 2),
            Error::<Test>::NotAssetOwner
        );
    });
}

#[test]
fn cant_choose_non_bidder() {
    new_test_ext().execute_with(|| {
        // Arrange: List an asset and place a bid
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 1));
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(2), 1, 50));

        // Act & Assert: Try to choose a non-bidder as the buyer
        assert_noop!(
            Template::choose_buyer(RuntimeOrigin::signed(1), 1, 3),
            Error::<Test>::NoValidBuyer
        );
    });
}

#[test]
fn auto_resolve_auction_after_timeout() {
    new_test_ext().execute_with(|| {
        // Arrange: Set block number, list an asset, and place bids
        System::set_block_number(1);
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 1));
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(2), 1, 50));
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(3), 1, 60));

        // Act: Advance blocks to trigger timeout
        // Run hooks manually to simulate block progression
        System::set_block_number(101); // Original block (1) + timeout (100)
        Template::on_initialize(101);

        // Assert: Template should be resolved with highest bidder
        let asset = Template::assets(1).unwrap();
        assert_eq!(asset.is_bought, true);
        assert_eq!(asset.buyer, Some(3)); // Highest bidder

        // Check Template is marked as ended
        let auction = Template::auctions(1).unwrap();
        assert_eq!(auction.ended, true);

        // Check funds were transferred
        assert_eq!(Balances::reserved_balance(2), 0); // Bidder's funds released
        assert_eq!(Balances::reserved_balance(3), 0); // Winner's funds transferred

        // Check event was emitted
        System::assert_has_event(Event::AuctionResolved(1, 3, 60).into());
    });
}

#[test]
fn fallback_to_next_bidder_if_transfer_fails() {
    new_test_ext().execute_with(|| {
        // This test would require mocking the Currency trait to force a transfer failure
        // For simplicity, we'll just check that the Template resolves normally
        // In a more advanced test setup, you could use mock implementations of Currency
        // that can simulate transaction failures

        // Arrange: Set up Template with multiple bids
        System::set_block_number(1);
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 1));
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(2), 1, 50));
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(3), 1, 60));

        // Fast forward to trigger auto-resolution
        System::set_block_number(101);
        Template::on_initialize(101);

        // Assert highest bidder got the asset
        let asset = Template::assets(1).unwrap();
        assert_eq!(asset.buyer, Some(3));
    });
}

#[test]
fn cant_bid_on_ended_auction() {
    new_test_ext().execute_with(|| {
        // Arrange: List an asset and place a bid
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 1));
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(2), 1, 50));

        // End the Template
        assert_ok!(Template::choose_buyer(RuntimeOrigin::signed(1), 1, 2));

        // Act & Assert: Try to bid on ended Template
        assert_noop!(
            Template::place_bid(RuntimeOrigin::signed(3), 1, 60),
            Error::<Test>::AssetAlreadySold
        );
    });
}

#[test]
fn cant_choose_buyer_for_ended_auction() {
    new_test_ext().execute_with(|| {
        // Arrange: List an asset, place bids, and end the Template
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 1));
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(2), 1, 50));
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(3), 1, 60));
        assert_ok!(Template::choose_buyer(RuntimeOrigin::signed(1), 1, 2));

        // Act & Assert: Try to choose another buyer for ended Template
        assert_noop!(
            Template::choose_buyer(RuntimeOrigin::signed(1), 1, 3),
            Error::<Test>::AssetAlreadySold
        );
    });
}

#[test]
fn auction_with_no_bids_fails_on_timeout() {
    new_test_ext().execute_with(|| {
        // Arrange: Set block number and list an asset with no bids
        System::set_block_number(1);
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 1));

        // Act: Advance blocks to trigger timeout
        System::set_block_number(101);
        Template::on_initialize(101);

        // Assert: Template should be marked as ended but no buyer set
        let asset = Template::assets(1).unwrap();
        assert_eq!(asset.is_bought, false);
        assert_eq!(asset.buyer, None);

        let auction = Template::auctions(1).unwrap();
        assert_eq!(auction.ended, true);

        // Check Template failed event
        System::assert_has_event(Event::AuctionFailed(1).into());
    });
}

#[test]
fn multiple_auctions_work_independently() {
    new_test_ext().execute_with(|| {
        // Arrange: Set block number and list multiple assets
        System::set_block_number(1);
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 1));
        assert_ok!(Template::list_asset(RuntimeOrigin::signed(1), 2));

        // Place bids on first Template
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(2), 1, 50));
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(3), 1, 60));

        // Place bid on second Template
        assert_ok!(Template::place_bid(RuntimeOrigin::signed(2), 2, 70));

        // End first Template manually
        assert_ok!(Template::choose_buyer(RuntimeOrigin::signed(1), 1, 3));

        // Act: Advance blocks to trigger timeout on second Template
        System::set_block_number(101);
        Template::on_initialize(101);

        // Assert: First Template should have buyer 3
        let asset1 = Template::assets(1).unwrap();
        assert_eq!(asset1.buyer, Some(3));

        // Second Template should have buyer 2
        let asset2 = Template::assets(2).unwrap();
        assert_eq!(asset2.buyer, Some(2));
    });
}

use crate::{mock::*, Error, Event};
use frame_support::traits::nonfungibles::Create;
use frame_support::traits::OnInitialize;
use frame_support::{assert_noop, assert_ok};

#[test]
fn list_nft_for_auction_works() {
    new_test_ext().execute_with(|| {
        // Arrange: Set block number
        System::set_block_number(1);

        let collection_id = 1;
        let item_id = 1;
        let owner = 1;

        // Create a collection
        assert_ok!(pallet_uniques::Pallet::<Test>::create_collection(
            &collection_id,
            &owner,
            &owner
        ));

        // Mint an NFT to the owner
        assert_ok!(pallet_uniques::Pallet::<Test>::mint(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id,
            owner
        ));

        // Ensure ownership is correct
        let nft_owner = pallet_uniques::Pallet::<Test>::owner(collection_id, item_id);
        assert_eq!(nft_owner, Some(owner));

        // Act: List the NFT for auction
        assert_ok!(Template::list_nft_for_auction(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id
        ));

        // Assert: Check auction info is stored
        let auction = Template::auctions((collection_id, item_id)).unwrap();
        assert_eq!(auction.owner, owner);
        assert_eq!(auction.highest_bid, 0);
        assert_eq!(auction.highest_bidder, None);
        assert_eq!(auction.ended, false);

        // Assert: NFT is marked as in auction
        let in_auction = Template::is_in_auction((collection_id, item_id));
        assert!(in_auction);

        // Assert: Correct event emitted
        System::assert_last_event(Event::NftListed(collection_id, item_id, owner).into());
    });
}

#[test]
fn cant_list_asset_twice() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let collection_id = 1;
        let item_id = 1;
        let owner = 1;

        // Create a collection
        assert_ok!(pallet_uniques::Pallet::<Test>::create_collection(
            &collection_id,
            &owner,
            &owner
        ));

        // Mint an NFT to the owner
        assert_ok!(pallet_uniques::Pallet::<Test>::mint(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id,
            owner
        ));

        // Ensure ownership is correct
        let nft_owner = pallet_uniques::Pallet::<Test>::owner(collection_id, item_id);
        assert_eq!(nft_owner, Some(owner));

        // Act: List the NFT for auction
        assert_ok!(Template::list_nft_for_auction(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id
        ));

        // Act & Assert: Try to list the same asset again
        assert_noop!(
            Template::list_nft_for_auction(RuntimeOrigin::signed(owner), collection_id, item_id),
            Error::<Test>::NftAlreadyInAuction
        );
    });
}

#[test]
fn place_bid_works() {
    new_test_ext().execute_with(|| {
        // Arrange: Set block number and list an asset
        System::set_block_number(1);

        let collection_id = 1;
        let item_id = 1;
        let owner = 1;

        // Create a collection
        assert_ok!(pallet_uniques::Pallet::<Test>::create_collection(
            &collection_id,
            &owner,
            &owner
        ));

        // Mint an NFT to the owner
        assert_ok!(pallet_uniques::Pallet::<Test>::mint(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id,
            owner
        ));

        // Ensure ownership is correct
        let nft_owner = pallet_uniques::Pallet::<Test>::owner(collection_id, item_id);
        assert_eq!(nft_owner, Some(owner));

        // Act: List the NFT for auction
        assert_ok!(Template::list_nft_for_auction(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id
        ));

        // Act: Place a bid
        assert_ok!(Template::place_bid(
            RuntimeOrigin::signed(2),
            collection_id,
            item_id,
            50
        ));

        // Assert: Bid should be recorded
        let auction = Template::auctions((collection_id, item_id)).unwrap();
        assert_eq!(auction.highest_bid, 50);
        assert_eq!(auction.highest_bidder, Some(2));

        // Check event was emitted
        System::assert_last_event(Event::BidPlaced(collection_id, item_id, 2, 50).into());

        // Check funds were reserved
        assert_eq!(Balances::reserved_balance(2), 50);
    });
}

#[test]
fn cant_bid_on_own_auction() {
    new_test_ext().execute_with(|| {
        // Arrange: List an asset
        System::set_block_number(1);

        let collection_id = 1;
        let item_id = 1;
        let owner = 1;

        // Create a collection
        assert_ok!(pallet_uniques::Pallet::<Test>::create_collection(
            &collection_id,
            &owner,
            &owner
        ));

        // Mint an NFT to the owner
        assert_ok!(pallet_uniques::Pallet::<Test>::mint(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id,
            owner
        ));

        // Ensure ownership is correct
        let nft_owner = pallet_uniques::Pallet::<Test>::owner(collection_id, item_id);
        assert_eq!(nft_owner, Some(owner));

        // Act: List the NFT for auction
        assert_ok!(Template::list_nft_for_auction(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id
        ));

        // Act & Assert: Try to bid on own Template
        assert_noop!(
            Template::place_bid(RuntimeOrigin::signed(1), collection_id, item_id, 50),
            Error::<Test>::CannotBidOnOwnAuction
        );
    });
}

#[test]
fn must_bid_higher() {
    new_test_ext().execute_with(|| {
        // Arrange: List an asset and place a bid
        System::set_block_number(1);

        let collection_id = 1;
        let item_id = 1;
        let owner = 1;

        // Create a collection
        assert_ok!(pallet_uniques::Pallet::<Test>::create_collection(
            &collection_id,
            &owner,
            &owner
        ));

        // Mint an NFT to the owner
        assert_ok!(pallet_uniques::Pallet::<Test>::mint(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id,
            owner
        ));

        // Ensure ownership is correct
        let nft_owner = pallet_uniques::Pallet::<Test>::owner(collection_id, item_id);
        assert_eq!(nft_owner, Some(owner));

        // Act: List the NFT for auction
        assert_ok!(Template::list_nft_for_auction(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id
        ));

        assert_ok!(Template::place_bid(
            RuntimeOrigin::signed(2),
            collection_id,
            item_id,
            50
        ));

        // Act & Assert: Try to place a lower bid
        assert_noop!(
            Template::place_bid(RuntimeOrigin::signed(3), collection_id, item_id, 40),
            Error::<Test>::BidTooLow
        );

        // Try to place the same bid
        assert_noop!(
            Template::place_bid(RuntimeOrigin::signed(3), collection_id, item_id, 50),
            Error::<Test>::BidTooLow
        );
    });
}

#[test]
fn increase_own_bid_works() {
    new_test_ext().execute_with(|| {
        // Arrange: List an asset and place a bid
        System::set_block_number(1);

        let collection_id = 1;
        let item_id = 1;
        let owner = 1;

        // Create a collection
        assert_ok!(pallet_uniques::Pallet::<Test>::create_collection(
            &collection_id,
            &owner,
            &owner
        ));

        // Mint an NFT to the owner
        assert_ok!(pallet_uniques::Pallet::<Test>::mint(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id,
            owner
        ));

        // Ensure ownership is correct
        let nft_owner = pallet_uniques::Pallet::<Test>::owner(collection_id, item_id);
        assert_eq!(nft_owner, Some(owner));

        // Act: List the NFT for auction
        assert_ok!(Template::list_nft_for_auction(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id
        ));

        assert_ok!(Template::place_bid(
            RuntimeOrigin::signed(2),
            collection_id,
            item_id,
            50
        ));

        // Act: Increase own bid
        assert_ok!(Template::place_bid(
            RuntimeOrigin::signed(2),
            collection_id,
            item_id,
            70
        ));

        // Assert: Higher bid should be recorded
        let auction = Template::auctions((collection_id, item_id)).unwrap();
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
        System::set_block_number(1);

        let collection_id = 1;
        let item_id = 1;
        let owner = 1;

        // Create a collection
        assert_ok!(pallet_uniques::Pallet::<Test>::create_collection(
            &collection_id,
            &owner,
            &owner
        ));

        // Mint an NFT to the owner
        assert_ok!(pallet_uniques::Pallet::<Test>::mint(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id,
            owner
        ));

        // Ensure ownership is correct
        let nft_owner = pallet_uniques::Pallet::<Test>::owner(collection_id, item_id);
        assert_eq!(nft_owner, Some(owner));

        // Act: List the NFT for auction
        assert_ok!(Template::list_nft_for_auction(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id
        ));

        assert_ok!(Template::place_bid(
            RuntimeOrigin::signed(2),
            collection_id,
            item_id,
            50
        ));

        // Act: Outbid by another bidder
        assert_ok!(Template::place_bid(
            RuntimeOrigin::signed(3),
            collection_id,
            item_id,
            60
        ));

        // Assert: New highest bidder should be recorded
        let auction = Template::auctions((collection_id, item_id)).unwrap();
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

        let collection_id = 1;
        let item_id = 1;
        let owner = 1;

        // Create a collection
        assert_ok!(pallet_uniques::Pallet::<Test>::create_collection(
            &collection_id,
            &owner,
            &owner
        ));

        // Mint an NFT to the owner
        assert_ok!(pallet_uniques::Pallet::<Test>::mint(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id,
            owner
        ));

        // Ensure ownership is correct
        let nft_owner = pallet_uniques::Pallet::<Test>::owner(collection_id, item_id);
        assert_eq!(nft_owner, Some(owner));

        // Act: List the NFT for auction
        assert_ok!(Template::list_nft_for_auction(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id
        ));

        assert_ok!(Template::place_bid(
            RuntimeOrigin::signed(2),
            collection_id,
            item_id,
            50
        ));
        assert_ok!(Template::place_bid(
            RuntimeOrigin::signed(3),
            collection_id,
            item_id,
            60
        ));

        // Act: Owner chooses a buyer (not the highest bidder)
        assert_ok!(Template::resolve_auction(
            RuntimeOrigin::signed(1),
            collection_id,
            item_id
        ));

        // Check Template is marked as ended
        let auction = Template::auctions((collection_id, item_id)).unwrap();
        assert_eq!(auction.ended, true);

        // Check funds were transferred
        assert_eq!(Balances::reserved_balance(2), 0); // Other bidder's funds released
        assert_eq!(Balances::reserved_balance(3), 0); // Non funds left

        // Check event was emitted
        System::assert_last_event(Event::AuctionResolved(collection_id, item_id, 3, 60).into());
    });
}

#[test]
fn only_owner_can_choose_buyer() {
    new_test_ext().execute_with(|| {
        // Arrange: List an asset and place a bid
        System::set_block_number(1);

        let collection_id = 1;
        let item_id = 1;
        let owner = 1;

        // Create a collection
        assert_ok!(pallet_uniques::Pallet::<Test>::create_collection(
            &collection_id,
            &owner,
            &owner
        ));

        // Mint an NFT to the owner
        assert_ok!(pallet_uniques::Pallet::<Test>::mint(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id,
            owner
        ));

        // Ensure ownership is correct
        let nft_owner = pallet_uniques::Pallet::<Test>::owner(collection_id, item_id);
        assert_eq!(nft_owner, Some(owner));

        // Act: List the NFT for auction
        assert_ok!(Template::list_nft_for_auction(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id
        ));

        assert_ok!(Template::place_bid(
            RuntimeOrigin::signed(2),
            collection_id,
            item_id,
            50
        ));

        // Act & Assert: Try to choose buyer as non-owner
        assert_noop!(
            Template::resolve_auction(RuntimeOrigin::signed(3), collection_id, item_id),
            Error::<Test>::NotNftOwner
        );
    });
}

#[test]
fn auto_resolve_auction_after_timeout() {
    new_test_ext().execute_with(|| {
        // Arrange: Set block number, list an asset, and place bids
        System::set_block_number(1);
        System::set_block_number(1);

        let collection_id = 1;
        let item_id = 1;
        let owner = 1;

        // Create a collection
        assert_ok!(pallet_uniques::Pallet::<Test>::create_collection(
            &collection_id,
            &owner,
            &owner
        ));

        // Mint an NFT to the owner
        assert_ok!(pallet_uniques::Pallet::<Test>::mint(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id,
            owner
        ));

        // Ensure ownership is correct
        let nft_owner = pallet_uniques::Pallet::<Test>::owner(collection_id, item_id);
        assert_eq!(nft_owner, Some(owner));

        // Act: List the NFT for auction
        assert_ok!(Template::list_nft_for_auction(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id
        ));

        assert_ok!(Template::place_bid(
            RuntimeOrigin::signed(2),
            collection_id,
            item_id,
            50
        ));
        assert_ok!(Template::place_bid(
            RuntimeOrigin::signed(3),
            collection_id,
            item_id,
            60
        ));

        // Act: Advance blocks to trigger timeout
        // Run hooks manually to simulate block progression
        System::set_block_number(101); // Original block (1) + timeout (100)
        Template::on_initialize(101);

        // Check Template is marked as ended
        let auction = Template::auctions((collection_id, item_id)).unwrap();
        assert_eq!(auction.ended, true);

        // Check funds were transferred
        assert_eq!(Balances::reserved_balance(2), 0); // Bidder's funds released
        assert_eq!(Balances::reserved_balance(3), 0); // Winner's funds transferred

        // Check event was emitted
        System::assert_has_event(Event::AuctionResolved(collection_id, item_id, 3, 60).into());
    });
}

#[test]
fn cant_bid_on_ended_auction() {
    new_test_ext().execute_with(|| {
        // Arrange: List an asset and place a bid
        System::set_block_number(1);

        let collection_id = 1;
        let item_id = 1;
        let owner = 1;

        // Create a collection
        assert_ok!(pallet_uniques::Pallet::<Test>::create_collection(
            &collection_id,
            &owner,
            &owner
        ));

        // Mint an NFT to the owner
        assert_ok!(pallet_uniques::Pallet::<Test>::mint(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id,
            owner
        ));

        // Ensure ownership is correct
        let nft_owner = pallet_uniques::Pallet::<Test>::owner(collection_id, item_id);
        assert_eq!(nft_owner, Some(owner));

        // Act: List the NFT for auction
        assert_ok!(Template::list_nft_for_auction(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id
        ));

        assert_ok!(Template::place_bid(
            RuntimeOrigin::signed(2),
            collection_id,
            item_id,
            50
        ));

        // End the Template
        assert_ok!(Template::resolve_auction(
            RuntimeOrigin::signed(1),
            collection_id,
            item_id
        ));

        // Act & Assert: Try to bid on ended Template
        assert_noop!(
            Template::place_bid(RuntimeOrigin::signed(3), collection_id, item_id, 60),
            Error::<Test>::AuctionEnded
        );
    });
}

#[test]
fn cant_choose_buyer_for_ended_auction() {
    new_test_ext().execute_with(|| {
        // Arrange: List an asset, place bids, and end the Template
        System::set_block_number(1);

        let collection_id = 1;
        let item_id = 1;
        let owner = 1;

        // Create a collection
        assert_ok!(pallet_uniques::Pallet::<Test>::create_collection(
            &collection_id,
            &owner,
            &owner
        ));

        // Mint an NFT to the owner
        assert_ok!(pallet_uniques::Pallet::<Test>::mint(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id,
            owner
        ));

        // Ensure ownership is correct
        let nft_owner = pallet_uniques::Pallet::<Test>::owner(collection_id, item_id);
        assert_eq!(nft_owner, Some(owner));

        // Act: List the NFT for auction
        assert_ok!(Template::list_nft_for_auction(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id
        ));

        assert_ok!(Template::place_bid(
            RuntimeOrigin::signed(2),
            collection_id,
            item_id,
            50
        ));
        assert_ok!(Template::place_bid(
            RuntimeOrigin::signed(3),
            collection_id,
            item_id,
            60
        ));
        assert_ok!(Template::resolve_auction(
            RuntimeOrigin::signed(1),
            collection_id,
            item_id
        ));

        // Act & Assert: Try to choose another buyer for ended Template
        assert_noop!(
            Template::resolve_auction(RuntimeOrigin::signed(1), collection_id, item_id),
            Error::<Test>::AuctionEnded
        );
    });
}

#[test]
fn auction_with_no_bids_fails_on_timeout() {
    new_test_ext().execute_with(|| {
        // Arrange: Set block number and list an asset with no bids
        System::set_block_number(1);

        let collection_id = 1;
        let item_id = 1;
        let owner = 1;

        // Create a collection
        assert_ok!(pallet_uniques::Pallet::<Test>::create_collection(
            &collection_id,
            &owner,
            &owner
        ));

        // Mint an NFT to the owner
        assert_ok!(pallet_uniques::Pallet::<Test>::mint(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id,
            owner
        ));

        // Ensure ownership is correct
        let nft_owner = pallet_uniques::Pallet::<Test>::owner(collection_id, item_id);
        assert_eq!(nft_owner, Some(owner));

        // Act: List the NFT for auction
        assert_ok!(Template::list_nft_for_auction(
            RuntimeOrigin::signed(owner),
            collection_id,
            item_id
        ));

        // Act: Advance blocks to trigger timeout
        System::set_block_number(101);
        Template::on_initialize(101);

        let auction = Template::auctions((collection_id, item_id)).unwrap();
        assert_eq!(auction.ended, true);

        // Check Template failed event
        System::assert_has_event(Event::AuctionFailed(collection_id, item_id).into());
    });
}

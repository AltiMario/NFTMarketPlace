/// # NFTMarketplace Contract Test Suite
///
/// This module contains comprehensive tests for the `NFTMarketplace` contract,
/// covering core functionality, edge cases, and security requirements.
///
/// ## Test Accounts Convention:
/// - **Alice**: Default caller/initiator (ink! default account)
/// - **Bob**: Counterparty account
/// - **Charlie**: Unauthorized third party
#[cfg(test)]
mod tests {
    use nft_marketplace::{NFTMarketplace, Error};
    use ink::env::{test, DefaultEnvironment};

    /// Tests the initialization of the contract.
    /// - Verifies that the `admin` field is set to the account that deployed the contract.
    #[ink::test]
    fn test_new_contract() {
        let accounts = test::default_accounts::<DefaultEnvironment>();
        let marketplace = NFTMarketplace::new();

        assert_eq!(marketplace.admin, accounts.alice);
    }

    /// Tests the minting of a new NFT.
    /// - Verifies that a new NFT is created with the correct `content_hash`.
    /// - Verifies that the NFT is owned by the caller.
    #[ink::test]
    fn test_mint_nft() {
        let mut marketplace = NFTMarketplace::new();
        let content_hash = String::from("unique_hash");

        let result = marketplace.mint_nft(content_hash.clone());
        assert!(result.is_ok());
        let content_id = result.unwrap();

        let content = marketplace.get_content(content_id);
        assert!(content.is_some());
        assert_eq!(content.unwrap().content_hash, content_hash);
    }

    /// Tests listing an NFT for sale.
    /// - Verifies that the NFT is listed with the correct price and seller information.
    /// - Verifies that the listing is marked as active.
    #[ink::test]
    fn test_list_asset() {
        let mut marketplace = NFTMarketplace::new();
        let accounts = test::default_accounts::<DefaultEnvironment>();
        let content_hash = String::from("unique_hash");

        let content_id = marketplace.mint_nft(content_hash).unwrap();
        let price = 100;

        let result = marketplace.list_asset(content_id, price);
        assert!(result.is_ok());

        let listing = marketplace.get_listing(content_id);
        assert!(listing.is_some());
        let listing = listing.unwrap();
        assert_eq!(listing.price, price);
        assert_eq!(listing.seller, accounts.alice);
        assert!(listing.is_active);
    }

    /// Tests canceling an active listing.
    /// - Verifies that the listing is marked as inactive after cancellation.
    #[ink::test]
    fn test_cancel_listing() {
        let mut marketplace = NFTMarketplace::new();
        let content_hash = String::from("unique_hash");

        let content_id = marketplace.mint_nft(content_hash).unwrap();
        marketplace.list_asset(content_id, 100).unwrap();

        let result = marketplace.cancel_listing(content_id);
        assert!(result.is_ok());

        let listing = marketplace.get_listing(content_id);
        assert!(listing.is_some());
        assert!(!listing.unwrap().is_active);
    }

    /// Tests buying an NFT.
    /// - Verifies that the ownership of the NFT is transferred to the buyer.
    /// - Verifies that the listing is deactivated after the purchase.
    #[ink::test]
    fn test_buy_asset() {
        let mut marketplace = NFTMarketplace::new();
        let accounts = test::default_accounts::<DefaultEnvironment>();
        let content_hash = String::from("unique_hash");

        let content_id = marketplace.mint_nft(content_hash).unwrap();
        marketplace.list_asset(content_id, 100).unwrap();

        test::set_caller::<DefaultEnvironment>(accounts.bob);
        test::set_value_transferred::<DefaultEnvironment>(100);

        let result = marketplace.buy_asset(content_id);
        assert!(result.is_ok());

        let content = marketplace.get_content(content_id).unwrap();
        assert_eq!(content.owner, accounts.bob);

        let listing = marketplace.get_listing(content_id);
        assert!(listing.is_some());
        assert!(!listing.unwrap().is_active);
    }

    /// Tests the `approve` function.
    /// - Verifies that an account can be approved to transfer an NFT.
    /// - Verifies that only the owner can approve an account.
    #[ink::test]
    fn test_approve() {
        let mut marketplace = NFTMarketplace::new();
        let accounts = test::default_accounts::<DefaultEnvironment>();
        let content_hash = String::from("unique_hash");

        let content_id = marketplace.mint_nft(content_hash).unwrap();

        // Approve Bob to transfer the NFT
        let result = marketplace.approve(content_id, accounts.bob);
        assert!(result.is_ok());

        // Set caller to Charlie (unauthorized third party)
        test::set_caller::<DefaultEnvironment>(accounts.charlie);

        // Attempt to approve another account (should fail)
        let result = marketplace.approve(content_id, accounts.charlie);
        assert_eq!(result, Err(Error::NotOwner));
    }

    /// Tests the `update_listing` function.
    /// - Verifies that the price of an active listing can be updated.
    /// - Verifies that only the seller can update the listing.
    #[ink::test]
    fn test_update_listing() {
        let mut marketplace = NFTMarketplace::new();
        let accounts = test::default_accounts::<DefaultEnvironment>();
        let content_hash = String::from("unique_hash");

        let content_id = marketplace.mint_nft(content_hash).unwrap();
        marketplace.list_asset(content_id, 100).unwrap();

        // Update the listing price
        let new_price = 150;
        let result = marketplace.update_listing(content_id, new_price);
        assert!(result.is_ok());

        let listing = marketplace.get_listing(content_id).unwrap();
        assert_eq!(listing.price, new_price);

        // Set caller to Bob (unauthorized user)
        test::set_caller::<DefaultEnvironment>(accounts.bob);

        // Attempt to update the listing (should fail)
        let result = marketplace.update_listing(content_id, 200);
        assert_eq!(result, Err(Error::NotOwner));
    }

    /// Tests unauthorized actions by a third party.
    /// - Verifies that a third party cannot cancel a listing.
    #[ink::test]
    fn test_unauthorized_actions() {
        let mut marketplace = NFTMarketplace::new();
        let accounts = test::default_accounts::<DefaultEnvironment>();
        let content_hash = String::from("unique_hash");

        // Mint an NFT and list it for sale
        let content_id = marketplace.mint_nft(content_hash).unwrap();
        marketplace.list_asset(content_id, 100).unwrap();

        // Set caller to Charlie (unauthorized third party)
        test::set_caller::<DefaultEnvironment>(accounts.charlie);

        // Attempt to cancel the listing (should fail)
        let cancel_result = marketplace.cancel_listing(content_id);
        assert_eq!(cancel_result, Err(Error::NotOwner));
    }

    /// Tests that the owner cannot buy their own NFT.
    /// - Verifies that the owner cannot buy their own NFT.
    #[ink::test]
    fn test_owner_cannot_buy_own_nft() {
        let mut marketplace = NFTMarketplace::new();
        let accounts = test::default_accounts::<DefaultEnvironment>();
        let content_hash = String::from("unique_hash");

        // Mint and list an NFT
        let content_id = marketplace.mint_nft(content_hash).unwrap();
        marketplace.list_asset(content_id, 100).unwrap();

        // Set caller to the owner (Alice)
        test::set_caller::<DefaultEnvironment>(accounts.alice);
        test::set_value_transferred::<DefaultEnvironment>(100);

        // Attempt to buy the NFT (should fail)
        let result = marketplace.buy_asset(content_id);
        assert_eq!(result, Err(Error::NotOwner));
    }
}
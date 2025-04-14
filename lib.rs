#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod nft_marketplace {
    use ink::storage::Mapping;
    use ink::prelude::string::String;
    use ink::prelude::collections::BTreeMap;

    /// NFT content with ownership and approval
    #[derive(scale::Encode, scale::Decode, Clone, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
    pub struct Content {
        content_hash: String,
        owner: AccountId,
        approved: Option<AccountId>,
    }

    /// Marketplace listing structure
    #[derive(scale::Encode, scale::Decode, Clone, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
    pub struct Listing {
        pub asset_id: u64,
        pub seller: AccountId,
        pub price: Balance,
        pub is_active: bool,
    }

    /// Event emitted when NFT is listed
    #[ink(event)]
    pub struct AssetListed {
        #[ink(topic)]
        asset_id: u64,
        seller: AccountId,
        price: Balance,
    }

    /// Event emitted when NFT is sold
    #[ink(event)]
    pub struct AssetSold {
        #[ink(topic)]
        asset_id: u64,
        seller: AccountId,
        buyer: AccountId,
        price: Balance,
    }

    /// Event emitted when listing is updated
    #[ink(event)]
    pub struct ListingUpdated {
        #[ink(topic)]
        asset_id: u64,
        new_price: Balance,
    }

    /// Event emitted when listing is canceled
    #[ink(event)]
    pub struct ListingCanceled {
        #[ink(topic)]
        asset_id: u64,
    }

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        NotAdmin = 0,
        ContentNotFound = 1,
        NotOwner = 2,
        CounterOverflow = 3,
        InvalidContent = 4,
        AssetAlreadyListed = 5,
        ListingNotFound = 6,
        ListingNotActive = 7,
        InvalidPrice = 8,
        TransferFailed = 9,
        UnauthorizedTransfer = 10,
    }

    pub type Result<T> = core::result::Result<T, Error>;

    #[ink(storage)]
    pub struct NFTMarketplace {
        admin: AccountId,
        contents: Mapping<u64, Content>,
        next_content_id: u64,
        content_hash_to_id: BTreeMap<String, u64>,
        listings: Mapping<u64, Listing>,
    }

    impl Default for NFTMarketplace {
        fn default() -> Self {
            Self {
                admin: AccountId::from([0u8; 32]),
                contents: Mapping::default(),
                next_content_id: 1,
                content_hash_to_id: BTreeMap::new(),
                listings: Mapping::default(),
            }
        }
    }

    impl NFTMarketplace {
        /// Constructor to initialize the contract with the caller as the admin.
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {
                admin: Self::env().caller(),
                ..Default::default()
            }
        }


        /// Mints a new NFT with the given content hash.
        /// 
        /// # Parameters
        /// - `content_hash`: A unique hash representing the content of the NFT.
        /// 
        /// # Returns
        /// - `Ok(content_id)` with the ID of the newly minted NFT.
        /// - `Err(Error::CounterOverflow)` if the `next_content_id` exceeds the maximum value.
        #[ink(message)]
        pub fn mint_nft(&mut self, content_hash: String) -> Result<u64> {
            if let Some(existing_id) = self.content_hash_to_id.get(&content_hash) {
                return Ok(*existing_id);
            }

            let content_id = self.next_content_id;
            self.next_content_id = self.next_content_id
                .checked_add(1)
                .ok_or(Error::CounterOverflow)?;

            let record = Content {
                content_hash: content_hash.clone(),
                owner: self.env().caller(),
                approved: None,
            };

            self.contents.insert(content_id, &record);
            self.content_hash_to_id.insert(content_hash, content_id);
            Ok(content_id)
        }
        
        /// Internal function to transfer ownership of an NFT without permission checks.
        /// 
        /// # Parameters
        /// - `asset_id`: The ID of the NFT to transfer.
        /// - `new_owner`: The account ID of the new owner.
        /// 
        /// # Returns
        /// - `Ok(())` if the transfer is successful.
        /// - `Err(Error::ContentNotFound)` if the NFT does not exist.
        fn _transfer_ownership(&mut self, asset_id: u64, new_owner: AccountId) -> Result<()> {
            let mut content = self.contents.get(asset_id).ok_or(Error::ContentNotFound)?;
            content.owner = new_owner;
            content.approved = None;
            self.contents.insert(asset_id, &content);
            Ok(())
        }

        /// Cancels an active listing for an NFT.
        /// 
        /// # Parameters
        /// - `asset_id`: The ID of the NFT whose listing is to be canceled.
        /// 
        /// # Returns
        /// - `Ok(())` if the cancellation is successful.
        /// - `Err(Error::ListingNotFound)` if the listing does not exist.
        /// - `Err(Error::NotOwner)` if the caller is not the seller of the listing.
        /// - `Err(Error::ListingNotActive)` if the listing is not active.
        #[ink(message)]
        pub fn cancel_listing(&mut self, asset_id: u64) -> Result<()> {
            let mut listing = self.listings.get(asset_id).ok_or(Error::ListingNotFound)?;

            if self.env().caller() != listing.seller {
                return Err(Error::NotOwner);
            }

            if !listing.is_active {
                return Err(Error::ListingNotActive);
            }

            listing.is_active = false;
            self.listings.insert(asset_id, &listing);

            // Revoke contract approval if it exists
            let mut content = self.contents.get(asset_id).ok_or(Error::ContentNotFound)?;
            content.approved = None;
            self.contents.insert(asset_id, &content);

            self.env().emit_event(ListingCanceled { asset_id });

            Ok(())
        }

        /// Transfers ownership of an NFT to a new owner.
        /// 
        /// # Parameters
        /// - `content_id`: The ID of the NFT to transfer.
        /// - `new_owner`: The account ID of the new owner.
        /// 
        /// # Returns
        /// - `Ok(())` if the transfer is successful.
        /// - `Err(Error::ContentNotFound)` if the NFT does not exist.
        /// - `Err(Error::NotOwner)` if the caller is neither the owner nor the approved account for the NFT.
        #[ink(message)]
        pub fn transfer_ownership(&mut self, content_id: u64, new_owner: AccountId) -> Result<()> {
            let caller = self.env().caller();
            let content = self.contents.get(content_id).ok_or(Error::ContentNotFound)?;

            if caller != content.owner && Some(caller) != content.approved {
                return Err(Error::NotOwner);
            }

            // Invalidate any active listing
            if let Some(mut listing) = self.listings.get(content_id) {
                if listing.is_active {
                    listing.is_active = false;
                }
                self.listings.insert(content_id, &listing);
                self.env().emit_event(ListingCanceled { asset_id: content_id });
            }

            self._transfer_ownership(content_id, new_owner)
        }

        /// Approves an account to transfer the specified NFT.
        /// 
        /// # Parameters
        /// - `asset_id`: The ID of the NFT to approve.
        /// - `approved`: The account ID to approve.
        /// 
        /// # Returns
        /// - `Ok(())` if the approval is successful.
        /// - `Err(Error::ContentNotFound)` if the NFT does not exist.
        /// - `Err(Error::NotOwner)` if the caller is not the owner of the NFT.
        #[ink(message)]
        pub fn approve(&mut self, asset_id: u64, approved: AccountId) -> Result<()> {
            let mut content = self.contents.get(asset_id).ok_or(Error::ContentNotFound)?;

            if self.env().caller() != content.owner {
                return Err(Error::NotOwner);
            }

            content.approved = Some(approved);
            self.contents.insert(asset_id, &content);
            Ok(())
        }

        /// Lists an NFT for sale at the specified price.
        /// 
        /// # Parameters
        /// - `asset_id`: The ID of the NFT to list.
        /// - `price`: The price at which the NFT is listed.
        /// 
        /// # Returns
        /// - `Ok(())` if the listing is successful.
        /// - `Err(Error::ContentNotFound)` if the NFT does not exist.
        /// - `Err(Error::NotOwner)` if the caller is not the owner of the NFT.
        /// - `Err(Error::InvalidPrice)` if the price is zero.
        /// - `Err(Error::AssetAlreadyListed)` if the NFT is already listed.
        #[ink(message)]
        pub fn list_asset(&mut self, asset_id: u64, price: Balance) -> Result<()> {
            if price == 0 {
                return Err(Error::InvalidPrice);
            }

            let content = self.contents.get(asset_id).ok_or(Error::ContentNotFound)?;
            let caller = self.env().caller();

            if caller != content.owner {
                return Err(Error::NotOwner);
            }

            if let Some(existing) = self.listings.get(asset_id) {
                if existing.is_active {
                    return Err(Error::AssetAlreadyListed);
                }
            }

            // Approve the contract to handle transfers
            self.approve(asset_id, self.env().account_id())?;

            let listing = Listing {
                asset_id,
                seller: caller,
                price,
                is_active: true,
            };

            self.listings.insert(asset_id, &listing);
            self.env().emit_event(AssetListed {
                asset_id,
                seller: caller,
                price,
            });

            Ok(())
        }

        #[ink(message, payable)]
        pub fn buy_asset(&mut self, asset_id: u64) -> Result<()> {
            let mut listing = self.listings.get(asset_id).ok_or(Error::ListingNotFound)?;
            let content = self.contents.get(asset_id).ok_or(Error::ContentNotFound)?; // Still need content for approval check
            let caller = self.env().caller();
            let transferred = self.env().transferred_value();

            // Validate listing state
            if !listing.is_active {
                return Err(Error::ListingNotActive);
            }

            // This is the critical approval check
            if content.approved != Some(self.env().account_id()) {
                return Err(Error::UnauthorizedTransfer);
            }

            // Validate payment
            if transferred < listing.price {
                // Consider returning the sent funds if payment is insufficient
                // Currently, the funds remain in the contract if this error occurs.
                return Err(Error::InvalidPrice);
            }

            // --- Rest of the function ---
            // Update state FIRST
            listing.is_active = false;
            self.listings.insert(asset_id, &listing);

            // Transfer NFT ownership internally
            self._transfer_ownership(asset_id, caller)?;

            // Handle funds
            let excess = transferred.checked_sub(listing.price).ok_or(Error::TransferFailed)?; // Should not happen if transferred >= listing.price

            // Transfer payment to seller
            self
                .env()
                .transfer(listing.seller, listing.price)
                .map_err(|_| Error::TransferFailed)?;

            // Refund excess
            if excess > 0 {
                self
                    .env()
                    .transfer(caller, excess)
                    .map_err(|_| Error::TransferFailed)?;
            }

            self.env().emit_event(AssetSold {
                asset_id,
                seller: listing.seller,
                buyer: caller,
                price: listing.price,
            });

            Ok(())
        }

        #[ink(message)]
        pub fn update_listing(&mut self, asset_id: u64, new_price: Balance) -> Result<()> {
            if new_price == 0 {
                return Err(Error::InvalidPrice);
            }

            let mut listing = self.listings.get(asset_id).ok_or(Error::ListingNotFound)?;

            if self.env().caller() != listing.seller {
                return Err(Error::NotOwner);
            }

            if !listing.is_active {
                return Err(Error::ListingNotActive);
            }

            listing.price = new_price;
            self.listings.insert(asset_id, &listing);

            self.env().emit_event(ListingUpdated {
                asset_id,
                new_price,
            });

            Ok(())
        }

        /// Retrieves the details of an NFT.
        /// 
        /// # Parameters
        /// - `asset_id`: The ID of the NFT to retrieve.
        /// 
        /// # Returns
        /// - `Some(Content)` if the NFT exists.
        /// - `None` if the NFT does not exist.
        #[ink(message)]
        pub fn get_content(&self, asset_id: u64) -> Option<Content> {
            self.contents.get(asset_id)
        }

        /// Retrieves the details of a listing.
        /// 
        /// # Parameters
        /// - `asset_id`: The ID of the NFT whose listing is to be retrieved.
        /// 
        /// # Returns
        /// - `Some(Listing)` if the listing exists.
        /// - `None` if the listing does not exist.
        #[ink(message)]
        pub fn get_listing(&self, asset_id: u64) -> Option<Listing> {
            self.listings.get(asset_id)
        }
    }
}

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_support::traits::Get;
use frame_system::{
    self as system,
    offchain::{
        AppCrypto, CreateInherent, CreateSignedTransaction, SendSignedTransaction,
        SendUnsignedTransaction, SignedPayload, Signer, SigningTypes, SubmitTransaction,
    },
    pallet_prelude::BlockNumberFor,
};
use lite_json::json::JsonValue;
use sp_core::crypto::KeyTypeId;
use sp_runtime::{
    offchain::{
        http,
        storage::{MutateStorageError, StorageRetrievalError, StorageValueRef},
        Duration,
    },
    traits::Zero,
    transaction_validity::{InvalidTransaction, TransactionValidity, ValidTransaction},
    RuntimeDebug,
};

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"btc!");

pub mod crypto {
    use super::KEY_TYPE;
    use sp_core::sr25519::Signature as Sr25519Signature;
    use sp_runtime::{
        app_crypto::{app_crypto, sr25519},
        traits::Verify,
        MultiSignature, MultiSigner,
    };
    app_crypto!(sr25519, KEY_TYPE);

    pub struct TestAuthId;

    impl frame_system::offchain::AppCrypto<MultiSigner, Sr25519Signature> for TestAuthId {
        type RuntimeAppPublic = Public;
        type GenericSignature = Signature;
        type GenericPublic = sp_core::sr25519::Public;
    }

    impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
        type RuntimeAppPublic = Public;
        type GenericSignature = sp_core::sr25519::Signature;
        type GenericPublic = sp_core::sr25519::Public;
    }

    // implemented for mock runtime in test
    // impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature>
    //     for TestAuthId
    // {
    //     type RuntimeAppPublic = Public;
    //     type GenericSignature = sp_core::sr25519::Signature;
    //     type GenericPublic = sp_core::sr25519::Public;
    // }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    /// This pallet's configuration trait
    #[pallet::config]
    pub trait Config:
        CreateSignedTransaction<Call<Self>> + CreateInherent<Call<Self>> + frame_system::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The identifier type for an offchain worker.
        type AuthorityId: AppCrypto<Self::Public, Self::Signature>;

        #[pallet::constant]
        type GracePeriod: Get<BlockNumberFor<Self>>;

        #[pallet::constant]
        type UnsignedInterval: Get<BlockNumberFor<Self>>;

        #[pallet::constant]
        type UnsignedPriority: Get<TransactionPriority>;

        #[pallet::constant]
        type MaxPrices: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn offchain_worker(block_number: BlockNumberFor<T>) {
            log::info!("Hello World from offchain workers!");

            let parent_hash = <system::Pallet<T>>::block_hash(block_number - 1u32.into());
            log::debug!(
                "Current block: {:?} (parent hash: {:?})",
                block_number,
                parent_hash
            );

            let average: Option<u32> = Self::average_price();
            log::debug!("Current price: {:?}", average);

            let should_send = Self::choose_transaction_type(block_number);
            let res = match should_send {
                TransactionType::Signed => Self::fetch_price_and_send_signed(),
                TransactionType::UnsignedForAny => {
                    Self::fetch_price_and_send_unsigned_for_any_account(block_number)
                }
                TransactionType::UnsignedForAll => {
                    Self::fetch_price_and_send_unsigned_for_all_accounts(block_number)
                }
                TransactionType::Raw => Self::fetch_price_and_send_raw_unsigned(block_number),
                TransactionType::None => Ok(()),
            };
            if let Err(e) = res {
                log::error!("Error: {}", e);
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight({0})]
        pub fn submit_price(origin: OriginFor<T>, price: u32) -> DispatchResultWithPostInfo {
            // Retrieve sender of the transaction.
            let who = ensure_signed(origin)?;
            // Add the price to the on-chain list.
            Self::add_price(Some(who), price);
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight({0})]
        pub fn submit_price_unsigned(
            origin: OriginFor<T>,
            _block_number: BlockNumberFor<T>,
            price: u32,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;
            Self::add_price(None, price);
            let current_block = <system::Pallet<T>>::block_number();
            <NextUnsignedAt<T>>::put(current_block + T::UnsignedInterval::get());
            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight({0})]
        pub fn submit_price_unsigned_with_signed_payload(
            origin: OriginFor<T>,
            price_payload: PricePayload<T::Public, BlockNumberFor<T>>,
            _signature: T::Signature,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;
            Self::add_price(None, price_payload.price);
            let current_block = <system::Pallet<T>>::block_number();
            <NextUnsignedAt<T>>::put(current_block + T::UnsignedInterval::get());
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        NewPrice {
            price: u32,
            maybe_who: Option<T::AccountId>,
        },
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            if let Call::submit_price_unsigned_with_signed_payload {
                price_payload: ref payload,
                ref signature,
            } = call
            {
                let signature_valid =
                    SignedPayload::<T>::verify::<T::AuthorityId>(payload, signature.clone());
                if !signature_valid {
                    return InvalidTransaction::BadProof.into();
                }
                Self::validate_transaction_parameters(&payload.block_number, &payload.price)
            } else if let Call::submit_price_unsigned {
                block_number,
                price: new_price,
            } = call
            {
                Self::validate_transaction_parameters(block_number, new_price)
            } else {
                InvalidTransaction::Call.into()
            }
        }
    }

    #[pallet::storage]
    pub(super) type Prices<T: Config> = StorageValue<_, BoundedVec<u32, T::MaxPrices>, ValueQuery>;

    #[pallet::storage]
    pub(super) type NextUnsignedAt<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo,
)]
pub struct PricePayload<Public, BlockNumber> {
    block_number: BlockNumber,
    price: u32,
    public: Public,
}

impl<T: SigningTypes> SignedPayload<T> for PricePayload<T::Public, BlockNumberFor<T>> {
    fn public(&self) -> T::Public {
        self.public.clone()
    }
}

enum TransactionType {
    Signed,
    UnsignedForAny,
    UnsignedForAll,
    Raw,
    None,
}

impl<T: Config> Pallet<T> {
    fn choose_transaction_type(block_number: BlockNumberFor<T>) -> TransactionType {
        const RECENTLY_SENT: () = ();

        let val = StorageValueRef::persistent(b"example_ocw::last_send");
        let res = val.mutate(
            |last_send: Result<Option<BlockNumberFor<T>>, StorageRetrievalError>| {
                match last_send {
                    // If we already have a value in storage and the block number is recent enough
                    // we avoid sending another transaction at this time.
                    Ok(Some(block)) if block_number < block + T::GracePeriod::get() => {
                        Err(RECENTLY_SENT)
                    }
                    // In every other case we attempt to acquire the lock and send a transaction.
                    _ => Ok(block_number),
                }
            },
        );

        match res {
            Ok(block_number) => {
                let transaction_type = block_number % 4u32.into();
                if transaction_type == Zero::zero() {
                    TransactionType::Signed
                } else if transaction_type == BlockNumberFor::<T>::from(1u32) {
                    TransactionType::UnsignedForAny
                } else if transaction_type == BlockNumberFor::<T>::from(2u32) {
                    TransactionType::UnsignedForAll
                } else {
                    TransactionType::Raw
                }
            }
            Err(MutateStorageError::ValueFunctionFailed(RECENTLY_SENT)) => TransactionType::None,
            Err(MutateStorageError::ConcurrentModification(_)) => TransactionType::None,
        }
    }

    fn fetch_price_and_send_signed() -> Result<(), &'static str> {
        let signer = Signer::<T, T::AuthorityId>::all_accounts();
        if !signer.can_sign() {
            return Err(
                "No local accounts available. Consider adding one via `author_insertKey` RPC.",
            );
        }
        let price = Self::fetch_price().map_err(|_| "Failed to fetch price")?;

        let results = signer.send_signed_transaction(|_account| Call::submit_price { price });

        for (acc, res) in &results {
            match res {
                Ok(()) => log::info!("[{:?}] Submitted price of {} cents", acc.id, price),
                Err(e) => log::error!("[{:?}] Failed to submit transaction: {:?}", acc.id, e),
            }
        }

        Ok(())
    }

    fn fetch_price_and_send_raw_unsigned(
        block_number: BlockNumberFor<T>,
    ) -> Result<(), &'static str> {
        let next_unsigned_at = NextUnsignedAt::<T>::get();
        if next_unsigned_at > block_number {
            return Err("Too early to send unsigned transaction");
        }

        let price = Self::fetch_price().map_err(|_| "Failed to fetch price")?;

        let call = Call::submit_price_unsigned {
            block_number,
            price,
        };

        let xt = T::create_inherent(call.into());
        SubmitTransaction::<T, Call<T>>::submit_transaction(xt)
            .map_err(|()| "Unable to submit unsigned transaction.")?;

        Ok(())
    }

    fn fetch_price_and_send_unsigned_for_any_account(
        block_number: BlockNumberFor<T>,
    ) -> Result<(), &'static str> {
        let next_unsigned_at = NextUnsignedAt::<T>::get();
        if next_unsigned_at > block_number {
            return Err("Too early to send unsigned transaction");
        }

        let price = Self::fetch_price().map_err(|_| "Failed to fetch price")?;

        let (_, result) = Signer::<T, T::AuthorityId>::any_account()
            .send_unsigned_transaction(
                |account| PricePayload {
                    price,
                    block_number,
                    public: account.public.clone(),
                },
                |payload, signature| Call::submit_price_unsigned_with_signed_payload {
                    price_payload: payload,
                    signature,
                },
            )
            .ok_or("No local accounts accounts available.")?;
        result.map_err(|()| "Unable to submit transaction")?;

        Ok(())
    }

    fn fetch_price_and_send_unsigned_for_all_accounts(
        block_number: BlockNumberFor<T>,
    ) -> Result<(), &'static str> {
        let next_unsigned_at = NextUnsignedAt::<T>::get();
        if next_unsigned_at > block_number {
            return Err("Too early to send unsigned transaction");
        }

        let price = Self::fetch_price().map_err(|_| "Failed to fetch price")?;

        let transaction_results = Signer::<T, T::AuthorityId>::all_accounts()
            .send_unsigned_transaction(
                |account| PricePayload {
                    price,
                    block_number,
                    public: account.public.clone(),
                },
                |payload, signature| Call::submit_price_unsigned_with_signed_payload {
                    price_payload: payload,
                    signature,
                },
            );
        for (_account_id, result) in transaction_results.into_iter() {
            if result.is_err() {
                return Err("Unable to submit transaction");
            }
        }

        Ok(())
    }

    fn fetch_price() -> Result<u32, http::Error> {
        let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(2_000));
        let request =
            http::Request::get("https://min-api.cryptocompare.com/data/price?fsym=BTC&tsyms=USD");

        let pending = request
            .deadline(deadline)
            .send()
            .map_err(|_| http::Error::IoError)?;

        let response = pending
            .try_wait(deadline)
            .map_err(|_| http::Error::DeadlineReached)??;
        if response.code != 200 {
            log::warn!("Unexpected status code: {}", response.code);
            return Err(http::Error::Unknown);
        }

        let body = response.body().collect::<Vec<u8>>();

        let body_str = alloc::str::from_utf8(&body).map_err(|_| {
            log::warn!("No UTF8 body");
            http::Error::Unknown
        })?;

        let price = match Self::parse_price(body_str) {
            Some(price) => Ok(price),
            None => {
                log::warn!("Unable to extract price from the response: {:?}", body_str);
                Err(http::Error::Unknown)
            }
        }?;

        log::warn!("Got price: {} cents", price);

        Ok(price)
    }

    fn parse_price(price_str: &str) -> Option<u32> {
        let val = lite_json::parse_json(price_str);
        let price = match val.ok()? {
            JsonValue::Object(obj) => {
                let (_, v) = obj
                    .into_iter()
                    .find(|(k, _)| k.iter().copied().eq("USD".chars()))?;
                match v {
                    JsonValue::Number(number) => number,
                    _ => return None,
                }
            }
            _ => return None,
        };

        let exp = price.fraction_length.saturating_sub(2);
        Some(price.integer as u32 * 100 + (price.fraction / 10_u64.pow(exp)) as u32)
    }

    fn add_price(maybe_who: Option<T::AccountId>, price: u32) {
        log::info!("Adding to the average: {}", price);
        <Prices<T>>::mutate(|prices| {
            if prices.try_push(price).is_err() {
                prices[(price % T::MaxPrices::get()) as usize] = price;
            }
        });

        let average = Self::average_price()
            .expect("The average is not empty, because it was just mutated; qed");
        log::info!("Current average price is: {}", average);
        // here we are raising the NewPrice event
        Self::deposit_event(Event::NewPrice { price, maybe_who });
    }

    fn average_price() -> Option<u32> {
        let prices = Prices::<T>::get();
        if prices.is_empty() {
            None
        } else {
            Some(prices.iter().fold(0_u32, |a, b| a.saturating_add(*b)) / prices.len() as u32)
        }
    }

    fn validate_transaction_parameters(
        block_number: &BlockNumberFor<T>,
        new_price: &u32,
    ) -> TransactionValidity {
        let next_unsigned_at = NextUnsignedAt::<T>::get();
        if &next_unsigned_at > block_number {
            return InvalidTransaction::Stale.into();
        }

        let current_block = <system::Pallet<T>>::block_number();
        if &current_block < block_number {
            return InvalidTransaction::Future.into();
        }

        let avg_price = Self::average_price()
            .map(|price| {
                if &price > new_price {
                    price - new_price
                } else {
                    new_price - price
                }
            })
            .unwrap_or(0);

        ValidTransaction::with_tag_prefix("ExampleOffchainWorker")
            .priority(T::UnsignedPriority::get().saturating_add(avg_price as _))
            .and_provides(next_unsigned_at)
            .longevity(5)
            .propagate(true)
            .build()
    }
}

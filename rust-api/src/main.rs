use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use rand::TryRngCore;
use serde::{Deserialize, Serialize};
use sp_core::Decode;
use sp_keyring::sr25519::Keyring as AccountKeyring;
use std::path::Path as stdPath;
use std::str::FromStr;
use subxt::config::substrate::AccountId32;
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::{bip39::Mnemonic, sr25519::Keypair};
use tokio::fs;
use tower_http::cors::CorsLayer;

// Add these constants based on your chain configuration
const MILLI_UNIT: u128 = 1_000_000_000;

// Generate the metadata at compile time
#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
pub mod polkadot {}

#[derive(Serialize, Deserialize)]
struct WalletData {
    pub address: String,
    pub seed_phrase: Mnemonic,
    pub secret_uri: String,
}

// API State
#[derive(Clone)]
pub struct AppState {
    client: OnlineClient<PolkadotConfig>,
    wallet_keypair: Keypair,
}

// Request/Response types
#[derive(Deserialize)]
pub struct TransferRequest {
    pub to: String, // Account address
    pub amount: u128,
}

#[derive(Deserialize)]
pub struct RemarkRequest {
    pub remark: String,
}

#[derive(Deserialize)]
pub struct BatchRequest {
    pub calls: Vec<CallData>,
}

#[derive(Serialize, Deserialize)]
pub struct ListNftRequest {
    pub collection_id: u32,
    pub item_id: u32,
}

#[derive(Serialize, Deserialize)]
pub struct PlaceBidRequest {
    pub collection_id: u32,
    pub item_id: u32,
    pub bid_amount: u128,
}

#[derive(Serialize, Deserialize)]
pub struct AuctionResponse {
    pub tx_hash: String,
}

#[derive(Serialize, Deserialize)]
pub struct AuctionInfo {
    pub owner: String,
    pub start_block: u64,
    pub highest_bid: u128,
    pub highest_bidder: Option<String>,
    pub ended: bool,
}

#[derive(Deserialize)]
pub struct CallData {
    pub pallet: String,
    pub call: String,
    pub args: serde_json::Value,
}

#[derive(Serialize)]
pub struct TransactionResponse {
    pub tx_hash: String,
    pub status: String,
}

#[derive(Serialize)]
pub struct BalanceResponse {
    pub account: String,
    pub free_balance: u128,
    pub reserved_balance: u128,
}

#[derive(Serialize, Debug)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Deserialize)]
pub struct QueryParams {
    pub wait_for_finalization: Option<bool>,
}

#[derive(Serialize)]
pub struct AllAuctionsResponse {
    pub auctions: Vec<AuctionWithKey>,
}

#[derive(Serialize)]
pub struct AuctionWithKey {
    pub collection_id: u32,
    pub item_id: u32,
    pub auction_info: AuctionInfo,
}

impl AppState {
    pub async fn new(
        endpoint: &str,
        wallet_path: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let client = OnlineClient::<PolkadotConfig>::from_url(endpoint).await?;
        let wallet_keypair = get_or_create_wallet(wallet_path, &client).await?;

        Ok(Self {
            client,
            wallet_keypair,
        })
    }
}

// Create a new wallet and save to file
async fn create_and_save_wallet(
    file_path: &str,
    client: &OnlineClient<PolkadotConfig>,
) -> Result<Keypair, Box<dyn std::error::Error>> {
    // Generate a new keypair with random entropy
    let mut entropy = [0u8; 16];
    rand::rng()
        .try_fill_bytes(&mut entropy)
        .map_err(|e| format!("Failed to generate entropy: {}", e))?;
    let mnemonic = Mnemonic::from_entropy(&mut entropy).unwrap();
    let keypair = Keypair::from_phrase(&mnemonic, None).unwrap();

    let wallet_data = WalletData {
        address: keypair.public_key().to_account_id().to_string(),
        seed_phrase: mnemonic.clone(),
        secret_uri: format!("/{}", mnemonic), // Using seed phrase as URI
    };

    // Save wallet to file
    let wallet_json = serde_json::to_string_pretty(&wallet_data)?;
    fs::write(file_path, wallet_json).await?;

    println!("💾 Wallet created and saved to: {}", file_path);
    println!("📍 Address: {}", wallet_data.address);

    // Auto-fund the wallet from Alice (for development)
    let alice_keypair = get_keypair_from_keyring("alice").map_err(|e| {
        format!(
            "Alice keyring not available: {}. Make sure you're running on a dev chain.",
            e
        )
    })?;

    println!("🔑 Alice keypair loaded successfully");

    // Use a reasonable funding amount (100 units = 100 * 1_000_000_000)
    let funding_amount = 100 * MILLI_UNIT; // 100_000_000_000
    let wallet_address = keypair.public_key().to_account_id();

    let transfer_tx = polkadot::tx()
        .balances()
        .transfer_allow_death(wallet_address.into(), funding_amount);

    println!(
        "📤 Submitting funding transaction of {} MILLI_UNITS...",
        funding_amount / MILLI_UNIT
    );

    let tx_progress = client
        .tx()
        .sign_and_submit_then_watch_default(&transfer_tx, &alice_keypair)
        .await
        .map_err(|e| format!("Failed to submit funding transaction: {}", e))?;

    println!("⏳ Waiting for transaction finalization...");

    let _events = tx_progress
        .wait_for_finalized_success()
        .await
        .map_err(|e| format!("Funding transaction failed: {}", e))?;

    println!(
        "✅ Wallet auto-funded with {} MILLI_UNITS",
        funding_amount / MILLI_UNIT
    );

    Ok(keypair)
}

// Load wallet from file
async fn load_wallet_from_file(file_path: &str) -> Result<Keypair, Box<dyn std::error::Error>> {
    let wallet_content = fs::read_to_string(file_path).await?;
    let wallet_data: WalletData = serde_json::from_str(&wallet_content)?;

    // Recreate keypair from seed phrase
    let keypair = Keypair::from_phrase(&wallet_data.seed_phrase, None)?;

    println!("📂 Wallet loaded from: {}", file_path);
    println!("📍 Address: {}", wallet_data.address);

    Ok(keypair)
}

// Get or create wallet
async fn get_or_create_wallet(
    file_path: &str,
    client: &OnlineClient<PolkadotConfig>,
) -> Result<Keypair, Box<dyn std::error::Error>> {
    if stdPath::new(file_path).exists() {
        load_wallet_from_file(file_path).await
    } else {
        create_and_save_wallet(file_path, client).await
    }
}

// Helper function to get Keypair from keyring account name
fn get_keypair_from_keyring(name: &str) -> Result<Keypair, &'static str> {
    let _keyring_account = match name.to_lowercase().as_str() {
        "alice" => AccountKeyring::Alice,
        "bob" => AccountKeyring::Bob,
        "charlie" => AccountKeyring::Charlie,
        "dave" => AccountKeyring::Dave,
        "eve" => AccountKeyring::Eve,
        "ferdie" => AccountKeyring::Ferdie,
        _ => return Err("Invalid keyring account name"),
    };

    // Get the URI for the keyring account
    let uri = match name.to_lowercase().as_str() {
        "alice" => "//Alice",
        "bob" => "//Bob",
        "charlie" => "//Charlie",
        "dave" => "//Dave",
        "eve" => "//Eve",
        "ferdie" => "//Ferdie",
        _ => return Err("Invalid keyring account name"),
    };

    // Create keypair from URI
    let secret_uri = subxt_signer::SecretUri::from_str(uri).map_err(|_| "Failed to parse URI")?;
    let keypair =
        Keypair::from_uri(&secret_uri).map_err(|_| "Failed to create keypair from URI")?;
    Ok(keypair)
}

async fn check_balance(
    client: &OnlineClient<PolkadotConfig>,
    account_id: &AccountId32,
) -> Result<u128, Box<dyn std::error::Error>> {
    let account_info = client
        .storage()
        .at_latest()
        .await?
        .fetch(&polkadot::storage().system().account(account_id))
        .await?;

    match account_info {
        Some(info) => Ok(info.data.free),
        None => Ok(0), // Account doesn't exist = 0 balance
    }
}

// API Endpoints

// POST /api/transfer - Transfer tokens
async fn transfer_tokens(
    State(state): State<AppState>,
    Query(params): Query<QueryParams>,
    Json(payload): Json<TransferRequest>,
) -> Result<Json<TransactionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let dest = subxt::utils::AccountId32::from_str(&payload.to).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    // Before submitting your remark transaction:
    let balance = check_balance(
        &state.client,
        &state.wallet_keypair.public_key().to_account_id(),
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    });

    match balance {
        Ok(amount) => {
            println!("Current balance: {}", amount);

            if amount == 0 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Insufficient funds. Please fund your wallet first.".to_string(),
                    }),
                ));
            }
        }
        Err(e) => {
            return Err(e);
        }
    }

    let transfer_tx = polkadot::tx()
        .balances()
        .transfer_allow_death(dest.into(), payload.amount);

    let hash = state
        .client
        .tx()
        .sign_and_submit_then_watch_default(&transfer_tx, &state.wallet_keypair)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?
        .wait_for_finalized_success()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    let tx_hash = format!("0x{}", hex::encode(hash.extrinsic_hash()));

    Ok(Json(TransactionResponse {
        tx_hash,
        status: if params.wait_for_finalization.unwrap_or(false) {
            "finalized".to_string()
        } else {
            "submitted".to_string()
        },
    }))
}

// POST /api/remark - Create a remark transaction
async fn create_remark(
    State(state): State<AppState>,
    Json(payload): Json<RemarkRequest>,
) -> Result<Json<TransactionResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Before submitting your remark transaction:
    let balance = check_balance(
        &state.client,
        &state.wallet_keypair.public_key().to_account_id(),
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    });

    match balance {
        Ok(amount) => {
            println!("Current balance: {}", amount);

            if amount == 0 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Insufficient funds. Please fund your wallet first.".to_string(),
                    }),
                ));
            }
        }
        Err(e) => {
            return Err(e);
        }
    }

    let remark_tx = polkadot::tx().system().remark(payload.remark.into_bytes());

    let hash = state
        .client
        .tx()
        .sign_and_submit_then_watch_default(&remark_tx, &state.wallet_keypair)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?
        .wait_for_finalized_success()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(Json(TransactionResponse {
        tx_hash: format!("0x{}", hex::encode(hash.extrinsic_hash())),
        status: "submitted".to_string(),
    }))
}

// GET /api/balance/{account} - Get account balance
async fn get_balance(
    State(state): State<AppState>,
    Path(account): Path<String>,
) -> Result<Json<BalanceResponse>, (StatusCode, Json<ErrorResponse>)> {
    let account_id = subxt::utils::AccountId32::from_str(&account).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    let balance_query = polkadot::storage().system().account(&account_id);

    let account_info = state
        .client
        .storage()
        .at_latest()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?
        .fetch(&balance_query)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    let (free_balance, reserved_balance) = if let Some(info) = account_info {
        (info.data.free, info.data.reserved)
    } else {
        (0, 0)
    };

    Ok(Json(BalanceResponse {
        account,
        free_balance,
        reserved_balance,
    }))
}

// GET /api/block/latest - Get latest block info
async fn get_latest_block(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let latest_block = state.client.blocks().at_latest().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(serde_json::json!({
        "block_number": latest_block.number(),
        "block_hash": format!("0x{}", hex::encode(latest_block.hash().0)),
        "parent_hash": format!("0x{}", hex::encode(latest_block.header().parent_hash.0))
    })))
}

// GET /api/accounts - Get predefined keyring accounts
async fn get_accounts() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "accounts": [
            {
                "name": "Alice",
                "address": AccountKeyring::Alice.to_account_id().to_string()
            },
            {
                "name": "Bob",
                "address": AccountKeyring::Bob.to_account_id().to_string()
            },
            {
                "name": "Charlie",
                "address": AccountKeyring::Charlie.to_account_id().to_string()
            },
            {
                "name": "Dave",
                "address": AccountKeyring::Dave.to_account_id().to_string()
            },
            {
                "name": "Eve",
                "address": AccountKeyring::Eve.to_account_id().to_string()
            },
            {
                "name": "Ferdie",
                "address": AccountKeyring::Ferdie.to_account_id().to_string()
            }
        ]
    }))
}

// Health check endpoint
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

// GET /api/wallet - Get wallet information
async fn get_wallet_info(State(state): State<AppState>) -> Json<serde_json::Value> {
    let address = state
        .wallet_keypair
        .public_key()
        .to_account_id()
        .to_string();

    Json(serde_json::json!({
        "address": address,
        "public_key": format!("0x{}", hex::encode(state.wallet_keypair.public_key().0))
    }))
}

// List NFT for auction
pub async fn list_nft_for_auction(
    State(state): State<AppState>,
    Json(payload): Json<ListNftRequest>,
) -> Result<Json<AuctionResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Before submitting your remark transaction:
    let balance = check_balance(
        &state.client,
        &state.wallet_keypair.public_key().to_account_id(),
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    });

    match balance {
        Ok(amount) => {
            println!("Current balance: {}", amount);

            if amount == 0 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Insufficient funds. Please fund your wallet first.".to_string(),
                    }),
                ));
            }
        }
        Err(e) => {
            return Err(e);
        }
    }

    // Create the transaction
    let list_tx = polkadot::tx()
        .template()
        .list_nft_for_auction(payload.collection_id, payload.item_id);

    // Submit transaction
    let tx_progress = state
        .client
        .tx()
        .sign_and_submit_then_watch_default(&list_tx, &state.wallet_keypair)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to submit transaction: {}", e),
                }),
            )
        })?;

    let events = tx_progress
        .wait_for_finalized_success()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Transaction failed: {}", e),
                }),
            )
        })?;

    Ok(Json(AuctionResponse {
        tx_hash: format!("{:?}", events.extrinsic_hash()),
    }))
}

// Place bid on auction
pub async fn place_bid(
    State(state): State<AppState>,
    Json(payload): Json<PlaceBidRequest>,
) -> Result<Json<AuctionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let keypair = get_keypair_from_keyring(&"alice").map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    let bid_tx = polkadot::tx().template().place_bid(
        payload.collection_id,
        payload.item_id,
        payload.bid_amount,
    );

    let tx_progress = state
        .client
        .tx()
        .sign_and_submit_then_watch_default(&bid_tx, &keypair)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to submit bid: {}", e),
                }),
            )
        })?;

    let events = tx_progress
        .wait_for_finalized_success()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Bid transaction failed: {}", e),
                }),
            )
        })?;

    Ok(Json(AuctionResponse {
        tx_hash: format!("{:?}", events.extrinsic_hash()),
    }))
}

// Resolve auction
pub async fn resolve_auction(
    State(state): State<AppState>,
    Path((collection_id, item_id)): Path<(u32, u32)>,
) -> Result<Json<AuctionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let resolve_tx = polkadot::tx()
        .template()
        .resolve_auction(collection_id, item_id);

    let tx_progress = state
        .client
        .tx()
        .sign_and_submit_then_watch_default(&resolve_tx, &state.wallet_keypair)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to resolve auction: {}", e),
                }),
            )
        })?;

    let events = tx_progress
        .wait_for_finalized_success()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Resolve transaction failed: {}", e),
                }),
            )
        })?;

    Ok(Json(AuctionResponse {
        tx_hash: format!("{:?}", events.extrinsic_hash()),
    }))
}

// Get auction info (query storage)
pub async fn get_auction_info(
    State(state): State<AppState>,
    Path((collection_id, item_id)): Path<(u32, u32)>,
) -> Result<Json<Option<AuctionInfo>>, (StatusCode, Json<ErrorResponse>)> {
    let storage_query = polkadot::storage().template().auctions_iter();

    let mut auctions = Vec::new();

    let mut iter = state
        .client
        .storage()
        .at_latest()
        .await
        .map_err(|e| {
            println!("[ERROR] Failed to get latest block: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to query storage: {}", e),
                }),
            )
        })?
        .iter(storage_query)
        .await
        .map_err(|e| {
            println!("[ERROR] Failed to create storage iterator: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to iterate storage: {}", e),
                }),
            )
        })?;

    let mut count = 0;
    while let Some(result) = iter.next().await {
        match result {
            Ok(kv_pair) => {
                count += 1;
                let key_bytes = kv_pair.key_bytes;
                let auction_info = kv_pair.value;

                println!("[INFO] Processing auction #{}", count);
                println!("[DEBUG] Raw key: 0x{}", hex::encode(key_bytes.clone()));

                match decode_auction_key(&key_bytes) {
                    Ok((collection_id, item_id)) => {
                        let auction_with_key = AuctionWithKey {
                            collection_id,
                            item_id,
                            auction_info: AuctionInfo {
                                owner: auction_info.owner.to_string(),
                                start_block: auction_info.start_block as u64,
                                highest_bid: auction_info.highest_bid,
                                highest_bidder: auction_info.highest_bidder.map(|h| h.to_string()),
                                ended: auction_info.ended,
                            },
                        };

                        println!(
                                "[SUCCESS] ✅ Auction {} - Collection: {}, Item: {}, Owner: {}, Highest Bid: {}",
                                count, collection_id, item_id, auction_with_key.auction_info.owner, auction_with_key.auction_info.highest_bid
                            );

                        auctions.push(auction_with_key);
                    }
                    Err(e) => {
                        println!(
                            "[ERROR] ❌ Failed to decode key for auction #{}: {}",
                            count, e
                        );
                        println!("[DEBUG] Key bytes: {:?}", key_bytes);
                    }
                }
            }
            Err(e) => {
                println!("[ERROR] Error iterating auction: {:?}", e);
            }
        }
    }

    println!("[INFO] Total auctions found: {}", auctions.len());

    let result = auctions
        .into_iter()
        .find(|a| a.collection_id == collection_id && a.item_id == item_id);

    Ok(Json(result.map(|a| a.auction_info)))
}

fn decode_auction_key(key: &[u8]) -> Result<(u32, u32), &'static str> {
    // Blake2_128Concat hasher structure:
    // [16 bytes hash] + [original encoded data]

    if key.len() < 16 {
        return Err("Key too short for Blake2_128Concat");
    }

    // Skip 32-byte storage prefix + 16-byte Blake2_128 hash
    let encoded_key = &key[32 + 16..];

    // The original key is a tuple (CollectionId, ItemId) encoded with SCALE codec
    // Assuming CollectionId and ItemId are both u32
    if encoded_key.len() < 8 {
        return Err("Insufficient data for (u32, u32) tuple");
    }

    // Decode the tuple (CollectionId, ItemId)
    match <(u32, u32)>::decode(&mut &encoded_key[..]) {
        Ok((collection_id, item_id)) => Ok((collection_id, item_id)),
        Err(_) => Err("Failed to decode (CollectionId, ItemId) tuple"),
    }
}

// Admin functions (require root/sudo)
pub async fn set_fee_percentage(
    State(state): State<AppState>,
    Path(fee): Path<u8>,
) -> Result<Json<AuctionResponse>, (StatusCode, Json<ErrorResponse>)> {
    if fee > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Fee percentage cannot exceed 100%".to_string(),
            }),
        ));
    }

    let set_fee_tx = polkadot::tx().template().set_fee_percentage(fee);

    let tx_progress = state
        .client
        .tx()
        .sign_and_submit_then_watch_default(&set_fee_tx, &state.wallet_keypair)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to set fee: {}", e),
                }),
            )
        })?;

    let events = tx_progress
        .wait_for_finalized_success()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Set fee transaction failed: {}", e),
                }),
            )
        })?;

    Ok(Json(AuctionResponse {
        tx_hash: format!("{:?}", events.extrinsic_hash()),
    }))
}

pub async fn withdraw_fees(
    State(state): State<AppState>,
    Path(to): Path<String>,
) -> Result<Json<AuctionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let keypair = get_keypair_from_keyring(&"alice").map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    let to_account = AccountId32::from_str(&to).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid account address".to_string(),
            }),
        )
    })?;

    let withdraw_tx = polkadot::tx().template().withdraw_fees(to_account);

    let tx_progress = state
        .client
        .tx()
        .sign_and_submit_then_watch_default(&withdraw_tx, &keypair)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to withdraw fees: {}", e),
                }),
            )
        })?;

    let events = tx_progress
        .wait_for_finalized_success()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Withdraw transaction failed: {}", e),
                }),
            )
        })?;

    Ok(Json(AuctionResponse {
        tx_hash: format!("{:?}", events.extrinsic_hash()),
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Initialize the Substrate client with wallet
    let wallet_path = "wallet.json";
    let app_state = AppState::new("ws://localhost:9944", wallet_path).await?;

    // Build the router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/wallet", get(get_wallet_info))
        .route("/api/accounts", get(get_accounts))
        .route("/api/transfer", post(transfer_tokens))
        .route("/api/remark", post(create_remark))
        .route("/api/balance/{account}", get(get_balance))
        .route("/api/block/latest", get(get_latest_block))
        .route("/api/auction/list", post(list_nft_for_auction))
        .route("/api/auction/bid", post(place_bid))
        .route(
            "/api/auction/resolve/{collection_id}/{item_id}",
            post(resolve_auction),
        )
        .route(
            "/api/auction/info/{collection_id}/{item_id}",
            get(get_auction_info),
        )
        // .route(path("/api/auction/{collection_id}/{item_id}"), get(get_all_auctions(state.clone())))
        .route("/api/auction/set-fee/{fee}", post(set_fee_percentage))
        .route("/api/auction/withdraw-fees/{to}", post(withdraw_fees))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    // Start the server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("🚀 Substrate REST API server running on http://127.0.0.1:3000");
    println!("📖 API Documentation:");
    println!("  GET  /health                     - Health check");
    println!("  GET  /api/accounts               - Get keyring accounts");
    println!("  POST /api/transfer               - Transfer tokens");
    println!("  POST /api/remark                 - Create remark");
    println!("  GET  /api/balance/:account       - Get account balance");
    println!("  GET  /api/block/latest           - Get latest block info");
    println!("  GET  /api/wallet                 - Get wallet info");

    axum::serve(listener, app).await?;

    Ok(())
}

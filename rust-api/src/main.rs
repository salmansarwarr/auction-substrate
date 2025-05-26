use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use rand::TryRngCore;
use serde::{Deserialize, Serialize};
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
const EXISTENTIAL_DEPOSIT: u128 = MILLI_UNIT;

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

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Deserialize)]
pub struct QueryParams {
    pub wait_for_finalization: Option<bool>,
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
    let alice_keypair = get_keypair_from_keyring("alice")
        .map_err(|e| format!("Alice keyring not available: {}. Make sure you're running on a dev chain.", e))?;
    
    println!("🔑 Alice keypair loaded successfully");
    
    // Use a reasonable funding amount (100 units = 100 * 1_000_000_000)
    let funding_amount = 100 * MILLI_UNIT; // 100_000_000_000
    let wallet_address = keypair.public_key().to_account_id();
    
    let transfer_tx = polkadot::tx()
        .balances()
        .transfer_allow_death(wallet_address.into(), funding_amount);
    
    println!("📤 Submitting funding transaction of {} MILLI_UNITS...", funding_amount / MILLI_UNIT);
    
    let tx_progress = client
        .tx()
        .sign_and_submit_then_watch_default(&transfer_tx, &alice_keypair)
        .await
        .map_err(|e| format!("Failed to submit funding transaction: {}", e))?;
    
    println!("⏳ Waiting for transaction finalization...");
    
    let _events = tx_progress.wait_for_finalized_success().await
        .map_err(|e| format!("Funding transaction failed: {}", e))?;
    
    println!("✅ Wallet auto-funded with {} MILLI_UNITS", funding_amount / MILLI_UNIT);
    
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

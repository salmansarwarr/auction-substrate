use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sp_keyring::sr25519::Keyring as AccountKeyring;
use subxt::{OnlineClient, PolkadotConfig};
use tower_http::cors::CorsLayer;
use std::str::FromStr;
use subxt_signer::sr25519::Keypair;

// Generate the metadata at compile time
#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
pub mod polkadot {}

// API State
#[derive(Clone)]
pub struct AppState {
    client: OnlineClient<PolkadotConfig>,
}

// Request/Response types
#[derive(Deserialize)]
pub struct TransferRequest {
    pub from: String,  // "Alice", "Bob", etc.
    pub to: String,    // Account address
    pub amount: u128,
}

#[derive(Deserialize)]
pub struct RemarkRequest {
    pub from: String,  // Keyring account name
    pub remark: String,
}

#[derive(Deserialize)]
pub struct BatchRequest {
    pub from: String,
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
    pub async fn new(endpoint: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = OnlineClient::<PolkadotConfig>::from_url(endpoint).await?;
        Ok(Self { client })
    }
}

// Helper function to get Keypair from keyring account name
fn get_keypair_from_keyring(name: &str) -> Result<Keypair, &'static str> {
    let keyring_account = match name.to_lowercase().as_str() {
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
    let keypair = Keypair::from_uri(&secret_uri).map_err(|_| "Failed to create keypair from URI")?;
    Ok(keypair)
}

// Helper function to get AccountKeyring from string
fn get_keyring_account(name: &str) -> Result<AccountKeyring, &'static str> {
    match name.to_lowercase().as_str() {
        "alice" => Ok(AccountKeyring::Alice),
        "bob" => Ok(AccountKeyring::Bob),
        "charlie" => Ok(AccountKeyring::Charlie),
        "dave" => Ok(AccountKeyring::Dave),
        "eve" => Ok(AccountKeyring::Eve),
        "ferdie" => Ok(AccountKeyring::Ferdie),
        _ => Err("Invalid keyring account name"),
    }
}

// API Endpoints

// POST /api/transfer - Transfer tokens
async fn transfer_tokens(
    State(state): State<AppState>,
    Query(params): Query<QueryParams>,
    Json(payload): Json<TransferRequest>,
) -> Result<Json<TransactionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let keypair = get_keypair_from_keyring(&payload.from)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e.to_string() })))?;

    let dest = subxt::utils::AccountId32::from_str(&payload.to)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e.to_string() })))?;

    let transfer_tx = polkadot::tx().balances().transfer_allow_death(
        dest.into(),
        payload.amount,
    );

    let hash = state
        .client
        .tx()
        .sign_and_submit_then_watch_default(&transfer_tx, &keypair)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?
        .wait_for_finalized_success()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?;

    let tx_hash = format!("0x{}", hex::encode(hash.extrinsic_hash()));

    Ok(Json(TransactionResponse {
        tx_hash,
        status: if params.wait_for_finalization.unwrap_or(false) { "finalized".to_string() } else { "submitted".to_string() },
    }))
}

// POST /api/remark - Create a remark transaction
async fn create_remark(
    State(state): State<AppState>,
    Json(payload): Json<RemarkRequest>,
) -> Result<Json<TransactionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let keypair = get_keypair_from_keyring(&payload.from)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e.to_string() })))?;
    let remark_tx = polkadot::tx().system().remark(payload.remark.into_bytes());

    let hash = state
        .client
        .tx()
        .sign_and_submit_then_watch_default(&remark_tx, &keypair)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?
        .wait_for_finalized_success()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?;

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
    let account_id = subxt::utils::AccountId32::from_str(&account)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e.to_string() })))?;

    let balance_query = polkadot::storage()
        .system()
        .account(&account_id);

    let account_info = state
        .client
        .storage()
        .at_latest()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?
        .fetch(&balance_query)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?;

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
    let latest_block = state
        .client
        .blocks()
        .at_latest()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Initialize the Substrate client
    let app_state = AppState::new("ws://localhost:9944").await?;

    // Build the router
    let app = Router::new()
        .route("/health", get(health_check))
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

    axum::serve(listener, app).await?;

    Ok(())
}
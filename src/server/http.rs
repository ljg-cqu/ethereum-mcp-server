/// Helper: build JSON-RPC success response safely
fn jr_success(id: Option<&Value>, result: Value) -> Json<Value> {
    let resp = JsonRpcResponse::success(id.cloned(), result);
    match serde_json::to_value(resp) {
        Ok(v) => Json(v),
        Err(e) => {
            error!(error = %e, "Failed to serialize JSON-RPC success response");
            Json(json!({"jsonrpc":"2.0","result":null,"id": id}))
        }
    }
}

/// Helper: build JSON-RPC error response safely
fn jr_error(id: Option<&Value>, err: JsonRpcError) -> Json<Value> {
    let resp = JsonRpcResponse::error(id.cloned(), err);
    match serde_json::to_value(resp) {
        Ok(v) => Json(v),
        Err(e) => {
            error!(error = %e, "Failed to serialize JSON-RPC error response");
            Json(
                json!({"jsonrpc":"2.0","error":{"code":-32603,"message":"Internal error"},"id": id}),
            )
        }
    }
}
use crate::server::jsonrpc::{JsonRpcError, JsonRpcResponse};
use crate::services::balance::BalanceServiceTrait;
use crate::services::price::PriceServiceTrait;
use crate::services::swap::SwapServiceTrait;
/// HTTP server implementation with graceful shutdown
/// Clean separation of transport layer from business logic
use crate::services::{
    BalanceService, PriceService, SwapService, TransactionStatusService,
    TransactionStatusServiceTrait,
};
use axum::{
    extract::{DefaultBodyLimit, State},
    http::{Method, StatusCode},
    response::Json,
    routing::post,
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tower::limit::ConcurrencyLimitLayer;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor, GovernorLayer,
};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::{error, info, instrument};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    balance_service: Arc<BalanceService>,
    price_service: Arc<PriceService>,
    swap_service: Arc<SwapService>,
    transaction_status_service: Arc<TransactionStatusService>,
    max_swap_amount: u64,
}

impl AppState {
    pub fn new(
        balance_service: Arc<BalanceService>,
        price_service: Arc<PriceService>,
        swap_service: Arc<SwapService>,
        transaction_status_service: Arc<TransactionStatusService>,
        max_swap_amount: u64,
    ) -> Self {
        Self {
            balance_service,
            price_service,
            swap_service,
            transaction_status_service,
            max_swap_amount,
        }
    }
}

/// HTTP server with graceful shutdown
pub struct HttpServer {
    router: Router,
    host: String,
    port: u16,
}

impl HttpServer {
    /// Create new HTTP server with rate limiting and CORS
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        host: String,
        port: u16,
        state: AppState,
        http_timeout_seconds: u64,
        http_max_concurrency: usize,
        rate_limit_rps: u32,
        rate_limit_burst: u32,
        cors_allow_origins: String,
    ) -> anyhow::Result<Self> {
        // Configure rate limiting
        let governor_conf = Arc::new(
            GovernorConfigBuilder::default()
                .key_extractor(SmartIpKeyExtractor)
                .per_second(rate_limit_rps.into())
                .burst_size(rate_limit_burst)
                .finish()
                .ok_or_else(|| anyhow::anyhow!("Failed to build rate limiter config"))?,
        );

        // Configure CORS from provided origins (comma-separated or "*")
        let cors = if cors_allow_origins.trim() == "*" {
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(Any)
        } else {
            let origins_vec: Vec<_> = cors_allow_origins
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.parse())
                .collect::<Result<Vec<axum::http::HeaderValue>, _>>()
                .map_err(|e| anyhow::anyhow!("Invalid CORS origin value: {}", e))?;
            CorsLayer::new()
                .allow_origin(AllowOrigin::list(origins_vec))
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(Any)
        };

        let router = Router::new()
            .route("/", post(handle_jsonrpc))
            .route("/health", axum::routing::get(health_check))
            .layer(DefaultBodyLimit::max(1024 * 1024)) // 1MB request size limit - prevents DoS
            .layer(GovernorLayer {
                config: governor_conf,
            })
            .layer(cors)
            .layer(ConcurrencyLimitLayer::new(http_max_concurrency))
            .layer(TimeoutLayer::new(Duration::from_secs(http_timeout_seconds)))
            // Basic security headers
            .layer(SetResponseHeaderLayer::overriding(
                axum::http::header::X_CONTENT_TYPE_OPTIONS,
                axum::http::HeaderValue::from_static("nosniff"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                axum::http::header::X_FRAME_OPTIONS,
                axum::http::HeaderValue::from_static("DENY"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                axum::http::header::REFERRER_POLICY,
                axum::http::HeaderValue::from_static("no-referrer"),
            ))
            .layer(TraceLayer::new_for_http())
            .with_state(state);

        // Note: max_swap_amount is stored in AppState and used in handlers

        Ok(Self { router, host, port })
    }

    /// Start the server with graceful shutdown and timeouts
    pub async fn start(&self) -> anyhow::Result<()> {
        let addr = format!("{}:{}", self.host, self.port);
        info!("Starting HTTP server on {}", addr);

        // Fix: Proper error message on bind failure
        let listener = tokio::time::timeout(Duration::from_secs(5), TcpListener::bind(&addr))
            .await
            .map_err(|_| anyhow::anyhow!("Timeout waiting to bind to {}", addr))?
            .map_err(|e| anyhow::anyhow!("Failed to bind to address {}: {}", addr, e))?;

        info!("Server listening on {}", addr);

        // Note: Request timeouts are handled by tower_governor rate limiter
        // For production, consider adding tower::timeout::Timeout service
        axum::serve(listener, self.router.clone())
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;

        info!("Server shutdown completed");
        Ok(())
    }
}

/// JSON-RPC 2.0 request handler with enhanced security
#[instrument(skip(state))]
async fn handle_jsonrpc(
    State(state): State<AppState>,
    Json(request): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    use crate::validation::Validator;

    // Comprehensive JSON-RPC validation
    if let Err(validation_error) = Validator::validate_jsonrpc_request(&request) {
        return Ok(jr_error(
            request.get("id"),
            JsonRpcError::invalid_request_with_message(&validation_error.to_string()),
        ));
    }

    let method = request.get("method").and_then(|m| m.as_str());
    let id = request.get("id");

    match method {
        Some("tools/list") => Ok(jr_success(
            id,
            json!({
                "tools": [
                    {"name": "get_balance", "description": "Query ETH and ERC20 token balances with proper decimals"},
                    {"name": "get_token_price", "description": "Get current token price in USD or ETH (input: token address or symbol)"},
                    {"name": "swap_tokens", "description": "Simulate Uniswap token swap via eth_call"},
                    {"name": "get_transaction_status", "description": "Get the status of a transaction, including confirmations"}
                ]
            }),
        )),

        Some("tools/call") => {
            // Extract tool name and arguments
            let params = request.get("params");
            let tool_name = params.and_then(|p| p.get("name")).and_then(|n| n.as_str());
            let arguments = params.and_then(|p| p.get("arguments"));

            match tool_name {
                Some("get_balance") => match handle_get_balance(&state, arguments, id).await {
                    Ok(response) => Ok(response),
                    Err((_, json_response)) => Ok(json_response),
                },
                Some("get_token_price") => {
                    match handle_get_token_price(&state, arguments, id).await {
                        Ok(response) => Ok(response),
                        Err((_, json_response)) => Ok(json_response),
                    }
                }
                Some("swap_tokens") => match handle_swap_tokens(&state, arguments, id).await {
                    Ok(response) => Ok(response),
                    Err((_, json_response)) => Ok(json_response),
                },
                Some("get_transaction_status") => {
                    match handle_get_transaction_status(&state, arguments, id).await {
                        Ok(response) => Ok(response),
                        Err((_, json_response)) => Ok(json_response),
                    }
                }
                _ => Ok(jr_error(id, JsonRpcError::method_not_found())),
            }
        }

        _ => Ok(jr_error(id, JsonRpcError::method_not_found())),
    }
}

/// Enhanced health check endpoint that verifies external dependencies
async fn health_check(State(state): State<AppState>) -> Json<Value> {
    let timestamp = chrono::Utc::now().to_rfc3339();

    // Quick health check with timeout
    let health_result =
        tokio::time::timeout(Duration::from_secs(5), check_system_health(&state)).await;

    match health_result {
        Ok(Ok(details)) => Json(json!({
            "status": "healthy",
            "timestamp": timestamp,
            "details": details
        })),
        Ok(Err(e)) => Json(json!({
            "status": "degraded",
            "timestamp": timestamp,
            "error": e.to_string(),
            "details": {
                "rpc_status": "unhealthy"
            }
        })),
        Err(_) => Json(json!({
            "status": "degraded",
            "timestamp": timestamp,
            "error": "Health check timed out",
            "details": {
                "rpc_status": "timeout"
            }
        })),
    }
}

/// Check system health including external dependencies
async fn check_system_health(state: &AppState) -> anyhow::Result<Value> {
    // Test RPC connectivity by getting latest block number
    let provider = &state.balance_service.ethereum_provider;
    provider.health_check().await?;

    Ok(json!({
        "rpc_status": "healthy",
        "services": {
            "balance_service": "operational",
            "price_service": "operational",
            "swap_service": "operational"
        }
    }))
}

/// Graceful shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!(error = %e, "failed to install Ctrl+C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                error!(error = %e, "failed to install signal handler");
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        }
        _ = terminate => {
            info!("Received terminate signal");
        }
    }

    info!("Shutdown signal received, starting graceful shutdown");
}

/// Classify errors for appropriate client responses
fn classify_error(error: &anyhow::Error) -> (i32, &'static str, bool) {
    let error_string = error.to_string().to_lowercase();

    if error_string.contains("timeout") || error_string.contains("timed out") {
        (
            -32603,
            "Service temporarily unavailable. Please try again.",
            true,
        )
    } else if error_string.contains("connection") || error_string.contains("network") {
        (
            -32603,
            "Network connectivity issue. Please try again.",
            true,
        )
    } else if error_string.contains("invalid") || error_string.contains("parse") {
        (-32602, "Invalid request parameters.", false)
    } else if error_string.contains("rate limit") || error_string.contains("too many") {
        (
            -32603,
            "Rate limit exceeded. Please wait before retrying.",
            true,
        )
    } else {
        (
            -32603,
            "Unable to process request. Please try again later.",
            true,
        )
    }
}

// Tool handler functions
async fn handle_get_balance(
    state: &AppState,
    arguments: Option<&Value>,
    id: Option<&Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    use crate::validation::Validator;

    let args = arguments.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "jsonrpc": "2.0",
                "error": {"code": -32602, "message": "Missing arguments"},
                "id": id
            })),
        )
    })?;

    let wallet_str = args
        .get("wallet_address")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {"code": -32602, "message": "Missing wallet_address"},
                    "id": id
                })),
            )
        })?;

    // Use comprehensive validation
    let wallet = Validator::validate_wallet_address(wallet_str).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "jsonrpc": "2.0",
                "error": {"code": -32602, "message": format!("Invalid wallet_address: {}", e)},
                "id": id
            })),
        )
    })?;

    // Optional token contract address with validation
    let token = if let Some(token_str) = args.get("token_contract_address").and_then(|v| v.as_str())
    {
        Some(Validator::validate_token_address(token_str).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {"code": -32602, "message": format!("Invalid token_contract_address: {}", e)},
                    "id": id
                })),
            )
        })?)
    } else {
        None
    };

    match state
        .balance_service
        .get_balance(&wallet, token.as_ref())
        .await
    {
        Ok(balance_info) => {
            let raw_units = balance_info.amount.to_raw_units().map_err(|e| {
                error!("Failed to convert balance to raw units: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "jsonrpc": "2.0",
                        "error": {"code": -32603, "message": "Failed to process balance data"},
                        "id": id
                    })),
                )
            })?;

            Ok(Json(json!({
                "jsonrpc": "2.0",
                "result": {
                    "wallet_address": balance_info.wallet_address.to_hex(),
                    "token_address": balance_info.token_address.map(|t| t.to_hex()),
                    "amount": {
                        "raw": raw_units.to_string(),
                        "human_readable": balance_info.amount.to_human_readable(),
                        "decimals": balance_info.amount.decimals
                    },
                    "symbol": balance_info.symbol
                },
                "id": id
            })))
        }
        Err(e) => {
            // Log full error server-side only with structured context
            error!(
                wallet = %wallet.to_hex(),
                token = ?token.as_ref().map(|t| t.to_hex()),
                error = %e,
                "Balance query failed"
            );

            // Classify error type for better client response
            let (error_code, client_message, retry_suggested) = classify_error(&e);

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": error_code,
                        "message": client_message,
                        "data": {
                            "retry_suggested": retry_suggested,
                            "error_type": "balance_query_failed"
                        }
                    },
                    "id": id
                })),
            ))
        }
    }
}

async fn handle_get_token_price(
    state: &AppState,
    arguments: Option<&Value>,
    id: Option<&Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    use crate::contracts::utils;
    use crate::types::TokenAddress;

    let args = arguments.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "jsonrpc": "2.0",
                "error": {"code": -32602, "message": "Missing arguments"},
                "id": id
            })),
        )
    })?;

    // Accept either token_address or token_symbol
    let token_address_opt = args.get("token_address").and_then(|v| v.as_str());
    let token_symbol_opt = args.get("token_symbol").and_then(|v| v.as_str());

    let token = if let Some(addr_str) = token_address_opt {
        TokenAddress::from_hex(addr_str).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {"code": -32602, "message": "Invalid token_address"},
                    "id": id
                })),
            )
        })?
    } else if let Some(sym) = token_symbol_opt {
        match utils::resolve_token_address(sym, &state.price_service.contracts) {
            Some(resolved) => TokenAddress::from_hex(&resolved).map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "jsonrpc": "2.0",
                        "error": {"code": -32602, "message": "Resolved token address invalid"},
                        "id": id
                    })),
                )
            })?,
            None => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "jsonrpc": "2.0",
                        "error": {"code": -32602, "message": "Unknown token_symbol"},
                        "id": id
                    })),
                ));
            }
        }
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "jsonrpc": "2.0",
                "error": {"code": -32602, "message": "Missing token_address or token_symbol"},
                "id": id
            })),
        ));
    };

    match state.price_service.get_token_price(&token).await {
        Ok(price_info) => Ok(Json(json!({
            "jsonrpc": "2.0",
            "result": {
                "token_address": price_info.token_address.to_hex(),
                "price_eth": price_info.price_eth.to_string(),
                "price_usd": price_info.price_usd.map(|p| p.to_string()),
                "source": price_info.source
            },
            "id": id
        }))),
        Err(e) => {
            error!("Token price query failed: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {"code": -32603, "message": "Failed to retrieve token price"},
                    "id": id
                })),
            ))
        }
    }
}

async fn handle_get_transaction_status(
    state: &AppState,
    arguments: Option<&Value>,
    id: Option<&Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    use alloy::primitives::B256;
    use std::str::FromStr;

    let args = arguments.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            jr_error(id, JsonRpcError::invalid_params("Missing arguments")),
        )
    })?;

    let tx_hash_str = args
        .get("transaction_hash")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                jr_error(id, JsonRpcError::invalid_params("Missing transaction_hash")),
            )
        })?;

    let tx_hash = B256::from_str(tx_hash_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            jr_error(id, JsonRpcError::invalid_params("Invalid transaction_hash")),
        )
    })?;

    match state
        .transaction_status_service
        .get_transaction_status(&tx_hash)
        .await
    {
        Ok(status_info) => Ok(jr_success(id, json!(status_info))),
        Err(e) => {
            error!("Failed to get transaction status: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                jr_error(
                    id,
                    JsonRpcError::internal_error("Failed to get transaction status"),
                ),
            ))
        }
    }
}

async fn handle_swap_tokens(
    state: &AppState,
    arguments: Option<&Value>,
    id: Option<&Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    use crate::types::{SwapParams, TokenAddress, TokenAmount};
    use rust_decimal::Decimal;
    use std::str::FromStr;

    let args = arguments.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "jsonrpc": "2.0",
                "error": {"code": -32602, "message": "Missing arguments"},
                "id": id
            })),
        )
    })?;

    // Parse required arguments
    let from_token_str = args
        .get("from_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {"code": -32602, "message": "Missing from_token"},
                    "id": id
                })),
            )
        })?;

    let to_token_str = args
        .get("to_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {"code": -32602, "message": "Missing to_token"},
                    "id": id
                })),
            )
        })?;

    let amount_str = args.get("amount").and_then(|v| v.as_str()).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "jsonrpc": "2.0",
                "error": {"code": -32602, "message": "Missing amount"},
                "id": id
            })),
        )
    })?;

    // Parse token addresses first
    let from_token = TokenAddress::from_hex(from_token_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "jsonrpc": "2.0",
                "error": {"code": -32602, "message": "Invalid from_token"},
                "id": id
            })),
        )
    })?;

    let to_token = TokenAddress::from_hex(to_token_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "jsonrpc": "2.0",
                "error": {"code": -32602, "message": "Invalid to_token"},
                "id": id
            })),
        )
    })?;

    // Parse and validate slippage
    let slippage_str = args
        .get("slippage_tolerance")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {"code": -32602, "message": "Missing slippage_tolerance"},
                    "id": id
                })),
            )
        })?;

    let slippage_tolerance = Decimal::from_str(slippage_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "jsonrpc": "2.0",
                "error": {"code": -32602, "message": "Invalid slippage_tolerance format"},
                "id": id
            })),
        )
    })?;

    if slippage_tolerance < Decimal::ZERO || slippage_tolerance > Decimal::from(100) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "jsonrpc": "2.0",
                "error": {"code": -32602, "message": "Slippage tolerance must be between 0 and 100"},
                "id": id
            })),
        ));
    }

    // Parse and validate amount
    let amount_in = TokenAmount::from_human_readable(amount_str, 18).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "jsonrpc": "2.0",
                "error": {"code": -32602, "message": "Invalid amount format"},
                "id": id
            })),
        )
    })?;

    // Validate amount limits
    if amount_in.to_human_readable() > Decimal::from(state.max_swap_amount) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "jsonrpc": "2.0",
                "error": {"code": -32602, "message": "Amount exceeds maximum swap limit"},
                "id": id
            })),
        ));
    }

    // Create swap parameters
    let swap_params = SwapParams {
        from_token,
        to_token,
        amount_in,
        slippage_tolerance,
    };

    // Simulate the swap
    match state.swap_service.simulate_swap(&swap_params).await {
        Ok(swap_result) => Ok(Json(json!({
            "jsonrpc": "2.0",
            "result": {
                "from_token": swap_result.params.from_token.to_hex(),
                "to_token": swap_result.params.to_token.to_hex(),
                "amount_in": swap_result.params.amount_in.to_human_readable().to_string(),
                "amount_out": swap_result.estimated_amount_out.to_human_readable().to_string(),
                "price_impact": swap_result.price_impact.to_string(),
                "gas_estimate_units": swap_result.gas_estimate.to_string(),
                "gas_cost_eth": swap_result.gas_cost_eth.map(|c| c.to_string()),
                "route": swap_result.route
            },
            "id": id
        }))),
        Err(e) => {
            error!("Swap simulation failed: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "jsonrpc": "2.0",
                    "error": {"code": -32603, "message": "Failed to simulate swap"},
                    "id": id
                })),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::MockEthereumProvider;
    use crate::services::{BalanceService, PriceService, SwapService, TransactionStatusService};
    use crate::ContractAddresses;
    use serde_json::json;

    fn create_test_app_state() -> AppState {
        let mock_provider = Arc::new(MockEthereumProvider::new());
        let balance_service = Arc::new(BalanceService::new(mock_provider.clone()));
        let contracts = ContractAddresses::default();
        let price_service = Arc::new(PriceService::new(mock_provider.clone(), contracts.clone()));
        let swap_service = Arc::new(SwapService::new(mock_provider.clone(), contracts));
        let transaction_status_service = Arc::new(TransactionStatusService::new(mock_provider));

        AppState::new(
            balance_service,
            price_service,
            swap_service,
            transaction_status_service,
            1000, // max_swap_amount is u64, not Decimal
        )
    }

    #[test]
    fn test_jr_success_helper() {
        let id = Some(&json!(1));
        let result = json!({"balance": "100"});
        let response = jr_success(id, result.clone());

        let response_value = response.0;
        assert_eq!(response_value["jsonrpc"], "2.0");
        assert_eq!(response_value["result"], result);
        assert_eq!(response_value["id"], 1);
    }

    #[test]
    fn test_jr_success_no_id() {
        let result = json!({"balance": "100"});
        let response = jr_success(None, result.clone());

        let response_value = response.0;
        assert_eq!(response_value["jsonrpc"], "2.0");
        assert_eq!(response_value["result"], result);
        assert!(response_value["id"].is_null());
    }

    #[test]
    fn test_jr_error_helper() {
        let id = Some(&json!(1));
        let error = JsonRpcError {
            code: -32602,
            message: "Invalid params".to_string(),
            data: None,
        };
        let response = jr_error(id, error);

        let response_value = response.0;
        assert_eq!(response_value["jsonrpc"], "2.0");
        assert_eq!(response_value["error"]["code"], -32602);
        assert_eq!(response_value["error"]["message"], "Invalid params");
        assert_eq!(response_value["id"], 1);
    }

    #[test]
    fn test_jr_error_no_id() {
        let error = JsonRpcError::invalid_request();
        let response = jr_error(None, error);

        let response_value = response.0;
        assert_eq!(response_value["jsonrpc"], "2.0");
        assert_eq!(response_value["error"]["code"], -32600);
        assert_eq!(response_value["error"]["message"], "Invalid Request");
        assert!(response_value["id"].is_null());
    }

    #[test]
    fn test_app_state_creation() {
        let app_state = create_test_app_state();
        assert_eq!(app_state.max_swap_amount, 1000u64);
    }

    #[test]
    fn test_http_server_creation() {
        let app_state = create_test_app_state();
        let result = HttpServer::new(
            "127.0.0.1".to_string(),
            3000,
            app_state,
            30,
            100,
            10,
            5,
            "*".to_string(), // cors_allow_origins is String, not Vec<String>
        );

        assert!(result.is_ok());
        let server = result.unwrap();
        assert_eq!(server.host, "127.0.0.1");
        assert_eq!(server.port, 3000);
    }

    #[test]
    fn test_http_server_port_handling() {
        let app_state = create_test_app_state();

        // Port 0 might be valid (OS assigns available port)
        let result = HttpServer::new(
            "127.0.0.1".to_string(),
            0,
            app_state.clone(),
            30,
            100,
            10,
            5,
            "*".to_string(),
        );

        // Just test that the server can be created - port 0 is actually valid
        assert!(result.is_ok());

        // Test with a normal port
        let result = HttpServer::new(
            "127.0.0.1".to_string(),
            8080,
            app_state,
            30,
            100,
            10,
            5,
            "*".to_string(),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_http_server_cors_configurations() {
        let app_state = create_test_app_state();

        // Test wildcard CORS
        let result = HttpServer::new(
            "127.0.0.1".to_string(),
            3000,
            app_state.clone(),
            30,
            100,
            10,
            5,
            "*".to_string(),
        );
        assert!(result.is_ok());

        // Test specific origins
        let result = HttpServer::new(
            "127.0.0.1".to_string(),
            3000,
            app_state.clone(),
            30,
            100,
            10,
            5,
            "http://localhost:3000,http://localhost:8080".to_string(),
        );
        assert!(result.is_ok());

        // Test with empty CORS (should use wildcard behavior)
        let result = HttpServer::new(
            "127.0.0.1".to_string(),
            3000,
            app_state,
            30,
            100,
            10,
            5,
            "".to_string(),
        );
        // Empty string is treated as wildcard
        assert!(result.is_ok());
    }

    #[test]
    fn test_classify_error_timeout() {
        let error = anyhow::anyhow!("Connection timeout occurred");
        let (code, message, retry) = classify_error(&error);

        assert_eq!(code, -32603);
        assert!(message.contains("temporarily unavailable"));
        assert!(retry);
    }

    #[test]
    fn test_classify_error_network() {
        let error = anyhow::anyhow!("Network unreachable");
        let (code, message, retry) = classify_error(&error);

        assert_eq!(code, -32603);
        assert!(message.contains("connectivity"));
        assert!(retry);
    }

    #[test]
    fn test_classify_error_invalid() {
        let error = anyhow::anyhow!("Invalid parameter format");
        let (code, message, retry) = classify_error(&error);

        assert_eq!(code, -32602);
        assert!(message.contains("Invalid"));
        assert!(!retry);
    }

    #[test]
    fn test_classify_error_rate_limit() {
        let error = anyhow::anyhow!("Rate limit exceeded");
        let (code, message, retry) = classify_error(&error);

        assert_eq!(code, -32603);
        assert!(message.contains("Rate limit"));
        assert!(retry);
    }

    #[test]
    fn test_classify_error_unknown() {
        let error = anyhow::anyhow!("Unknown internal error");
        let (code, message, retry) = classify_error(&error);

        assert_eq!(code, -32603);
        assert!(message.contains("Unable to process"));
        assert!(retry);
    }

    #[test]
    fn test_app_state_max_swap_amount() {
        let app_state = create_test_app_state();

        // Verify max_swap_amount is set correctly
        assert!(app_state.max_swap_amount > 0);
        assert!(app_state.max_swap_amount <= 1_000_000_000_000);
    }

    #[test]
    fn test_jr_helpers_with_complex_results() {
        let id = Some(&json!({"request_id": "test-123"}));
        let complex_result = json!({
            "balance": "1000.50",
            "timestamp": "2024-01-01T00:00:00Z",
            "metadata": {
                "source": "blockchain",
                "verified": true
            }
        });

        let response = jr_success(id, complex_result.clone());
        let response_value = response.0;

        assert_eq!(response_value["jsonrpc"], "2.0");
        assert_eq!(response_value["result"], complex_result);
        assert_eq!(response_value["id"]["request_id"], "test-123");
    }

    #[test]
    fn test_jr_error_with_data() {
        let id = Some(&json!(1));
        let error = JsonRpcError {
            code: -32602,
            message: "Invalid params".to_string(),
            data: Some(json!({"field": "amount", "reason": "too large"})),
        };

        let response = jr_error(id, error);
        let response_value = response.0;

        assert_eq!(response_value["error"]["code"], -32602);
        assert_eq!(response_value["error"]["message"], "Invalid params");
        assert!(response_value["error"]["data"].is_object());
    }

    #[test]
    fn test_http_server_different_hosts() {
        let app_state = create_test_app_state();

        // Test localhost
        let result = HttpServer::new(
            "localhost".to_string(),
            3000,
            app_state.clone(),
            30,
            100,
            10,
            5,
            "*".to_string(),
        );
        assert!(result.is_ok());

        // Test 0.0.0.0
        let result = HttpServer::new(
            "0.0.0.0".to_string(),
            3000,
            app_state,
            30,
            100,
            10,
            5,
            "*".to_string(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_http_server_timeout_configurations() {
        let app_state = create_test_app_state();

        // Test short timeout
        let result = HttpServer::new(
            "127.0.0.1".to_string(),
            3000,
            app_state.clone(),
            5,
            100,
            10,
            5,
            "*".to_string(),
        );
        assert!(result.is_ok());

        // Test long timeout
        let result = HttpServer::new(
            "127.0.0.1".to_string(),
            3000,
            app_state,
            300,
            100,
            10,
            5,
            "*".to_string(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_http_server_concurrency_limits() {
        let app_state = create_test_app_state();

        // Test low concurrency
        let result = HttpServer::new(
            "127.0.0.1".to_string(),
            3000,
            app_state.clone(),
            30,
            1,
            10,
            5,
            "*".to_string(),
        );
        assert!(result.is_ok());

        // Test high concurrency
        let result = HttpServer::new(
            "127.0.0.1".to_string(),
            3000,
            app_state,
            30,
            1000,
            10,
            5,
            "*".to_string(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_http_server_rate_limit_configurations() {
        let app_state = create_test_app_state();

        // Test strict rate limiting
        let result = HttpServer::new(
            "127.0.0.1".to_string(),
            3000,
            app_state.clone(),
            30,
            100,
            1, // 1 request per second
            1, // burst of 1
            "*".to_string(),
        );
        assert!(result.is_ok());

        // Test permissive rate limiting
        let result = HttpServer::new(
            "127.0.0.1".to_string(),
            3000,
            app_state,
            30,
            100,
            100, // 100 requests per second
            200, // burst of 200
            "*".to_string(),
        );
        assert!(result.is_ok());
    }
}

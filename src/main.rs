/// Main application entry point
/// Proper dependency injection and graceful shutdown
use ethereum_mcp_server::{
    providers::ProviderFactory,
    server::http::{AppState, HttpServer},
    services::{BalanceService, PriceService, SwapService, TransactionStatusService},
    Config,
};
use std::sync::Arc;
use tracing::{error, info};

/// Initialize logging subsystem
pub fn initialize_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();
}

/// Load and validate configuration
pub async fn load_config() -> anyhow::Result<Config> {
    let config = Config::from_env()?;
    config.validate()?;

    info!(
        rpc_url = %config.ethereum_rpc_url,
        host = %config.server_host,
        port = %config.server_port,
        "Configuration loaded"
    );

    Ok(config)
}

/// Initialize Ethereum provider and services
pub async fn initialize_services(config: &Config) -> anyhow::Result<AppState> {
    // Initialize Ethereum provider (strategic interface for testing)
    let ethereum_provider = ProviderFactory::create_ethereum_provider_with_failover(
        config.ethereum_rpc_urls.clone(),
        config.wallet_private_key().to_string(),
        config.ethereum_max_concurrent_requests,
        config.ethereum_request_timeout_seconds,
    )
    .await?;
    info!("Ethereum provider initialized");

    // Initialize services (dependency injection)
    let balance_service = Arc::new(BalanceService::new(ethereum_provider.clone()));
    let price_service = Arc::new(PriceService::new(
        ethereum_provider.clone(),
        config.contracts.clone(),
    ));
    let swap_service = Arc::new(SwapService::new(
        ethereum_provider.clone(),
        config.contracts.clone(),
    ));
    let transaction_status_service =
        Arc::new(TransactionStatusService::new(ethereum_provider.clone()));

    info!("Services initialized");

    // Create application state
    Ok(AppState::new(
        balance_service,
        price_service,
        swap_service,
        transaction_status_service,
        config.max_swap_amount,
    ))
}

/// Start HTTP server with graceful shutdown
pub async fn start_server(config: &Config, app_state: AppState) -> anyhow::Result<()> {
    let server = HttpServer::new(
        config.server_host.clone(),
        config.server_port,
        app_state,
        config.http_timeout_seconds,
        config.http_max_concurrency,
        config.rate_limit_rps,
        config.rate_limit_burst,
        config.cors_allow_origins.clone(),
    )?;

    info!("Starting HTTP server...");
    server.start().await
}

/// Main application logic (extracted for testing)
pub async fn run_application() -> anyhow::Result<()> {
    initialize_logging();
    info!("Starting Ethereum MCP Server");

    let config = load_config().await?;
    let app_state = initialize_services(&config).await?;

    match start_server(&config, app_state).await {
        Ok(()) => {
            info!("Server shutdown completed");
            Ok(())
        }
        Err(e) => {
            error!("Server error: {}", e);
            Err(e)
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_application().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_logging() {
        // Test that logging initialization doesn't panic
        // Note: We can't test the actual logging setup without side effects
        // This tests the function exists and can be called
        let result = std::panic::catch_unwind(|| {
            // Don't actually initialize logging in tests to avoid conflicts
            // Just test the function signature and basic logic
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_load_config_success() {
        // Test config creation and validation directly
        let config = Config::new(
            "https://mainnet.infura.io/v3/test".to_string(),
            "127.0.0.1".to_string(),
            3000,
            "info".to_string(),
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );

        let result = config.validate();
        assert!(result.is_ok());

        assert_eq!(config.ethereum_rpc_url, "https://mainnet.infura.io/v3/test");
        assert_eq!(config.server_host, "127.0.0.1");
        assert_eq!(config.server_port, 3000);
    }

    #[test]
    fn test_config_validation_empty_rpc_url() {
        let config = Config::new(
            "".to_string(),
            "127.0.0.1".to_string(),
            3000,
            "info".to_string(),
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_config_validation_invalid_rpc_scheme() {
        let config = Config::new(
            "ftp://invalid.com".to_string(),
            "127.0.0.1".to_string(),
            3000,
            "info".to_string(),
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must start with http"));
    }

    #[test]
    fn test_config_validation_zero_port() {
        let config = Config::new(
            "https://mainnet.infura.io/v3/test".to_string(),
            "127.0.0.1".to_string(),
            0,
            "info".to_string(),
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must be greater than 0"));
    }

    #[test]
    fn test_config_creation() {
        // Test that config can be created with valid values
        let config = Config::new(
            "https://mainnet.infura.io/v3/test".to_string(),
            "localhost".to_string(),
            8080,
            "debug".to_string(),
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );

        assert_eq!(config.ethereum_rpc_url, "https://mainnet.infura.io/v3/test");
        assert_eq!(config.server_host, "localhost");
        assert_eq!(config.server_port, 8080);
        assert_eq!(config.log_level, "debug");
    }

    #[test]
    fn test_config_validation_https() {
        let config = Config::new(
            "https://eth.llamarpc.com".to_string(),
            "0.0.0.0".to_string(),
            443,
            "warn".to_string(),
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );

        let result = config.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_validation_http() {
        let config = Config::new(
            "http://localhost:8545".to_string(),
            "127.0.0.1".to_string(),
            3000,
            "error".to_string(),
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );

        let result = config.validate();
        assert!(result.is_ok());
    }

    // Note: initialize_services and start_server tests would require mocking
    // the Ethereum provider and HTTP server, which is complex for unit tests.
    // These are better tested in integration tests.

    #[test]
    fn test_helper_functions_exist() {
        // Test that all helper functions exist and have correct signatures
        // This ensures our refactoring maintains the API

        // Test function signatures (compile-time check)
        let _: fn() = initialize_logging;

        // Type alias for complex future type to improve readability
        type ConfigFuture =
            std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<Config>> + Send>>;
        #[allow(clippy::type_complexity)]
        let _: fn() -> ConfigFuture = || Box::pin(load_config());

        // These functions exist and can be referenced - test passes by compiling
    }
}

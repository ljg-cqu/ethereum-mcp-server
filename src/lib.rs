/// Ethereum MCP Server Library
/// Clean public API following SOLID principles
use std::fmt;

pub mod contracts;
pub mod providers;
pub mod server;
pub mod services;
pub mod types;
pub mod validation;

// Re-export key types for public API
pub use providers::{EthereumProvider, ProviderFactory};
pub use types::{
    BalanceInfo, SwapParams, SwapResult, TokenAddress, TokenAmount, TokenPrice, WalletAddress,
};
/// Holds all configurable contract addresses
#[derive(Clone, Debug)]
pub struct ContractAddresses {
    pub usdc: String,
    pub usdt: String,
    pub dai: String,
    pub weth: String,
    pub uniswap_v3_factory: String,
    pub uniswap_v3_router: String,
    pub uniswap_v3_quoter: String,
    pub chainlink_eth_usd_feed: String,
}

impl Default for ContractAddresses {
    fn default() -> Self {
        Self {
            usdc: "0xA0b86a33E6441E4c5f1A8e9B5e8d5c5d5e5f5g5h".to_string(),
            usdt: "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(),
            dai: "0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string(),
            weth: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
            uniswap_v3_factory: "0x1F98431c8aD98523631AE4a59f267346ea31F984".to_string(),
            uniswap_v3_router: "0xE592427A0AEce92De3Edee1F18E0157C05861564".to_string(),
            uniswap_v3_quoter: "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6".to_string(),
            chainlink_eth_usd_feed: "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419".to_string(),
        }
    }
}

/// Library configuration
#[derive(Clone)]
pub struct Config {
    pub ethereum_rpc_url: String,
    pub ethereum_rpc_urls: Vec<String>,
    pub server_host: String,
    pub server_port: u16,
    pub log_level: String,
    wallet_private_key: String, // Private to prevent accidental exposure
    // HTTP and rate limiting config
    pub http_timeout_seconds: u64,
    pub http_max_concurrency: usize,
    pub rate_limit_rps: u32,
    pub rate_limit_burst: u32,
    pub cors_allow_origins: String,
    // Trading limits
    pub max_swap_amount: u64,
    // Network configuration
    pub ethereum_request_timeout_seconds: u64,
    pub ethereum_max_concurrent_requests: usize,
    // Contract addresses
    pub contracts: ContractAddresses,
}

// Custom Debug implementation that redacts sensitive information
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("ethereum_rpc_url", &self.ethereum_rpc_url)
            .field("server_host", &self.server_host)
            .field("server_port", &self.server_port)
            .field("log_level", &self.log_level)
            .field("wallet_private_key", &"[REDACTED]")
            .field("contracts", &self.contracts)
            .finish()
    }
}

impl Config {
    /// Create a new Config instance (for testing)
    pub fn new(
        ethereum_rpc_url: String,
        server_host: String,
        server_port: u16,
        log_level: String,
        wallet_private_key: String,
    ) -> Self {
        Self {
            ethereum_rpc_url: ethereum_rpc_url.clone(),
            ethereum_rpc_urls: vec![ethereum_rpc_url],
            server_host,
            server_port,
            log_level,
            wallet_private_key,
            http_timeout_seconds: 15,
            http_max_concurrency: 100,
            rate_limit_rps: 2,
            rate_limit_burst: 10,
            cors_allow_origins: "*".to_string(),
            max_swap_amount: 1_000_000_000, // 1B tokens default
            ethereum_request_timeout_seconds: 30,
            ethereum_max_concurrent_requests: 10,
            contracts: ContractAddresses {
                usdc: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
                usdt: "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(),
                dai: "0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string(),
                weth: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                uniswap_v3_factory: "0x1F98431c8aD98523631AE4a59f267346ea31F984".to_string(),
                uniswap_v3_router: "0xE592427A0AEce92De3Edee1F18E0157C05861564".to_string(),
                uniswap_v3_quoter: "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6".to_string(),
                chainlink_eth_usd_feed: "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419".to_string(),
            },
        }
    }

    /// Get the wallet private key (accessor method for private field)
    pub fn wallet_private_key(&self) -> &str {
        &self.wallet_private_key
    }

    /// Create configuration from environment variables
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok(); // Load .env file if present

        // Support multiple RPC URLs via ETHEREUM_RPC_URLS (CSV). Fallback to single ETHEREUM_RPC_URL.
        let ethereum_rpc_urls: Vec<String> = if let Ok(list) = std::env::var("ETHEREUM_RPC_URLS") {
            list.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            vec![std::env::var("ETHEREUM_RPC_URL").map_err(|_| {
                anyhow::anyhow!(
                    "ETHEREUM_RPC_URL or ETHEREUM_RPC_URLS environment variable is required"
                )
            })?]
        };
        let ethereum_rpc_url = ethereum_rpc_urls[0].clone();

        let server_host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let server_port = std::env::var("SERVER_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid SERVER_PORT value"))?;

        let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        let wallet_private_key = std::env::var("WALLET_PRIVATE_KEY")
            .map_err(|_| anyhow::anyhow!("WALLET_PRIVATE_KEY environment variable is required"))?;

        let http_timeout_seconds = std::env::var("HTTP_TIMEOUT_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(15);

        let http_max_concurrency = std::env::var("HTTP_MAX_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(100);

        let rate_limit_rps = std::env::var("RATE_LIMIT_RPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(2);

        let rate_limit_burst = std::env::var("RATE_LIMIT_BURST")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(10);

        let cors_allow_origins =
            std::env::var("CORS_ALLOW_ORIGINS").unwrap_or_else(|_| "*".to_string());

        let max_swap_amount = std::env::var("MAX_SWAP_AMOUNT")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(1_000_000_000);

        let ethereum_request_timeout_seconds = std::env::var("ETHEREUM_REQUEST_TIMEOUT_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(30);

        let ethereum_max_concurrent_requests = std::env::var("ETHEREUM_MAX_CONCURRENT_REQUESTS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(10);

        let contracts = ContractAddresses {
            usdc: std::env::var("USDC_ADDRESS")
                .unwrap_or_else(|_| "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string()),
            usdt: std::env::var("USDT_ADDRESS")
                .unwrap_or_else(|_| "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string()),
            dai: std::env::var("DAI_ADDRESS")
                .unwrap_or_else(|_| "0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string()),
            weth: std::env::var("WETH_ADDRESS")
                .unwrap_or_else(|_| "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string()),
            uniswap_v3_factory: std::env::var("UNISWAP_V3_FACTORY")
                .unwrap_or_else(|_| "0x1F98431c8aD98523631AE4a59f267346ea31F984".to_string()),
            uniswap_v3_router: std::env::var("UNISWAP_V3_ROUTER")
                .unwrap_or_else(|_| "0xE592427A0AEce92De3Edee1F18E0157C05861564".to_string()),
            uniswap_v3_quoter: std::env::var("UNISWAP_V3_QUOTER")
                .unwrap_or_else(|_| "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6".to_string()),
            chainlink_eth_usd_feed: std::env::var("CHAINLINK_ETH_USD_FEED")
                .unwrap_or_else(|_| "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419".to_string()),
        };

        Ok(Self {
            ethereum_rpc_url,
            ethereum_rpc_urls,
            server_host,
            server_port,
            log_level,
            wallet_private_key,
            http_timeout_seconds,
            http_max_concurrency,
            rate_limit_rps,
            rate_limit_burst,
            cors_allow_origins,
            max_swap_amount,
            ethereum_request_timeout_seconds,
            ethereum_max_concurrent_requests,
            contracts,
        })
    }

    /// Validate configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.ethereum_rpc_url.is_empty() {
            return Err(anyhow::anyhow!("Ethereum RPC URL cannot be empty"));
        }

        if !self.ethereum_rpc_url.starts_with("http") && !self.ethereum_rpc_url.starts_with("ws") {
            return Err(anyhow::anyhow!(
                "Ethereum RPC URL must start with http, https, ws, or wss"
            ));
        }
        if self.ethereum_rpc_urls.is_empty() {
            return Err(anyhow::anyhow!(
                "At least one Ethereum RPC URL must be provided"
            ));
        }
        if let Some(bad) = self
            .ethereum_rpc_urls
            .iter()
            .find(|u| !u.starts_with("http") && !u.starts_with("ws"))
        {
            return Err(anyhow::anyhow!(format!(
                "Invalid RPC URL (must start with http/https or ws/wss): {}",
                bad
            )));
        }

        if self.server_port == 0 {
            return Err(anyhow::anyhow!("Server port must be greater than 0"));
        }

        if self.wallet_private_key.is_empty() {
            return Err(anyhow::anyhow!("Wallet private key cannot be empty"));
        }

        let normalized = self.wallet_private_key.trim_start_matches("0x");
        if normalized.len() != 64 || !normalized.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(anyhow::anyhow!(
                "Wallet private key must be a 64-character hex string (optionally prefixed with 0x)"
            ));
        }

        // HTTP configs
        if self.http_timeout_seconds == 0 || self.http_timeout_seconds > 300 {
            return Err(anyhow::anyhow!(
                "HTTP timeout must be between 1 and 300 seconds"
            ));
        }
        if self.http_max_concurrency == 0 || self.http_max_concurrency > 10_000 {
            return Err(anyhow::anyhow!(
                "HTTP max concurrency must be between 1 and 10000"
            ));
        }
        if self.rate_limit_rps == 0 || self.rate_limit_rps > 10_000 {
            return Err(anyhow::anyhow!(
                "RATE_LIMIT_RPS must be between 1 and 10000"
            ));
        }
        if self.rate_limit_burst == 0 || self.rate_limit_burst > 10_000 {
            return Err(anyhow::anyhow!(
                "RATE_LIMIT_BURST must be between 1 and 10000"
            ));
        }

        // CORS origins basic validation (non-empty)
        if self.cors_allow_origins.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "CORS_ALLOW_ORIGINS cannot be empty (use * or CSV list)"
            ));
        }

        // Trading limits validation
        if self.max_swap_amount == 0 {
            return Err(anyhow::anyhow!("MAX_SWAP_AMOUNT must be greater than 0"));
        }

        // Network configuration validation
        if self.ethereum_request_timeout_seconds == 0 || self.ethereum_request_timeout_seconds > 300
        {
            return Err(anyhow::anyhow!(
                "ETHEREUM_REQUEST_TIMEOUT_SECONDS must be between 1 and 300"
            ));
        }
        if self.ethereum_max_concurrent_requests == 0 || self.ethereum_max_concurrent_requests > 100
        {
            return Err(anyhow::anyhow!(
                "ETHEREUM_MAX_CONCURRENT_REQUESTS must be between 1 and 100"
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_config_validation_valid() {
        let config = Config::new(
            "https://mainnet.infura.io/v3/test".to_string(),
            "127.0.0.1".to_string(),
            3000,
            "info".to_string(),
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );

        assert!(config.validate().is_ok());
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

        assert!(config.validate().is_err());
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

        assert!(config.validate().is_err());
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

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_from_env_functionality() {
        // Test that Config::from_env exists and can be called
        // Note: This test validates that the function exists and works correctly
        // when environment variables are properly set

        // Save original environment state
        let original_rpc = env::var("ETHEREUM_RPC_URL").ok();
        let original_host = env::var("SERVER_HOST").ok();
        let original_port = env::var("SERVER_PORT").ok();
        let original_log = env::var("RUST_LOG").ok();
        let original_wallet = env::var("WALLET_PRIVATE_KEY").ok();

        // Test with valid complete environment
        env::set_var("ETHEREUM_RPC_URL", "https://mainnet.infura.io/v3/test123");
        env::set_var("SERVER_HOST", "0.0.0.0");
        env::set_var("SERVER_PORT", "8080");
        env::set_var("RUST_LOG", "debug");
        env::set_var(
            "WALLET_PRIVATE_KEY",
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );

        let config_result = Config::from_env();
        assert!(
            config_result.is_ok(),
            "Config::from_env should succeed with valid environment"
        );

        if let Ok(config) = config_result {
            assert_eq!(
                config.ethereum_rpc_url,
                "https://mainnet.infura.io/v3/test123"
            );
            assert_eq!(config.server_host, "0.0.0.0");
            assert_eq!(config.server_port, 8080);
            assert_eq!(config.log_level, "debug");
            assert_eq!(
                config.wallet_private_key,
                "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            );
        }

        // Note: Testing failure cases is complex due to dotenvy loading .env files
        // The main test here is that from_env() works with valid environment

        // Test with invalid port
        env::set_var("ETHEREUM_RPC_URL", "https://mainnet.infura.io/v3/test");
        env::set_var("SERVER_PORT", "invalid_port");
        env::set_var(
            "WALLET_PRIVATE_KEY",
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );
        let result = Config::from_env();
        assert!(
            result.is_err(),
            "Config::from_env should fail with invalid port"
        );

        // Restore original environment
        match original_rpc {
            Some(val) => env::set_var("ETHEREUM_RPC_URL", val),
            None => env::remove_var("ETHEREUM_RPC_URL"),
        }
        match original_host {
            Some(val) => env::set_var("SERVER_HOST", val),
            None => env::remove_var("SERVER_HOST"),
        }
        match original_port {
            Some(val) => env::set_var("SERVER_PORT", val),
            None => env::remove_var("SERVER_PORT"),
        }
        match original_log {
            Some(val) => env::set_var("RUST_LOG", val),
            None => env::remove_var("RUST_LOG"),
        }
        match original_wallet {
            Some(val) => env::set_var("WALLET_PRIVATE_KEY", val),
            None => env::remove_var("WALLET_PRIVATE_KEY"),
        }
    }

    #[test]
    fn test_config_validation_https_url() {
        let config = Config::new(
            "https://eth.llamarpc.com".to_string(),
            "localhost".to_string(),
            443,
            "warn".to_string(),
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_http_url() {
        let config = Config::new(
            "http://localhost:8545".to_string(),
            "127.0.0.1".to_string(),
            8545,
            "error".to_string(),
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_debug_format() {
        let config = Config::new(
            "https://mainnet.infura.io/v3/test".to_string(),
            "127.0.0.1".to_string(),
            3000,
            "info".to_string(),
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );

        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("ethereum_rpc_url"));
        assert!(debug_str.contains("server_host"));
        assert!(debug_str.contains("server_port"));
        assert!(debug_str.contains("log_level"));
        // Verify private key is REDACTED, not exposed
        assert!(debug_str.contains("[REDACTED]"));
        assert!(!debug_str.contains("0x0123456789"));

        // Verify accessor method works
        assert_eq!(
            config.wallet_private_key(),
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
    }

    #[test]
    fn test_config_clone() {
        let config = Config::new(
            "https://mainnet.infura.io/v3/test".to_string(),
            "127.0.0.1".to_string(),
            3000,
            "info".to_string(),
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );

        let cloned = config.clone();
        assert_eq!(config.ethereum_rpc_url, cloned.ethereum_rpc_url);
        assert_eq!(config.server_host, cloned.server_host);
        assert_eq!(config.server_port, cloned.server_port);
        assert_eq!(config.log_level, cloned.log_level);
        assert_eq!(config.wallet_private_key, cloned.wallet_private_key);
    }

    #[test]
    fn test_config_validation_edge_cases() {
        // Test with maximum valid port
        let config = Config::new(
            "https://mainnet.infura.io/v3/test".to_string(),
            "127.0.0.1".to_string(),
            65535,
            "info".to_string(),
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );
        assert!(config.validate().is_ok());

        // Test with minimum valid port
        let config = Config::new(
            "https://mainnet.infura.io/v3/test".to_string(),
            "127.0.0.1".to_string(),
            1,
            "info".to_string(),
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_private_key_formats() {
        // Test valid format with 0x prefix
        let config = Config::new(
            "https://mainnet.infura.io/v3/test".to_string(),
            "127.0.0.1".to_string(),
            3000,
            "info".to_string(),
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );
        assert!(config.validate().is_ok());

        // Test valid format without 0x prefix
        let config = Config::new(
            "https://mainnet.infura.io/v3/test".to_string(),
            "127.0.0.1".to_string(),
            3000,
            "info".to_string(),
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );
        assert!(config.validate().is_ok());

        // Test invalid short key
        let config = Config::new(
            "https://mainnet.infura.io/v3/test".to_string(),
            "127.0.0.1".to_string(),
            3000,
            "info".to_string(),
            "0x123".to_string(),
        );
        assert!(config.validate().is_err());

        // Test empty key
        let config = Config::new(
            "https://mainnet.infura.io/v3/test".to_string(),
            "127.0.0.1".to_string(),
            3000,
            "info".to_string(),
            "".to_string(),
        );
        assert!(config.validate().is_err());
    }
}

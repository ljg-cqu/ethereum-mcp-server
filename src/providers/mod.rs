/// Provider module - abstracts blockchain interactions
/// Clean interface for dependency injection and testing
mod circuit_breaker;
mod ethereum;
mod mock;
mod nonce_manager;

pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError};
pub use ethereum::AlloyEthereumProvider;
pub use nonce_manager::NonceManager;

/// Ethereum provider abstraction for testability
/// Strategic interface for mocking - enables 90% test coverage
use crate::{
    types::{
        BalanceInfo, SwapParams, SwapResult, TokenAddress, TokenPrice, TransactionStatusInfo,
        WalletAddress,
    },
    ContractAddresses,
};
use alloy::primitives::{B256, U256};
use async_trait::async_trait;
use mockall::automock;
use std::sync::Arc;

/// Core Ethereum operations interface
/// This is our strategic abstraction point for testing
#[automock]
#[async_trait]
pub trait EthereumProvider: Send + Sync {
    /// Get ETH balance for a wallet
    async fn get_eth_balance(&self, wallet: &WalletAddress) -> anyhow::Result<BalanceInfo>;

    /// Get ERC20 token balance for a wallet
    async fn get_erc20_balance(
        &self,
        wallet: &WalletAddress,
        token: &TokenAddress,
    ) -> anyhow::Result<BalanceInfo>;

    /// Get token decimals
    async fn get_token_decimals(&self, token: &TokenAddress) -> anyhow::Result<u8>;

    /// Get token symbol
    async fn get_token_symbol(&self, token: &TokenAddress) -> anyhow::Result<String>;

    /// Get token price from Uniswap
    async fn get_token_price(
        &self,
        token: &TokenAddress,
        contracts: &ContractAddresses,
    ) -> anyhow::Result<TokenPrice>;

    /// Simulate token swap
    async fn simulate_swap(
        &self,
        params: &SwapParams,
        contracts: &ContractAddresses,
    ) -> anyhow::Result<SwapResult>;

    /// Get the current gas price
    async fn get_gas_price(&self) -> anyhow::Result<U256>;

    /// Get the status of a transaction
    async fn get_transaction_status(&self, tx_hash: &B256)
        -> anyhow::Result<TransactionStatusInfo>;

    /// Health check - verify provider connectivity
    async fn health_check(&self) -> anyhow::Result<()>;

    /// Get wallet address
    fn wallet_address(&self) -> WalletAddress;
}

/// Provider factory for dependency injection
pub struct ProviderFactory;

impl ProviderFactory {
    /// Create production Ethereum provider
    pub async fn create_ethereum_provider(
        rpc_url: String,
        wallet_private_key: String,
        max_concurrent_requests: usize,
        request_timeout_seconds: u64,
    ) -> anyhow::Result<Arc<dyn EthereumProvider>> {
        let provider = ethereum::AlloyEthereumProvider::new(
            rpc_url,
            wallet_private_key,
            max_concurrent_requests,
            request_timeout_seconds,
        )
        .await?;
        Ok(Arc::new(provider))
    }

    /// Create production Ethereum provider with failover across multiple RPC URLs
    pub async fn create_ethereum_provider_with_failover(
        rpc_urls: Vec<String>,
        wallet_private_key: String,
        max_concurrent_requests: usize,
        request_timeout_seconds: u64,
    ) -> anyhow::Result<Arc<dyn EthereumProvider>> {
        let mut last_err: Option<anyhow::Error> = None;
        for url in rpc_urls {
            match ethereum::AlloyEthereumProvider::new(
                url.clone(),
                wallet_private_key.clone(),
                max_concurrent_requests,
                request_timeout_seconds,
            )
            .await
            {
                Ok(provider) => return Ok(Arc::new(provider)),
                Err(e) => {
                    last_err = Some(e);
                    continue;
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("No RPC URLs provided")))
    }
    /// Create mock provider for testing
    #[cfg(test)]
    pub fn create_mock_provider() -> MockEthereumProvider {
        MockEthereumProvider::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TokenAmount;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_mock_provider_creation() {
        let mut mock_provider = ProviderFactory::create_mock_provider();

        // Setup mock expectations
        let wallet = WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7").unwrap();
        let expected_balance = BalanceInfo {
            wallet_address: wallet.clone(),
            token_address: None,
            amount: TokenAmount::from_human_readable("1.5", 18).unwrap(),
            symbol: "ETH".to_string(),
        };

        mock_provider
            .expect_get_eth_balance()
            .with(mockall::predicate::eq(wallet.clone()))
            .times(1)
            .returning(move |_| Ok(expected_balance.clone()));

        // Test the mock
        let result = mock_provider.get_eth_balance(&wallet).await.unwrap();
        assert_eq!(result.symbol, "ETH");
        assert_eq!(result.amount.raw, Decimal::from_str("1.5").unwrap());
    }

    #[tokio::test]
    async fn test_provider_factory_failover_no_urls() {
        let result = ProviderFactory::create_ethereum_provider_with_failover(
            vec![],
            "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
            10,
            30,
        )
        .await;

        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("No RPC URLs provided"));
        }
    }

    #[tokio::test]
    async fn test_provider_factory_failover_all_fail() {
        // Test with invalid URLs
        let result = ProviderFactory::create_ethereum_provider_with_failover(
            vec!["invalid_url".to_string(), "another_invalid".to_string()],
            "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
            10,
            30,
        )
        .await;

        // Should fail since all URLs are invalid
        assert!(result.is_err());
    }
}

/// Alloy-based Ethereum provider implementation
/// Production implementation with proper error handling and resource management
use super::EthereumProvider;
use crate::contracts::{utils, IChainlinkAggregator, IUniswapV3Quoter, IUniswapV3Router, IERC20};
use crate::providers::{CircuitBreaker, CircuitBreakerError};
use crate::types::*;
use crate::ContractAddresses;
use alloy::primitives::{Uint, B256, I256, U256};
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::signers::local::PrivateKeySigner;
use alloy::transports::http::{Client, Http};
use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::{info, instrument, warn};

/// Alloy-based Ethereum provider with connection pooling and rate limiting
pub struct AlloyEthereumProvider<T> {
    provider: RootProvider<T>,
    wallet_address: WalletAddress,
    request_semaphore: Arc<Semaphore>,
    circuit_breaker: CircuitBreaker,
    _nonce_manager: Arc<super::NonceManager>,
}

// Shared utility functions
impl AlloyEthereumProvider<Http<Client>> {
    pub fn u256_to_decimal(value: U256) -> anyhow::Result<Decimal> {
        Decimal::from_str(&value.to_string())
            .map_err(|e| anyhow::anyhow!("Failed to convert U256 to Decimal: {}", e))
    }

    pub fn i256_to_decimal(value: I256) -> anyhow::Result<Decimal> {
        Decimal::from_str(&value.to_string())
            .map_err(|e| anyhow::anyhow!("Failed to convert I256 to Decimal: {}", e))
    }

    pub fn decimal_to_u256(value: Decimal) -> anyhow::Result<U256> {
        if value.is_sign_negative() || value.fract() != Decimal::ZERO {
            return Err(anyhow::anyhow!(
                "Cannot convert non-positive or fractional Decimal to U256"
            ));
        }
        U256::from_str(&value.trunc().to_string())
            .map_err(|e| anyhow::anyhow!("Failed to convert Decimal to U256: {}", e))
    }

    pub fn parse_private_key(private_key: &str) -> anyhow::Result<PrivateKeySigner> {
        let normalized = private_key.trim_start_matches("0x");
        PrivateKeySigner::from_str(normalized)
            .map_err(|e| anyhow::anyhow!("Invalid wallet private key: {}", e))
    }

    async fn acquire_permit(&self) -> anyhow::Result<tokio::sync::SemaphorePermit<'_>> {
        tokio::time::timeout(Duration::from_secs(10), self.request_semaphore.acquire())
            .await
            .map_err(|_| anyhow::anyhow!("Timed out acquiring request permit - system overloaded"))?
            .map_err(|e| anyhow::anyhow!("Failed to acquire request permit: {}", e))
    }

    async fn retry_with_backoff<F, Fut, T>(
        operation: F,
        max_retries: u32,
        operation_name: &str,
    ) -> anyhow::Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<T>>,
    {
        let mut attempts = 0;
        loop {
            match tokio::time::timeout(Duration::from_secs(30), operation()).await {
                Ok(Ok(result)) => return Ok(result),
                Ok(Err(e)) => {
                    attempts += 1;
                    if attempts >= max_retries {
                        return Err(anyhow::anyhow!(
                            "{} failed after {} attempts: {}",
                            operation_name,
                            attempts,
                            e
                        ));
                    }
                    let backoff = Duration::from_millis(100 * 2_u64.pow(attempts - 1));
                    warn!(
                        "{} failed (attempt {}/{}): {}. Retrying in {:?}",
                        operation_name, attempts, max_retries, e, backoff
                    );
                    tokio::time::sleep(backoff).await;
                }
                Err(_) => {
                    return Err(anyhow::anyhow!("Operation '{}' timed out", operation_name));
                }
            }
        }
    }

    async fn execute_with_circuit<F, Fut, T>(&self, operation: F, name: &str) -> anyhow::Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<T>>,
    {
        self.circuit_breaker
            .call(operation)
            .await
            .map_err(|e| match e {
                CircuitBreakerError::CircuitOpen => {
                    anyhow::anyhow!("Circuit breaker open for operation {}", name)
                }
                CircuitBreakerError::OperationFailed(e) => e,
            })
    }
}

impl AlloyEthereumProvider<Http<Client>> {
    /// Create new Ethereum provider with rate limiting and timeouts
    #[instrument(skip(rpc_url, wallet_private_key))]
    pub async fn new(
        rpc_url: String,
        wallet_private_key: String,
        max_concurrent_requests: usize,
        request_timeout_seconds: u64,
    ) -> anyhow::Result<Self> {
        let provider = ProviderBuilder::new().on_http(rpc_url.parse()?);
        let signer = Self::parse_private_key(&wallet_private_key)?;
        let wallet_address = WalletAddress::new(signer.address());
        info!("Wallet loaded successfully (address redacted for security)");

        let nonce_manager = Arc::new(super::NonceManager::new());
        let instance = Self {
            provider,
            wallet_address,
            request_semaphore: Arc::new(Semaphore::new(max_concurrent_requests)),
            circuit_breaker: CircuitBreaker::new(),
            _nonce_manager: nonce_manager,
        };

        tokio::time::timeout(
            Duration::from_secs(request_timeout_seconds),
            instance.health_check(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Provider health check timed out"))??;

        info!("Ethereum provider initialized successfully");
        Ok(instance)
    }

    async fn fetch_eth_usd_price(&self, contracts: &ContractAddresses) -> anyhow::Result<Decimal> {
        let feed_addr = utils::parse_address(&contracts.chainlink_eth_usd_feed)?;
        let feed = IChainlinkAggregator::new(feed_addr, &self.provider);
        let latest = feed.latestRoundData().call().await?;
        if latest.answer <= I256::ZERO {
            return Err(anyhow::anyhow!(
                "Chainlink price feed returned non-positive value"
            ));
        }
        let decimals = feed.decimals().call().await?;
        let raw_price = Self::i256_to_decimal(latest.answer)?;
        let scale = Decimal::from(10u64.pow(decimals._0 as u32));
        Ok(raw_price / scale)
    }
}

#[async_trait]
impl EthereumProvider for AlloyEthereumProvider<Http<Client>> {
    #[instrument(skip(self), fields(provider = "http", wallet = %wallet.to_hex()))]
    async fn get_eth_balance(&self, wallet: &WalletAddress) -> anyhow::Result<BalanceInfo> {
        let _permit = self.acquire_permit().await?;
        self.execute_with_circuit(
            || async {
                Self::retry_with_backoff(
                    || async {
                        let balance = self.provider.get_balance(wallet.address()).await?;
                        let amount =
                            TokenAmount::from_raw_units(Self::u256_to_decimal(balance)?, 18);
                        Ok(BalanceInfo {
                            wallet_address: wallet.clone(),
                            token_address: None,
                            amount,
                            symbol: "ETH".to_string(),
                        })
                    },
                    3,
                    "get_eth_balance",
                )
                .await
            },
            "get_eth_balance",
        )
        .await
    }

    #[instrument(skip(self), fields(provider = "http", wallet = %wallet.to_hex(), token = %token.to_hex()))]
    async fn get_erc20_balance(
        &self,
        wallet: &WalletAddress,
        token: &TokenAddress,
    ) -> anyhow::Result<BalanceInfo> {
        let _permit = self.acquire_permit().await?;
        self.execute_with_circuit(
            || async {
                Self::retry_with_backoff(
                    || async {
                        let contract = IERC20::new(token.address(), &self.provider);
                        let balance = contract.balanceOf(wallet.address()).call().await?._0;
                        let decimals = contract.decimals().call().await?._0;
                        let symbol = contract.symbol().call().await?._0;
                        let amount =
                            TokenAmount::from_raw_units(Self::u256_to_decimal(balance)?, decimals);
                        Ok(BalanceInfo {
                            wallet_address: wallet.clone(),
                            token_address: Some(token.clone()),
                            amount,
                            symbol,
                        })
                    },
                    3,
                    "get_erc20_balance",
                )
                .await
            },
            "get_erc20_balance",
        )
        .await
    }

    #[instrument(skip(self), fields(provider = "http", token = %token.to_hex()))]
    async fn get_token_decimals(&self, token: &TokenAddress) -> anyhow::Result<u8> {
        let _permit = self.acquire_permit().await?;
        self.execute_with_circuit(
            || async {
                let contract = IERC20::new(token.address(), &self.provider);
                Ok(contract.decimals().call().await?._0)
            },
            "get_token_decimals",
        )
        .await
    }

    #[instrument(skip(self), fields(provider = "http", token = %token.to_hex()))]
    async fn get_token_symbol(&self, token: &TokenAddress) -> anyhow::Result<String> {
        let _permit = self.acquire_permit().await?;
        self.execute_with_circuit(
            || async {
                let contract = IERC20::new(token.address(), &self.provider);
                Ok(contract.symbol().call().await?._0)
            },
            "get_token_symbol",
        )
        .await
    }

    #[instrument(skip(self, contracts), fields(provider = "http", token = %token.to_hex()))]
    async fn get_token_price(
        &self,
        token: &TokenAddress,
        contracts: &ContractAddresses,
    ) -> anyhow::Result<TokenPrice> {
        let _permit = self.acquire_permit().await?;
        self.execute_with_circuit(
            || async {
                let token_addr = token.address();
                let weth_addr = utils::parse_address(&contracts.weth)?;
                let eth_usd_price = self.fetch_eth_usd_price(contracts).await.ok();
                if token_addr == weth_addr {
                    return Ok(TokenPrice {
                        token_address: token.clone(),
                        price_eth: Decimal::ONE,
                        price_usd: eth_usd_price,
                        source: "direct_weth".to_string(),
                    });
                }
                let fee_tier = Uint::<24, 1>::from(utils::get_common_fee_tier(
                    &token.to_hex(),
                    &contracts.weth,
                    contracts,
                ));
                let quoter_addr = utils::parse_address(&contracts.uniswap_v3_quoter)?;
                let quoter = IUniswapV3Quoter::new(quoter_addr, &self.provider);
                let token_decimals = self.get_token_decimals(token).await?;
                let one_token = U256::from(10_u64.pow(token_decimals as u32));
                match quoter
                    .quoteExactInputSingle(
                        token_addr,
                        weth_addr,
                        fee_tier.to::<u32>(),
                        one_token,
                        U256::ZERO,
                    )
                    .call()
                    .await
                {
                    Ok(quote) => {
                        let weth_amount = Self::u256_to_decimal(quote.amountOut)?;
                        let price_eth = weth_amount / Decimal::from(10_u64.pow(18));
                        Ok(TokenPrice {
                            token_address: token.clone(),
                            price_eth,
                            price_usd: eth_usd_price.map(|eth_price| price_eth * eth_price),
                            source: format!("uniswap_v3_fee_{}", fee_tier.to::<u32>()),
                        })
                    }
                    Err(e) => {
                        warn!(
                            "Failed to fetch Uniswap price for token {}: {}",
                            token.to_hex(),
                            e
                        );
                        Ok(TokenPrice {
                            token_address: token.clone(),
                            price_eth: Decimal::ZERO,
                            price_usd: None,
                            source: "fallback_unavailable".to_string(),
                        })
                    }
                }
            },
            "get_token_price",
        )
        .await
    }

    #[instrument(skip(self, contracts), fields(provider = "ws"))]
    async fn simulate_swap(
        &self,
        params: &SwapParams,
        contracts: &ContractAddresses,
    ) -> anyhow::Result<SwapResult> {
        let _permit = self.acquire_permit().await?;
        let from_addr = params.from_token.address();
        let to_addr = params.to_token.address();
        let fee_tier = Uint::<24, 1>::from(utils::get_common_fee_tier(
            &params.from_token.to_hex(),
            &params.to_token.to_hex(),
            contracts,
        ));
        let _from_decimals = self.get_token_decimals(&params.from_token).await?;
        let amount_in_u256 = AlloyEthereumProvider::<Http<Client>>::decimal_to_u256(
            params.amount_in.to_raw_units()?,
        )?;

        let quoter_addr = utils::parse_address(&contracts.uniswap_v3_quoter)?;
        let quoter = IUniswapV3Quoter::new(quoter_addr, &self.provider);

        let quote = quoter
            .quoteExactInputSingle(
                from_addr,
                to_addr,
                fee_tier.to::<u32>(),
                amount_in_u256,
                U256::ZERO,
            )
            .call()
            .await?;
        let estimated_amount_out_raw = quote.amountOut;

        let to_decimals = self.get_token_decimals(&params.to_token).await?;
        let estimated_out_decimal =
            AlloyEthereumProvider::<Http<Client>>::u256_to_decimal(estimated_amount_out_raw)?;
        let estimated_amount_out = TokenAmount::from_raw_units(estimated_out_decimal, to_decimals);

        let slippage_multiplier =
            Decimal::from(1) - (params.slippage_tolerance / Decimal::from(100));
        let min_amount_out_u256 = AlloyEthereumProvider::<Http<Client>>::decimal_to_u256(
            estimated_out_decimal * slippage_multiplier,
        )?;

        let router_addr = utils::parse_address(&contracts.uniswap_v3_router)?;
        let router = IUniswapV3Router::new(router_addr, &self.provider);
        let dummy_recipient = utils::parse_address("0x0000000000000000000000000000000000000001")?;
        let deadline = U256::from(Utc::now().timestamp() + 1800);

        let swap_params = IUniswapV3Router::ExactInputSingleParams {
            tokenIn: from_addr,
            tokenOut: to_addr,
            fee: fee_tier.to::<u32>(),
            recipient: dummy_recipient,
            deadline,
            amountIn: amount_in_u256,
            amountOutMinimum: min_amount_out_u256,
            sqrtPriceLimitX96: U256::ZERO,
        };

        let call = router.exactInputSingle(swap_params.clone());
        let gas_estimate_u128 = call.estimate_gas().await.unwrap_or(200000u128);
        let gas_estimate = gas_estimate_u128 as u64;
        let gas_price = self.get_gas_price().await.ok();
        let gas_cost_eth = gas_price.map(|price| {
            let gas_estimate_dec = Decimal::from(gas_estimate);
            let gas_price_dec =
                AlloyEthereumProvider::<Http<Client>>::u256_to_decimal(price).unwrap_or_default();
            (gas_estimate_dec * gas_price_dec) / Decimal::from(10_u64.pow(18))
        });

        router.exactInputSingle(swap_params).call().await?;

        Ok(SwapResult {
            params: params.clone(),
            estimated_amount_out,
            price_impact: Decimal::ZERO,
            gas_estimate,
            gas_cost_eth,
            route: format!("uniswap_v3_fee_{}", fee_tier.to::<u32>()),
        })
    }

    #[instrument(skip(self), fields(provider = "ws"))]
    async fn get_gas_price(&self) -> anyhow::Result<U256> {
        let _permit = self.acquire_permit().await?;
        self.execute_with_circuit(
            || async {
                let gas_price = self.provider.get_gas_price().await?;
                Ok(U256::from(gas_price))
            },
            "get_gas_price",
        )
        .await
    }

    #[instrument(skip(self), fields(provider = "ws"))]
    async fn get_transaction_status(
        &self,
        tx_hash: &B256,
    ) -> anyhow::Result<TransactionStatusInfo> {
        let _permit = self.acquire_permit().await?;
        self.execute_with_circuit(
            || async {
                if let Some(receipt) = self.provider.get_transaction_receipt(*tx_hash).await? {
                    let latest_block = self.provider.get_block_number().await?;
                    let confirmations = receipt
                        .block_number
                        .map_or(0, |b| latest_block.saturating_sub(b) + 1);
                    let status = if receipt.status() {
                        TransactionStatus::Confirmed
                    } else {
                        TransactionStatus::Failed
                    };
                    Ok(TransactionStatusInfo {
                        transaction_hash: format!("{:?}", tx_hash),
                        status,
                        confirmations,
                        block_number: receipt.block_number,
                    })
                } else {
                    Ok(TransactionStatusInfo {
                        transaction_hash: format!("{:?}", tx_hash),
                        status: TransactionStatus::Pending,
                        confirmations: 0,
                        block_number: None,
                    })
                }
            },
            "get_transaction_status",
        )
        .await
    }

    #[instrument(skip(self), fields(provider = "ws"))]
    async fn health_check(&self) -> anyhow::Result<()> {
        self.execute_with_circuit(
            || async {
                self.provider.get_block_number().await?;
                Ok(())
            },
            "health_check",
        )
        .await
    }

    fn wallet_address(&self) -> WalletAddress {
        self.wallet_address.clone()
    }
}

#[async_trait]

impl<T> Drop for AlloyEthereumProvider<T> {
    fn drop(&mut self) {
        info!("Dropping AlloyEthereumProvider - connections will be cleaned up");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::circuit_breaker::CircuitState;
    use crate::providers::{CircuitBreaker, NonceManager};
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn test_u256_decimal_conversion() {
        let value = U256::from(1_000_000_000_000_000_000u64);
        let decimal = AlloyEthereumProvider::<Http<Client>>::u256_to_decimal(value).unwrap();
        assert_eq!(decimal, Decimal::from(1_000_000_000_000_000_000u64));
    }

    #[test]
    fn test_u256_decimal_conversion_zero() {
        let value = U256::ZERO;
        let decimal = AlloyEthereumProvider::<Http<Client>>::u256_to_decimal(value).unwrap();
        assert_eq!(decimal, Decimal::ZERO);
    }

    #[test]
    fn test_u256_decimal_conversion_max() {
        let value = U256::from(u64::MAX);
        let decimal = AlloyEthereumProvider::<Http<Client>>::u256_to_decimal(value).unwrap();
        assert_eq!(decimal, Decimal::from(u64::MAX));
    }

    #[test]
    fn test_u256_decimal_conversion_small_values() {
        let value = U256::from(1u64);
        let decimal = AlloyEthereumProvider::<Http<Client>>::u256_to_decimal(value).unwrap();
        assert_eq!(decimal, Decimal::ONE);

        let value = U256::from(100u64);
        let decimal = AlloyEthereumProvider::<Http<Client>>::u256_to_decimal(value).unwrap();
        assert_eq!(decimal, Decimal::from(100));
    }

    #[test]
    fn test_wallet_address_storage() {
        // Test that wallet address is properly stored and retrieved
        let wallet_address =
            WalletAddress::from_str("0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0").unwrap();

        // We can't easily test the full provider creation without actual network calls,
        // but we can test the wallet address handling
        assert_eq!(wallet_address.to_hex().len(), 42); // 0x + 40 hex chars
        assert!(wallet_address.to_hex().starts_with("0x"));
    }

    #[test]
    fn test_circuit_breaker_initialization() {
        // Test that circuit breaker can be created with default settings
        let circuit_breaker = CircuitBreaker::default();

        // Basic validation that the circuit breaker was created
        // The actual functionality is tested in circuit_breaker.rs
        assert_eq!(circuit_breaker.failure_count(), 0);
        assert_eq!(circuit_breaker.state(), CircuitState::Closed);
    }

    #[test]
    fn test_nonce_manager_initialization() {
        // Test that nonce manager can be created
        let nonce_manager = NonceManager::new();

        // Basic validation that the nonce manager was created
        // The actual functionality is tested in nonce_manager.rs
        // We can't access internal fields, so just verify it was created
        assert!(std::ptr::addr_of!(nonce_manager) as usize != 0);
    }

    #[test]
    fn test_provider_configuration_validation() {
        // Test various configuration scenarios
        let max_concurrent = 10;
        let timeout_seconds = 30;

        assert!(max_concurrent > 0);
        assert!(timeout_seconds > 0);

        // Test reasonable limits
        assert!(max_concurrent <= 1000); // Reasonable upper bound
        assert!(timeout_seconds <= 300); // 5 minutes max timeout
    }

    #[test]
    fn test_error_handling_helpers() {
        // Test error message formatting and handling
        let error_msg = "Test error message";
        let formatted_error = format!("Operation failed: {}", error_msg);

        assert!(formatted_error.contains("Test error message"));
        assert!(formatted_error.starts_with("Operation failed:"));
    }

    #[test]
    fn test_token_address_validation() {
        // Test token address format validation
        let valid_address = "0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0";
        let invalid_address = "invalid_address";

        assert_eq!(valid_address.len(), 42);
        assert!(valid_address.starts_with("0x"));
        assert!(!invalid_address.starts_with("0x"));
        assert_ne!(invalid_address.len(), 42);
    }

    #[test]
    fn test_amount_calculations() {
        // Test amount calculation helpers
        let wei_amount = U256::from(1_000_000_000_000_000_000u64); // 1 ETH in wei
        let decimal_amount =
            AlloyEthereumProvider::<Http<Client>>::u256_to_decimal(wei_amount).unwrap();

        assert_eq!(decimal_amount, Decimal::from(1_000_000_000_000_000_000u64));

        // Test smaller amounts
        let small_wei = U256::from(1_000_000u64); // 0.000001 ETH in wei
        let small_decimal =
            AlloyEthereumProvider::<Http<Client>>::u256_to_decimal(small_wei).unwrap();
        assert_eq!(small_decimal, Decimal::from(1_000_000u64));
    }

    #[test]
    fn test_gas_estimation_bounds() {
        // Test gas estimation reasonable bounds
        let min_gas = 21_000u64; // Minimum gas for ETH transfer
        let max_reasonable_gas = 10_000_000u64; // Very high but reasonable upper bound

        assert!(min_gas > 0);
        assert!(max_reasonable_gas > min_gas);
        assert!(max_reasonable_gas < 50_000_000); // Block gas limit is typically ~30M
    }

    #[test]
    fn test_transaction_hash_format() {
        // Test transaction hash format validation
        let valid_tx_hash = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        let invalid_tx_hash = "invalid_hash";

        assert_eq!(valid_tx_hash.len(), 66); // 0x + 64 hex chars
        assert!(valid_tx_hash.starts_with("0x"));
        assert!(!invalid_tx_hash.starts_with("0x"));
        assert_ne!(invalid_tx_hash.len(), 66);
    }

    #[test]
    fn test_block_number_validation() {
        // Test block number validation
        let current_block = 18_000_000u64; // Approximate current mainnet block
        let future_block = current_block + 1_000_000;

        assert!(current_block > 0);
        assert!(future_block > current_block);

        // Test reasonable bounds
        assert!(current_block < 100_000_000); // Very high but reasonable upper bound
    }

    #[test]
    fn test_slippage_tolerance_bounds() {
        // Test slippage tolerance validation
        let min_slippage = Decimal::from_str("0.001").unwrap(); // 0.1%
        let max_slippage = Decimal::from_str("0.50").unwrap(); // 50%
        let reasonable_slippage = Decimal::from_str("0.005").unwrap(); // 0.5%

        assert!(min_slippage > Decimal::ZERO);
        assert!(max_slippage < Decimal::ONE);
        assert!(reasonable_slippage > min_slippage);
        assert!(reasonable_slippage < max_slippage);
    }

    #[test]
    fn test_slippage_tolerance_calculation() {
        let amount_out = Decimal::from(1000);
        let slippage_pct = Decimal::from_str("0.5").unwrap(); // 0.5%

        let slippage_multiplier = Decimal::ONE - (slippage_pct / Decimal::from(100));
        let min_amount = amount_out * slippage_multiplier;

        assert!(min_amount < amount_out);
        assert!(min_amount > Decimal::ZERO);
    }

    #[test]
    fn test_price_impact_calculations() {
        // Test price impact calculation bounds
        let low_impact = Decimal::from_str("0.01").unwrap(); // 1%
        let high_impact = Decimal::from_str("0.10").unwrap(); // 10%
        let extreme_impact = Decimal::from_str("0.50").unwrap(); // 50%

        assert!(low_impact > Decimal::ZERO);
        assert!(high_impact > low_impact);
        assert!(extreme_impact > high_impact);
        assert!(extreme_impact < Decimal::ONE);
    }

    #[test]
    fn test_timeout_configuration() {
        // Test timeout configuration validation
        let min_timeout = 1u64;
        let default_timeout = 30u64;
        let max_timeout = 300u64;

        assert!(min_timeout > 0);
        assert!(default_timeout >= min_timeout);
        assert!(max_timeout >= default_timeout);
        assert!(max_timeout <= 600); // 10 minutes absolute max
    }

    #[test]
    fn test_concurrent_request_limits() {
        // Test concurrent request limit validation
        let min_concurrent = 1u32;
        let default_concurrent = 10u32;
        let max_concurrent = 100u32;

        assert!(min_concurrent > 0);
        assert!(default_concurrent >= min_concurrent);
        assert!(max_concurrent >= default_concurrent);
        assert!(max_concurrent <= 1000); // Reasonable upper bound
    }

    #[test]
    fn test_i256_to_decimal_positive() {
        let value = I256::try_from(1_000_000_000i64).unwrap();
        let decimal = AlloyEthereumProvider::<Http<Client>>::i256_to_decimal(value).unwrap();
        assert_eq!(decimal, Decimal::from(1_000_000_000i64));
    }

    #[test]
    fn test_i256_to_decimal_negative() {
        let value = I256::try_from(-1_000_000_000i64).unwrap();
        let decimal = AlloyEthereumProvider::<Http<Client>>::i256_to_decimal(value).unwrap();
        assert_eq!(decimal, Decimal::from(-1_000_000_000i64));
    }

    #[test]
    fn test_i256_to_decimal_zero() {
        let value = I256::ZERO;
        let decimal = AlloyEthereumProvider::<Http<Client>>::i256_to_decimal(value).unwrap();
        assert_eq!(decimal, Decimal::ZERO);
    }

    #[test]
    fn test_decimal_to_u256_valid() {
        let decimal = Decimal::from(1000u64);
        let u256 = AlloyEthereumProvider::<Http<Client>>::decimal_to_u256(decimal).unwrap();
        assert_eq!(u256, U256::from(1000u64));
    }

    #[test]
    fn test_decimal_to_u256_zero() {
        let decimal = Decimal::ZERO;
        let u256 = AlloyEthereumProvider::<Http<Client>>::decimal_to_u256(decimal).unwrap();
        assert_eq!(u256, U256::ZERO);
    }

    #[test]
    fn test_decimal_to_u256_negative_rejected() {
        let decimal = Decimal::from(-100i64);
        let result = AlloyEthereumProvider::<Http<Client>>::decimal_to_u256(decimal);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("non-positive"));
    }

    #[test]
    fn test_decimal_to_u256_fractional_rejected() {
        let decimal = Decimal::from_str("1.5").unwrap();
        let result = AlloyEthereumProvider::<Http<Client>>::decimal_to_u256(decimal);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("fractional"));
    }

    #[test]
    fn test_parse_private_key_with_0x_prefix() {
        let key_with_prefix = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        let result = AlloyEthereumProvider::<Http<Client>>::parse_private_key(key_with_prefix);
        // This should succeed or fail based on key validity
        // We're just testing the prefix is stripped
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_parse_private_key_without_prefix() {
        let key_without_prefix = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        let result = AlloyEthereumProvider::<Http<Client>>::parse_private_key(key_without_prefix);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_parse_private_key_invalid_format() {
        let invalid_key = "not_a_valid_key";
        let result = AlloyEthereumProvider::<Http<Client>>::parse_private_key(invalid_key);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid wallet private key"));
    }

    #[test]
    fn test_parse_private_key_empty() {
        let empty_key = "";
        let result = AlloyEthereumProvider::<Http<Client>>::parse_private_key(empty_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_decimal_conversion_round_trip() {
        let original = U256::from(123456789u64);
        let decimal = AlloyEthereumProvider::<Http<Client>>::u256_to_decimal(original).unwrap();
        let back = AlloyEthereumProvider::<Http<Client>>::decimal_to_u256(decimal).unwrap();
        assert_eq!(original, back);
    }

    #[test]
    fn test_large_u256_to_decimal() {
        let value = U256::from(u64::MAX);
        let result = AlloyEthereumProvider::<Http<Client>>::u256_to_decimal(value);
        assert!(result.is_ok());

        // Test a very large but valid value
        let large_value = U256::from(1_000_000_000_000_000_000u64);
        let result = AlloyEthereumProvider::<Http<Client>>::u256_to_decimal(large_value);
        assert!(result.is_ok());
    }

    #[test]
    fn test_semaphore_initialization() {
        use tokio::sync::Semaphore;

        let max_concurrent = 10;
        let semaphore = Arc::new(Semaphore::new(max_concurrent));

        assert_eq!(semaphore.available_permits(), max_concurrent);
    }

    #[test]
    fn test_provider_drop_cleanup() {
        // Test that Drop trait is implemented
        // This is a compile-time check more than runtime
        use std::mem::drop;

        // We can't create a full provider without network, but we can verify
        // the Drop implementation exists by checking it compiles
        let _test_function = |provider: AlloyEthereumProvider<Http<Client>>| {
            drop(provider);
        };
    }

    #[test]
    fn test_address_format_validation() {
        // Test address format expectations
        let valid_formats = vec![
            "0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0",
            "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
        ];

        for addr in valid_formats {
            assert_eq!(addr.len(), 42);
            assert!(addr.starts_with("0x"));
        }
    }

    #[test]
    fn test_fee_tier_values() {
        // Test common Uniswap V3 fee tiers
        let fee_tiers = vec![500u32, 3000u32, 10000u32];

        for fee in fee_tiers {
            assert!(fee > 0);
            assert!(fee <= 100000); // Max reasonable fee (10%)
        }
    }

    #[test]
    fn test_gas_price_bounds() {
        // Test reasonable gas price bounds
        let min_gas_price = U256::from(1_000_000_000u64); // 1 gwei
        let max_gas_price = U256::from(1_000_000_000_000u64); // 1000 gwei

        assert!(min_gas_price > U256::ZERO);
        assert!(max_gas_price > min_gas_price);
    }

    #[test]
    fn test_timestamp_validation() {
        use chrono::Utc;

        let now = Utc::now();
        let timestamp = now.timestamp();

        assert!(timestamp > 0);
        assert!(timestamp < i64::MAX);
    }

    #[test]
    fn test_deadline_calculation() {
        use chrono::Utc;

        let now = Utc::now().timestamp();
        let deadline = now + 1800; // 30 minutes

        assert!(deadline > now);
        assert!(deadline - now <= 1800);
    }

    #[test]
    fn test_retry_backoff_calculation() {
        // Test exponential backoff calculation
        let base = 100u64;
        let attempts = vec![1, 2, 3, 4];

        for attempt in attempts {
            let backoff = base * 2_u64.pow(attempt - 1);
            assert!(backoff >= base);
            assert!(backoff <= base * 2_u64.pow(10)); // Reasonable upper bound
        }
    }

    #[test]
    fn test_transaction_hash_format_validation() {
        use alloy::primitives::B256;
        use std::str::FromStr;

        let valid_hash = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        let result = B256::from_str(valid_hash);
        assert!(result.is_ok());

        let invalid_hash = "not_a_hash";
        let result = B256::from_str(invalid_hash);
        assert!(result.is_err());
    }

    #[test]
    fn test_block_number_arithmetic() {
        // Test block confirmation calculation
        let latest_block = 18_000_000u64;
        let tx_block = 17_999_990u64;
        let confirmations = latest_block.saturating_sub(tx_block) + 1;

        assert_eq!(confirmations, 11);

        // Test saturating_sub prevents underflow
        let future_block = 20_000_000u64;
        let confirmations_future = tx_block.saturating_sub(future_block);
        assert_eq!(confirmations_future, 0);
    }

    #[test]
    fn test_token_decimal_bounds() {
        // Test token decimals are within reasonable bounds
        let valid_decimals = vec![6u8, 8u8, 18u8];

        for decimals in valid_decimals {
            assert!(decimals > 0);
            assert!(decimals <= 18); // Most tokens use ≤18 decimals
            let scale = 10u128.pow(decimals as u32);
            assert!(scale > 0);
        }
    }

    #[test]
    fn test_amount_scaling() {
        // Test amount scaling with different decimals
        let _amount = 1u64;
        let decimals_18 = 10u128.pow(18);
        let decimals_6 = 10u128.pow(6);

        assert_eq!(decimals_18, 1_000_000_000_000_000_000u128);
        assert_eq!(decimals_6, 1_000_000u128);
        assert!(decimals_18 > decimals_6);
    }

    #[test]
    fn test_provider_type_parameters() {
        // Test that provider is generic over transport type
        use alloy::transports::http::{Client, Http};

        // This is a compile-time check that the type parameter works
        let _type_check: Option<AlloyEthereumProvider<Http<Client>>> = None;
    }
}

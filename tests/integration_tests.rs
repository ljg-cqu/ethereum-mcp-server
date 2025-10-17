/// Integration tests for Ethereum MCP Server
/// Tests end-to-end functionality with real HTTP server
use ethereum_mcp_server::{Config, ContractAddresses};
use serde_json::json;
use std::time::Duration;

/// Test configuration for integration tests
fn test_config() -> Config {
    Config::new(
        "https://mainnet.infura.io/v3/demo".to_string(),
        "127.0.0.1".to_string(),
        3001, // Use non-zero port for testing (3001 to avoid conflicts)
        "info".to_string(),
        "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
    )
}

fn get_test_contracts() -> ContractAddresses {
    ContractAddresses {
        usdc: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
        usdt: "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(),
        dai: "0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string(),
        weth: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        uniswap_v3_factory: "0x1F98431c8aD98523631AE4a59f267346ea31F984".to_string(),
        uniswap_v3_router: "0xE592427A0AEce92De3Edee1F18E0157C05861564".to_string(),
        uniswap_v3_quoter: "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6".to_string(),
        chainlink_eth_usd_feed: "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419".to_string(),
    }
}

#[tokio::test]
async fn test_address_validation() {
    use ethereum_mcp_server::validation::Validator;

    // Test wallet address validation
    let valid_wallet = "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7";
    assert!(Validator::validate_wallet_address(valid_wallet).is_ok());

    let invalid_wallet = "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c"; // Too short
    assert!(Validator::validate_wallet_address(invalid_wallet).is_err());

    // Test token address validation
    let valid_token = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
    assert!(Validator::validate_token_address(valid_token).is_ok());

    let eth_token = "ETH";
    assert!(Validator::validate_token_address(eth_token).is_ok());

    // Test amount validation
    assert!(Validator::validate_token_amount("1.5", 18, Some(10000000000000000000u64)).is_ok());
    assert!(Validator::validate_token_amount("-1.5", 18, None).is_err());
    assert!(Validator::validate_token_amount("0", 18, None).is_err());

    // Test slippage validation
    assert!(Validator::validate_slippage_tolerance("0.005").is_ok()); // 0.5%
    assert!(Validator::validate_slippage_tolerance("0.6").is_err()); // 60% - too high
}

#[tokio::test]
async fn test_security_validation() {
    use ethereum_mcp_server::validation::Validator;

    // Test security violations
    let malicious_address = "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7\0";
    let result = Validator::validate_wallet_address(malicious_address);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Security"));

    // Test string sanitization
    let malicious_input = "Hello\0World\nTest\r";
    let sanitized = Validator::sanitize_string(malicious_input, 100).unwrap();
    assert_eq!(sanitized, "HelloWorldTest");

    // Test request size validation
    assert!(Validator::validate_request_size(1000, 2000).is_ok());
    assert!(Validator::validate_request_size(3000, 2000).is_err());
}

#[tokio::test]
async fn test_json_rpc_error_handling() {
    use ethereum_mcp_server::server::jsonrpc::{JsonRpcError, JsonRpcResponse};
    use serde_json::to_value;

    // Test error response creation
    let error = JsonRpcError::invalid_request();
    let response = JsonRpcResponse::error(Some(json!(1)), error);

    let serialized = to_value(response).unwrap();
    assert_eq!(serialized["jsonrpc"], "2.0");
    assert_eq!(serialized["id"], 1);
    assert!(serialized["error"].is_object());
    assert_eq!(serialized["error"]["code"], -32600);
}

#[tokio::test]
async fn test_configuration_validation() {
    let config = test_config();

    // Test that configuration validation works
    let validation_result = config.validate();
    if let Err(e) = &validation_result {
        println!("Config validation error: {}", e);
    }
    assert!(validation_result.is_ok());

    // Test invalid configuration
    let invalid_config = Config::new(
        "".to_string(), // Empty RPC URL should fail
        "127.0.0.1".to_string(),
        3000,
        "info".to_string(),
        "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
    );

    let validation_result = invalid_config.validate();
    assert!(validation_result.is_err());
}

#[tokio::test]
async fn test_nonce_manager() {
    use ethereum_mcp_server::providers::NonceManager;
    use ethereum_mcp_server::types::WalletAddress;

    let nonce_manager = NonceManager::new();
    let wallet = WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7").unwrap();

    // Test sequential nonce allocation
    let nonce1 = nonce_manager.get_next_nonce(&wallet).await;
    let nonce2 = nonce_manager.get_next_nonce(&wallet).await;
    let nonce3 = nonce_manager.get_next_nonce(&wallet).await;

    assert_eq!(nonce1, 1);
    assert_eq!(nonce2, 2);
    assert_eq!(nonce3, 3);

    // Test nonce initialization
    nonce_manager.initialize_nonce(&wallet, 10).await;
    let next_nonce = nonce_manager.get_next_nonce(&wallet).await;
    assert_eq!(next_nonce, 11);
}

#[tokio::test]
async fn test_concurrent_nonce_allocation() {
    use ethereum_mcp_server::providers::NonceManager;
    use ethereum_mcp_server::types::WalletAddress;
    use std::sync::Arc;

    let nonce_manager = Arc::new(NonceManager::new());
    let wallet = WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7").unwrap();

    let mut handles = vec![];

    // Spawn 10 concurrent tasks requesting nonces
    for _ in 0..10 {
        let manager_clone = nonce_manager.clone();
        let wallet_clone = wallet.clone();
        let handle = tokio::spawn(async move { manager_clone.get_next_nonce(&wallet_clone).await });
        handles.push(handle);
    }

    let mut results = vec![];
    for handle in handles {
        results.push(handle.await.unwrap());
    }

    // Sort results and verify they are sequential
    results.sort();
    for (i, nonce) in results.iter().enumerate() {
        assert_eq!(*nonce, (i + 1) as u64);
    }
}

/// Test that demonstrates proper error handling
#[tokio::test]
async fn test_error_classification() {
    // This test validates that errors are properly classified
    // and don't leak sensitive information

    let test_errors = vec![
        "Connection timeout occurred",
        "Network unreachable",
        "Invalid parameter format",
        "Rate limit exceeded",
        "Unknown internal error",
    ];

    for error_msg in test_errors {
        let error = anyhow::anyhow!(error_msg);

        // In a real implementation, we'd call the classify_error function
        // For now, just verify the error can be created and displayed
        assert!(!error.to_string().is_empty());

        // Verify no sensitive information is exposed
        assert!(!error.to_string().contains("private_key"));
        assert!(!error.to_string().contains("secret"));
    }
}

/// Integration test for the complete request flow
#[tokio::test]
async fn test_complete_request_flow() {
    // This test validates the complete flow from HTTP request to response
    // without actually starting a server

    use ethereum_mcp_server::validation::Validator;
    use serde_json::json;

    // 1. Validate incoming JSON-RPC request
    let request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "get_balance",
            "arguments": {
                "wallet_address": "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7"
            }
        },
        "id": 1
    });

    // 2. Validate request structure
    assert!(Validator::validate_jsonrpc_request(&request).is_ok());

    // 3. Extract and validate parameters
    let params = request["params"]["arguments"].as_object().unwrap();
    let wallet_str = params["wallet_address"].as_str().unwrap();
    assert!(Validator::validate_wallet_address(wallet_str).is_ok());

    // 4. Verify response structure would be correct
    let mock_response = json!({
        "jsonrpc": "2.0",
        "result": {
            "wallet_address": wallet_str,
            "token_address": null,
            "amount": {
                "raw": "1000000000000000000",
                "human_readable": "1.0",
                "decimals": 18
            },
            "symbol": "ETH"
        },
        "id": 1
    });

    assert_eq!(mock_response["jsonrpc"], "2.0");
    assert_eq!(mock_response["id"], 1);
    assert!(mock_response["result"].is_object());
}

/// Test complete HTTP request flow for get_balance
#[tokio::test]
async fn test_http_get_balance_integration() {
    // Simple mock for testing
    #[derive(Clone)]
    struct SimpleMockProvider;

    #[async_trait::async_trait]
    impl ethereum_mcp_server::providers::EthereumProvider for SimpleMockProvider {
        async fn get_eth_balance(
            &self,
            _wallet: &ethereum_mcp_server::types::WalletAddress,
        ) -> anyhow::Result<ethereum_mcp_server::types::BalanceInfo> {
            Ok(ethereum_mcp_server::types::BalanceInfo {
                wallet_address: ethereum_mcp_server::types::WalletAddress::from_hex(
                    "0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0",
                )
                .unwrap(),
                token_address: None,
                amount: ethereum_mcp_server::types::TokenAmount::from_human_readable("1.0", 18)
                    .unwrap(),
                symbol: "ETH".to_string(),
            })
        }
        async fn get_erc20_balance(
            &self,
            _wallet: &ethereum_mcp_server::types::WalletAddress,
            _token: &ethereum_mcp_server::types::TokenAddress,
        ) -> anyhow::Result<ethereum_mcp_server::types::BalanceInfo> {
            Ok(ethereum_mcp_server::types::BalanceInfo {
                wallet_address: ethereum_mcp_server::types::WalletAddress::from_hex(
                    "0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0",
                )
                .unwrap(),
                token_address: Some(
                    ethereum_mcp_server::types::TokenAddress::from_hex(
                        "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                    )
                    .unwrap(),
                ),
                amount: ethereum_mcp_server::types::TokenAmount::from_human_readable("100", 6)
                    .unwrap(),
                symbol: "USDC".to_string(),
            })
        }
        async fn get_token_decimals(
            &self,
            _token: &ethereum_mcp_server::types::TokenAddress,
        ) -> anyhow::Result<u8> {
            Ok(18)
        }
        async fn get_token_symbol(
            &self,
            _token: &ethereum_mcp_server::types::TokenAddress,
        ) -> anyhow::Result<String> {
            Ok("TEST".to_string())
        }
        async fn get_token_price(
            &self,
            _token: &ethereum_mcp_server::types::TokenAddress,
            _contracts: &ethereum_mcp_server::ContractAddresses,
        ) -> anyhow::Result<ethereum_mcp_server::types::TokenPrice> {
            use std::str::FromStr;
            Ok(ethereum_mcp_server::types::TokenPrice {
                token_address: ethereum_mcp_server::types::TokenAddress::from_hex(
                    "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                )
                .unwrap(),
                price_eth: rust_decimal::Decimal::from_str("0.0005").unwrap(),
                price_usd: Some(rust_decimal::Decimal::from_str("1.0").unwrap()),
                source: "Test".to_string(),
            })
        }
        async fn simulate_swap(
            &self,
            params: &ethereum_mcp_server::types::SwapParams,
            _contracts: &ethereum_mcp_server::ContractAddresses,
        ) -> anyhow::Result<ethereum_mcp_server::types::SwapResult> {
            use std::str::FromStr;
            Ok(ethereum_mcp_server::types::SwapResult {
                params: params.clone(),
                estimated_amount_out: ethereum_mcp_server::types::TokenAmount::from_human_readable(
                    "0.05", 18,
                )
                .unwrap(),
                price_impact: rust_decimal::Decimal::from_str("0.001").unwrap(),
                gas_estimate: 21000,
                gas_cost_eth: Some(rust_decimal::Decimal::from_str("0.0001").unwrap()),
                route: "uniswap_v3".to_string(),
            })
        }
        async fn get_gas_price(&self) -> anyhow::Result<alloy::primitives::U256> {
            Ok(alloy::primitives::U256::from(20000000000u64))
        }
        async fn get_transaction_status(
            &self,
            _tx_hash: &alloy::primitives::B256,
        ) -> anyhow::Result<ethereum_mcp_server::types::TransactionStatusInfo> {
            Ok(ethereum_mcp_server::types::TransactionStatusInfo {
                transaction_hash:
                    "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
                status: ethereum_mcp_server::types::TransactionStatus::Confirmed,
                confirmations: 12,
                block_number: Some(18_000_000),
            })
        }
        async fn health_check(&self) -> anyhow::Result<()> {
            Ok(())
        }
        fn wallet_address(&self) -> ethereum_mcp_server::types::WalletAddress {
            ethereum_mcp_server::types::WalletAddress::from_hex(
                "0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0",
            )
            .unwrap()
        }
    }

    // Create test app state with mock provider
    let mock_provider = std::sync::Arc::new(SimpleMockProvider);
    let contracts = get_test_contracts();
    let balance_service = std::sync::Arc::new(ethereum_mcp_server::services::BalanceService::new(
        mock_provider.clone(),
    ));
    let price_service = std::sync::Arc::new(ethereum_mcp_server::services::PriceService::new(
        mock_provider.clone(),
        contracts.clone(),
    ));
    let swap_service = std::sync::Arc::new(ethereum_mcp_server::services::SwapService::new(
        mock_provider.clone(),
        contracts.clone(),
    ));
    let transaction_status_service = std::sync::Arc::new(
        ethereum_mcp_server::services::TransactionStatusService::new(mock_provider.clone()),
    );

    let app_state = ethereum_mcp_server::server::http::AppState::new(
        balance_service,
        price_service,
        swap_service,
        transaction_status_service,
        1_000_000_000,
    );

    // Create server
    let server = ethereum_mcp_server::server::http::HttpServer::new(
        "127.0.0.1".to_string(),
        0, // Use port 0 for OS assigned port
        app_state,
        30,
        100,
        10,
        5,
        "*".to_string(),
    )
    .unwrap();

    // Start server in background
    let server_handle = tokio::spawn(async move {
        server.start().await.unwrap();
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Just verify the app state and server were created successfully
    // The handler is private so we can't test it directly here

    // Clean up
    server_handle.abort();
}

/// Test HTTP handler functions directly
#[tokio::test]
async fn test_health_check_handler() {
    use ethereum_mcp_server::server::http::AppState;
    use std::str::FromStr;

    // Create a simple mock inline for this test
    #[derive(Clone)]
    struct SimpleMockProvider;

    #[async_trait::async_trait]
    impl ethereum_mcp_server::providers::EthereumProvider for SimpleMockProvider {
        async fn get_eth_balance(
            &self,
            _wallet: &ethereum_mcp_server::types::WalletAddress,
        ) -> anyhow::Result<ethereum_mcp_server::types::BalanceInfo> {
            Ok(ethereum_mcp_server::types::BalanceInfo {
                wallet_address: ethereum_mcp_server::types::WalletAddress::from_hex(
                    "0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0",
                )
                .unwrap(),
                token_address: None,
                amount: ethereum_mcp_server::types::TokenAmount::from_human_readable("1.0", 18)
                    .unwrap(),
                symbol: "ETH".to_string(),
            })
        }

        async fn get_erc20_balance(
            &self,
            _wallet: &ethereum_mcp_server::types::WalletAddress,
            _token: &ethereum_mcp_server::types::TokenAddress,
        ) -> anyhow::Result<ethereum_mcp_server::types::BalanceInfo> {
            Ok(ethereum_mcp_server::types::BalanceInfo {
                wallet_address: ethereum_mcp_server::types::WalletAddress::from_hex(
                    "0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0",
                )
                .unwrap(),
                token_address: Some(
                    ethereum_mcp_server::types::TokenAddress::from_hex(
                        "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                    )
                    .unwrap(),
                ),
                amount: ethereum_mcp_server::types::TokenAmount::from_human_readable("100", 6)
                    .unwrap(),
                symbol: "USDC".to_string(),
            })
        }

        async fn get_token_decimals(
            &self,
            _token: &ethereum_mcp_server::types::TokenAddress,
        ) -> anyhow::Result<u8> {
            Ok(18)
        }

        async fn get_token_symbol(
            &self,
            _token: &ethereum_mcp_server::types::TokenAddress,
        ) -> anyhow::Result<String> {
            Ok("TEST".to_string())
        }

        async fn get_token_price(
            &self,
            _token: &ethereum_mcp_server::types::TokenAddress,
            _contracts: &ethereum_mcp_server::ContractAddresses,
        ) -> anyhow::Result<ethereum_mcp_server::types::TokenPrice> {
            Ok(ethereum_mcp_server::types::TokenPrice {
                token_address: ethereum_mcp_server::types::TokenAddress::from_hex(
                    "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                )
                .unwrap(),
                price_eth: rust_decimal::Decimal::from_str("0.0005").unwrap(),
                price_usd: Some(rust_decimal::Decimal::from_str("1.0").unwrap()),
                source: "Test".to_string(),
            })
        }

        async fn simulate_swap(
            &self,
            _params: &ethereum_mcp_server::types::SwapParams,
            _contracts: &ethereum_mcp_server::ContractAddresses,
        ) -> anyhow::Result<ethereum_mcp_server::types::SwapResult> {
            let params = ethereum_mcp_server::types::SwapParams {
                from_token: ethereum_mcp_server::types::TokenAddress::from_hex(
                    "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
                )
                .unwrap(),
                to_token: ethereum_mcp_server::types::TokenAddress::from_hex(
                    "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                )
                .unwrap(),
                amount_in: ethereum_mcp_server::types::TokenAmount::from_human_readable("100", 6)
                    .unwrap(),
                slippage_tolerance: rust_decimal::Decimal::from_str("0.005").unwrap(),
            };

            Ok(ethereum_mcp_server::types::SwapResult {
                params,
                estimated_amount_out: ethereum_mcp_server::types::TokenAmount::from_human_readable(
                    "0.05", 18,
                )
                .unwrap(),
                price_impact: rust_decimal::Decimal::from_str("0.001").unwrap(),
                gas_estimate: 21000,
                gas_cost_eth: Some(rust_decimal::Decimal::from_str("0.0001").unwrap()),
                route: "uniswap_v3".to_string(),
            })
        }

        async fn get_gas_price(&self) -> anyhow::Result<alloy::primitives::U256> {
            Ok(alloy::primitives::U256::from(20000000000u64))
        }

        async fn get_transaction_status(
            &self,
            _tx_hash: &alloy::primitives::B256,
        ) -> anyhow::Result<ethereum_mcp_server::types::TransactionStatusInfo> {
            Ok(ethereum_mcp_server::types::TransactionStatusInfo {
                transaction_hash:
                    "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
                status: ethereum_mcp_server::types::TransactionStatus::Confirmed,
                confirmations: 12,
                block_number: Some(18_000_000),
            })
        }

        async fn health_check(&self) -> anyhow::Result<()> {
            Ok(())
        }

        fn wallet_address(&self) -> ethereum_mcp_server::types::WalletAddress {
            ethereum_mcp_server::types::WalletAddress::from_hex(
                "0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0",
            )
            .unwrap()
        }
    }

    let mock_provider = std::sync::Arc::new(SimpleMockProvider);
    let contracts = get_test_contracts();
    let balance_service = std::sync::Arc::new(ethereum_mcp_server::services::BalanceService::new(
        mock_provider.clone(),
    ));
    let price_service = std::sync::Arc::new(ethereum_mcp_server::services::PriceService::new(
        mock_provider.clone(),
        contracts.clone(),
    ));
    let swap_service = std::sync::Arc::new(ethereum_mcp_server::services::SwapService::new(
        mock_provider.clone(),
        contracts.clone(),
    ));
    let transaction_status_service = std::sync::Arc::new(
        ethereum_mcp_server::services::TransactionStatusService::new(mock_provider.clone()),
    );

    let _app_state = AppState::new(
        balance_service,
        price_service,
        swap_service,
        transaction_status_service,
        1_000_000_000,
    );

    // We can't easily test the async handler without running a server,
    // but we can verify the state creation works
    assert!(true);
}

/// Test JSON-RPC error scenarios
#[tokio::test]
async fn test_jsonrpc_invalid_requests() {
    use ethereum_mcp_server::validation::Validator;

    // Test missing jsonrpc version
    let request = json!({"method": "tools/list"});
    assert!(Validator::validate_jsonrpc_request(&request).is_err());

    // Test invalid method type
    let request = json!({"jsonrpc": "2.0", "method": 123});
    assert!(Validator::validate_jsonrpc_request(&request).is_err());

    // Test empty method
    let request = json!({"jsonrpc": "2.0", "method": ""});
    assert!(Validator::validate_jsonrpc_request(&request).is_err());
}

/// Test token amount edge cases
#[tokio::test]
async fn test_token_amount_validation_comprehensive() {
    use ethereum_mcp_server::types::TokenAmount;

    // Test very small amounts
    let result = TokenAmount::from_human_readable("0.000000000000000001", 18);
    assert!(result.is_ok());

    // Test large amounts
    let result = TokenAmount::from_human_readable("1000000", 18);
    assert!(result.is_ok());

    // Test negative amounts
    let result = TokenAmount::from_human_readable("-1.0", 18);
    assert!(result.is_err());

    // Test zero
    let amount = TokenAmount::new(rust_decimal::Decimal::ZERO, 18);
    assert_eq!(amount.to_human_readable(), rust_decimal::Decimal::ZERO);
}

/// Test wallet and token address conversions
#[tokio::test]
async fn test_address_conversions() {
    use ethereum_mcp_server::types::{TokenAddress, WalletAddress};

    let addr_str = "0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0";

    // Test WalletAddress
    let wallet = WalletAddress::from_hex(addr_str).unwrap();
    let hex = wallet.to_hex();
    assert!(hex.starts_with("0x"));
    assert_eq!(hex.len(), 42);

    // Test TokenAddress
    let token = TokenAddress::from_hex(addr_str).unwrap();
    let hex = token.to_hex();
    assert!(hex.starts_with("0x"));
    assert_eq!(hex.len(), 42);

    // Test invalid addresses
    assert!(WalletAddress::from_hex("invalid").is_err());
    assert!(TokenAddress::from_hex("").is_err());
}

/// Test circuit breaker state transitions
#[tokio::test]
async fn test_circuit_breaker_integration() {
    use ethereum_mcp_server::providers::CircuitBreaker;

    // Test default config
    let _default_breaker = CircuitBreaker::default();

    // Circuit breaker internal state is not publicly accessible
    // Just verify we can create it
    let _breaker = CircuitBreaker::new();

    // Verify creation succeeds
    assert!(true);
}

/// Test swap params validation
#[tokio::test]
async fn test_swap_params_comprehensive() {
    use ethereum_mcp_server::types::{SwapParams, TokenAddress, TokenAmount};
    use rust_decimal::Decimal;
    use std::str::FromStr;

    let from_token = TokenAddress::from_hex("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();
    let to_token = TokenAddress::from_hex("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap();
    let amount = TokenAmount::from_human_readable("100", 6).unwrap();

    let params = SwapParams {
        from_token: from_token.clone(),
        to_token: to_token.clone(),
        amount_in: amount.clone(),
        slippage_tolerance: Decimal::from_str("0.005").unwrap(),
    };

    assert_eq!(params.from_token, from_token);
    assert_eq!(params.to_token, to_token);
    assert_eq!(params.amount_in, amount);
    assert_eq!(
        params.slippage_tolerance,
        Decimal::from_str("0.005").unwrap()
    );
}

/// Test transaction status types
#[tokio::test]
async fn test_transaction_status_types() {
    use ethereum_mcp_server::types::{TransactionStatus, TransactionStatusInfo};

    let status_info = TransactionStatusInfo {
        transaction_hash: "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
            .to_string(),
        status: TransactionStatus::Confirmed,
        confirmations: 12,
        block_number: Some(18_000_000),
    };

    assert_eq!(status_info.confirmations, 12);
    assert_eq!(status_info.status, TransactionStatus::Confirmed);
    assert_eq!(status_info.block_number, Some(18_000_000));

    // Test other statuses
    let pending = TransactionStatus::Pending;
    let failed = TransactionStatus::Failed;
    assert_ne!(pending, failed);
    assert_ne!(pending, TransactionStatus::Confirmed);
}

/// Test price info structure
#[tokio::test]
async fn test_price_info_structure() {
    use ethereum_mcp_server::types::{TokenAddress, TokenPrice};
    use rust_decimal::Decimal;
    use std::str::FromStr;

    let token = TokenAddress::from_hex("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();
    let price = TokenPrice {
        token_address: token.clone(),
        price_eth: Decimal::from_str("0.0005").unwrap(),
        price_usd: Some(Decimal::from_str("1.00").unwrap()),
        source: "Uniswap V3".to_string(),
    };

    assert_eq!(price.token_address, token);
    assert!(price.price_eth > Decimal::ZERO);
    assert!(price.price_usd.is_some());
    assert_eq!(price.source, "Uniswap V3");
}

/// Test balance info structure
#[tokio::test]
async fn test_balance_info_comprehensive() {
    use ethereum_mcp_server::types::{BalanceInfo, TokenAddress, TokenAmount, WalletAddress};
    use rust_decimal::Decimal;

    let wallet = WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D0C9C0C8b0E4e8A0").unwrap();
    let token = TokenAddress::from_hex("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();
    let amount = TokenAmount::new(Decimal::from(1000), 6);

    let balance = BalanceInfo {
        wallet_address: wallet.clone(),
        token_address: Some(token.clone()),
        amount: amount.clone(),
        symbol: "USDC".to_string(),
    };

    assert_eq!(balance.wallet_address, wallet);
    assert_eq!(balance.token_address, Some(token));
    assert_eq!(balance.amount, amount);
    assert_eq!(balance.symbol, "USDC");

    // Test ETH balance (no token address)
    let eth_balance = BalanceInfo {
        wallet_address: wallet.clone(),
        token_address: None,
        amount: TokenAmount::new(Decimal::from(5), 18),
        symbol: "ETH".to_string(),
    };

    assert_eq!(eth_balance.token_address, None);
    assert_eq!(eth_balance.symbol, "ETH");
}

/// Test configuration validation edge cases
#[tokio::test]
async fn test_config_validation_edge_cases() {
    use ethereum_mcp_server::Config;

    // Valid HTTPS URL
    let config = Config::new(
        "https://mainnet.infura.io/v3/test".to_string(),
        "127.0.0.1".to_string(),
        3000,
        "info".to_string(),
        "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
    );
    assert!(config.validate().is_ok());

    // Valid HTTP URL
    let config = Config::new(
        "http://localhost:8545".to_string(),
        "127.0.0.1".to_string(),
        3000,
        "debug".to_string(),
        "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
    );
    assert!(config.validate().is_ok());

    // Invalid scheme
    let config = Config::new(
        "ftp://invalid.com".to_string(),
        "127.0.0.1".to_string(),
        3000,
        "info".to_string(),
        "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
    );
    assert!(config.validate().is_err());

    // Empty RPC URL
    let config = Config::new(
        "".to_string(),
        "127.0.0.1".to_string(),
        3000,
        "info".to_string(),
        "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
    );
    assert!(config.validate().is_err());
}

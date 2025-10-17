/// Test fixtures and sample data for integration tests

/// Sample Ethereum addresses for testing
pub mod addresses {
    pub const VALID_WALLET_1: &str = "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7";
    pub const VALID_WALLET_2: &str = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
    
    pub const USDC_ADDRESS: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
    pub const WETH_ADDRESS: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
    pub const DAI_ADDRESS: &str = "0x6B175474E89094C44Da98b954EedeAC495271d0F";
    
    pub const ETH_ADDRESS: &str = "0x0000000000000000000000000000000000000000";
}

/// Sample JSON-RPC requests for testing
pub mod json_rpc {
    use serde_json::{json, Value};
    
    pub fn tools_list_request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "tools/list",
            "id": 1
        })
    }
    
    pub fn get_balance_request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "get_balance",
                "arguments": {
                    "wallet_address": super::addresses::VALID_WALLET_1
                }
            },
            "id": 2
        })
    }
    
    pub fn simulate_swap_request() -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "tools/call", 
            "params": {
                "name": "simulate_swap",
                "arguments": {
                    "from_token": super::addresses::USDC_ADDRESS,
                    "to_token": super::addresses::ETH_ADDRESS,
                    "amount_in": "100.0",
                    "slippage_tolerance": "0.005"
                }
            },
            "id": 3
        })
    }
    
    pub fn invalid_request_no_method() -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": 1
            // Missing method field
        })
    }
    
    pub fn invalid_request_wrong_version() -> Value {
        json!({
            "jsonrpc": "1.0",  // Wrong version
            "method": "tools/list",
            "id": 1
        })
    }
}

/// Test configuration values
pub mod config {
    pub const TEST_RPC_URL: &str = "https://mainnet.infura.io/v3/demo";
    pub const TEST_HOST: &str = "127.0.0.1";
    pub const TEST_PORT: u16 = 3001;
    pub const TEST_PRIVATE_KEY: &str = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
}

/// Performance test data
pub mod performance {
    /// Large dataset for stress testing
    pub fn generate_addresses(count: usize) -> Vec<String> {
        (0..count)
            .map(|i| format!("0x742d35Cc6634C0532925a3b8D8b5d0f8988D{:04x}", i))
            .collect()
    }
    
    /// Sample amounts for testing
    pub const AMOUNTS: &[&str] = &[
        "0.001", "0.1", "1.0", "10.0", "100.0", "1000.0"
    ];
    
    /// Sample slippage tolerances  
    pub const SLIPPAGES: &[&str] = &[
        "0.0001", "0.001", "0.005", "0.01", "0.05", "0.1"
    ];
}

/// Security test cases
pub mod security {
    /// Malicious inputs for security testing
    pub const ADDRESSES_WITH_NULL_BYTES: &[&str] = &[
        "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7\0",
        "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7\0\0",
    ];
    
    pub const ADDRESSES_WITH_CONTROL_CHARS: &[&str] = &[
        "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7\n",
        "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7\r",
        "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7\t",
    ];
    
    pub const INVALID_ADDRESSES: &[&str] = &[
        "",
        "0x",
        "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c", // Too short
        "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7a", // Too long  
        "742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7", // Missing 0x
        "0xGGGd35Cc6634C0532925a3b8D8b5d0f8988Db8c7", // Invalid hex
    ];
    
    pub const MALICIOUS_AMOUNTS: &[&str] = &[
        "-1.0",           // Negative
        "0",              // Zero  
        "999999999999999999999999999999999.0", // Too large
        "1.0\0",          // Null byte
        "1.0\n",          // Control character
        "",               // Empty
        "abc",            // Not a number
    ];
}

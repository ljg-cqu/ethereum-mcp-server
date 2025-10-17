/// Smart contract interfaces and addresses for Ethereum integration
/// Production-ready contract addresses and ABIs using alloy sol! macro
use alloy::sol;

// Chainlink AggregatorV3 interface for price feeds
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    interface IChainlinkAggregator {
        function latestRoundData()
            external
            view
            returns (
                uint80 roundId,
                int256 answer,
                uint256 startedAt,
                uint256 updatedAt,
                uint80 answeredInRound
            );

        function decimals() external view returns (uint8);

        function description() external view returns (string memory);
    }
}

/// Uniswap V3 fee tiers (in hundredths of a bip, so 3000 = 0.30%)
pub mod fees {
    pub const LOW: u32 = 500; // 0.05% for stablecoin pairs
    pub const MEDIUM: u32 = 3000; // 0.30% for most pairs
    pub const HIGH: u32 = 10000; // 1.00% for exotic pairs
}

// ERC20 token standard interface
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    interface IERC20 {
        function totalSupply() external view returns (uint256);
        function balanceOf(address account) external view returns (uint256);
        function decimals() external view returns (uint8);
        function symbol() external view returns (string memory);
        function name() external view returns (string memory);
    }
}

// Uniswap V3 Quoter interface for price queries
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    interface IUniswapV3Quoter {
        function quoteExactInputSingle(
            address tokenIn,
            address tokenOut,
            uint24 fee,
            uint256 amountIn,
            uint160 sqrtPriceLimitX96
        ) external returns (uint256 amountOut);

        function quoteExactOutputSingle(
            address tokenIn,
            address tokenOut,
            uint24 fee,
            uint256 amountOut,
            uint160 sqrtPriceLimitX96
        ) external returns (uint256 amountIn);
    }
}

// Uniswap V3 Router interface for swap simulations
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    interface IUniswapV3Router {
        struct ExactInputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 deadline;
            uint256 amountIn;
            uint256 amountOutMinimum;
            uint160 sqrtPriceLimitX96;
        }

        function exactInputSingle(ExactInputSingleParams calldata params)
            external payable returns (uint256 amountOut);

        struct ExactOutputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 deadline;
            uint256 amountOut;
            uint256 amountInMaximum;
            uint160 sqrtPriceLimitX96;
        }

        function exactOutputSingle(ExactOutputSingleParams calldata params)
            external payable returns (uint256 amountIn);
    }
}

// Uniswap V3 Factory interface for pool information
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    interface IUniswapV3Factory {
        function getPool(
            address tokenA,
            address tokenB,
            uint24 fee
        ) external view returns (address pool);
    }
}

/// Common utility functions for working with contracts
pub mod utils {
    use crate::types::{TokenAddress, WalletAddress};
    use crate::ContractAddresses;
    use alloy::primitives::Address;
    use anyhow::Result;

    /// Convert our TokenAddress type to alloy Address
    pub fn token_address_to_alloy(addr: &TokenAddress) -> Address {
        addr.address()
    }

    /// Convert our WalletAddress type to alloy Address  
    pub fn wallet_address_to_alloy(addr: &WalletAddress) -> Address {
        addr.address()
    }

    /// Parse string address to alloy Address with validation
    pub fn parse_address(addr_str: &str) -> Result<Address> {
        addr_str
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid address format '{}': {}", addr_str, e))
    }

    /// Get the most common fee tier for a token pair
    pub fn get_common_fee_tier(token_a: &str, token_b: &str, contracts: &ContractAddresses) -> u32 {
        use super::fees;

        // Stablecoin pairs use low fees
        let stablecoins = [&contracts.usdc, &contracts.usdt, &contracts.dai];
        let is_stablecoin_pair = stablecoins.contains(&&token_a.to_string())
            && stablecoins.contains(&&token_b.to_string());

        if is_stablecoin_pair {
            fees::LOW
        } else {
            fees::MEDIUM // ETH pairs and default to medium fees
        }
    }

    /// Resolve a token symbol to a known mainnet address
    pub fn resolve_token_address(symbol: &str, contracts: &ContractAddresses) -> Option<String> {
        let normalized = symbol.trim().to_ascii_uppercase();
        match normalized.as_str() {
            "USDC" => Some(contracts.usdc.clone()),
            "USDT" => Some(contracts.usdt.clone()),
            "DAI" => Some(contracts.dai.clone()),
            "WETH" | "ETH" => Some(contracts.weth.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fees, utils};
    use crate::types::{TokenAddress, WalletAddress};
    use crate::ContractAddresses;

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

    #[test]
    fn test_address_parsing() {
        let contracts = get_test_contracts();
        let addr = utils::parse_address(&contracts.usdc).unwrap();
        assert_eq!(
            addr.to_string().to_lowercase(),
            contracts.usdc.to_lowercase()
        );
    }

    #[test]
    fn test_address_parsing_invalid() {
        let result = utils::parse_address("invalid_address");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid address format"));
    }

    #[test]
    fn test_address_parsing_empty() {
        let result = utils::parse_address("");
        assert!(result.is_err());
    }

    #[test]
    fn test_fee_tier_selection() {
        let contracts = get_test_contracts();
        // Stablecoin pair should get low fees
        assert_eq!(
            utils::get_common_fee_tier(&contracts.usdc, &contracts.dai, &contracts),
            fees::LOW
        );

        // ETH pair should get medium fees
        assert_eq!(
            utils::get_common_fee_tier(&contracts.weth, &contracts.usdc, &contracts),
            fees::MEDIUM
        );

        // Other pairs should get medium fees (default)
        assert_eq!(
            utils::get_common_fee_tier("0x1234", "0x5678", &contracts),
            fees::MEDIUM
        );
    }

    #[test]
    fn test_fee_tier_stablecoin_combinations() {
        let contracts = get_test_contracts();
        // Test all stablecoin combinations
        assert_eq!(
            utils::get_common_fee_tier(&contracts.usdc, &contracts.usdt, &contracts),
            fees::LOW
        );
        assert_eq!(
            utils::get_common_fee_tier(&contracts.usdt, &contracts.dai, &contracts),
            fees::LOW
        );
        assert_eq!(
            utils::get_common_fee_tier(&contracts.dai, &contracts.usdc, &contracts),
            fees::LOW
        );

        // Reverse order should also work
        assert_eq!(
            utils::get_common_fee_tier(&contracts.usdt, &contracts.usdc, &contracts),
            fees::LOW
        );
        assert_eq!(
            utils::get_common_fee_tier(&contracts.dai, &contracts.usdt, &contracts),
            fees::LOW
        );
    }

    #[test]
    fn test_symbol_resolution() {
        let contracts = get_test_contracts();
        assert_eq!(
            utils::resolve_token_address("usdc", &contracts),
            Some(contracts.usdc.clone())
        );
        assert_eq!(
            utils::resolve_token_address("USDT", &contracts),
            Some(contracts.usdt.clone())
        );
        assert_eq!(
            utils::resolve_token_address("Eth", &contracts),
            Some(contracts.weth.clone())
        );
        assert_eq!(utils::resolve_token_address("unknown", &contracts), None);
    }

    #[test]
    fn test_fee_tier_eth_pairs() {
        let contracts = get_test_contracts();
        // Test ETH pairs with different tokens
        assert_eq!(
            utils::get_common_fee_tier(&contracts.weth, &contracts.usdt, &contracts),
            fees::MEDIUM
        );
        assert_eq!(
            utils::get_common_fee_tier(&contracts.weth, &contracts.dai, &contracts),
            fees::MEDIUM
        );
        assert_eq!(
            utils::get_common_fee_tier(&contracts.usdc, &contracts.weth, &contracts),
            fees::MEDIUM
        );

        // Non-ETH, non-stablecoin pairs
        assert_eq!(
            utils::get_common_fee_tier("0xabc123", "0xdef456", &contracts),
            fees::MEDIUM
        );
    }

    #[test]
    fn test_token_address_conversion() {
        let contracts = get_test_contracts();
        let token_addr = TokenAddress::from_hex(&contracts.usdc).unwrap();
        let alloy_addr = utils::token_address_to_alloy(&token_addr);
        assert_eq!(
            alloy_addr.to_string().to_lowercase(),
            contracts.usdc.to_lowercase()
        );
    }

    #[test]
    fn test_wallet_address_conversion() {
        let wallet_addr =
            WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D4C4C0b8047cc6E1").unwrap();
        let alloy_addr = utils::wallet_address_to_alloy(&wallet_addr);
        assert_eq!(
            alloy_addr.to_string().to_lowercase(),
            "0x742d35cc6634c0532925a3b8d4c4c0b8047cc6e1"
        );
    }

    #[test]
    fn test_contract_addresses_format() {
        let contracts = get_test_contracts();
        // Test that all contract addresses are valid Ethereum addresses
        let addresses_to_test = [
            &contracts.usdc,
            &contracts.usdt,
            &contracts.dai,
            &contracts.weth,
            &contracts.uniswap_v3_factory,
            &contracts.uniswap_v3_router,
            &contracts.uniswap_v3_quoter,
            &contracts.chainlink_eth_usd_feed,
        ];

        for addr in addresses_to_test {
            assert!(
                utils::parse_address(addr).is_ok(),
                "Invalid address: {}",
                addr
            );
            assert!(
                addr.starts_with("0x"),
                "Address should start with 0x: {}",
                addr
            );
            assert_eq!(addr.len(), 42, "Address should be 42 characters: {}", addr);
        }
    }

    #[test]
    fn test_fee_constants() {
        // Test that fee constants are reasonable values
        assert_eq!(fees::LOW, 500);
        assert_eq!(fees::MEDIUM, 3000);
        assert_eq!(fees::HIGH, 10000);

        // Ensure they're in ascending order (compile-time constants)
        // These assertions are on constants and will be optimized out
        #[allow(clippy::assertions_on_constants)]
        {
            assert!(fees::LOW < fees::MEDIUM);
            assert!(fees::MEDIUM < fees::HIGH);
        }
    }

    #[test]
    fn test_address_case_insensitive_parsing() {
        let contracts = get_test_contracts();
        // Test that address parsing works with different cases
        // Use the USDC address which is already 42 characters
        let original = &contracts.usdc;
        let lowercase = original.to_lowercase();

        let addr1 = utils::parse_address(&lowercase).unwrap();
        let addr2 = utils::parse_address(original).unwrap();

        // Both should parse to the same address
        assert_eq!(addr1, addr2);
    }
}

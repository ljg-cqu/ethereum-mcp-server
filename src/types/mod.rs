/// Domain types for Ethereum MCP server
/// Following SOLID principles with clear separation of concerns
use alloy::primitives::Address;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Ethereum wallet address with validation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WalletAddress(Address);

impl WalletAddress {
    /// Create a new wallet address with validation
    pub fn new(address: Address) -> Self {
        Self(address)
    }

    /// Get the inner address
    pub fn address(&self) -> Address {
        self.0
    }

    /// Create from hex string with validation
    pub fn from_hex(hex: &str) -> anyhow::Result<Self> {
        let address = Address::from_str(hex)
            .map_err(|_| anyhow::anyhow!("Invalid Ethereum address format: {}", hex))?;
        Ok(Self(address))
    }

    /// Convert to checksummed hex string
    pub fn to_hex(&self) -> String {
        format!("{:#x}", self.0)
    }
}

impl FromStr for WalletAddress {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

/// Token contract address
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenAddress(Address);

impl TokenAddress {
    pub fn new(address: Address) -> Self {
        Self(address)
    }

    pub fn address(&self) -> Address {
        self.0
    }

    pub fn from_hex(hex: &str) -> anyhow::Result<Self> {
        let address = Address::from_str(hex)
            .map_err(|_| anyhow::anyhow!("Invalid token contract address: {}", hex))?;
        Ok(Self(address))
    }

    pub fn to_hex(&self) -> String {
        format!("{:#x}", self.0)
    }
}

impl FromStr for TokenAddress {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

/// Token amount with proper decimal handling
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenAmount {
    pub raw: Decimal,
    pub decimals: u8,
}

impl TokenAmount {
    /// Create new token amount with explicit decimals
    pub fn new(raw: Decimal, decimals: u8) -> Self {
        Self { raw, decimals }
    }

    /// Create from human-readable amount (e.g., "1.5" for 1.5 tokens)
    pub fn from_human_readable(amount: &str, decimals: u8) -> anyhow::Result<Self> {
        let value = Decimal::from_str(amount)?;
        if value.is_sign_negative() {
            return Err(anyhow::anyhow!("Token amounts cannot be negative"));
        }
        Ok(Self::new(value, decimals))
    }

    /// Create from raw units (e.g., wei for ETH)
    pub fn from_raw_units(raw_value: Decimal, decimals: u8) -> Self {
        let divisor = Decimal::from(10_u64.pow(decimals as u32));
        let value = raw_value / divisor;
        Self::new(value, decimals)
    }

    /// Get raw units (multiply by 10^decimals) with overflow checking
    pub fn to_raw_units(&self) -> anyhow::Result<Decimal> {
        let multiplier = Decimal::from(10_u64.pow(self.decimals as u32));
        self.raw.checked_mul(multiplier).ok_or_else(|| {
            anyhow::anyhow!(
                "Overflow when converting {} to raw units with {} decimals",
                self.raw,
                self.decimals
            )
        })
    }

    /// Get human-readable decimal value
    pub fn to_human_readable(&self) -> Decimal {
        self.raw
    }

    /// Format for display
    pub fn format(&self) -> String {
        format!("{}", self.raw)
    }
}

/// Balance information for a wallet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceInfo {
    pub wallet_address: WalletAddress,
    pub token_address: Option<TokenAddress>,
    pub amount: TokenAmount,
    pub symbol: String,
}

/// Token price information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPrice {
    pub token_address: TokenAddress,
    pub price_eth: Decimal,
    pub price_usd: Option<Decimal>,
    pub source: String,
}

/// Swap simulation parameters
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwapParams {
    pub from_token: TokenAddress,
    pub to_token: TokenAddress,
    pub amount_in: TokenAmount,
    pub slippage_tolerance: Decimal,
}

/// Swap simulation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapResult {
    pub params: SwapParams,
    pub estimated_amount_out: TokenAmount,
    pub price_impact: Decimal,
    pub gas_estimate: u64,
    pub gas_cost_eth: Option<Decimal>,
    pub route: String,
}

/// The status of an on-chain transaction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
    NotFound,
}

/// Information about a transaction's status and confirmations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionStatusInfo {
    pub transaction_hash: String,
    pub status: TransactionStatus,
    pub confirmations: u64,
    pub block_number: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn test_wallet_address_creation() {
        let addr_str = "0x742d35Cc6634C0532925a3b8D4C4C0b8047cc6E1";
        let wallet_addr = WalletAddress::from_hex(addr_str).unwrap();
        assert_eq!(wallet_addr.to_hex().to_lowercase(), addr_str.to_lowercase());
    }

    #[test]
    fn test_wallet_address_invalid() {
        assert!(WalletAddress::from_hex("invalid").is_err());
        assert!(WalletAddress::from_hex("0x123").is_err()); // Too short
    }

    #[test]
    fn test_token_address_creation() {
        let addr_str = "0xA0b86a33E6441E12Ecdf119F4ce5e6B76e252D3F";
        let token_addr = TokenAddress::from_hex(addr_str).unwrap();
        assert_eq!(token_addr.to_hex().to_lowercase(), addr_str.to_lowercase());
    }

    #[test]
    fn test_token_address_invalid() {
        assert!(TokenAddress::from_hex("invalid").is_err());
        assert!(TokenAddress::from_hex("").is_err());
    }

    #[test]
    fn test_token_amount_creation() {
        let amount = TokenAmount::new(Decimal::from_str("1.5").unwrap(), 18);
        assert_eq!(amount.raw, Decimal::from_str("1.5").unwrap());
        assert_eq!(amount.decimals, 18);
    }

    #[test]
    fn test_token_amount_from_human_readable() {
        let amount = TokenAmount::from_human_readable("1.5", 18).unwrap();
        assert_eq!(amount.raw, Decimal::from_str("1.5").unwrap());
        assert_eq!(amount.decimals, 18);
    }

    #[test]
    fn test_token_amount_negative_rejected() {
        assert!(TokenAmount::from_human_readable("-1.0", 18).is_err());
    }

    #[test]
    fn test_token_amount_from_raw_units() {
        let raw = Decimal::from(1500000000000000000u64); // 1.5 ETH in wei
        let amount = TokenAmount::from_raw_units(raw, 18);
        assert_eq!(amount.raw, Decimal::from_str("1.5").unwrap());
        assert_eq!(amount.decimals, 18);
    }

    #[test]
    fn test_token_amount_to_raw_units() {
        let amount = TokenAmount::new(Decimal::from_str("1.5").unwrap(), 18);
        let raw = amount.to_raw_units().unwrap();
        assert_eq!(raw, Decimal::from(1500000000000000000u64));
    }

    #[test]
    fn test_token_amount_round_trip() {
        let original = Decimal::from_str("123.456789").unwrap();
        let amount = TokenAmount::new(original, 18);
        let raw = amount.to_raw_units().unwrap();
        let reconstructed = TokenAmount::from_raw_units(raw, 18);
        assert_eq!(amount.raw, reconstructed.raw);
    }

    #[test]
    fn test_token_amount_different_decimals() {
        // Test with 6 decimals (like USDC)
        let amount = TokenAmount::from_human_readable("1.5", 6).unwrap();
        let raw = amount.to_raw_units().unwrap();
        assert_eq!(raw, Decimal::from(1500000u64));

        // Test with 8 decimals
        let amount = TokenAmount::from_human_readable("1.5", 8).unwrap();
        let raw = amount.to_raw_units().unwrap();
        assert_eq!(raw, Decimal::from(150000000u64));
    }

    #[test]
    fn test_token_amount_format() {
        let amount = TokenAmount::new(Decimal::from_str("1.5").unwrap(), 18);
        assert_eq!(amount.format(), "1.5");
    }

    #[test]
    fn test_balance_info_creation() {
        let wallet = WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D4C4C0b8047cc6E1").unwrap();
        let token = TokenAddress::from_hex("0xA0b86a33E6441E12Ecdf119F4ce5e6B76e252D3F").unwrap();
        let amount = TokenAmount::new(Decimal::from_str("42.5").unwrap(), 18);

        let balance_info = BalanceInfo {
            wallet_address: wallet.clone(),
            token_address: Some(token.clone()),
            amount: amount.clone(),
            symbol: "USDC".to_string(),
        };

        assert_eq!(balance_info.wallet_address, wallet);
        assert_eq!(balance_info.token_address, Some(token));
        assert_eq!(balance_info.amount, amount);
        assert_eq!(balance_info.symbol, "USDC");
    }

    #[test]
    fn test_token_price_creation() {
        let token = TokenAddress::from_hex("0xA0b86a33E6441E12Ecdf119F4ce5e6B76e252D3F").unwrap();
        let price = TokenPrice {
            token_address: token.clone(),
            price_eth: Decimal::from_str("0.001").unwrap(),
            price_usd: Some(Decimal::from_str("2.50").unwrap()),
            source: "Uniswap".to_string(),
        };

        assert_eq!(price.token_address, token);
        assert_eq!(price.price_eth, Decimal::from_str("0.001").unwrap());
        assert_eq!(price.price_usd, Some(Decimal::from_str("2.50").unwrap()));
        assert_eq!(price.source, "Uniswap");
    }

    #[test]
    fn test_swap_params_creation() {
        let from_token =
            TokenAddress::from_hex("0xA0b86a33E6441E12Ecdf119F4ce5e6B76e252D3F").unwrap();
        let to_token =
            TokenAddress::from_hex("0xdAC17F958D2ee523a2206206994597C13D831ec7").unwrap();
        let amount = TokenAmount::new(Decimal::from_str("100.0").unwrap(), 6);

        let params = SwapParams {
            from_token: from_token.clone(),
            to_token: to_token.clone(),
            amount_in: amount.clone(),
            slippage_tolerance: Decimal::from_str("0.01").unwrap(), // 1%
        };

        assert_eq!(params.from_token, from_token);
        assert_eq!(params.to_token, to_token);
        assert_eq!(params.amount_in, amount);
        assert_eq!(
            params.slippage_tolerance,
            Decimal::from_str("0.01").unwrap()
        );
    }

    #[test]
    fn test_swap_result_creation() {
        let from_token =
            TokenAddress::from_hex("0xA0b86a33E6441E12Ecdf119F4ce5e6B76e252D3F").unwrap();
        let to_token =
            TokenAddress::from_hex("0xdAC17F958D2ee523a2206206994597C13D831ec7").unwrap();
        let amount_in = TokenAmount::new(Decimal::from_str("100.0").unwrap(), 6);
        let amount_out = TokenAmount::new(Decimal::from_str("2500.0").unwrap(), 6);

        let params = SwapParams {
            from_token,
            to_token,
            amount_in,
            slippage_tolerance: Decimal::from_str("0.01").unwrap(),
        };

        let result = SwapResult {
            params: params.clone(),
            estimated_amount_out: amount_out.clone(),
            price_impact: Decimal::from_str("0.05").unwrap(),
            gas_estimate: 150000,
            gas_cost_eth: Some(Decimal::from_str("0.012").unwrap()),
            route: "uniswap_v3".to_string(),
        };

        assert_eq!(result.params, params);
        assert_eq!(result.estimated_amount_out, amount_out);
        assert_eq!(result.price_impact, Decimal::from_str("0.05").unwrap());
        assert_eq!(result.gas_estimate, 150000);
        assert_eq!(
            result.gas_cost_eth,
            Some(Decimal::from_str("0.012").unwrap())
        );
    }

    #[test]
    fn test_address_from_str_trait() {
        let addr_str = "0x742d35Cc6634C0532925a3b8D4C4C0b8047cc6E1";
        let wallet_addr: WalletAddress = addr_str.parse().unwrap();
        let token_addr: TokenAddress = addr_str.parse().unwrap();

        assert_eq!(wallet_addr.to_hex().to_lowercase(), addr_str.to_lowercase());
        assert_eq!(token_addr.to_hex().to_lowercase(), addr_str.to_lowercase());
    }

    #[test]
    fn test_token_amount_zero() {
        let amount = TokenAmount::new(Decimal::ZERO, 18);
        assert_eq!(amount.raw, Decimal::ZERO);
        assert_eq!(amount.to_raw_units().unwrap(), Decimal::ZERO);
    }

    #[test]
    fn test_token_amount_large_values() {
        let large_amount = TokenAmount::new(Decimal::from_str("1000000000.0").unwrap(), 18);
        let raw = large_amount.to_raw_units().unwrap();
        let reconstructed = TokenAmount::from_raw_units(raw, 18);
        assert_eq!(large_amount.raw, reconstructed.raw);
    }

    #[test]
    fn test_token_amount_overflow_detection() {
        // Test that very large values don't overflow
        let huge = TokenAmount::new(Decimal::MAX, 18);
        let result = huge.to_raw_units();
        assert!(result.is_err(), "Should detect overflow");
        assert!(result.unwrap_err().to_string().contains("Overflow"));
    }
}

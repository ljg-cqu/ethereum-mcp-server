/// Price service implementation
/// Single Responsibility: Handle token price queries
use crate::providers::EthereumProvider;
use crate::{
    types::{TokenAddress, TokenPrice},
    ContractAddresses,
};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, instrument};

#[async_trait]
pub trait PriceServiceTrait: Send + Sync {
    async fn get_token_price(&self, token: &TokenAddress) -> anyhow::Result<TokenPrice>;
}

pub struct PriceService {
    pub ethereum_provider: Arc<dyn EthereumProvider>,
    pub contracts: ContractAddresses,
}

impl PriceService {
    pub fn new(ethereum_provider: Arc<dyn EthereumProvider>, contracts: ContractAddresses) -> Self {
        Self {
            ethereum_provider,
            contracts,
        }
    }

    /// Get access to the ethereum provider (needed for fetching token decimals)
    pub fn ethereum_provider(&self) -> Arc<dyn EthereumProvider> {
        self.ethereum_provider.clone()
    }
}

#[async_trait]
impl PriceServiceTrait for PriceService {
    #[instrument(skip(self), fields(token = %token.to_hex()))]
    async fn get_token_price(&self, token: &TokenAddress) -> anyhow::Result<TokenPrice> {
        debug!("Getting price for token");
        self.ethereum_provider
            .get_token_price(token, &self.contracts)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::MockEthereumProvider;
    use crate::ContractAddresses;
    use rust_decimal::Decimal;
    use std::str::FromStr;

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
    async fn test_get_token_price() {
        let mut mock_provider = MockEthereumProvider::new();
        let contracts = get_test_contracts();
        let token = TokenAddress::from_hex(&contracts.usdc).unwrap();

        let expected_price = TokenPrice {
            token_address: token.clone(),
            price_eth: Decimal::from_str("0.001234").unwrap(),
            price_usd: Some(Decimal::from_str("2.45").unwrap()),
            source: "uniswap_v3".to_string(),
        };

        let token_clone = token.clone();
        let expected_price_clone = expected_price.clone();
        mock_provider
            .expect_get_token_price()
            .withf(move |t, c| t == &token_clone && c.usdc == contracts.usdc)
            .times(1)
            .returning(move |_, _| Ok(expected_price_clone.clone()));

        let service = PriceService::new(Arc::new(mock_provider), get_test_contracts());
        let result = service.get_token_price(&token).await.unwrap();

        assert_eq!(result.price_eth, Decimal::from_str("0.001234").unwrap());
        assert_eq!(result.source, "uniswap_v3");
    }
}

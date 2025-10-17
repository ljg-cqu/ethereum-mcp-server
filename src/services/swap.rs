/// Swap service implementation
/// Single Responsibility: Handle token swap simulations
use crate::providers::EthereumProvider;
use crate::{
    types::{SwapParams, SwapResult},
    ContractAddresses,
};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, instrument};

#[async_trait]
pub trait SwapServiceTrait: Send + Sync {
    async fn simulate_swap(&self, params: &SwapParams) -> anyhow::Result<SwapResult>;
}

pub struct SwapService {
    pub ethereum_provider: Arc<dyn EthereumProvider>,
    pub contracts: ContractAddresses,
}

impl SwapService {
    pub fn new(ethereum_provider: Arc<dyn EthereumProvider>, contracts: ContractAddresses) -> Self {
        Self {
            ethereum_provider,
            contracts,
        }
    }
}

#[async_trait]
impl SwapServiceTrait for SwapService {
    #[instrument(skip(self), fields(from_token = %params.from_token.to_hex(), to_token = %params.to_token.to_hex()))]
    async fn simulate_swap(&self, params: &SwapParams) -> anyhow::Result<SwapResult> {
        debug!("Simulating token swap");
        self.ethereum_provider
            .simulate_swap(params, &self.contracts)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::MockEthereumProvider;
    use crate::types::{TokenAddress, TokenAmount};
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
    async fn test_simulate_swap() {
        let mut mock_provider = MockEthereumProvider::new();
        let contracts = get_test_contracts();
        let from_token = TokenAddress::from_hex(&contracts.usdc).unwrap();
        let to_token = TokenAddress::from_hex(&contracts.dai).unwrap();

        let swap_params = SwapParams {
            from_token: from_token.clone(),
            to_token: to_token.clone(),
            amount_in: TokenAmount::from_human_readable("1.0", 18).unwrap(),
            slippage_tolerance: Decimal::from_str("0.5").unwrap(),
        };

        let expected_result = SwapResult {
            params: swap_params.clone(),
            estimated_amount_out: TokenAmount::from_human_readable("98.5", 18).unwrap(),
            price_impact: Decimal::from_str("0.12").unwrap(),
            gas_estimate: 180000,
            gas_cost_eth: Some(Decimal::from_str("0.012").unwrap()),
            route: "uniswap_v3".to_string(),
        };

        let swap_params_clone = swap_params.clone();
        let expected_result_clone = expected_result.clone();
        mock_provider
            .expect_simulate_swap()
            .withf(move |p, c| p == &swap_params_clone && c.usdc == contracts.usdc)
            .times(1)
            .returning(move |_, _| Ok(expected_result_clone.clone()));

        let service = SwapService::new(Arc::new(mock_provider), get_test_contracts());
        let result = service.simulate_swap(&swap_params).await.unwrap();

        assert_eq!(result.gas_estimate, 180000);
        assert_eq!(result.route, "uniswap_v3");
    }
}

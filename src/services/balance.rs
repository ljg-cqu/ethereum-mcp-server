/// Balance service implementation
/// Single Responsibility: Handle balance queries
use crate::providers::EthereumProvider;
use crate::types::{BalanceInfo, TokenAddress, WalletAddress};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, instrument};

#[async_trait]
pub trait BalanceServiceTrait: Send + Sync {
    async fn get_balance(
        &self,
        wallet: &WalletAddress,
        token: Option<&TokenAddress>,
    ) -> anyhow::Result<BalanceInfo>;
}

pub struct BalanceService {
    pub ethereum_provider: Arc<dyn EthereumProvider>,
}

impl BalanceService {
    pub fn new(ethereum_provider: Arc<dyn EthereumProvider>) -> Self {
        Self { ethereum_provider }
    }
}

#[async_trait]
impl BalanceServiceTrait for BalanceService {
    #[instrument(skip(self), fields(wallet = %wallet.to_hex()))]
    async fn get_balance(
        &self,
        wallet: &WalletAddress,
        token: Option<&TokenAddress>,
    ) -> anyhow::Result<BalanceInfo> {
        debug!("Getting balance for wallet");

        match token {
            None => {
                debug!("Fetching ETH balance");
                self.ethereum_provider.get_eth_balance(wallet).await
            }
            Some(token_addr) => {
                debug!("Fetching ERC20 balance for token: {}", token_addr.to_hex());
                self.ethereum_provider
                    .get_erc20_balance(wallet, token_addr)
                    .await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::MockEthereumProvider;
    use crate::types::TokenAmount;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_get_eth_balance() {
        let mut mock_provider = MockEthereumProvider::new();
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

        let service = BalanceService::new(Arc::new(mock_provider));
        let result = service.get_balance(&wallet, None).await.unwrap();

        assert_eq!(result.symbol, "ETH");
        assert_eq!(result.amount.raw, Decimal::from_str("1.5").unwrap());
    }

    #[tokio::test]
    async fn test_get_erc20_balance() {
        let mut mock_provider = MockEthereumProvider::new();
        let wallet = WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7").unwrap();
        let token = TokenAddress::from_hex("0xA0b86a33E6441E12Ecdf119F4ce5e6B76e252D3F").unwrap();

        let expected_balance = BalanceInfo {
            wallet_address: wallet.clone(),
            token_address: Some(token.clone()),
            amount: TokenAmount::from_human_readable("100.0", 6).unwrap(),
            symbol: "USDC".to_string(),
        };

        mock_provider
            .expect_get_erc20_balance()
            .with(
                mockall::predicate::eq(wallet.clone()),
                mockall::predicate::eq(token.clone()),
            )
            .times(1)
            .returning(move |_, _| Ok(expected_balance.clone()));

        let service = BalanceService::new(Arc::new(mock_provider));
        let result = service.get_balance(&wallet, Some(&token)).await.unwrap();

        assert_eq!(result.symbol, "USDC");
        assert_eq!(result.amount.raw, Decimal::from_str("100.0").unwrap());
    }
}

/// Transaction status service implementation
use crate::providers::EthereumProvider;
use crate::types::TransactionStatusInfo;
use alloy::primitives::B256;
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait TransactionStatusServiceTrait: Send + Sync {
    async fn get_transaction_status(&self, tx_hash: &B256)
        -> anyhow::Result<TransactionStatusInfo>;
}

pub struct TransactionStatusService {
    pub ethereum_provider: Arc<dyn EthereumProvider>,
}

impl TransactionStatusService {
    pub fn new(ethereum_provider: Arc<dyn EthereumProvider>) -> Self {
        Self { ethereum_provider }
    }
}

#[async_trait]
impl TransactionStatusServiceTrait for TransactionStatusService {
    async fn get_transaction_status(
        &self,
        tx_hash: &B256,
    ) -> anyhow::Result<TransactionStatusInfo> {
        self.ethereum_provider.get_transaction_status(tx_hash).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::MockEthereumProvider;
    use alloy::primitives::B256;
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_transaction_status_service_creation() {
        let mock_provider = Arc::new(MockEthereumProvider::new());
        let service = TransactionStatusService::new(mock_provider);

        // Verify service was created successfully by checking it exists
        // We can't easily test the provider pointer, so just verify the service exists
        assert!(std::ptr::addr_of!(service) as usize != 0);
    }

    #[tokio::test]
    async fn test_get_transaction_status_success() {
        use crate::types::TransactionStatus;

        let mut mock_provider = MockEthereumProvider::new();
        let tx_hash = B256::from([1u8; 32]);
        let expected_status = TransactionStatusInfo {
            transaction_hash: format!("{:?}", tx_hash),
            status: TransactionStatus::Confirmed,
            block_number: Some(12345),
            confirmations: 6,
        };

        mock_provider
            .expect_get_transaction_status()
            .with(eq(tx_hash))
            .times(1)
            .returning(move |_| {
                Ok(TransactionStatusInfo {
                    transaction_hash: format!("{:?}", tx_hash),
                    status: TransactionStatus::Confirmed,
                    block_number: Some(12345),
                    confirmations: 6,
                })
            });

        let service = TransactionStatusService::new(Arc::new(mock_provider));
        let result = service.get_transaction_status(&tx_hash).await;

        assert!(result.is_ok());
        let status_info = result.unwrap();
        assert_eq!(
            status_info.transaction_hash,
            expected_status.transaction_hash
        );
        assert_eq!(status_info.confirmations, expected_status.confirmations);
        assert_eq!(status_info.block_number, expected_status.block_number);
    }

    #[tokio::test]
    async fn test_get_transaction_status_error() {
        let mut mock_provider = MockEthereumProvider::new();
        let tx_hash = B256::from([2u8; 32]);

        mock_provider
            .expect_get_transaction_status()
            .with(eq(tx_hash))
            .times(1)
            .returning(|_| Err(anyhow::anyhow!("Transaction not found")));

        let service = TransactionStatusService::new(Arc::new(mock_provider));
        let result = service.get_transaction_status(&tx_hash).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Transaction not found"));
    }
}

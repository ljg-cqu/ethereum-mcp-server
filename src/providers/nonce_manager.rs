use crate::types::WalletAddress;
/// Nonce management for sequential transaction ordering
/// Prevents nonce conflicts in concurrent transaction scenarios
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, warn};

/// Thread-safe nonce manager for Ethereum transactions
#[derive(Debug)]
pub struct NonceManager {
    /// Current nonce for each wallet address
    nonces: Arc<Mutex<HashMap<WalletAddress, u64>>>,
}

impl NonceManager {
    /// Create a new nonce manager
    pub fn new() -> Self {
        Self {
            nonces: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get the next nonce for a wallet address
    /// This method is thread-safe and ensures sequential nonce allocation
    pub async fn get_next_nonce(&self, wallet_address: &WalletAddress) -> u64 {
        let mut nonces = self.nonces.lock().await;
        let current_nonce = nonces.get(wallet_address).copied().unwrap_or(0);
        let next_nonce = current_nonce + 1;
        nonces.insert(wallet_address.clone(), next_nonce);

        debug!(
            wallet = %wallet_address.to_hex(),
            nonce = next_nonce,
            "Allocated next nonce"
        );

        next_nonce
    }

    /// Initialize nonce for a wallet address from the blockchain
    /// Should be called when first connecting to ensure nonce synchronization
    pub async fn initialize_nonce(&self, wallet_address: &WalletAddress, blockchain_nonce: u64) {
        let mut nonces = self.nonces.lock().await;
        let current_local_nonce = nonces.get(wallet_address).copied().unwrap_or(0);

        if blockchain_nonce > current_local_nonce {
            nonces.insert(wallet_address.clone(), blockchain_nonce);
            debug!(
                wallet = %wallet_address.to_hex(),
                blockchain_nonce = blockchain_nonce,
                local_nonce = current_local_nonce,
                "Synchronized nonce with blockchain"
            );
        } else if blockchain_nonce < current_local_nonce {
            warn!(
                wallet = %wallet_address.to_hex(),
                blockchain_nonce = blockchain_nonce,
                local_nonce = current_local_nonce,
                "Local nonce ahead of blockchain - possible pending transactions"
            );
        }
    }

    /// Reset nonce for a wallet address (use with caution)
    /// This should only be used in error recovery scenarios
    pub async fn reset_nonce(&self, wallet_address: &WalletAddress, new_nonce: u64) {
        let mut nonces = self.nonces.lock().await;
        nonces.insert(wallet_address.clone(), new_nonce);

        warn!(
            wallet = %wallet_address.to_hex(),
            new_nonce = new_nonce,
            "Nonce reset - this may indicate transaction failures"
        );
    }

    /// Get current nonce without incrementing (for read-only operations)
    pub async fn get_current_nonce(&self, wallet_address: &WalletAddress) -> Option<u64> {
        let nonces = self.nonces.lock().await;
        nonces.get(wallet_address).copied()
    }

    /// Handle nonce conflict by resynchronizing with blockchain
    /// Returns the corrected nonce that should be used
    pub async fn handle_nonce_conflict(
        &self,
        wallet_address: &WalletAddress,
        failed_nonce: u64,
        blockchain_nonce: u64,
    ) -> u64 {
        let mut nonces = self.nonces.lock().await;

        warn!(
            wallet = %wallet_address.to_hex(),
            failed_nonce = failed_nonce,
            blockchain_nonce = blockchain_nonce,
            "Handling nonce conflict - resynchronizing"
        );

        // Use the blockchain nonce as the source of truth
        nonces.insert(wallet_address.clone(), blockchain_nonce);
        blockchain_nonce + 1
    }
}

impl Default for NonceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::WalletAddress;

    #[tokio::test]
    async fn test_nonce_allocation() {
        let manager = NonceManager::new();
        let wallet = WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7").unwrap();

        let nonce1 = manager.get_next_nonce(&wallet).await;
        let nonce2 = manager.get_next_nonce(&wallet).await;
        let nonce3 = manager.get_next_nonce(&wallet).await;

        assert_eq!(nonce1, 1);
        assert_eq!(nonce2, 2);
        assert_eq!(nonce3, 3);
    }

    #[tokio::test]
    async fn test_nonce_initialization() {
        let manager = NonceManager::new();
        let wallet = WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7").unwrap();

        // Initialize with blockchain nonce
        manager.initialize_nonce(&wallet, 10).await;

        let next_nonce = manager.get_next_nonce(&wallet).await;
        assert_eq!(next_nonce, 11);
    }

    #[tokio::test]
    async fn test_concurrent_nonce_allocation() {
        let manager = Arc::new(NonceManager::new());
        let wallet = WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7").unwrap();

        let mut handles = vec![];

        // Spawn 10 concurrent tasks requesting nonces
        for _ in 0..10 {
            let manager_clone = manager.clone();
            let wallet_clone = wallet.clone();
            let handle =
                tokio::spawn(async move { manager_clone.get_next_nonce(&wallet_clone).await });
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

    #[tokio::test]
    async fn test_nonce_conflict_handling() {
        let manager = NonceManager::new();
        let wallet = WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7").unwrap();

        // Simulate a nonce conflict scenario
        manager.initialize_nonce(&wallet, 5).await;
        let _nonce1 = manager.get_next_nonce(&wallet).await; // Should be 6
        let _nonce2 = manager.get_next_nonce(&wallet).await; // Should be 7

        // Simulate blockchain showing nonce 5 (transaction failed)
        let corrected_nonce = manager.handle_nonce_conflict(&wallet, 6, 5).await;
        assert_eq!(corrected_nonce, 6);

        // Next nonce should continue from corrected value
        let next_nonce = manager.get_next_nonce(&wallet).await;
        assert_eq!(next_nonce, 6);
    }

    #[tokio::test]
    async fn test_reset_nonce() {
        let manager = NonceManager::new();
        let wallet = WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7").unwrap();

        // Allocate some nonces
        let _nonce1 = manager.get_next_nonce(&wallet).await;
        let _nonce2 = manager.get_next_nonce(&wallet).await;

        // Reset to a specific nonce
        manager.reset_nonce(&wallet, 10).await;

        // Next nonce should be 11
        let next_nonce = manager.get_next_nonce(&wallet).await;
        assert_eq!(next_nonce, 11);
    }

    #[tokio::test]
    async fn test_get_current_nonce() {
        let manager = NonceManager::new();
        let wallet = WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7").unwrap();

        // Initially, no nonce should exist
        assert_eq!(manager.get_current_nonce(&wallet).await, None);

        // After getting a nonce
        let nonce = manager.get_next_nonce(&wallet).await;
        assert_eq!(nonce, 1);

        // Current nonce should be 1
        assert_eq!(manager.get_current_nonce(&wallet).await, Some(1));

        // Get another nonce
        let nonce2 = manager.get_next_nonce(&wallet).await;
        assert_eq!(nonce2, 2);

        // Current nonce should now be 2
        assert_eq!(manager.get_current_nonce(&wallet).await, Some(2));
    }

    #[tokio::test]
    async fn test_initialize_nonce_local_ahead() {
        let manager = NonceManager::new();
        let wallet = WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7").unwrap();

        // Set local nonce ahead
        manager.initialize_nonce(&wallet, 10).await;
        let _nonce = manager.get_next_nonce(&wallet).await; // Local is now 11

        // Try to initialize with lower blockchain nonce
        manager.initialize_nonce(&wallet, 5).await;

        // Local nonce should remain at 11 (not overwritten)
        let current = manager.get_current_nonce(&wallet).await;
        assert_eq!(current, Some(11));
    }

    #[tokio::test]
    async fn test_initialize_nonce_equal() {
        let manager = NonceManager::new();
        let wallet = WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7").unwrap();

        // Set local nonce
        manager.initialize_nonce(&wallet, 10).await;

        // Initialize with same nonce - should not change
        manager.initialize_nonce(&wallet, 10).await;

        let current = manager.get_current_nonce(&wallet).await;
        assert_eq!(current, Some(10));
    }

    #[tokio::test]
    async fn test_default_nonce_manager() {
        let manager = NonceManager::default();
        let wallet = WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7").unwrap();

        let nonce = manager.get_next_nonce(&wallet).await;
        assert_eq!(nonce, 1);
    }

    #[tokio::test]
    async fn test_multiple_wallets() {
        let manager = NonceManager::new();
        let wallet1 =
            WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7").unwrap();
        let wallet2 =
            WalletAddress::from_hex("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();

        // Get nonces for both wallets
        let nonce1_w1 = manager.get_next_nonce(&wallet1).await;
        let nonce1_w2 = manager.get_next_nonce(&wallet2).await;

        // Both should start at 1
        assert_eq!(nonce1_w1, 1);
        assert_eq!(nonce1_w2, 1);

        // Get second nonces
        let nonce2_w1 = manager.get_next_nonce(&wallet1).await;
        let nonce2_w2 = manager.get_next_nonce(&wallet2).await;

        // Both should be 2
        assert_eq!(nonce2_w1, 2);
        assert_eq!(nonce2_w2, 2);
    }
}

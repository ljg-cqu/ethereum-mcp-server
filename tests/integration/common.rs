/// Common utilities for integration tests
use ethereum_mcp_server::{
    server::http::AppState,
    services::{BalanceService, PriceService, SwapService},
    Config,
};
use std::sync::Arc;

/// Test configuration for integration tests
pub fn test_config() -> Config {
    Config::new(
        "https://mainnet.infura.io/v3/demo".to_string(),
        "127.0.0.1".to_string(),
        3001, // Use non-zero port for testing (3001 to avoid conflicts)
        "info".to_string(),
        "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
    )
}

/// Create test app state with mock provider
pub async fn create_test_app_state() -> anyhow::Result<AppState> {
    use ethereum_mcp_server::providers::MockEthereumProvider;

    let mock_provider = Arc::new(MockEthereumProvider::new());
    let balance_service = Arc::new(BalanceService::new(mock_provider.clone()));
    let price_service = Arc::new(PriceService::new(mock_provider.clone()));
    let swap_service = Arc::new(SwapService::new(mock_provider.clone()));

    Ok(AppState::new(
        balance_service,
        price_service,
        swap_service,
        1_000_000_000,
    ))
}

/// Test HTTP client for making requests
pub struct TestClient {
    pub base_url: String,
    pub client: reqwest::Client,
}

impl TestClient {
    pub fn new(port: u16) -> Self {
        Self {
            base_url: format!("http://127.0.0.1:{}", port),
            client: reqwest::Client::new(),
        }
    }

    pub async fn post_json(&self, body: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let response = self
            .client
            .post(&self.base_url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let text = response.text().await?;
        let json: serde_json::Value = serde_json::from_str(&text)?;
        Ok(json)
    }
}

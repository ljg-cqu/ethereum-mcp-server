/// Performance benchmarks for Ethereum MCP Server operations
/// Tests performance of core operations under various loads
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ethereum_mcp_server::{
    providers::MockEthereumProvider,
    services::{
        balance::BalanceServiceTrait, price::PriceServiceTrait, swap::SwapServiceTrait,
        BalanceService, PriceService, SwapService,
    },
    types::{SwapParams, TokenAddress, TokenAmount, WalletAddress},
    ContractAddresses,
};
use std::str::FromStr;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Benchmark balance retrieval operations
fn bench_balance_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut provider = MockEthereumProvider::new();
    provider.expect_get_eth_balance().returning(|_| {
        Ok(ethereum_mcp_server::types::BalanceInfo {
            wallet_address: WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7")
                .unwrap(),
            token_address: None,
            amount: TokenAmount::from_human_readable("1.0", 18).unwrap(),
            symbol: "ETH".to_string(),
        })
    });
    provider.expect_get_erc20_balance().returning(|_, _| {
        Ok(ethereum_mcp_server::types::BalanceInfo {
            wallet_address: WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7")
                .unwrap(),
            token_address: Some(
                TokenAddress::from_hex("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap(),
            ),
            amount: TokenAmount::from_human_readable("100.0", 6).unwrap(),
            symbol: "USDC".to_string(),
        })
    });
    let balance_service = Arc::new(BalanceService::new(Arc::new(provider)));

    let wallet_address =
        WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7").unwrap();
    let token_address =
        TokenAddress::from_hex("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();

    c.bench_function("get_eth_balance", |b| {
        b.iter(|| {
            rt.block_on(async {
                let result = balance_service
                    .get_balance(black_box(&wallet_address), None)
                    .await;
                black_box(result)
            })
        });
    });

    c.bench_function("get_erc20_balance", |b| {
        b.iter(|| {
            rt.block_on(async {
                let result = balance_service
                    .get_balance(black_box(&wallet_address), Some(black_box(&token_address)))
                    .await;
                black_box(result)
            })
        });
    });
}

/// Benchmark price retrieval operations
fn bench_price_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut provider = MockEthereumProvider::new();
    provider.expect_get_token_price().returning(|_, _| {
        Ok(ethereum_mcp_server::types::TokenPrice {
            token_address: TokenAddress::from_hex("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
                .unwrap(),
            price_eth: rust_decimal::Decimal::from_str("0.001").unwrap(),
            price_usd: None,
            source: "mock".to_string(),
        })
    });
    let price_service = Arc::new(PriceService::new(Arc::new(provider), get_test_contracts()));

    let token_address =
        TokenAddress::from_hex("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();

    c.bench_function("get_token_price", |b| {
        b.iter(|| {
            rt.block_on(async {
                let result = price_service
                    .get_token_price(black_box(&token_address))
                    .await;
                black_box(result)
            })
        });
    });
}

/// Benchmark swap simulation operations  
fn bench_swap_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut provider = MockEthereumProvider::new();
    provider.expect_simulate_swap().returning(|_, _| {
        Ok(ethereum_mcp_server::types::SwapResult {
            params: SwapParams {
                from_token: TokenAddress::from_hex("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
                    .unwrap(),
                to_token: TokenAddress::from_hex("0x0000000000000000000000000000000000000000")
                    .unwrap(),
                amount_in: TokenAmount::from_human_readable("100.0", 6).unwrap(),
                slippage_tolerance: rust_decimal::Decimal::from_str("0.005").unwrap(),
            },
            estimated_amount_out: TokenAmount::from_human_readable("0.03", 18).unwrap(),
            price_impact: rust_decimal::Decimal::ZERO,
            gas_estimate: 200000,
            gas_cost_eth: None,
            route: "mock".to_string(),
        })
    });
    let swap_service = Arc::new(SwapService::new(Arc::new(provider), get_test_contracts()));

    let swap_params = SwapParams {
        from_token: TokenAddress::from_hex("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap(), // USDC
        to_token: TokenAddress::from_hex("0x0000000000000000000000000000000000000000").unwrap(), // ETH
        amount_in: TokenAmount::from_human_readable("100.0", 6).unwrap(), // 100 USDC
        slippage_tolerance: rust_decimal::Decimal::from_str("0.005").unwrap(), // 0.5%
    };

    c.bench_function("simulate_swap", |b| {
        b.iter(|| {
            rt.block_on(async {
                let result = swap_service.simulate_swap(black_box(&swap_params)).await;
                black_box(result)
            })
        });
    });
}

/// Benchmark concurrent operations
fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut provider = MockEthereumProvider::new();
    provider.expect_get_eth_balance().returning(|_| {
        Ok(ethereum_mcp_server::types::BalanceInfo {
            wallet_address: WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7")
                .unwrap(),
            token_address: None,
            amount: TokenAmount::from_human_readable("1.0", 18).unwrap(),
            symbol: "ETH".to_string(),
        })
    });
    let balance_service = Arc::new(BalanceService::new(Arc::new(provider)));

    let wallet_address =
        WalletAddress::from_hex("0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7").unwrap();

    c.bench_function("concurrent_balance_requests", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Simulate 10 concurrent balance requests
                let tasks = (0..10).map(|_| {
                    let service = balance_service.clone();
                    let address = wallet_address.clone();
                    tokio::spawn(async move { service.get_balance(&address, None).await })
                });

                let results = futures::future::join_all(tasks).await;
                black_box(results)
            })
        });
    });
}

/// Benchmark validation operations
fn bench_validation_operations(c: &mut Criterion) {
    use ethereum_mcp_server::validation::Validator;

    c.bench_function("validate_wallet_address", |b| {
        b.iter(|| {
            let result = Validator::validate_wallet_address(black_box(
                "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7",
            ));
            black_box(result)
        });
    });

    c.bench_function("validate_token_amount", |b| {
        b.iter(|| {
            let result = Validator::validate_token_amount(
                black_box("100.5"),
                black_box(18),
                black_box(Some(1_000_000_000_000_000_000u64)), // 1 ETH in wei
            );
            black_box(result)
        });
    });

    c.bench_function("sanitize_string", |b| {
        b.iter(|| {
            let result =
                Validator::sanitize_string(black_box("Hello\0World\nTest\r!"), black_box(1000));
            black_box(result)
        });
    });
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

criterion_group!(
    benches,
    bench_validation_operations,
    bench_balance_operations,
    bench_price_operations,
    bench_swap_operations,
    bench_concurrent_operations,
);
criterion_main!(benches);

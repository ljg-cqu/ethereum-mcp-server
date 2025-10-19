# Ethereum Trading MCP Server
**✅ ALL ISSUES RESOLVED** - All identified security issues have been addressed.  
**Status:** Production Ready | 211 tests passing | 51.2% coverage | Zero warnings | HTTP Transport Only

Model Context Protocol (MCP) server in Rust that enables AI agents to query balances and execute token swaps on Ethereum with enterprise-grade security and reliability.

### Key Features

- **Real ETH Balances**: Fetches actual on-chain ETH balances via alloy
- **ERC20 Support**: Query any ERC20 token balance with proper decimals and symbols
- **Uniswap V3 Integration**: Get real token prices in ETH from Uniswap V3 quoter contracts
- **USD Pricing via Chainlink**: Convert token prices to USD using the Chainlink ETH/USD oracle
- **Symbol Support**: `get_token_price` accepts either `token_address` or `token_symbol` (USDC/USDT/DAI/WETH)
- **Production Swap Simulation**: Construct and simulate real Uniswap V3 transactions via `eth_call`
- **Smart Contract Integration**: Production addresses for USDC, USDT, DAI, WETH, and Uniswap V3 contracts
- **Fee Tier Optimization**: Automatic selection of optimal Uniswap V3 fee tiers (no execution)

### Transport Support

- **HTTP/HTTPS**: Full support with connection pooling, rate limiting, and retry logic
- **WebSocket**: Not currently supported (see [`docs/WEBSOCKET_IMPLEMENTATION.md`](docs/WEBSOCKET_IMPLEMENTATION.md))
- **Failover**: Automatic failover across multiple HTTP RPC URLs
- **Concurrency**: Configurable concurrent request limits with semaphore-based throttling

## 🛠️ Tech Stack

- **Rust + tokio** - Async runtime
- **alloy** - Ethereum RPC client  
- **tracing** - Structured logging (required)
- **rust_decimal** - Financial precision (required)
- **serde** - JSON serialization

## 🚀 Quick Start

- Rust 1.75+
- Ethereum RPC endpoint (Infura/Alchemy)

### Setup
```bash
git clone https://github.com/ljg-cqu/ethereum-mcp-server
cd ethereum-mcp-server

# Copy and configure environment
cp .env.example .env
# Edit .env with your Ethereum RPC URL(s) and WALLET_PRIVATE_KEY (64-hex, with or without 0x)
# You can provide a single URL:
#   ETHEREUM_RPC_URL=https://mainnet.infura.io/v3/YOUR_PROJECT_ID
# Or multiple for failover (CSV):
#   ETHEREUM_RPC_URLS=https://rpc1.example,https://rpc2.example

# Build and test
cargo build --release
cargo test

# Run the server
make dev
```

### Usage

Start the server:
```bash
# Using Makefile (recommended)
make dev

# Or directly with cargo
ETHEREUM_RPC_URL="https://mainnet.infura.io/v3/YOUR_KEY" \
WALLET_PRIVATE_KEY="0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef" \
cargo run

# Or using Docker
cp .env.example .env  # Configure your RPC URL
make docker-build && make docker-run
```

The server will be available at `http://localhost:3000`

## 📖 API Usage

See [`docs/API_REFERENCE.md`](docs/API_REFERENCE.md) for complete API documentation and examples.

## 🧪 Testing

**Comprehensive Test Suite - 211 Tests Passing ✅**

```bash
# Run all tests (unit + integration + main)
make test

# Individual test categories
make test-unit         # 183 unit tests
make test-integration  # 19 integration tests

# Coverage report
make coverage          # Requires cargo-tarpaulin

# Development testing
make test-watch        # Watch mode for TDD
```

**Test Coverage: 51.2% (551/1076 lines)**

**Unit Tests (183)**: Core functionality, validation, nonce management, circuit breaker, ethereum provider utilities, HTTP server configuration, type conversions, configuration loading

**Integration Tests (19)**: End-to-end flows, security validation, JSON-RPC protocol compliance, address conversions, transaction status types, configuration validation

**Main Tests (9)**: Configuration loading, validation, helper functions

## 🏗️ Architecture

**Simple & Focused**: HTTP server → JSON-RPC 2.0 → Ethereum RPC → Smart contracts.
{{ ... }}
See [`docs/SYSTEM_DESIGN.md`](docs/SYSTEM_DESIGN.md) for detailed architecture and design decisions.

## 📝 Production Features

- Multi-provider RPC failover
- Thread-safe nonce management  
- Circuit breaker pattern
- Rate limiting and concurrency controls
- Enterprise-grade input validation

**Security**: All audit issues resolved. See [`docs/SECURITY_AUDIT_REPORT.md`](docs/SECURITY_AUDIT_REPORT.md).

## ⚙️ Operational Defaults

- **Rate limiting**: Requests are limited to ~2 req/sec per IP with a small burst (`tower_governor`). See `HttpServer::new()` in `src/server/http.rs`.
- **Request timeouts**: Each HTTP request has a 15s timeout via `TimeoutLayer`.
- **Concurrency control**: Ethereum RPC calls are limited via a semaphore (10 permits) with a 5s acquisition timeout to avoid indefinite waits.
- **HTTP concurrency limit**: The HTTP router is capped at 100 in-flight requests using `ConcurrencyLimitLayer`.
- **Circuit breaker**: External Ethereum RPC operations are executed through a circuit breaker to fail fast on repeated errors and auto-recover.
- **RPC failover**: If `ETHEREUM_RPC_URLS` is provided (CSV), the provider will attempt each URL in order until initialization succeeds.
- **CORS**: Configure allowed origins with `CORS_ALLOW_ORIGINS` ("*" or CSV list of origins).
- **USDC address corrected**: `src/contracts.rs` now uses the verified mainnet USDC address `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`.
- **Security**: No hardcoded credentials - all sensitive configuration via `.env` file. Never commit real keys.
- **Nonce management**: Thread-safe nonce allocation prevents blockchain transaction conflicts.

## 📊 Performance & Testing

### **Test Results: 211 Tests Passing ✅**
```bash
make test              # All tests (211 total)
make test-unit         # 183 unit tests  
make test-integration  # 19 integration tests
make bench             # Performance benchmarks
```

## 📚 Documentation

- **[API Reference](docs/API_REFERENCE.md)** - Complete API documentation
- **[System Design](docs/SYSTEM_DESIGN.md)** - Architecture details
- **[Requirements](docs/REQUIREMENTS.md)** - Project specifications  
- **[Security Audit](docs/SECURITY_AUDIT_REPORT.md)** - Security findings
- **[WebSocket Implementation](docs/WEBSOCKET_IMPLEMENTATION.md)** - WebSocket attempt details
- **[Docker Deployment](docs/DOCKER_DEPLOYMENT.md)** - Container deployment guide
- **[Literate Code Maps Overview](docs/LITERATE_CODE_MAPS_README.md)** - How to navigate architecture and flow diagrams
- **[Call Graph Diagram Guide](docs/CALL_GRAPH_DIAGRAM_GUIDE.md)** - Comparison of diagram types and decision flow

**Architecture Diagrams**: View PlantUML diagrams in `docs/diagrams/` or run `plantuml docs/diagrams/**/*.puml` to generate PNGs locally.

## 🔒 Security Status

✅ **Production Ready** - All 27 security issues resolved  
✅ **211 Tests Passing** - Comprehensive test coverage (51.2%)  
✅ **Zero Warnings** - Clean compilation  
✅ **OWASP Compliant** - Enterprise security standards

See [`docs/SECURITY_AUDIT_REPORT.md`](docs/SECURITY_AUDIT_REPORT.md) for complete details.

Built with ❤️ for the Ethereum ecosystem.

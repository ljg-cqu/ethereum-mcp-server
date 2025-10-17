# Ethereum MCP Server - Development Makefile

.PHONY: build test test-unit test-integration clean fmt check coverage dev install-tools

# Default target
all: fmt check test build

# Build targets
build:
	@echo "🔨 Building ethereum-mcp-server..."
	cargo build

build-release:
	@echo "🚀 Building release version..."
	cargo build --release

# Testing targets (TDD workflow)
test: test-unit test-integration
	@echo "✅ All tests completed"

test-unit:
	@echo "🧪 Running unit tests..."
	cargo test --lib --bins

test-integration:
	@echo "🔗 Running integration tests..."
	@if find tests -name "*.rs" -type f | grep -q .; then \
		cargo test --test '*'; \
	else \
		echo "ℹ️  No integration tests found - skipping"; \
	fi

test-watch:
	@echo "👀 Running tests in watch mode..."
	cargo watch -x 'test --lib'

coverage:
	@echo "📊 Generating test coverage report..."
	cargo tarpaulin --out Html --output-dir target/coverage --timeout 120

# Code quality
fmt:
	@echo "🎨 Formatting code..."
	cargo fmt

check:
	@echo "🔍 Running clippy lints..."
	cargo clippy -- -D warnings

audit:
	@echo "🛡️  Security audit..."
	cargo audit

# Development
dev:
	@echo "🚀 Starting development server..."
	@echo "⚠️  Make sure to set ETHEREUM_RPC_URL and WALLET_PRIVATE_KEY in your .env file"
	@if [ ! -f .env ]; then echo "❌ .env file not found. Copy .env.example to .env and configure it."; exit 1; fi
	RUST_LOG=debug cargo run

dev-watch:
	@echo "🔄 Starting development server with auto-reload..."
	@echo "⚠️  Make sure to set ETHEREUM_RPC_URL and WALLET_PRIVATE_KEY in your .env file"
	@if [ ! -f .env ]; then echo "❌ .env file not found. Copy .env.example to .env and configure it."; exit 1; fi
	cargo watch -x run

# Example API calls
test-api:
	@echo "🧪 Testing MCP API endpoints..."
	@echo "Testing tools/list:"
	@curl -X POST http://localhost:3000 \
		-H "Content-Type: application/json" \
		-d '{"jsonrpc": "2.0", "method": "tools/list", "id": 1}' | jq .
	@echo "\n\nTesting get_balance:"
	@curl -X POST http://localhost:3000 \
		-H "Content-Type: application/json" \
		-d '{"jsonrpc": "2.0", "method": "tools/call", "params": {"name": "get_balance", "arguments": {"wallet_address": "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7"}}, "id": 2}' | jq .

# Benchmarks
bench:
	@echo "⚡ Running benchmarks..."
	cargo bench

# Clean
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean

# Install development tools
install-tools:
	@echo "🔧 Installing development tools..."
	cargo install cargo-watch cargo-tarpaulin cargo-audit

# Docker targets
docker-build:
	@echo "🐳 Building Docker image..."
	@echo "📦 First building release binary locally..."
	make build-release
	@echo "🐳 Now creating Docker image..."
	docker build -t ethereum-mcp-server .

docker-dev:
	@echo "🐳 Starting development environment..."
	docker-compose up --build

docker-run:
	@echo "🐳 Running Docker container..."
	@echo "⚠️  Make sure to set ETHEREUM_RPC_URL and WALLET_PRIVATE_KEY in your .env file"
	@if [ ! -f .env ]; then echo "❌ .env file not found. Copy .env.example to .env and configure it."; exit 1; fi
	docker run --rm -p 3000:3000 --env-file .env ethereum-mcp-server

# Pre-commit checks (run before commits)
pre-commit: fmt check test-unit
	@echo "✅ Pre-commit checks passed"

# Help
help:
	@echo "Ethereum MCP Server - Available targets:"
	@echo "  build          - Build the project"
	@echo "  test           - Run all tests"
	@echo "  test-unit      - Run unit tests only"
	@echo "  test-integration - Run integration tests only"
	@echo "  coverage       - Generate test coverage report"
	@echo "  fmt            - Format code"
	@echo "  check          - Run lints"
	@echo "  dev            - Start development server"
	@echo "  pre-commit     - Run pre-commit checks"
	@echo "  install-tools  - Install development tools"
	@echo "  docker-build   - Build Docker image"
	@echo "  docker-run     - Run Docker container"
	@echo "  docker-dev     - Start Docker development environment"

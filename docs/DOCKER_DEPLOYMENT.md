# Docker Deployment Guide

## 🐳 **Production-Ready Docker Setup**

### **Overview**
The Ethereum MCP Server includes a streamlined Docker deployment solution that creates lightweight, secure containers ready for production use.

### **Docker Build Strategy**
- **Method**: Local compilation with Alpine container deployment
- **Size**: ~26.5MB (highly optimized)
- **Security**: Non-root user, health checks, minimal attack surface
- **Performance**: Excellent (no in-container compilation overhead)

### **Quick Start**

```bash
# Build and run Docker image
make docker-build
make docker-run
```

### **Recommended Docker Workflow**

1. **Development**:
   ```bash
   # Native development (fastest)
   make dev
   ```

2. **Production Deployment**:
   ```bash
   # Build Docker image
   make docker-build
   
   # Run with environment configuration
   cp .env.example .env
   # Edit .env with your configuration
   make docker-run
   ```

3. **Testing Docker**:
   ```bash
   # Test that container builds successfully
   make docker-build
   
   # Test container startup (will fail without valid RPC)
   docker run --rm ethereum-mcp-server --help
   ```

### **Docker Command Reference**

| Command | Status | Purpose |
|---------|--------|---------|
| `make docker-build` | ✅ **Working** | Build Docker image (standard) |
| `make docker-run` | ✅ **Working** | Run Docker container |
| `make docker-dev` | ✅ **Working** | Docker Compose development |

### **Container Details**

- **Base Image**: Alpine 3.19 (lightweight)
- **Size**: ~26.5MB
- **User**: Non-root `app` user (security)
- **Port**: 3000
- **Health Check**: `/health` endpoint
- **Environment**: Configurable via `.env` file

### **Environment Variables Required**

```bash
ETHEREUM_RPC_URL=https://mainnet.infura.io/v3/YOUR_PROJECT_ID
WALLET_PRIVATE_KEY=0x...  # Your wallet private key
SERVER_HOST=0.0.0.0       # For Docker networking
SERVER_PORT=3000
RUST_LOG=info

# Optional: Contract addresses (defaults to mainnet)
# WETH_ADDRESS=0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2
# USDC_ADDRESS=0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48
# USDT_ADDRESS=0xdAC17F958D2ee523a2206206994597C13D831ec7
# DAI_ADDRESS=0x6B175474E89094C44Da98b954EedeAC495271d0F
# UNISWAP_V3_FACTORY=0x1F98431c8aD98523631AE4a59f267346ea31F984
# UNISWAP_V3_ROUTER=0xE592427A0AEce92De3Edee1F18E0157C05861564
# UNISWAP_V3_QUOTER=0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6
# CHAINLINK_ETH_USD_FEED=0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419
```

### **Production Deployment**

```bash
# 1. Build the image
make docker-build

# 2. Tag for registry
docker tag ethereum-mcp-server your-registry/ethereum-mcp-server:latest

# 3. Push to registry
docker push your-registry/ethereum-mcp-server:latest

# 4. Deploy
docker run -d \
  --name ethereum-mcp-server \
  --restart unless-stopped \
  -p 3000:3000 \
  --env-file .env \
  your-registry/ethereum-mcp-server:latest
```

## 🚀 **Production Features**

✅ **Lightweight & Fast**: 26.5MB optimized Alpine container  
✅ **Secure by Design**: Non-root user, minimal dependencies  
✅ **Health Monitoring**: Built-in health checks for orchestration  
✅ **Environment Driven**: Full configuration via environment variables  
✅ **Production Ready**: Suitable for enterprise deployment  

The Docker deployment provides a robust, secure foundation for production workloads! 🎯

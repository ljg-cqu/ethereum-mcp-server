# Ethereum MCP Server - API Reference

This document provides a comprehensive reference for the JSON-RPC 2.0 API exposed by the Ethereum MCP Server.

## JSON-RPC 2.0 Interface

The server exposes a single HTTP endpoint that accepts POST requests with a JSON-RPC 2.0 payload.

- **Endpoint**: `/`
- **Method**: `POST`

### `tools/list`

Lists the available tools.

**Request:**

```json
{
  "jsonrpc": "2.0",
  "method": "tools/list",
  "id": 1
}
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "result": {
    "tools": [
      {
        "name": "get_balance",
        "description": "Query ETH and ERC20 token balances with proper decimals"
      },
      {
        "name": "get_token_price",
        "description": "Get current token price in USD or ETH (input: token address or symbol)"
      },
      {
        "name": "swap_tokens",
        "description": "Simulate Uniswap token swap via eth_call"
      },
      {
        "name": "get_transaction_status",
        "description": "Get the status of a transaction, including confirmations"
      }
    ]
  },
  "id": 1
}
```

### `tools/call`

Calls a specific tool with the given arguments.

## Tools

### `get_balance`

**Description:**

Queries the ETH or an ERC20 token balance for a given wallet address.

**Arguments:**

- `wallet_address` (string, required): The wallet address to query.
- `token_contract_address` (string, optional): The contract address of the ERC20 token. If omitted, the native ETH balance is returned.

**Example Request:**

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "get_balance",
    "arguments": {
      "wallet_address": "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7",
      "token_contract_address": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
    }
  },
  "id": 2
}
```

**Example Response:**

```json
{
  "jsonrpc": "2.0",
  "result": {
    "wallet_address": "0x742d35Cc6634C0532925a3b8D8b5d0f8988Db8c7",
    "token_address": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
    "amount": {
      "raw": "100000000",
      "human_readable": "100.0",
      "decimals": 6
    },
    "symbol": "USDC"
  },
  "id": 2
}
```

### `get_token_price`

**Description:**

Gets the current price of a token in ETH and USD. USD price is optional and depends on Chainlink ETH/USD feed availability.

**Arguments:**

- `token_address` (string, optional): The contract address of the token.
- `token_symbol` (string, optional): The symbol of the token (e.g., "USDC", "WETH"). One of `token_address` or `token_symbol` is required.

**Example Request:**

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "get_token_price",
    "arguments": {
      "token_symbol": "USDC"
    }
  },
  "id": 3
}
```

**Example Response:**

```json
{
  "jsonrpc": "2.0",
  "result": {
    "token_address": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
    "price_eth": "0.00029",
    "price_usd": "1.00",
    "source": "uniswap_v3_fee_500"
  },
  "id": 3
}
```

### `swap_tokens`

**Description:**

Simulates a token swap on Uniswap V3 and returns the estimated output amount and gas costs.

**Arguments:**

- `from_token` (string, required): The contract address of the token to swap from.
- `to_token` (string, required): The contract address of the token to swap to.
- `amount` (string, required): The human-readable amount to swap (e.g., "100.0").
- `slippage_tolerance` (string, required): The slippage tolerance percentage (e.g., "0.5" for 0.5%).

**Example Request:**

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "swap_tokens",
    "arguments": {
      "from_token": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
      "to_token": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
      "amount": "100.0",
      "slippage_tolerance": "0.5"
    }
  },
  "id": 4
}
```

**Example Response:**

```json
{
  "jsonrpc": "2.0",
  "result": {
    "params": {
      "from_token": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
      "to_token": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
      "amount_in": {
        "raw": "100000000",
        "human_readable": "100.0",
        "decimals": 6
      },
      "slippage_tolerance": "0.5"
    },
    "estimated_amount_out": {
      "raw": "29000000000000000",
      "human_readable": "0.029",
      "decimals": 18
    },
    "price_impact": "0.01",
    "gas_estimate": 180000,
    "gas_cost_eth": "0.0054",
    "route": "uniswap_v3_fee_500"
  },
  "id": 4
}
```

### `get_transaction_status`

**Description:**

Gets the status of an on-chain transaction, including the number of confirmations.

**Arguments:**

- `transaction_hash` (string, required): The hash of the transaction to query.

**Example Request:**

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "get_transaction_status",
    "arguments": {
      "transaction_hash": "0x..."
    }
  },
  "id": 5
}
```

**Example Response (Confirmed):**

```json
{
  "jsonrpc": "2.0",
  "result": {
    "transaction_hash": "0x...",
    "status": "Confirmed",
    "confirmations": 12,
    "block_number": 12345678
  },
  "id": 5
}
```

**Example Response (Pending):**

```json
{
  "jsonrpc": "2.0",
  "result": {
    "transaction_hash": "0x...",
    "status": "Pending",
    "confirmations": 0,
    "block_number": null
  },
  "id": 5
}
```

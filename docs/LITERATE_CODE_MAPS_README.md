# Literate Code Maps

This directory contains Literate Code Maps for the Ethereum MCP Server project, created according to the methodology described at https://github.com/abulka/lcodemaps.

## What are Literate Code Maps?

Literate Code Maps are a diagramming methodology for understanding source code that combines:
- **Visual structure**: Clear boxes representing code namespaces (modules, functions, classes)
- **Code fragments**: Actual source code embedded in diagram boxes for precision
- **Cross-references**: Numbered steps and references between components
- **High data density**: Maximize information per square inch

## File Organization

```
docs/
├── diagrams/
│   ├── architecture/          # System architecture diagrams
│   │   ├── ethereum_mcp_architecture.puml + .png
│   │   ├── balance_query_flow.puml + .png
│   │   ├── swap_simulation_flow.puml + .png
│   │   ├── http_request_processing_flow.puml + .png
│   │   └── error_handling_flow.puml + .png
│   └── call-graphs/          # Function call relationship diagrams
│       ├── balance_query_call_graph.puml + .png
│       ├── price_service_call_graph.puml + .png
│       ├── swap_simulation_call_graph.puml + .png
│       ├── transaction_status_call_graph.puml + .png
│       └── system_initialization_call_graph.puml + .png
├── CALL_GRAPH_DIAGRAM_GUIDE.md    # Comprehensive diagram comparison
└── LITERATE_CODE_MAPS_README.md   # This file
```

> **Note:** Only the diagrams under `docs/diagrams/architecture/` are Literate Code Maps. The sequence diagrams in `docs/diagrams/call-graphs/` are complementary call graphs that document runtime execution paths.

## Maps in this Project

### 1. Architecture Overview (`ethereum_mcp_architecture.puml`)
Shows the overall system structure with:
- Entry point and initialization flow
- Module dependencies and relationships
- Key interfaces and data types
- Service layer organization

### 2. Balance Query Flow (`balance_query_flow.puml`)
Detailed flow for balance retrieval operations:
- HTTP request handling
- Input validation and parsing
- ERC20 vs ETH balance logic
- Circuit breaker protection

### 3. Swap Simulation Flow (`swap_simulation_flow.puml`)
Complex flow for Uniswap V3 swap simulations:
- Parameter validation and parsing
- Fee tier optimization
- Price impact calculations
- Gas estimation
- Contract interaction patterns

### 4. HTTP Request Processing Flow (`http_request_processing_flow.puml`)
Complete JSON-RPC request lifecycle:
- HTTP request routing and validation
- Tool-based API dispatch
- Service handler execution
- Error classification and response formatting
- Cross-cutting concerns (rate limiting, CORS)

### 5. Error Handling Flow (`error_handling_flow.puml`)
Comprehensive error management system:
- Error classification by type (timeout, network, validation)
- Client-friendly error messages
- Retry suggestions and error metadata
- Server-side logging and monitoring

## Call Graphs (PlantUML Sequence Diagrams)

Complementing the Literate Code Maps, we provide call graphs (PlantUML sequence diagrams) showing the exact function-to-function call relationships. Literate Code Maps focus on architectural flows with embedded code snippets; call graphs document the runtime execution detail. Together they give a complete picture.

### 1. Balance Query Call Graph (`balance_query_call_graph.puml`) ✅ **Working**
Shows the complete call chain for ERC20 balance queries:
- Input validation flow
- Service delegation
- Provider operations with circuit breaker
- Multiple RPC calls (symbol, decimals, balanceOf)
- Response serialization

### 2. Price Service Call Graph (`price_service_call_graph.puml`) ✅ **Working**
Token price retrieval with multiple data sources:
- HTTP request handling
- Cache checking strategy
- Chainlink oracle integration
- Uniswap V3 price fallback
- Price aggregation and validation
- Caching for performance

### 3. Swap Simulation Call Graph (`swap_simulation_call_graph.puml`) ✅ **Working**
Complex multi-step call flow for Uniswap V3 simulations:
- HTTP request handling
- Fee tier optimization algorithm
- Liquidity checking across multiple pools
- Price impact calculations
- Gas estimation
- Error handling paths

### 4. Transaction Status Call Graph (`transaction_status_call_graph.puml`) ✅ **Working**
Transaction monitoring and status checking:
- HTTP request processing
- Transaction hash validation
- Receipt checking strategy
- Block confirmation tracking
- Event log parsing
- Status caching and updates

### 5. System Initialization Call Graph (`system_initialization_call_graph.puml`) ✅ **Working**
Application bootstrap sequence:
- Configuration loading
- Provider layer setup (RPC, signer, circuit breaker, nonce manager)
- Service layer initialization
- HTTP server configuration
- Graceful shutdown setup

## Call Graph vs Literate Code Map

| Aspect | Literate Code Maps | Call Graphs |
|--------|-------------------|-------------|
| **Purpose** | Architecture + code overview | Function call relationships |
| **Detail Level** | High-level flows with code | Detailed call sequences |
| **Scope** | System components + logic | Execution paths |
| **Best For** | Understanding system design | Debugging call flows |
| **Format** | PlantUML component diagrams | PlantUML sequence diagrams |

## How to View the Maps

### Option 1: PlantUML Web Viewer
1. Visit https://www.plantuml.com/plantuml
2. Copy and paste the contents of any `.puml` file
3. The diagram will render automatically

### Option 2: VS Code Extension
1. Install the "PlantUML" extension in VS Code
2. Open any `.puml` file
3. Use Ctrl+Shift+P → "PlantUML: Preview Current Diagram"

### Option 3: Local PlantUML
1. Install PlantUML: `sudo apt install plantuml` (Ubuntu/Debian)
2. Generate PNG: `plantuml architecture_literate_code_map.puml`
3. Generate SVG: `plantuml -tsvg architecture_literate_code_map.puml`

## Code Map Conventions Used

### Box Types
- **Blue boxes**: Entry points and external interfaces
- **Green boxes**: Core library components
- **Pink boxes**: Service layer components
- **Salmon boxes**: HTTP/web layer components
- **Cyan boxes**: Provider/infrastructure layer
- **Orange boxes**: Reliability components (circuit breakers, etc.)
- **Gray boxes**: External dependencies (contracts, RPC)

### Code Fragments
- Use `$code` prefix for code lines (PlantUML macro)
- Include actual function signatures and key logic
- Focus on the most important 3-5 lines per component
- Show control flow and data transformations

### Arrows and Flow
- Solid arrows: Direct function calls
- Dashed arrows: Dependency relationships
- Arrow direction: Data/control flow direction
- Labels on arrows where flow is non-obvious

### Notes and Cross-references
- **Right notes**: Detailed code fragments
- **Bottom notes**: Design decisions and rationale
- **Cross-references**: Key architectural principles

## Benefits of These Maps

1. **Onboarding**: New developers can understand the system quickly
2. **Documentation**: Living documentation that stays current with code
3. **Debugging**: Visual flow helps identify issues and bottlenecks
4. **Architecture Reviews**: Clear structure for design discussions
5. **Testing**: Visual coverage of code paths and integration points

## Updating the Maps

When the codebase changes significantly:
1. Review the affected flows
2. Update code fragments to match current implementation
3. Add new boxes/components as needed
4. Regenerate diagrams and commit changes

The maps serve as both documentation and design validation tools.

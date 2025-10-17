#!/bin/bash
# Load testing script for Ethereum MCP Server
# Tests performance under various loads and scenarios

set -e

echo "🚀 Ethereum MCP Server - Load Testing Suite"
echo "=============================================="

# Configuration
SERVER_URL="http://localhost:3000"
CONCURRENT_REQUESTS=10
TOTAL_REQUESTS=100

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Helper functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if server is running
check_server() {
    log_info "Checking if server is running at $SERVER_URL..."
    if curl -s --connect-timeout 5 "$SERVER_URL" > /dev/null 2>&1; then
        log_info "Server is running ✅"
        return 0
    else
        log_error "Server is not running at $SERVER_URL"
        log_info "Please start the server first: make dev"
        return 1
    fi
}

# Test basic functionality
test_basic_functionality() {
    log_info "Testing basic JSON-RPC functionality..."
    
    # Test tools/list
    response=$(curl -s -X POST "$SERVER_URL" \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc": "2.0", "method": "tools/list", "id": 1}' \
        --connect-timeout 10)
    
    if echo "$response" | grep -q '"result"'; then
        log_info "tools/list endpoint working ✅"
    else
        log_error "tools/list endpoint failed ❌"
        echo "Response: $response"
        return 1
    fi
}

# Load test with concurrent requests
load_test_concurrent() {
    log_info "Running concurrent load test ($CONCURRENT_REQUESTS concurrent, $TOTAL_REQUESTS total)..."
    
    local temp_dir=$(mktemp -d)
    local success_count=0
    local error_count=0
    
    # Generate test requests
    for i in $(seq 1 $TOTAL_REQUESTS); do
        {
            response=$(curl -s -w "%{http_code}" -X POST "$SERVER_URL" \
                -H "Content-Type: application/json" \
                -d '{"jsonrpc": "2.0", "method": "tools/list", "id": '$i'}' \
                --connect-timeout 5 --max-time 30)
            
            http_code="${response: -3}"
            if [ "$http_code" = "200" ]; then
                echo "success" > "$temp_dir/result_$i"
            else
                echo "error:$http_code" > "$temp_dir/result_$i"
            fi
        } &
        
        # Limit concurrent processes
        if (( i % CONCURRENT_REQUESTS == 0 )); then
            wait
        fi
    done
    wait
    
    # Count results
    success_count=$(find "$temp_dir" -name "result_*" -exec grep -l "success" {} \; | wc -l)
    error_count=$(find "$temp_dir" -name "result_*" -exec grep -l "error" {} \; | wc -l)
    
    log_info "Load test results:"
    log_info "  ✅ Successful requests: $success_count"
    log_info "  ❌ Failed requests: $error_count"
    log_info "  📊 Success rate: $(( success_count * 100 / TOTAL_REQUESTS ))%"
    
    rm -rf "$temp_dir"
    
    if [ $error_count -gt $(( TOTAL_REQUESTS / 10 )) ]; then
        log_warn "High error rate detected (>10%)"
        return 1
    fi
}

# Test rate limiting
test_rate_limiting() {
    log_info "Testing rate limiting (should see 429 responses after burst)..."
    
    local rate_limit_hits=0
    
    # Send rapid requests to trigger rate limiting
    for i in $(seq 1 20); do
        response=$(curl -s -w "%{http_code}" -X POST "$SERVER_URL" \
            -H "Content-Type: application/json" \
            -d '{"jsonrpc": "2.0", "method": "tools/list", "id": '$i'}' \
            --connect-timeout 5)
        
        http_code="${response: -3}"
        if [ "$http_code" = "429" ]; then
            ((rate_limit_hits++))
        fi
        
        # Small delay to avoid overwhelming
        sleep 0.1
    done
    
    if [ $rate_limit_hits -gt 0 ]; then
        log_info "Rate limiting working ✅ (triggered $rate_limit_hits times)"
    else
        log_warn "Rate limiting may not be working as expected"
    fi
}

# Test invalid requests
test_error_handling() {
    log_info "Testing error handling with invalid requests..."
    
    # Test invalid JSON
    response=$(curl -s -X POST "$SERVER_URL" \
        -H "Content-Type: application/json" \
        -d '{"invalid": "json"' \
        --connect-timeout 5)
    
    if echo "$response" | grep -q '"error"'; then
        log_info "Invalid JSON handling ✅"
    else
        log_warn "Invalid JSON handling may not be working"
    fi
    
    # Test invalid method
    response=$(curl -s -X POST "$SERVER_URL" \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc": "2.0", "method": "invalid_method", "id": 1}' \
        --connect-timeout 5)
    
    if echo "$response" | grep -q '"error"'; then
        log_info "Invalid method handling ✅"
    else
        log_warn "Invalid method handling may not be working"
    fi
}

# Memory and performance monitoring (if available)
monitor_performance() {
    if command -v ps >/dev/null 2>&1; then
        log_info "Server process information:"
        ps aux | grep ethereum-mcp-server | grep -v grep || log_warn "Server process not found"
    fi
    
    if command -v netstat >/dev/null 2>&1; then
        log_info "Network connections:"
        netstat -tlnp 2>/dev/null | grep :3000 || log_warn "Port 3000 not found in netstat"
    fi
}

# Main test execution
main() {
    log_info "Starting load testing suite..."
    
    if ! check_server; then
        exit 1
    fi
    
    echo ""
    test_basic_functionality
    echo ""
    load_test_concurrent
    echo ""
    test_rate_limiting  
    echo ""
    test_error_handling
    echo ""
    monitor_performance
    
    log_info "Load testing completed! 🎉"
    log_info "For detailed performance metrics, run: make bench"
}

# Execute main function
main "$@"

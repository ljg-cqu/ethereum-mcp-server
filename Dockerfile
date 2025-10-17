# Working Docker build for Ethereum MCP Server
# This approach uses local build then copies to container

FROM alpine:3.19

# Install runtime dependencies
RUN apk add --no-cache ca-certificates curl

# Create app user
RUN addgroup -g 1001 -S app && \
    adduser -S -D -H -u 1001 -h /app -s /sbin/nologin -G app app

# Set working directory
WORKDIR /app

# Copy pre-built binary (built with make build-release on host)
COPY target/release/ethereum-mcp-server /usr/local/bin/ethereum-mcp-server

# Change ownership
RUN chown app:app /usr/local/bin/ethereum-mcp-server && \
    chmod +x /usr/local/bin/ethereum-mcp-server

# Switch to app user
USER app

# Default logging
ENV RUST_LOG=info

# MCP server port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=30s --retries=3 \
  CMD curl -f http://localhost:3000/health || exit 1

# Entrypoint
ENTRYPOINT ["/usr/local/bin/ethereum-mcp-server"]

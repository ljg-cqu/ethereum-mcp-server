/// HTTP server and JSON-RPC 2.0 handling
/// Clean separation of transport layer
pub mod http;
pub mod jsonrpc;

// Re-export for convenience
pub use http::HttpServer;

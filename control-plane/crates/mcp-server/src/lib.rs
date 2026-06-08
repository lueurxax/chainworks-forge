// Raised from 128 to 256 for the large serde_json::json! macro in tools/reports.rs MCP output assembly.
#![recursion_limit = "256"]

pub mod hot_read_guard;
pub mod http;
pub mod protocol;
pub mod request_context;
pub mod server;
pub mod tools;

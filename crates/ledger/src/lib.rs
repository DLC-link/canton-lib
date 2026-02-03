pub mod active_contracts;
pub mod client;
pub mod common;
pub mod ledger_end;
pub mod submit;
pub mod utils;

// WebSocket module is only available on native targets (requires tokio-tungstenite)
#[cfg(not(target_arch = "wasm32"))]
pub mod websocket;

pub use canton_api_client::models;

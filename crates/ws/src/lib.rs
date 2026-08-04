//! kamin-ws — Rust-клиент WS-протокола kamin-host (plan/50 §2).
//! frame = wire-типы (protocol.ts) · endpoint = id-корреляция/хэндлеры ·
//! client = tungstenite-транспорт. Реконнект-политика — у владельца (shell).

pub mod client;
pub mod endpoint;
pub mod frame;

pub use client::{RETRY_DELAY_MS, WsClient};
pub use endpoint::{Endpoint, EventListener, HostReply, HostRequestHandler, RequestResult};
pub use frame::RpcFrame;

#[cfg(test)]
#[path = "endpoint_tests.rs"]
mod endpoint_tests;

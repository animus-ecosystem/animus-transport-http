//! HTTP REST transport plugin for Animus.
//!
//! See [`backend::HttpTransportBackend`] for the `TransportBackend`
//! implementation. The Axum router is assembled in [`server::build_router`].

pub mod backend;
pub mod config;
pub mod control_client;
pub mod handlers;
pub mod server;

pub use backend::HttpTransportBackend;

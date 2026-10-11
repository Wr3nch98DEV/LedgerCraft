//! LedgerCraft service layer: the engine dispatcher (`rpc`), background jobs, document
//! rendering and organisation state. Used by the native app and the optional HTTP server.

pub mod jobs;
pub mod render;
pub mod rpc;
pub mod state;

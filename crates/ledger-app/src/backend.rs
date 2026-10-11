//! Bridge between the UI and the accounting engine.
//!
//! The engine is called in-process through the same command dispatcher the REST API uses
//! (`ledger_server::rpc::handle`), so permissions, validation and posting rules are identical
//! everywhere. Calls run on Tokio's blocking pool so the window never freezes.

use iced::Task;
use ledger_server::state::AppState;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub struct Backend {
    pub st: Arc<AppState>,
    /// Session token once a user has signed in (only needed when passwords are set).
    pub token: Option<String>,
}

pub type Reply = Result<Value, String>;

impl Backend {
    pub fn new(data_dir: std::path::PathBuf) -> Backend {
        std::fs::create_dir_all(data_dir.join("orgs")).ok();
        let st = Arc::new(AppState::new(data_dir));
        st.auto_open();
        Backend { st, token: None }
    }

    /// Run an engine command synchronously (used for quick lookups during startup and in tests).
    pub fn call_now(&self, method: &str, params: Value) -> Reply {
        ledger_server::rpc::handle(&self.st, self.token.as_deref(), method, params).map_err(|e| err_text(&e))
    }

    /// Run an engine command in the background and deliver the reply as a message.
    pub fn call<M: Send + 'static>(&self, method: &str, params: Value, to_msg: impl Fn(Reply) -> M + Send + 'static) -> Task<M> {
        let st = self.st.clone();
        let token = self.token.clone();
        let method = method.to_string();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || ledger_server::rpc::handle(&st, token.as_deref(), &method, params).map_err(|e| err_text(&e)))
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()))
            },
            to_msg,
        )
    }
}

fn err_text(e: &ledger_server::rpc::ApiError) -> String {
    use ledger_server::rpc::ApiError::*;
    match e { Unauthorized(s) | Forbidden(s) | NotFound(s) | Bad(s) => s.clone() }
}

pub fn default_data_dir() -> std::path::PathBuf { ledger_server::state::default_data_dir() }

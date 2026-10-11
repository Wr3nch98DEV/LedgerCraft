//! Feature modules dispatch: fixed assets, expense claims, projects, payroll, tax returns,
//! import/export and adviser tools. Each module registers its RPC methods here.

use crate::{reports::ReportParams, Error, Ledger, Result};
use serde_json::Value;

pub fn dispatch(l: &mut Ledger, method: &str, p: &Value, need: &dyn Fn(&str) -> Result<()>) -> Result<Option<Value>> {
    let _ = (l, p, need);
    match method {
        _ => Ok(None),
    }
}

pub fn notifications(_l: &Ledger) -> Result<Vec<Value>> { Ok(vec![]) }

pub fn report(_l: &Ledger, name: &str, _p: &ReportParams) -> Result<Value> {
    Err(Error::NotFound(format!("report '{name}'")))
}

pub fn seed_demo_extras(_l: &Ledger) -> Result<()> { Ok(()) }

pub fn render_view(_l: &Ledger, kind: &str, _id: i64) -> Result<String> {
    Err(Error::NotFound(format!("view '{kind}'")))
}

pub fn download(_l: &Ledger, kind: &str, _q: &std::collections::HashMap<String, String>) -> Result<(String, String, Vec<u8>)> {
    Err(Error::NotFound(format!("download '{kind}'")))
}

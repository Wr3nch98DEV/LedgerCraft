//! JSON RPC dispatch: POST /api/rpc/{method} with a JSON params body.

use crate::state::{AppState, Session};
use ledger_core::{
    accounts::AccountInput, banking::{BankRuleInput, BankTxInput, TransferInput}, contacts::ContactInput, documents::DocInput,
    items::ItemInput, payments::PaymentInput, reports::ReportParams, seed::SetupInput, statements::CsvMapping, tax::TaxRateInput,
    users::UserInput, util, Error, Ledger,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::fmt;

#[derive(Debug)]
pub enum ApiError { Unauthorized(String), Forbidden(String), NotFound(String), Bad(String) }

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self { ApiError::Unauthorized(m) | ApiError::Forbidden(m) | ApiError::NotFound(m) | ApiError::Bad(m) => write!(f, "{m}") }
    }
}

impl From<Error> for ApiError {
    fn from(e: Error) -> Self {
        match e { Error::NotFound(_) => ApiError::NotFound(e.to_string()), Error::Forbidden(_) => ApiError::Forbidden(e.to_string()), _ => ApiError::Bad(e.to_string()) }
    }
}

pub type R = Result<Value, ApiError>;

fn de<T: DeserializeOwned>(v: &Value) -> Result<T, ApiError> {
    serde_json::from_value(v.clone()).map_err(|e| ApiError::Bad(format!("invalid request: {e}")))
}
fn id(p: &Value) -> Result<i64, ApiError> { i(p, "id") }
fn i(p: &Value, k: &str) -> Result<i64, ApiError> { p[k].as_i64().ok_or_else(|| ApiError::Bad(format!("missing '{k}'"))) }
fn oi(p: &Value, k: &str) -> Option<i64> { p[k].as_i64() }
fn s<'a>(p: &'a Value, k: &str) -> &'a str { p[k].as_str().unwrap_or("") }
fn os(p: &Value, k: &str) -> Option<String> { p[k].as_str().filter(|s| !s.is_empty()).map(String::from) }
fn ok() -> R { Ok(json!({"ok": true})) }

pub fn handle(st: &AppState, token: Option<&str>, method: &str, p: Value) -> R {
    // ---- methods that don't need an open organisation ----
    match method {
        "orgs.list" => return Ok(json!(st.list_orgs())),
        "orgs.create" => {
            let input: SetupInput = de(&p)?;
            let slug: String = input.org_name.chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect::<String>().trim().replace(' ', "-").to_lowercase();
            let mut path = st.orgs_dir().join(format!("{}.lcdb", if slug.is_empty() { "organisation".into() } else { slug.clone() }));
            let mut n = 2;
            while path.exists() { path = st.orgs_dir().join(format!("{slug}-{n}.lcdb")); n += 1; }
            {
                let mut l = Ledger::open(&path)?;
                if let Err(e) = l.setup_org(&input) { drop(l); let _ = std::fs::remove_file(&path); return Err(e.into()); }
                if p["demo"].as_bool().unwrap_or(false) { ledger_core::demo::seed_demo(&l)?; }
            }
            st.open_org(&path).map_err(ApiError::Bad)?;
            return Ok(json!({"file": path.file_name().unwrap().to_string_lossy()}));
        }
        "orgs.open" => {
            let file = s(&p, "file");
            if file.contains('/') || file.contains('\\') || file.contains("..") { return Err(ApiError::Bad("invalid file".into())); }
            st.open_org(&st.orgs_dir().join(file)).map_err(ApiError::Bad)?;
            return ok();
        }
        _ => {}
    }

    // Computed before taking the org lock (list_orgs locks it too).
    let orgs = if method == "session.get" { json!(st.list_orgs()) } else { Value::Null };
    let mut guard = st.org.lock().unwrap();
    let org = match guard.as_mut() {
        Some(o) => o,
        None => {
            if method == "session.get" { return Ok(json!({"org": null, "orgs": orgs})); }
            return Err(ApiError::Bad("no organisation is open".into()));
        }
    };
    let l = &mut org.ledger;

    // ---- authentication ----
    if method == "auth.login" {
        let (uid, role, perms) = l.authenticate(s(&p, "email"), s(&p, "password"))?;
        l.check_2fa(uid, p["code"].as_str().filter(|c| !c.is_empty()))?;
        let tok = util::random_token(32);
        st.sessions.lock().unwrap().insert(tok.clone(), Session { user_id: uid, role, perms });
        return Ok(json!({"token": tok}));
    }
    if method == "auth.logout" {
        if let Some(t) = token { st.sessions.lock().unwrap().remove(t); }
        return ok();
    }
    let session = match token.and_then(|t| st.sessions.lock().unwrap().get(t).cloned()) {
        Some(s) => s,
        None => {
            if l.any_password_set()? {
                if method == "session.get" {
                    return Ok(json!({"org": {"name": l.setting("org_name")?, "file": org.path.file_name().unwrap().to_string_lossy()}, "login_required": true, "orgs": orgs}));
                }
                return Err(ApiError::Unauthorized("please sign in".into()));
            }
            let (role, perms) = l.user_permissions(1)?;
            Session { user_id: 1, role, perms }
        }
    };
    l.user_id = Some(session.user_id);
    l.role = session.role.clone();
    // Jobs that do network I/O run without holding the organisation lock.
    if matches!(method, "rates.fetch" | "email.test" | "jobs.run") {
        if !session.perms.iter().any(|x| x == "settings:write" || x == "accounting:write") { return Err(ApiError::Forbidden("you don't have permission".into())); }
        drop(guard);
        return match method {
            "rates.fetch" => crate::jobs::fetch_ecb_rates(st).map_err(ApiError::Bad),
            "email.test" => { crate::jobs::test_email(st, s(&p, "to")).map_err(ApiError::Bad)?; ok() }
            _ => Ok(crate::jobs::run_once(st)),
        };
    }
    let need = |perm: &str| -> Result<(), ApiError> {
        if session.perms.iter().any(|p| p == perm) { Ok(()) } else { Err(ApiError::Forbidden(format!("you don't have permission ({perm})"))) }
    };
    let file = org.path.file_name().unwrap().to_string_lossy().to_string();
    dispatch(l, method, &p, &need, &session, &file, &orgs)
}

fn report(l: &Ledger, name: &str, p: &ReportParams) -> R {
    Ok(ledger_core::extra_reports::run_any(l, name, p)?)
}

fn dispatch(l: &mut Ledger, method: &str, p: &Value, need: &dyn Fn(&str) -> Result<(), ApiError>, session: &Session, file: &str, orgs: &Value) -> R {
    match method {
        "session.get" => {
            let user = ledger_core::util::query_one_json(&l.conn, "SELECT id, name, email, role FROM users WHERE id = ?1", [session.user_id])?;
            Ok(json!({"org": {"name": l.setting("org_name")?, "file": file}, "user": user, "role": session.role, "perms": session.perms,
                "settings": l.settings_json()?, "orgs": orgs, "version": env!("CARGO_PKG_VERSION")}))
        }
        // ---- settings / users / audit ----
        "settings.get" => { need("settings:read")?; Ok(l.settings_json()?) }
        "settings.update" => { need("settings:write")?; l.update_settings(p)?; ok() }
        "users.list" => { need("users:read")?; Ok(json!(l.list_users()?)) }
        "users.create" => { need("users:write")?; Ok(json!({"id": l.create_user(&de::<UserInput>(p)?)?})) }
        "users.update" => { need("users:write")?; l.update_user(id(p)?, &de::<UserInput>(&p["user"])?)?; ok() }
        "users.setActive" => { need("users:write")?; l.set_user_active(id(p)?, p["active"].as_bool().unwrap_or(true))?; ok() }
        "users.setPassword" => {
            let uid = id(p)?;
            if uid != session.user_id { need("users:write")?; }
            l.set_password(uid, p["password"].as_str())?; ok()
        }
        "users.begin2fa" => Ok(l.begin_2fa(session.user_id)?),
        "users.confirm2fa" => { l.confirm_2fa(session.user_id, s(p, "code"))?; ok() }
        "users.disable2fa" => { let uid = oi(p, "id").unwrap_or(session.user_id); if uid != session.user_id { need("users:write")?; } l.disable_2fa(uid)?; ok() }
        "audit.list" => { need("accounting:read")?; Ok(json!(l.audit_log(oi(p, "limit").unwrap_or(200), p["entity"].as_str())?)) }
        "history.get" => Ok(json!(l.history(s(p, "entity"), id(p)?)?)),
        "notes.add" => { let nid = l.add_note(s(p, "entity"), id(p)?, "NOTE", s(p, "text"))?; Ok(json!({"id": nid})) }
        "search" => Ok(json!(l.search(s(p, "q"), oi(p, "limit").unwrap_or(20))?)),
        "journal.get" => { need("accounting:read")?; Ok(l.journal_json(id(p)?)?) }
        "dashboard" => Ok(ledger_core::dashboard::dashboard(l)?),
        "notifications" => Ok(json!(ledger_core::dashboard::notifications(l)?)),
        "cashflow.forecast" => Ok(ledger_core::dashboard::cashflow_forecast(l, oi(p, "days").unwrap_or(30), p["adjustments"].as_array().cloned().unwrap_or_default())?),

        // ---- chart of accounts / tax ----
        "accounts.list" => Ok(json!(l.list_accounts()?)),
        "accounts.get" => Ok(l.get_account(id(p)?)?),
        "accounts.create" => { need("accounting:write")?; Ok(json!({"id": l.create_account(&de::<AccountInput>(p)?)?})) }
        "accounts.update" => { need("accounting:write")?; l.update_account(id(p)?, &de::<AccountInput>(&p["account"])?)?; ok() }
        "accounts.setStatus" => { need("accounting:write")?; l.set_account_status(id(p)?, s(p, "status"))?; ok() }
        "accounts.delete" => { need("accounting:write")?; l.delete_account(id(p)?)?; ok() }
        "accounts.bulkTax" => { need("accounting:write")?; l.bulk_set_account_tax(&de::<Vec<i64>>(&p["ids"])?, oi(p, "tax_rate_id"))?; ok() }
        "accounts.bank" => Ok(json!(l.bank_accounts()?)),
        "taxRates.list" => Ok(json!(l.list_tax_rates()?)),
        "taxRates.create" => { need("accounting:write")?; Ok(json!({"id": l.create_tax_rate(&de::<TaxRateInput>(p)?)?})) }
        "taxRates.update" => { need("accounting:write")?; l.update_tax_rate(id(p)?, &de::<TaxRateInput>(&p["rate"])?)?; ok() }
        "taxRates.setStatus" => { need("accounting:write")?; l.set_tax_rate_status(id(p)?, s(p, "status"))?; ok() }

        // ---- contacts ----
        "contacts.list" => { need("contacts:read")?; Ok(json!(l.list_contacts(p["filter"].as_str().unwrap_or("all"), s(p, "q"))?)) }
        "contacts.get" => { need("contacts:read")?; Ok(l.get_contact(id(p)?)?) }
        "contacts.activity" => { need("contacts:read")?; Ok(json!(l.contact_activity(id(p)?)?)) }
        "contacts.create" => { need("contacts:write")?; Ok(json!({"id": l.create_contact(&de::<ContactInput>(p)?)?})) }
        "contacts.update" => { need("contacts:write")?; l.update_contact(id(p)?, &de::<ContactInput>(&p["contact"])?)?; ok() }
        "contacts.setStatus" => { need("contacts:write")?; l.set_contact_status(id(p)?, s(p, "status"))?; ok() }
        "contacts.merge" => { need("contacts:write")?; l.merge_contacts(i(p, "from")?, i(p, "into")?)?; ok() }
        "contacts.credits" => Ok(json!(l.contact_credits(id(p)?)?)),
        "contactGroups.list" => Ok(json!(l.list_contact_groups()?)),
        "contactGroups.create" => { need("contacts:write")?; Ok(json!({"id": l.create_contact_group(s(p, "name"))?})) }
        "contactGroups.delete" => { need("contacts:write")?; l.delete_contact_group(id(p)?)?; ok() }

        // ---- items ----
        "items.list" => Ok(json!(l.list_items(s(p, "q"))?)),
        "items.get" => Ok(l.get_item(id(p)?)?),
        "items.create" => { need("inventory:write")?; Ok(json!({"id": l.create_item(&de::<ItemInput>(p)?)?})) }
        "items.update" => { need("inventory:write")?; l.update_item(id(p)?, &de::<ItemInput>(&p["item"])?)?; ok() }
        "items.setStatus" => { need("inventory:write")?; l.set_item_status(id(p)?, s(p, "status"))?; ok() }
        "items.delete" => { need("inventory:write")?; l.delete_item(id(p)?)?; ok() }
        "items.adjust" => {
            need("inventory:write")?;
            let qty = ledger_core::money::parse_fixed(s(p, "quantity"), 4).ok_or_else(|| ApiError::Bad("invalid quantity".into()))?;
            let cost = os(p, "unit_cost").and_then(|c| ledger_core::money::parse_fixed(&c, 4));
            Ok(json!({"journal_id": l.adjust_inventory(id(p)?, s(p, "date"), qty, cost, i(p, "account_id")?, s(p, "reason"))?}))
        }
        "items.valuation" => Ok(l.inventory_valuation()?),

        // ---- documents (invoices, bills, credit notes, quotes, POs) ----
        "docs.list" => {
            let kind = s(p, "type");
            need(if ledger_core::documents::is_sales(kind) || kind == "SALES" || kind == "CREDIT_SALES" { "sales:read" } else { "purchases:read" })?;
            Ok(l.list_documents(kind, p["tab"].as_str().unwrap_or("all"), s(p, "q"), oi(p, "contact_id"), oi(p, "limit").unwrap_or(200), oi(p, "offset").unwrap_or(0))?)
        }
        "docs.get" => { let d = l.get_document(id(p)?)?; need(doc_perm(&d, false))?; Ok(d) }
        "docs.save" => {
            let input: DocInput = de(&p["doc"])?;
            let kind = match oi(p, "id") { Some(x) => l.get_document(x)?["type"].as_str().unwrap_or("").to_string(), None => input.kind.clone() };
            need(if ledger_core::documents::is_sales(&kind) { "sales:write" } else { "purchases:write" })?;
            Ok(json!({"id": l.save_document(oi(p, "id"), &input)?}))
        }
        "docs.setStatus" => { let d = l.get_document(id(p)?)?; need(doc_perm(&d, true))?; l.set_document_status(id(p)?, s(p, "status"))?; ok() }
        "docs.copy" => { let d = l.get_document(id(p)?)?; need(doc_perm(&d, true))?; Ok(json!({"id": l.copy_document(id(p)?, s(p, "to"), p["date"].as_str())?})) }
        "docs.markSent" => { l.mark_sent(id(p)?)?; ok() }
        "docs.email" => { let d = l.get_document(id(p)?)?; need(doc_perm(&d, true))?; Ok(ledger_core::email::email_document(l, id(p)?, os(p, "to"), os(p, "subject"), os(p, "body"))?) }
        "docs.emailPreview" => Ok(ledger_core::email::preview_document_email(l, id(p)?)?),
        "payments.add" => { need("banking:write").or_else(|_| need("sales:write"))?; Ok(json!({"id": l.add_payment(&de::<PaymentInput>(p)?)?})) }
        "payments.delete" => { need("banking:write")?; l.delete_payment(id(p)?)?; ok() }
        "credits.allocate" => { need("sales:write").or_else(|_| need("purchases:write"))?; Ok(json!({"id": l.allocate_credit(i(p, "credit_id")?, i(p, "document_id")?, s(p, "amount"), p["date"].as_str())?})) }
        "credits.deleteAllocation" => { l.delete_allocation(id(p)?)?; ok() }
        "prepayments.allocate" => { Ok(json!({"id": l.allocate_prepayment(i(p, "transaction_id")?, i(p, "document_id")?, s(p, "amount"), p["date"].as_str())?})) }
        "batch.create" => {
            need("banking:write")?;
            let items: Vec<(i64, String)> = p["items"].as_array().cloned().unwrap_or_default().iter().map(|x| (x["document_id"].as_i64().unwrap_or(0), x["amount"].as_str().unwrap_or("").to_string())).collect();
            Ok(json!({"id": l.batch_payment(i(p, "account_id")?, s(p, "date"), s(p, "reference"), &items)?}))
        }
        "batch.list" => Ok(json!(l.list_batch_payments()?)),
        "batch.get" => Ok(l.batch_payment_detail(id(p)?)?),

        // ---- banking ----
        "banktx.save" => { need("banking:write")?; Ok(json!({"id": l.save_bank_transaction(oi(p, "id"), &de::<BankTxInput>(&p["tx"])?)?})) }
        "banktx.get" => { need("banking:read")?; Ok(l.get_bank_transaction(id(p)?)?) }
        "banktx.void" => { need("banking:write")?; l.void_bank_transaction(id(p)?)?; ok() }
        "transfers.save" => { need("banking:write")?; Ok(json!({"id": l.save_transfer(&de::<TransferInput>(p)?)?})) }
        "transfers.void" => { need("banking:write")?; l.void_transfer(id(p)?)?; ok() }
        "bank.transactions" => { need("banking:read")?; Ok(json!(l.bank_account_transactions(id(p)?, oi(p, "limit").unwrap_or(500))?)) }
        "bankRules.list" => { need("banking:read")?; Ok(json!(l.list_bank_rules()?)) }
        "bankRules.save" => { need("banking:write")?; Ok(json!({"id": l.save_bank_rule(oi(p, "id"), &de::<BankRuleInput>(&p["rule"])?)?})) }
        "bankRules.delete" => { need("banking:write")?; l.delete_bank_rule(id(p)?)?; ok() }
        "statements.import" => {
            need("banking:write")?;
            let mapping: Option<CsvMapping> = if p["mapping"].is_object() { Some(de(&p["mapping"])?) } else { None };
            if let (Some(m), Some(name)) = (&mapping, os(p, "save_mapping_as")) { l.save_csv_mapping(oi(p, "account_id"), &name, m)?; }
            Ok(l.import_statement(i(p, "account_id")?, s(p, "filename"), s(p, "text"), p["format"].as_str(), mapping.as_ref(), p["date_order"].as_str())?)
        }
        "statements.preview" => {
            let rows = ledger_core::statements::parse_csv_rows(s(p, "text"));
            let fmt = ledger_core::statements::detect_format(s(p, "filename"), s(p, "text"));
            Ok(json!({"format": fmt, "rows": rows.into_iter().take(8).collect::<Vec<_>>()}))
        }
        "statements.mappings" => Ok(json!(l.csv_mappings(i(p, "account_id")?)?)),
        "statements.lines" => { need("banking:read")?; Ok(json!(l.statement_lines(i(p, "account_id")?, p["status"].as_str().unwrap_or("UNRECONCILED"), oi(p, "limit").unwrap_or(500))?)) }
        "statements.deleteLine" => { need("banking:write")?; l.delete_statement_line(id(p)?)?; ok() }
        "statements.note" => { l.set_statement_note(id(p)?, s(p, "note"))?; ok() }
        "rec.suggestions" => { need("banking:read")?; Ok(l.reconcile_suggestions(id(p)?)?) }
        "rec.unreconciled" => { need("banking:read")?; Ok(json!(l.unreconciled_ledger_items(i(p, "account_id")?)?)) }
        "rec.match" => {
            need("banking:write")?;
            let t: Vec<(String, i64)> = p["targets"].as_array().cloned().unwrap_or_default().iter().map(|x| (x["kind"].as_str().unwrap_or("").to_string(), x["id"].as_i64().unwrap_or(0))).collect();
            l.reconcile_match(id(p)?, &t)?; ok()
        }
        "rec.create" => { need("banking:write")?; Ok(json!({"id": l.reconcile_create(id(p)?, de::<BankTxInput>(&p["tx"])?)?})) }
        "rec.pay" => {
            need("banking:write")?;
            let docs: Vec<(i64, String)> = p["documents"].as_array().cloned().unwrap_or_default().iter().map(|x| (x["id"].as_i64().unwrap_or(0), x["amount"].as_str().unwrap_or("").to_string())).collect();
            let extra: Vec<(String, i64)> = p["targets"].as_array().cloned().unwrap_or_default().iter().map(|x| (x["kind"].as_str().unwrap_or("").to_string(), x["id"].as_i64().unwrap_or(0))).collect();
            l.reconcile_pay_documents(id(p)?, &docs, &extra)?; ok()
        }
        "rec.transfer" => { need("banking:write")?; Ok(json!({"id": l.reconcile_transfer(id(p)?, i(p, "account_id")?, os(p, "other_amount"))?})) }
        "rec.unreconcile" => { need("banking:write")?; l.unreconcile(id(p)?)?; ok() }
        "rec.auto" => { need("banking:write")?; Ok(json!({"reconciled": l.auto_reconcile(i(p, "account_id")?)?})) }
        "rec.cashcode" => {
            need("banking:write")?;
            let rows: Vec<(i64, i64, Option<i64>, Option<String>, String)> = p["rows"].as_array().cloned().unwrap_or_default().iter().map(|x| (
                x["line_id"].as_i64().unwrap_or(0), x["account_id"].as_i64().unwrap_or(0), x["tax_rate_id"].as_i64(), x["contact"].as_str().filter(|s| !s.is_empty()).map(String::from), x["description"].as_str().unwrap_or("").to_string())).collect();
            Ok(json!({"created": l.cash_code(&rows)?}))
        }
        "rec.report" => { need("banking:read")?; Ok(l.reconciliation_report(i(p, "account_id")?, p["date"].as_str().unwrap_or(&util::today()))?) }

        // ---- reports ----
        "reports.run" => { need("reports:read")?; report(l, s(p, "name"), &de::<ReportParams>(&p["params"])?) }

        _ => ledger_core::ext::dispatch_ext(l, method, p, &|perm| need(perm).map_err(|e| Error::Forbidden(e.to_string())))
            .map_err(ApiError::from)
            .and_then(|o| o.ok_or_else(|| ApiError::NotFound(format!("unknown method {method}")))),
    }
}

fn doc_perm(d: &Value, write: bool) -> &'static str {
    let sales = ledger_core::documents::is_sales(d["type"].as_str().unwrap_or(""));
    match (sales, write) { (true, false) => "sales:read", (true, true) => "sales:write", (false, false) => "purchases:read", (false, true) => "purchases:write" }
}

/// Public REST API (token auth) — mirrors the domain for integrations. See docs/API.md.
pub fn rest(st: &AppState, token: Option<&str>, verb: &str, path: &str, body: Value) -> R {
    let mut guard = st.org.lock().unwrap();
    let org = guard.as_mut().ok_or_else(|| ApiError::Bad("no organisation is open".into()))?;
    let l = &mut org.ledger;
    let (uid, perms) = ledger_core::ext::api_token_auth(l, token.unwrap_or("")).map_err(|_| ApiError::Unauthorized("invalid API token".into()))?;
    l.user_id = Some(uid);
    l.role = "STANDARD".into();
    let need = |perm: &str| -> Result<(), ApiError> { if perms.iter().any(|p| p == perm || p == "*") { Ok(()) } else { Err(ApiError::Forbidden(format!("token lacks scope {perm}"))) } };
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    let num = |i: usize| parts.get(i).and_then(|x| x.parse::<i64>().ok());
    let r = match (verb, parts.as_slice()) {
        ("GET", ["contacts"]) => { need("contacts:read")?; json!(l.list_contacts("everything", body["q"].as_str().unwrap_or(""))?) }
        ("GET", ["contacts", _]) => { need("contacts:read")?; l.get_contact(num(1).unwrap_or(0))? }
        ("POST", ["contacts"]) => { need("contacts:write")?; let id = l.create_contact(&de(&body)?)?; ledger_core::ext::fire_webhooks(l, "contact.created", id)?; json!({"id": id}) }
        ("GET", ["invoices"]) => { need("sales:read")?; l.list_documents("ACCREC", body["status"].as_str().unwrap_or("all"), body["q"].as_str().unwrap_or(""), None, 500, 0)? }
        ("GET", ["bills"]) => { need("purchases:read")?; l.list_documents("ACCPAY", body["status"].as_str().unwrap_or("all"), body["q"].as_str().unwrap_or(""), None, 500, 0)? }
        ("GET", ["invoices", _]) | ("GET", ["bills", _]) | ("GET", ["documents", _]) => { need("sales:read")?; l.get_document(num(1).unwrap_or(0))? }
        ("POST", ["invoices"]) | ("POST", ["bills"]) | ("POST", ["documents"]) => {
            let mut d: DocInput = de(&body)?;
            if parts[0] == "invoices" { d.kind = "ACCREC".into(); } else if parts[0] == "bills" { d.kind = "ACCPAY".into(); }
            need(if ledger_core::documents::is_sales(&d.kind) { "sales:write" } else { "purchases:write" })?;
            let id = l.save_document(None, &d)?;
            ledger_core::ext::fire_webhooks(l, "document.created", id)?;
            json!({"id": id})
        }
        ("POST", ["payments"]) => { need("banking:write")?; let id = l.add_payment(&de(&body)?)?; ledger_core::ext::fire_webhooks(l, "payment.created", id)?; json!({"id": id}) }
        ("GET", ["accounts"]) => { need("accounting:read")?; json!(l.list_accounts()?) }
        ("GET", ["tax-rates"]) => { need("accounting:read")?; json!(l.list_tax_rates()?) }
        ("GET", ["items"]) => { need("inventory:read")?; json!(l.list_items("")?) }
        ("POST", ["bank-transactions"]) => { need("banking:write")?; let id = l.save_bank_transaction(None, &de(&body)?)?; ledger_core::ext::fire_webhooks(l, "banktransaction.created", id)?; json!({"id": id}) }
        ("GET", ["journals", _]) => { need("accounting:read")?; l.journal_json(num(1).unwrap_or(0))? }
        ("GET", ["reports", name]) => { need("reports:read")?; report(l, name, &de(&body)?)? }
        ("GET", ["openapi.json"]) => ledger_core::ext::openapi(),
        _ => return Err(ApiError::NotFound(format!("no route {verb} /api/v1/{path}"))),
    };
    Ok(r)
}

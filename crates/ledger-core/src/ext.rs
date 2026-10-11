//! Extension dispatch for feature modules beyond the core (called by the server's RPC layer),
//! API tokens, webhooks and shared helpers.

use crate::{advanced::ManualJournalInput, documents::DocInput, error::invalid, tax::Lat, util, Error, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};

type Need<'a> = &'a dyn Fn(&str) -> Result<()>;

fn i(p: &Value, k: &str) -> Result<i64> { p[k].as_i64().ok_or_else(|| Error::Validation(format!("missing '{k}'"))) }
fn s<'a>(p: &'a Value, k: &str) -> &'a str { p[k].as_str().unwrap_or("") }
fn ok() -> Result<Option<Value>> { Ok(Some(json!({"ok": true}))) }

pub fn dispatch_ext(l: &mut Ledger, method: &str, p: &Value, need: Need) -> Result<Option<Value>> {
    let r = match method {
        // tracking
        "tracking.list" => json!(l.list_tracking()?),
        "tracking.save" => { need("accounting:write")?; let opts: Vec<String> = serde_json::from_value(p["options"].clone()).unwrap_or_default(); json!({"id": l.save_tracking_category(p["id"].as_i64(), s(p, "name"), &opts)?}) }
        "tracking.optionStatus" => { need("accounting:write")?; l.set_tracking_option_status(i(p, "id")?, s(p, "status"))?; return ok(); }
        "tracking.renameOption" => { need("accounting:write")?; l.rename_tracking_option(i(p, "id")?, s(p, "name"))?; return ok(); }
        "tracking.categoryStatus" => { need("accounting:write")?; l.set_tracking_category_status(i(p, "id")?, s(p, "status"))?; return ok(); }
        // currencies
        "currencies.list" => json!(l.list_currencies()?),
        "currencies.enable" => { need("settings:write")?; l.enable_currency(s(p, "code"), p["enabled"].as_bool().unwrap_or(true))?; return ok(); }
        "rates.list" => json!(l.list_rates(s(p, "code"))?),
        "rates.set" => { need("accounting:write")?; l.set_rate(s(p, "code"), s(p, "date"), s(p, "rate"), "MANUAL")?; return ok(); }
        "rates.get" => json!({"rate_e8": l.rate_for(s(p, "code"), s(p, "date")).ok()}),
        "fx.revalue" => { need("accounting:write")?; l.fx_revalue(s(p, "date"))? }
        // manual journals
        "mj.list" => { need("accounting:read")?; json!(l.list_manual_journals(p["status"].as_str().unwrap_or("ALL"))?) }
        "mj.get" => { need("accounting:read")?; l.get_manual_journal(i(p, "id")?)? }
        "mj.save" => { need("accounting:write")?; json!({"id": l.save_manual_journal(p["id"].as_i64(), &serde_json::from_value::<ManualJournalInput>(p["journal"].clone())?)?}) }
        "mj.post" => { need("accounting:write")?; l.post_manual_journal(i(p, "id")?)?; return ok(); }
        "mj.void" => { need("accounting:write")?; l.void_manual_journal(i(p, "id")?)?; return ok(); }
        // repeating
        "repeating.list" => json!(l.list_repeating(p["kind"].as_str().unwrap_or("ALL"))?),
        "repeating.save" => { json!({"id": l.save_repeating(p["id"].as_i64(), &p["repeating"])?}) }
        "repeating.setStatus" => { l.set_repeating_status(i(p, "id")?, s(p, "status"))?; return ok(); }
        "repeating.run" => json!(l.run_repeating(p["date"].as_str().unwrap_or(&util::today()))?),
        // conversion balances
        "conversion.get" => l.conversion_balances()?,
        "conversion.save" => { need("accounting:write")?; json!({"journal_id": l.save_conversion_balances(s(p, "date"), p["lines"].as_array().map(|a| a.as_slice()).unwrap_or(&[]))?}) }
        // budgets
        "budgets.list" => json!(l.list_budgets()?),
        "budgets.get" => l.get_budget(i(p, "id")?)?,
        "budgets.save" => { need("accounting:write")?; json!({"id": l.save_budget(p["id"].as_i64(), s(p, "name"), p["tracking_option_id"].as_i64(), p["lines"].as_array().map(|a| a.as_slice()).unwrap_or(&[]))?}) }
        // find & recode
        "recode.find" => { need("accounting:read")?; json!(l.find_lines(p)?) }
        "recode.apply" => {
            need("accounting:write")?;
            let ids: Vec<i64> = serde_json::from_value(p["line_ids"].clone()).unwrap_or_default();
            json!({"transactions": l.recode_lines(&ids, p["account_id"].as_i64(), p["tax_rate_id"].as_i64(), p["tracking1"].as_i64(), p["tracking2"].as_i64())?})
        }
        // API tokens & webhooks
        "api.tokens" => { need("settings:read")?; json!(crate::util::query_json(&l.conn, "SELECT id, name, prefix, scopes, created_at, last_used, revoked FROM api_tokens ORDER BY id DESC", [])?) }
        "api.createToken" => { need("settings:write")?; create_api_token(l, s(p, "name"), &serde_json::from_value::<Vec<String>>(p["scopes"].clone()).unwrap_or_else(|_| vec!["*".into()]))? }
        "api.revokeToken" => { need("settings:write")?; l.conn.execute("UPDATE api_tokens SET revoked = 1 WHERE id = ?1", [i(p, "id")?])?; return ok(); }
        "webhooks.list" => json!(crate::util::query_json(&l.conn, "SELECT id, url, events, active FROM webhooks", [])?),
        "webhooks.save" => {
            need("settings:write")?;
            let url = s(p, "url");
            if !(url.starts_with("http://") || url.starts_with("https://")) { return invalid("webhook URL must start with http(s)://"); }
            l.conn.execute("INSERT INTO webhooks(url, events, secret) VALUES (?1, ?2, ?3)", params![url, p["events"].to_string(), util::random_token(32)])?;
            json!({"id": l.conn.last_insert_rowid()})
        }
        "webhooks.delete" => { need("settings:write")?; l.conn.execute("DELETE FROM webhooks WHERE id = ?1", [i(p, "id")?])?; return ok(); }
        "webhooks.deliveries" => json!(crate::util::query_json(&l.conn, "SELECT * FROM webhook_deliveries ORDER BY id DESC LIMIT 100", [])?),
        "openapi" => openapi(),
        // branding themes & email templates
        "branding.list" => json!(crate::util::query_json(&l.conn, "SELECT * FROM branding_themes ORDER BY is_default DESC, name", [])?),
        "branding.save" => {
            need("settings:write")?;
            let t = &p["theme"];
            let id = match p["id"].as_i64() {
                Some(id) => id,
                None => { l.conn.execute("INSERT INTO branding_themes(name) VALUES (?1)", [t["name"].as_str().unwrap_or("New theme")])?; l.conn.last_insert_rowid() }
            };
            l.conn.execute("UPDATE branding_themes SET name=?2, font=?3, accent=?4, terms=?5, payment_advice=?6, show_tax_number=?7, payment_instructions=?8, invoice_title=?9, credit_title=?10, quote_title=?11, po_title=?12, logo_attachment_id=?13 WHERE id=?1",
                params![id, t["name"].as_str().unwrap_or("Theme"), t["font"].as_str().unwrap_or("Inter"), t["accent"].as_str().unwrap_or("#0f766e"), t["terms"].as_str().unwrap_or(""),
                    t["payment_advice"].as_bool().unwrap_or(true) as i64, t["show_tax_number"].as_bool().unwrap_or(true) as i64, t["payment_instructions"].as_str().unwrap_or(""),
                    t["invoice_title"].as_str().unwrap_or("Tax Invoice"), t["credit_title"].as_str().unwrap_or("Credit Note"), t["quote_title"].as_str().unwrap_or("Quote"), t["po_title"].as_str().unwrap_or("Purchase Order"), t["logo_attachment_id"].as_i64()])?;
            if p["default"].as_bool() == Some(true) { l.conn.execute("UPDATE branding_themes SET is_default = (id = ?1)", [id])?; }
            json!({"id": id})
        }
        "branding.delete" => { need("settings:write")?; l.conn.execute("DELETE FROM branding_themes WHERE id = ?1 AND is_default = 0", [i(p, "id")?])?; return ok(); }
        "emailTemplates.list" => json!(crate::util::query_json(&l.conn, "SELECT * FROM email_templates ORDER BY kind, is_default DESC", [])?),
        "emailTemplates.save" => {
            need("settings:write")?;
            l.conn.execute("UPDATE email_templates SET subject = ?2, body = ?3, name = ?4 WHERE id = ?1", params![i(p, "id")?, s(p, "subject"), s(p, "body"), s(p, "name")])?;
            return ok();
        }
        "reminders.list" => json!(crate::util::query_json(&l.conn, "SELECT r.*, t.name AS template_name FROM reminder_rules r LEFT JOIN email_templates t ON t.id = r.template_id ORDER BY days", [])?),
        "reminders.save" => {
            need("settings:write")?;
            match p["id"].as_i64() {
                Some(id) => { l.conn.execute("UPDATE reminder_rules SET days = ?2, active = ?3 WHERE id = ?1", params![id, p["days"].as_i64().unwrap_or(7), p["active"].as_bool().unwrap_or(true) as i64])?; }
                None => { l.conn.execute("INSERT INTO reminder_rules(days, template_id, active) SELECT ?1, id, 1 FROM email_templates WHERE kind = 'REMINDER' LIMIT 1", [p["days"].as_i64().unwrap_or(7)])?; }
            }
            return ok();
        }
        "reminders.delete" => { l.conn.execute("DELETE FROM reminder_rules WHERE id = ?1", [i(p, "id")?])?; return ok(); }
        "reminders.run" => json!({"queued": crate::email::run_reminders(l)?}),
        "outbox.list" => json!(crate::email::outbox(l)?),
        "reports.catalogue" => json!(crate::extra_reports::CATALOGUE.iter().map(|(k, t, c, d)| json!({"key": k, "title": t, "category": c, "description": d})).collect::<Vec<_>>()),
        _ => return crate::modules::dispatch(l, method, p, need),
    };
    Ok(Some(r))
}

pub fn extra_notifications(l: &Ledger) -> Result<Vec<Value>> { crate::modules::notifications(l) }

/// Total of a repeating template (used for cash-flow projection).
pub fn estimate_template_total(l: &Ledger, t: &Value) -> Result<i64> {
    let d: DocInput = serde_json::from_value(t.clone()).unwrap_or_default();
    let lat = Lat::parse(d.line_amount_type.as_deref().unwrap_or("")).unwrap_or(Lat::Exclusive);
    let lines = l.compute_lines(lat, &d.lines, true)?;
    let total = lines.iter().map(|x| x.net + x.tax).sum::<i64>();
    let cur = d.currency.unwrap_or(l.base_currency()?);
    Ok(if cur == l.base_currency()? { total } else { l.rate_for(&cur, &util::today()).and_then(|r| l.to_base_amount(total, &cur, r)).unwrap_or(total) })
}

pub fn advance_date(date: &str, unit: &str, count: i64) -> Result<String> {
    let d = util::parse_date(date)?;
    Ok(util::fmt_date(if unit == "WEEKLY" { d + chrono::Duration::days(7 * count) } else { util::add_months(d, count as i32) }))
}

fn sha256_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(s.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

pub fn create_api_token(l: &Ledger, name: &str, scopes: &[String]) -> Result<Value> {
    if name.trim().is_empty() { return invalid("give the token a name"); }
    let token = format!("lc_{}", util::random_token(40));
    let prefix = token[..10].to_string();
    l.conn.execute("INSERT INTO api_tokens(name, token_hash, prefix, scopes, user_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![name, sha256_hex(&token), prefix, json!(scopes).to_string(), l.user_id.unwrap_or(1)])?;
    l.audit("CREATE", "api_token", Some(l.conn.last_insert_rowid()), None, Some(json!({"name": name})))?;
    Ok(json!({"token": token, "note": "Copy this token now; it won't be shown again."}))
}

/// Validate a bearer token. Returns (user_id, scopes).
pub fn api_token_auth(l: &Ledger, token: &str) -> Result<(i64, Vec<String>)> {
    if token.is_empty() { return Err(Error::Forbidden("missing token".into())); }
    let (id, uid, scopes): (i64, i64, String) = l.conn.query_row("SELECT id, user_id, scopes FROM api_tokens WHERE token_hash = ?1 AND revoked = 0", [sha256_hex(token)], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|_| Error::Forbidden("invalid token".into()))?;
    l.conn.execute("UPDATE api_tokens SET last_used = datetime('now') WHERE id = ?1", [id])?;
    let mut sc: Vec<String> = serde_json::from_str(&scopes).unwrap_or_default();
    if sc.iter().any(|x| x == "*") {
        sc = crate::users::AREAS.iter().flat_map(|a| [format!("{a}:read"), format!("{a}:write")]).collect();
    }
    Ok((uid, sc))
}

/// Queue webhook deliveries (sent by the server's background worker).
pub fn fire_webhooks(l: &Ledger, event: &str, id: i64) -> Result<()> {
    let hooks: Vec<(i64, String)> = {
        let mut st = l.conn.prepare("SELECT id, events FROM webhooks WHERE active = 1")?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (hid, events) in hooks {
        let ev: Vec<String> = serde_json::from_str(&events).unwrap_or_default();
        if ev.iter().any(|e| e == "*" || e == event) {
            l.conn.execute("INSERT INTO webhook_deliveries(webhook_id, event, payload) VALUES (?1, ?2, ?3)",
                params![hid, event, json!({"event": event, "id": id, "at": chrono::Utc::now().to_rfc3339()}).to_string()])?;
        }
    }
    Ok(())
}

pub fn openapi() -> Value {
    let path = |summary: &str, scope: &str| json!({"summary": summary, "security": [{"bearer": [scope]}], "responses": {"200": {"description": "OK"}}});
    json!({
        "openapi": "3.0.3",
        "info": {"title": "LedgerCraft Local API", "version": env!("CARGO_PKG_VERSION"), "description": "Local REST API (127.0.0.1 only). Authenticate with `Authorization: Bearer <token>` created under Settings → Connected apps."},
        "servers": [{"url": "http://127.0.0.1:4180/api/v1"}],
        "components": {"securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}}},
        "paths": {
            "/contacts": {"get": path("List contacts", "contacts:read"), "post": path("Create a contact", "contacts:write")},
            "/contacts/{id}": {"get": path("Get a contact", "contacts:read")},
            "/invoices": {"get": path("List invoices (status, q)", "sales:read"), "post": path("Create an invoice (DocInput)", "sales:write")},
            "/invoices/{id}": {"get": path("Get an invoice", "sales:read")},
            "/bills": {"get": path("List bills", "purchases:read"), "post": path("Create a bill", "purchases:write")},
            "/documents/{id}": {"get": path("Get any document", "sales:read")},
            "/payments": {"post": path("Apply a payment (PaymentInput)", "banking:write")},
            "/bank-transactions": {"post": path("Create spend/receive money", "banking:write")},
            "/accounts": {"get": path("Chart of accounts", "accounting:read")},
            "/tax-rates": {"get": path("Tax rates", "accounting:read")},
            "/items": {"get": path("Products & services", "inventory:read")},
            "/journals/{id}": {"get": path("A posted journal", "accounting:read")},
            "/reports/{name}": {"get": path("Run a report (profit-and-loss, balance-sheet, trial-balance, aged-receivables, ...)", "reports:read")},
        },
        "x-webhooks": ["contact.created", "document.created", "payment.created", "banktransaction.created"],
    })
}

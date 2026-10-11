//! Printable HTML documents (invoice, quote, PO, credit note, statement, payslip) and downloads.
//! Documents are rendered as self-contained HTML with a print stylesheet; "Save as PDF" uses
//! the system print dialog, which keeps the binary small (no PDF engine bundled).

use crate::state::AppState;
use ledger_core::{money::fmt_money, reports::fmt_long, util, Ledger};
use serde_json::Value;
use std::collections::HashMap;

fn b64(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len() * 4 / 3 + 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 { if i <= c.len() { out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char) } else { out.push('=') } }
    }
    out
}

fn logo_html(l: &Ledger, theme: &Value) -> String {
    let Some(id) = theme["logo_attachment_id"].as_i64() else { return String::new() };
    match l.attachment(id) {
        Ok((_, mime, data)) if mime.starts_with("image/") => format!("<img src=\"data:{mime};base64,{}\" alt=\"\" style=\"max-height:64px;max-width:220px;margin-bottom:10px\"><br>", b64(&data)),
        _ => String::new(),
    }
}

fn esc(s: &str) -> String { s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;") }
fn nl(s: &str) -> String { esc(s).replace('\n', "<br>") }
fn qty(e4: i64) -> String { let s = ledger_core::money::fmt_fixed(e4, 4); let s = s.trim_end_matches('0').trim_end_matches('.').to_string(); if s.is_empty() { "0".into() } else { s } }
fn price(e2: i64) -> String { let s = ledger_core::money::fmt_fixed(e2, 4); let t = s.trim_end_matches('0'); let dp = t.split('.').nth(1).map(|x| x.len()).unwrap_or(0); if dp < 2 { ledger_core::money::fmt_fixed(e2 / 100, 2) } else { t.to_string() } }
fn m(v: &Value) -> String { group(&fmt_money(v.as_i64().unwrap_or(0))) }
fn group(s: &str) -> String {
    let (neg, s) = if let Some(r) = s.strip_prefix('-') { (true, r) } else { (false, s) };
    let (w, f) = s.split_once('.').unwrap_or((s, ""));
    let mut out = String::new();
    for (i, c) in w.chars().enumerate() { if i > 0 && (w.len() - i) % 3 == 0 { out.push(','); } out.push(c); }
    let r = if f.is_empty() { out } else { format!("{out}.{f}") };
    if neg { format!("-{r}") } else { r }
}

const STYLE: &str = r#"
:root{--ink:#111827;--muted:#6b7280;--line:#e5e7eb;--accent:#0f766e}
*{box-sizing:border-box}body{font-family:Inter,Segoe UI,system-ui,sans-serif;color:var(--ink);margin:0;background:#f3f4f6;font-size:14px}
.page{background:#fff;max-width:820px;margin:24px auto;padding:48px 56px;box-shadow:0 1px 3px rgba(0,0,0,.1)}
.top{display:flex;justify-content:space-between;align-items:flex-start;gap:24px}
h1{font-size:28px;margin:0 0 4px;letter-spacing:.5px;color:var(--accent)}
.muted{color:var(--muted)}.right{text-align:right}.meta{display:grid;grid-template-columns:auto auto;gap:2px 16px;font-size:13px}
.meta b{font-weight:600}
table{width:100%;border-collapse:collapse;margin-top:28px}th{text-align:left;font-size:12px;text-transform:uppercase;color:var(--muted);border-bottom:2px solid var(--ink);padding:6px 4px}
td{padding:8px 4px;border-bottom:1px solid var(--line);vertical-align:top}td.n,th.n{text-align:right;white-space:nowrap}
.totals{margin-left:auto;width:300px;margin-top:12px}.totals div{display:flex;justify-content:space-between;padding:4px 4px}
.totals .grand{border-top:2px solid var(--ink);font-weight:700;font-size:16px;margin-top:4px;padding-top:8px}
.terms{margin-top:32px;font-size:13px;white-space:pre-wrap}
.advice{margin-top:40px;border-top:1px dashed var(--muted);padding-top:16px;font-size:13px}
.bar{max-width:820px;margin:16px auto 0;display:flex;gap:8px;justify-content:flex-end}
.bar button,.bar a{background:var(--accent);color:#fff;border:0;border-radius:6px;padding:8px 14px;font-weight:600;cursor:pointer;text-decoration:none;font-size:14px}
.status{display:inline-block;padding:2px 8px;border-radius:999px;font-size:12px;font-weight:600;background:#ecfdf5;color:#047857}
.status.due{background:#fef3c7;color:#92400e}
@media print{body{background:#fff}.page{box-shadow:none;margin:0;max-width:none;padding:0}.bar{display:none}}
"#;

fn org_block(l: &Ledger) -> String {
    let name = l.setting_or("org_name", "").unwrap_or_default();
    let addr = l.setting_or("address", "").unwrap_or_default();
    let phone = l.setting_or("phone", "").unwrap_or_default();
    let email = l.setting_or("email", "").unwrap_or_default();
    let tax = l.setting_or("tax_number", "").unwrap_or_default();
    let label = match l.setting_or("country", "AU").unwrap_or_default().as_str() { "AU" => "ABN", "NZ" => "GST No.", "GB" | "IE" => "VAT No.", "CA" => "BN", _ => "Tax No." };
    format!("<div class='right'><b>{}</b><br><span class='muted'>{}{}{}{}</span></div>", esc(&name), nl(&addr),
        if phone.is_empty() { String::new() } else { format!("<br>{}", esc(&phone)) }, if email.is_empty() { String::new() } else { format!("<br>{}", esc(&email)) },
        if tax.is_empty() { String::new() } else { format!("<br>{label} {}", esc(&tax)) })
}

pub fn document_html(l: &Ledger, id: i64, online: bool) -> Result<String, String> {
    let d = l.get_document(id).map_err(|e| e.to_string())?;
    let kind = d["type"].as_str().unwrap_or("");
    let theme = ledger_core::util::query_one_json(&l.conn, "SELECT * FROM branding_themes WHERE id = COALESCE(?1, (SELECT id FROM branding_themes WHERE is_default = 1))", [d["branding_theme_id"].as_i64()]).unwrap_or(Value::Null);
    let title = match kind {
        "ACCREC" => theme["invoice_title"].as_str().unwrap_or("Tax Invoice"), "ACCRECCREDIT" | "ACCPAYCREDIT" => theme["credit_title"].as_str().unwrap_or("Credit Note"),
        "QUOTE" => theme["quote_title"].as_str().unwrap_or("Quote"), "PO" => theme["po_title"].as_str().unwrap_or("Purchase Order"), _ => "Bill" };
    let accent = theme["accent"].as_str().unwrap_or("#0f766e");
    let tax_label = l.setting_or("tax_label", "Tax").unwrap_or_default();
    let lat = d["line_amount_type"].as_str().unwrap_or("EXCLUSIVE");
    let has_disc = d["lines"].as_array().unwrap().iter().any(|x| x["discount_bp"].as_i64().unwrap_or(0) != 0);
    let mut rows = String::new();
    for li in d["lines"].as_array().unwrap() {
        rows += &format!("<tr><td>{}</td><td class='n'>{}</td><td class='n'>{}</td>{}<td class='n'>{}</td><td class='n'>{}</td></tr>",
            nl(li["description"].as_str().unwrap_or("")), qty(li["qty_e4"].as_i64().unwrap_or(0)), price(li["unit_e2"].as_i64().unwrap_or(0)),
            if has_disc { format!("<td class='n'>{}%</td>", ledger_core::money::fmt_fixed(li["discount_bp"].as_i64().unwrap_or(0), 2).trim_end_matches('0').trim_end_matches('.')) } else { String::new() },
            if lat == "NOTAX" { "".into() } else { esc(li["tax_rate_name"].as_str().unwrap_or("")) }, m(&li["line_amount"]));
    }
    let mut totals = String::new();
    if lat == "INCLUSIVE" {
        totals += &format!("<div><span>Total includes {tax_label}</span><span>{}</span></div>", m(&d["total_tax"]));
    } else {
        totals += &format!("<div><span>Subtotal</span><span>{}</span></div>", m(&d["subtotal"]));
        if lat != "NOTAX" {
            for t in d["tax_summary"].as_array().unwrap() { totals += &format!("<div><span>Total {}</span><span>{}</span></div>", esc(t["name"].as_str().unwrap_or(&tax_label)), m(&t["tax"])); }
        }
    }
    totals += &format!("<div class='grand'><span>TOTAL {}</span><span>{}</span></div>", esc(d["currency"].as_str().unwrap_or("")), m(&d["total"]));
    if matches!(kind, "ACCREC" | "ACCPAY" | "ACCRECCREDIT" | "ACCPAYCREDIT") {
        let paid = d["amount_paid"].as_i64().unwrap_or(0) + d["amount_credited"].as_i64().unwrap_or(0);
        if paid > 0 { totals += &format!("<div><span>Less amount paid</span><span>{}</span></div>", group(&fmt_money(paid))); }
        totals += &format!("<div class='grand'><span>AMOUNT DUE {}</span><span>{}</span></div>", esc(d["currency"].as_str().unwrap_or("")), m(&d["amount_due"]));
    }
    let meta = match kind {
        "QUOTE" => format!("<b>Date</b><span>{}</span><b>Expiry</b><span>{}</span><b>Quote Number</b><span>{}</span>", fmt_long(d["date"].as_str().unwrap_or("")), d["expiry_date"].as_str().map(fmt_long).unwrap_or_default(), esc(d["number"].as_str().unwrap_or(""))),
        "PO" => format!("<b>Date</b><span>{}</span><b>Delivery Date</b><span>{}</span><b>Order Number</b><span>{}</span>", fmt_long(d["date"].as_str().unwrap_or("")), d["delivery_date"].as_str().map(fmt_long).unwrap_or_default(), esc(d["number"].as_str().unwrap_or(""))),
        _ => format!("<b>Invoice Date</b><span>{}</span><b>{}</b><span>{}</span><b>Due Date</b><span>{}</span>", fmt_long(d["date"].as_str().unwrap_or("")),
            if kind.ends_with("CREDIT") { "Credit Note Number" } else { "Invoice Number" }, esc(d["number"].as_str().unwrap_or("")), d["due_date"].as_str().map(fmt_long).unwrap_or_default()),
    };
    let reference = d["reference"].as_str().filter(|s| !s.is_empty()).map(|r| format!("<b>Reference</b><span>{}</span>", esc(r))).unwrap_or_default();
    let summary = if kind == "QUOTE" && !d["title"].as_str().unwrap_or("").is_empty() { format!("<h3 style='margin:24px 0 4px'>{}</h3><div class='muted'>{}</div>", esc(d["title"].as_str().unwrap_or("")), nl(d["summary"].as_str().unwrap_or(""))) } else { String::new() };
    let terms_text = format!("{}{}", d["terms"].as_str().unwrap_or(""), theme["terms"].as_str().map(|t| if t.is_empty() { String::new() } else { format!("\n{t}") }).unwrap_or_default());
    let terms = if terms_text.trim().is_empty() { String::new() } else { format!("<div class='terms'><b>Terms</b><br>{}</div>", esc(terms_text.trim())) };
    let instr = theme["payment_instructions"].as_str().unwrap_or("");
    let advice = if kind == "ACCREC" && theme["payment_advice"].as_i64() == Some(1) {
        format!("<div class='advice'><b>PAYMENT ADVICE</b><div style='display:flex;justify-content:space-between;margin-top:8px'><div>To: {}<br><span class='muted'>{}</span></div><div class='meta'><b>Customer</b><span>{}</span><b>Invoice Number</b><span>{}</span><b>Amount Due</b><span>{}</span><b>Due Date</b><span>{}</span></div></div></div>",
            esc(&l.setting_or("org_name", "").unwrap_or_default()), nl(instr), esc(d["contact_name"].as_str().unwrap_or("")), esc(d["number"].as_str().unwrap_or("")), m(&d["amount_due"]), d["due_date"].as_str().map(fmt_long).unwrap_or_default())
    } else { String::new() };
    let status = if online {
        let due = d["amount_due"].as_i64().unwrap_or(0);
        format!("<div style='margin-top:8px'>{}</div>", if d["status"] == "PAID" { "<span class='status'>Paid</span>".to_string() } else if due > 0 { format!("<span class='status due'>{} due {}</span>", m(&d["amount_due"]), d["due_date"].as_str().map(fmt_long).unwrap_or_default()) } else { String::new() })
    } else { String::new() };
    let disc_head = if has_disc { "<th class='n'>Disc.</th>" } else { "" };
    Ok(format!(r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{title} {num}</title><style>{STYLE}:root{{--accent:{accent}}}</style></head><body>
<div class="bar"><button onclick="window.print()">Print / Save as PDF</button></div>
<div class="page">
<div class="top"><div>{logo}<h1>{title_up}</h1><div>{contact}</div><div class="muted">{caddr}</div>{status}</div><div class="meta">{meta}{reference}</div>{org}</div>
{summary}
<table><thead><tr><th>Description</th><th class="n">Quantity</th><th class="n">Unit Price</th>{disc_head}<th class="n">{taxh}</th><th class="n">Amount {cur}</th></tr></thead><tbody>{rows}</tbody></table>
<div class="totals">{totals}</div>
{terms}{instructions}{advice}
</div></body></html>"#,
        num = esc(d["number"].as_str().unwrap_or("")), title_up = esc(&title.to_uppercase()), logo = logo_html(l, &theme), contact = esc(d["contact_name"].as_str().unwrap_or("")),
        caddr = nl(d["contact_address"].as_str().unwrap_or("")), org = org_block(l), taxh = if lat == "NOTAX" { "".to_string() } else { esc(&tax_label) },
        cur = esc(d["currency"].as_str().unwrap_or("")), instructions = if kind == "ACCREC" && !instr.is_empty() { format!("<div class='terms'><b>How to pay</b><br>{}</div>", nl(instr)) } else { String::new() }))
}

pub fn statement_html(l: &Ledger, contact_id: i64, from: &str, to: &str, kind: &str) -> Result<String, String> {
    let c = l.get_contact(contact_id).map_err(|e| e.to_string())?;
    let docs = util::query_json(&l.conn,
        "SELECT d.id, d.type, d.number, d.reference, d.date, d.due_date, d.total, d.total - d.amount_paid - d.amount_credited AS due, d.currency FROM documents d
         WHERE d.contact_id = ?1 AND d.type IN ('ACCREC','ACCRECCREDIT') AND d.status IN ('AUTHORISED','PAID') AND (?4 = 'ACTIVITY' AND d.date >= ?2 AND d.date <= ?3 OR ?4 = 'OUTSTANDING' AND d.status = 'AUTHORISED' AND d.date <= ?3)
         ORDER BY d.date", ledger_core::rusqlite::params![contact_id, from, to, kind]).map_err(|e| e.to_string())?;
    let pays = if kind == "ACTIVITY" { util::query_json(&l.conn, "SELECT p.date, p.amount, d.number FROM payments p JOIN documents d ON d.id = p.document_id WHERE d.contact_id = ?1 AND d.type = 'ACCREC' AND p.status <> 'DELETED' AND p.date >= ?2 AND p.date <= ?3 ORDER BY p.date",
        ledger_core::rusqlite::params![contact_id, from, to]).map_err(|e| e.to_string())? } else { vec![] };
    let mut rows = String::new();
    let mut bal = 0i64;
    let mut events: Vec<(String, String, i64, i64)> = Vec::new();
    for d in &docs {
        let credit = d["type"] == "ACCRECCREDIT";
        let amt = if kind == "OUTSTANDING" { d["due"].as_i64().unwrap_or(0) } else { d["total"].as_i64().unwrap_or(0) };
        events.push((d["date"].as_str().unwrap_or("").into(), format!("{} {}{}", if credit { "Credit Note" } else { "Invoice" }, d["number"].as_str().unwrap_or(""),
            d["due_date"].as_str().map(|x| format!(" · due {}", fmt_long(x))).unwrap_or_default()), if credit { 0 } else { amt }, if credit { amt } else { 0 }));
    }
    for p in &pays { events.push((p["date"].as_str().unwrap_or("").into(), format!("Payment — {}", p["number"].as_str().unwrap_or("")), 0, p["amount"].as_i64().unwrap_or(0))); }
    events.sort_by(|a, b| a.0.cmp(&b.0));
    for (date, desc, dr, cr) in events {
        bal += dr - cr;
        rows += &format!("<tr><td>{}</td><td>{}</td><td class='n'>{}</td><td class='n'>{}</td><td class='n'>{}</td></tr>", fmt_long(&date), esc(&desc),
            if dr != 0 { group(&fmt_money(dr)) } else { String::new() }, if cr != 0 { group(&fmt_money(cr)) } else { String::new() }, group(&fmt_money(bal)));
    }
    Ok(format!(r#"<!doctype html><html><head><meta charset="utf-8"><title>Statement {name}</title><style>{STYLE}</style></head><body>
<div class="bar"><button onclick="window.print()">Print / Save as PDF</button></div>
<div class="page"><div class="top"><div><h1>STATEMENT</h1><div>{name}</div><div class="muted">{addr}</div></div><div class="meta"><b>Period</b><span>{p}</span><b>Type</b><span>{k}</span></div>{org}</div>
<table><thead><tr><th>Date</th><th>Activity</th><th class="n">Invoiced</th><th class="n">Paid / Credited</th><th class="n">Balance</th></tr></thead><tbody>{rows}</tbody></table>
<div class="totals"><div class="grand"><span>BALANCE DUE</span><span>{bal}</span></div></div></div></body></html>"#,
        name = esc(c["name"].as_str().unwrap_or("")), addr = nl(c["postal_address"].as_str().unwrap_or("")), p = if kind == "ACTIVITY" { format!("{} – {}", fmt_long(from), fmt_long(to)) } else { format!("As at {}", fmt_long(to)) },
        k = if kind == "ACTIVITY" { "Activity" } else { "Outstanding" }, org = org_block(l), bal = group(&fmt_money(bal))))
}

pub fn view(st: &AppState, _token: Option<&str>, kind: &str, id: i64) -> Result<String, String> {
    let guard = st.org.lock().unwrap();
    let org = guard.as_ref().ok_or("no organisation open")?;
    let l = &org.ledger;
    match kind {
        "doc" => document_html(l, id, false),
        "statement" => {
            let today = util::today();
            statement_html(l, id, &l.fy_start(&today).map_err(|e| e.to_string())?, &today, "OUTSTANDING")
        }
        "statement-activity" => {
            let today = util::today();
            statement_html(l, id, &l.fy_start(&today).map_err(|e| e.to_string())?, &today, "ACTIVITY")
        }
        _ => ledger_core::modules::render_view(l, kind, id).map_err(|e| e.to_string()),
    }
}

pub fn online(st: &AppState, token: &str) -> Result<String, String> {
    let guard = st.org.lock().unwrap();
    let org = guard.as_ref().ok_or("no organisation open")?;
    let id = org.ledger.document_by_token(token).map_err(|e| e.to_string())?;
    document_html(&org.ledger, id, true)
}

/// Downloads: outbox .eml, org backup, CSV exports.
pub fn download(st: &AppState, kind: &str, q: &HashMap<String, String>) -> Result<(String, String, Vec<u8>), String> {
    let guard = st.org.lock().unwrap();
    let org = guard.as_ref().ok_or("no organisation open")?;
    let l = &org.ledger;
    match kind {
        "eml" => {
            let id: i64 = q.get("id").and_then(|x| x.parse().ok()).ok_or("missing id")?;
            Ok(("message/rfc822".into(), format!("message-{id}.eml"), ledger_core::email::eml(l, id).map_err(|e| e.to_string())?.into_bytes()))
        }
        "backup" => {
            let tmp = std::env::temp_dir().join(format!("ledgercraft-backup-{}.lcdb", util::random_token(8)));
            l.conn.execute("VACUUM INTO ?1", [tmp.display().to_string()]).map_err(|e| e.to_string())?;
            let bytes = std::fs::read(&tmp).map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(&tmp);
            let name = l.setting_or("org_name", "organisation").unwrap_or_default().replace(|c: char| !c.is_alphanumeric(), "-");
            Ok(("application/octet-stream".into(), format!("{name}-{}.lcdb", util::today()), bytes))
        }
        _ => ledger_core::modules::download(l, kind, q).map_err(|e| e.to_string()),
    }
}

//! Email templates with merge fields, and the outbox. Messages are queued in `outbox`;
//! the server's mailer sends them when SMTP is configured, otherwise they can be
//! downloaded as .eml files and sent from any mail client.

use crate::{money, util, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};

pub fn merge(template: &str, fields: &[(&str, String)]) -> String {
    let mut s = template.to_string();
    for (k, v) in fields { s = s.replace(&format!("[{k}]"), v); }
    s
}

fn doc_fields(l: &Ledger, d: &Value) -> Result<Vec<(&'static str, String)>> {
    let org = l.setting_or("org_name", "")?;
    let first = l.conn.query_row("SELECT COALESCE(NULLIF(first_name,''), name) FROM contacts WHERE id = ?1", [d["contact_id"].as_i64().unwrap_or(0)], |r| r.get::<_, String>(0)).unwrap_or_default();
    let num = d["number"].as_str().unwrap_or("").to_string();
    let link = format!("{}online/{}", l.setting_or("public_url", "http://127.0.0.1:4180/")?, d["online_token"].as_str().unwrap_or(""));
    Ok(vec![
        ("Trading Name", org.clone()), ("Contact Name", d["contact_name"].as_str().unwrap_or("").into()), ("Contact First Name", first),
        ("Invoice Number", num.clone()), ("Quote Number", num.clone()), ("Purchase Order Number", num.clone()), ("Credit Note Number", num.clone()),
        ("Currency Code", d["currency"].as_str().unwrap_or("").into()),
        ("Invoice Total", money::fmt_money_grouped(d["total"].as_i64().unwrap_or(0))), ("Quote Total", money::fmt_money_grouped(d["total"].as_i64().unwrap_or(0))),
        ("Credit Note Total", money::fmt_money_grouped(d["total"].as_i64().unwrap_or(0))), ("Amount Due", money::fmt_money_grouped(d["amount_due"].as_i64().unwrap_or(0))),
        ("Due Date", d["due_date"].as_str().map(crate::reports::fmt_long).unwrap_or_default()), ("Online Invoice Link", link),
        ("Reference", d["reference"].as_str().unwrap_or("").into()),
    ])
}

fn template_kind(doc_type: &str) -> &'static str {
    match doc_type { "QUOTE" => "QUOTE", "PO" => "PO", "ACCRECCREDIT" | "ACCPAYCREDIT" => "CREDIT", _ => "INVOICE" }
}

pub fn preview_document_email(l: &Ledger, id: i64) -> Result<Value> {
    let d = l.get_document(id)?;
    let (subject, body): (String, String) = l.conn.query_row("SELECT subject, body FROM email_templates WHERE kind = ?1 ORDER BY is_default DESC LIMIT 1",
        [template_kind(d["type"].as_str().unwrap_or(""))], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let f = doc_fields(l, &d)?;
    let mut to = d["contact_email"].as_str().unwrap_or("").to_string();
    let people: Vec<String> = util::query_json(&l.conn, "SELECT email FROM contact_people WHERE contact_id = ?1 AND include_in_emails = 1 AND email <> ''", [d["contact_id"].as_i64().unwrap_or(0)])?
        .into_iter().filter_map(|p| p["email"].as_str().map(String::from)).collect();
    for p in people { if !to.is_empty() { to.push_str(", "); } to.push_str(&p); }
    Ok(json!({"to": to, "subject": merge(&subject, &f), "body": merge(&body, &f)}))
}

/// Queue an email for a document and mark it as sent.
pub fn email_document(l: &Ledger, id: i64, to: Option<String>, subject: Option<String>, body: Option<String>) -> Result<Value> {
    let pv = preview_document_email(l, id)?;
    let to = to.unwrap_or_else(|| pv["to"].as_str().unwrap_or("").to_string());
    if to.trim().is_empty() { return crate::error::invalid("the contact has no email address"); }
    let subject = subject.unwrap_or_else(|| pv["subject"].as_str().unwrap_or("").to_string());
    let body = body.unwrap_or_else(|| pv["body"].as_str().unwrap_or("").to_string());
    let configured = !l.setting_or("smtp_host", "")?.is_empty();
    l.conn.execute("INSERT INTO outbox(to_addr, subject, body, entity, entity_id, status) VALUES (?1, ?2, ?3, 'document', ?4, ?5)",
        params![to, subject, body, id, if configured { "QUEUED" } else { "SAVED" }])?;
    let oid = l.conn.last_insert_rowid();
    l.mark_sent(id)?;
    l.add_note("document", id, "HISTORY", &format!("Emailed to {to}{}", if configured { "" } else { " (saved to outbox — SMTP not configured)" }))?;
    Ok(json!({"outbox_id": oid, "queued": configured}))
}

/// Remittance advice: tell suppliers which bills a payment (or a batch of payments) settled.
/// One email per supplier. Returns how many were queued and which suppliers have no email address.
pub fn email_remittance(l: &Ledger, payment_ids: &[i64]) -> Result<Value> {
    use std::collections::BTreeMap;
    let org = l.setting_or("org_name", "")?;
    let mut by_contact: BTreeMap<i64, Vec<Value>> = BTreeMap::new();
    for pid in payment_ids {
        let p = util::query_one_json(&l.conn,
            "SELECT p.id, p.date, p.amount, p.reference, p.document_id, d.number, d.reference AS doc_reference, d.currency, d.contact_id, d.type
             FROM payments p JOIN documents d ON d.id = p.document_id WHERE p.id = ?1 AND p.status <> 'DELETED'", [pid])?;
        if p.is_null() || p["type"] != "ACCPAY" { continue; }
        by_contact.entry(p["contact_id"].as_i64().unwrap_or(0)).or_default().push(p);
    }
    let (mut queued, mut missing) = (0, Vec::new());
    for (cid, pays) in by_contact {
        let (name, first, email): (String, String, String) = l.conn.query_row(
            "SELECT name, COALESCE(NULLIF(first_name,''), name), COALESCE(email,'') FROM contacts WHERE id = ?1", [cid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        if email.trim().is_empty() { missing.push(name); continue; }
        let total: i64 = pays.iter().map(|p| p["amount"].as_i64().unwrap_or(0)).sum();
        let cur = pays[0]["currency"].as_str().unwrap_or("").to_string();
        let date = pays[0]["date"].as_str().map(crate::reports::fmt_long).unwrap_or_default();
        let mut lines = String::new();
        for p in &pays {
            let refs = [p["number"].as_str().unwrap_or(""), p["doc_reference"].as_str().unwrap_or("")].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" / ");
            lines.push_str(&format!("  {:<32} {:>14}\n", if refs.is_empty() { "Bill".to_string() } else { refs }, money::fmt_money_grouped(p["amount"].as_i64().unwrap_or(0))));
        }
        let subject = format!("Remittance advice from {org}");
        let reference = pays[0]["reference"].as_str().filter(|s| !s.is_empty()).unwrap_or("—");
        let body = format!(
            "Hi {first},\n\nThis is to let you know that {org} has paid {cur} {} on {date} for:\n\n{lines}\n  {:<32} {:>14}\n\nPayment reference: {reference}\n\nRegards,\n{org}\n",
            money::fmt_money_grouped(total), "Total paid", money::fmt_money_grouped(total));
        queue_email(l, &email, &subject, &body, "contact", cid)?;
        for p in &pays { l.add_note("document", p["document_id"].as_i64().unwrap_or(0), "HISTORY", &format!("Remittance advice emailed to {email}"))?; }
        queued += 1;
    }
    Ok(json!({"queued": queued, "missing_email": missing}))
}

pub fn queue_email(l: &Ledger, to: &str, subject: &str, body: &str, entity: &str, entity_id: i64) -> Result<i64> {
    let configured = !l.setting_or("smtp_host", "")?.is_empty();
    l.conn.execute("INSERT INTO outbox(to_addr, subject, body, entity, entity_id, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![to, subject, body, entity, entity_id, if configured { "QUEUED" } else { "SAVED" }])?;
    Ok(l.conn.last_insert_rowid())
}

pub fn outbox(l: &Ledger) -> Result<Vec<Value>> {
    util::query_json(&l.conn, "SELECT * FROM outbox ORDER BY id DESC LIMIT 300", [])
}

/// RFC 5322 message for download (.eml).
pub fn eml(l: &Ledger, id: i64) -> Result<String> {
    let (to, subject, body): (String, String, String) = l.conn.query_row("SELECT to_addr, subject, body FROM outbox WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    let from = l.setting_or("smtp_from", "")?;
    let from = if from.is_empty() { l.setting_or("email", "noreply@localhost")? } else { from };
    Ok(format!("From: {from}\r\nTo: {to}\r\nSubject: {subject}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\nX-Unsent: 1\r\n\r\n{}", body.replace('\n', "\r\n")))
}

/// Overdue reminder run: queue reminders for invoices matching active reminder rules.
pub fn run_reminders(l: &Ledger) -> Result<i64> {
    if l.setting_or("invoice_reminders_enabled", "0")? != "1" { return Ok(0); }
    let today = util::today();
    let rules: Vec<(i64, i64, String, String)> = {
        let mut st = l.conn.prepare("SELECT r.id, r.days, t.subject, t.body FROM reminder_rules r JOIN email_templates t ON t.id = r.template_id WHERE r.active = 1")?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut n = 0;
    for (rid, days, subject, body) in rules {
        let cutoff = util::add_days(&today, -days)?;
        let docs: Vec<i64> = {
            let mut st = l.conn.prepare("SELECT d.id FROM documents d JOIN contacts c ON c.id = d.contact_id WHERE d.type = 'ACCREC' AND d.status = 'AUTHORISED' AND d.due_date <= ?1 AND c.email <> ''
                AND NOT EXISTS (SELECT 1 FROM reminders_sent s WHERE s.document_id = d.id AND s.rule_id = ?2)")?;
            st.query_map(params![cutoff, rid], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for d in docs {
            let doc = l.get_document(d)?;
            let f = doc_fields(l, &doc)?;
            queue_email(l, doc["contact_email"].as_str().unwrap_or(""), &merge(&subject, &f), &merge(&body, &f), "document", d)?;
            l.conn.execute("INSERT INTO reminders_sent(document_id, rule_id, sent_at) VALUES (?1, ?2, datetime('now'))", params![d, rid])?;
            l.add_note("document", d, "HISTORY", "Payment reminder queued")?;
            n += 1;
        }
    }
    Ok(n)
}

/// Queue statement emails for contacts that owe money. Returns number queued.
pub fn email_statements(l: &Ledger, contact_ids: &[i64]) -> Result<i64> {
    let (subject, body): (String, String) = l.conn.query_row("SELECT subject, body FROM email_templates WHERE kind = 'STATEMENT' ORDER BY is_default DESC LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let org = l.setting_or("org_name", "")?;
    let mut n = 0;
    for cid in contact_ids {
        let c = l.get_contact(*cid)?;
        let to = c["email"].as_str().unwrap_or("").to_string();
        if to.is_empty() { continue; }
        let docs = util::query_json(&l.conn, "SELECT number, date, due_date, total - amount_paid - amount_credited AS due, currency FROM documents WHERE contact_id = ?1 AND type = 'ACCREC' AND status = 'AUTHORISED' ORDER BY date", [cid])?;
        if docs.is_empty() { continue; }
        let total: i64 = docs.iter().map(|d| d["due"].as_i64().unwrap_or(0)).sum();
        let lines: Vec<String> = docs.iter().map(|d| format!("  {}  {}  due {}  {}", d["number"].as_str().unwrap_or(""), d["date"].as_str().unwrap_or(""), d["due_date"].as_str().unwrap_or(""), money::fmt_money_grouped(d["due"].as_i64().unwrap_or(0)))).collect();
        let first = c["first_name"].as_str().filter(|s| !s.is_empty()).or(c["name"].as_str()).unwrap_or("").to_string();
        let f = vec![("Trading Name", org.clone()), ("Contact Name", c["name"].as_str().unwrap_or("").to_string()), ("Contact First Name", first),
            ("Amount Due", money::fmt_money_grouped(total)), ("Currency Code", docs[0]["currency"].as_str().unwrap_or("").to_string())];
        let text = merge(&body, &f) + "\n\nOutstanding invoices:\n" + &lines.join("\n");
        queue_email(l, &to, &merge(&subject, &f), &text, "contact", *cid)?;
        l.add_note("contact", *cid, "HISTORY", "Statement emailed")?;
        n += 1;
    }
    Ok(n)
}

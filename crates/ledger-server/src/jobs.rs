//! Background jobs: repeating transactions, payment reminders, outgoing email (SMTP),
//! webhook delivery, and exchange-rate fetching. Network I/O happens without holding the
//! organisation lock.

use crate::state::AppState;
use ledger_core::{rusqlite::params, util, Ledger};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

pub fn start(st: Arc<AppState>) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(5));
        loop {
            run_once(&st);
            std::thread::sleep(Duration::from_secs(300));
        }
    });
}

pub fn run_once(st: &AppState) -> Value {
    let mut out = json!({});
    // 1. scheduled work inside the ledger
    {
        let guard = st.org.lock().unwrap();
        if let Some(org) = guard.as_ref() {
            let l = &org.ledger;
            if l.is_setup().unwrap_or(false) {
                out["repeating"] = json!(l.run_repeating(&util::today()).map(|v| v.len()).unwrap_or(0));
                out["reminders"] = json!(ledger_core::email::run_reminders(l).unwrap_or(0));
                let _ = l.sync_receipts();
            }
        }
    }
    out["emails"] = json!(send_queued_email(st));
    out["webhooks"] = json!(deliver_webhooks(st));
    out
}

struct Smtp { host: String, port: u16, user: String, pass: String, from: String, tls: String }

fn smtp_settings(l: &Ledger) -> Option<Smtp> {
    let host = l.setting("smtp_host").ok().flatten().filter(|h| !h.trim().is_empty())?;
    Some(Smtp {
        port: l.setting_or("smtp_port", "587").ok()?.parse().unwrap_or(587), user: l.setting_or("smtp_user", "").ok()?, pass: l.setting_or("smtp_pass", "").ok()?,
        from: { let f = l.setting_or("smtp_from", "").ok()?; if f.is_empty() { l.setting_or("email", "").ok()? } else { f } },
        tls: l.setting_or("smtp_tls", "STARTTLS").ok()?, host,
    })
}

/// An attachment: (file name, HTML content).
type Attachment = Option<(String, String)>;

fn send_one(cfg: &Smtp, to: &str, subject: &str, body: &str, attachment: &Attachment) -> Result<(), String> {
    use lettre::{message::{header::ContentType, Attachment as MailAttachment, Mailbox, MultiPart, SinglePart}, transport::smtp::authentication::Credentials, Message, SmtpTransport, Transport};
    let from: Mailbox = cfg.from.parse().map_err(|e| format!("invalid from address: {e}"))?;
    let mut b = Message::builder().from(from).subject(subject);
    for addr in to.split([',', ';']).map(str::trim).filter(|a| !a.is_empty()) {
        b = b.to(addr.parse().map_err(|e| format!("invalid address {addr}: {e}"))?);
    }
    let msg = match attachment {
        // The printable document goes along as a self-contained HTML file (open it and print to PDF).
        Some((name, html)) => b.multipart(MultiPart::mixed()
            .singlepart(SinglePart::plain(body.to_string()))
            .singlepart(MailAttachment::new(name.clone()).body(html.clone(), ContentType::TEXT_HTML))),
        None => b.header(ContentType::TEXT_PLAIN).body(body.to_string()),
    }.map_err(|e| e.to_string())?;
    let builder = match cfg.tls.as_str() {
        "TLS" => SmtpTransport::relay(&cfg.host).map_err(|e| e.to_string())?,
        "NONE" => SmtpTransport::builder_dangerous(&cfg.host),
        _ => SmtpTransport::starttls_relay(&cfg.host).map_err(|e| e.to_string())?,
    };
    let mut builder = builder.port(cfg.port).timeout(Some(Duration::from_secs(20)));
    if !cfg.user.is_empty() { builder = builder.credentials(Credentials::new(cfg.user.clone(), cfg.pass.clone())); }
    builder.build().send(&msg).map(|_| ()).map_err(|e| e.to_string())
}

/// Send QUEUED outbox messages. Returns number sent.
pub fn send_queued_email(st: &AppState) -> i64 {
    let (cfg, queued) = {
        let guard = st.org.lock().unwrap();
        let Some(org) = guard.as_ref() else { return 0 };
        let Some(cfg) = smtp_settings(&org.ledger) else { return 0 };
        let q = util::query_json(&org.ledger.conn, "SELECT id, to_addr, subject, body, entity, entity_id FROM outbox WHERE status = 'QUEUED' ORDER BY id LIMIT 50", []).unwrap_or_default();
        // Render document attachments while we hold the lock; sending happens without it.
        let q: Vec<(Value, Attachment)> = q.into_iter().map(|m| {
            let att = match (m["entity"].as_str(), m["entity_id"].as_i64()) {
                (Some("document"), Some(id)) => crate::render::document_html(&org.ledger, id, false).ok().map(|html| {
                    let number = org.ledger.get_document(id).ok().and_then(|d| d["number"].as_str().map(str::to_string)).filter(|n| !n.is_empty()).unwrap_or_else(|| format!("document-{id}"));
                    (format!("{}.html", number.replace(|c: char| "/\\:*?\"<>|".contains(c), "-")), html)
                }),
                _ => None,
            };
            (m, att)
        }).collect();
        (cfg, q)
    };
    let mut results = Vec::new();
    for (m, att) in &queued {
        let r = send_one(&cfg, m["to_addr"].as_str().unwrap_or(""), m["subject"].as_str().unwrap_or(""), m["body"].as_str().unwrap_or(""), att);
        results.push((m["id"].as_i64().unwrap_or(0), r));
    }
    let guard = st.org.lock().unwrap();
    let Some(org) = guard.as_ref() else { return 0 };
    let mut sent = 0;
    for (id, r) in results {
        match r {
            Ok(()) => { sent += 1; let _ = org.ledger.conn.execute("UPDATE outbox SET status = 'SENT', sent_at = datetime('now'), error = NULL WHERE id = ?1", [id]); }
            Err(e) => { let _ = org.ledger.conn.execute("UPDATE outbox SET status = 'FAILED', error = ?2 WHERE id = ?1", params![id, e]); }
        }
    }
    sent
}

pub fn test_email(st: &AppState, to: &str) -> Result<(), String> {
    let cfg = { let g = st.org.lock().unwrap(); let org = g.as_ref().ok_or("no organisation open")?; smtp_settings(&org.ledger).ok_or("SMTP is not configured")? };
    send_one(&cfg, to, "LedgerCraft test email", "Your SMTP settings work. Emails from LedgerCraft will be sent automatically.", &None)
}

/// POST queued webhook events with an HMAC-SHA256 signature header.
pub fn deliver_webhooks(st: &AppState) -> i64 {
    let pending = {
        let guard = st.org.lock().unwrap();
        let Some(org) = guard.as_ref() else { return 0 };
        util::query_json(&org.ledger.conn, "SELECT d.id, d.payload, w.url, w.secret FROM webhook_deliveries d JOIN webhooks w ON w.id = d.webhook_id WHERE d.status = 'PENDING' AND d.attempts < 5 ORDER BY d.id LIMIT 50", []).unwrap_or_default()
    };
    let mut results = Vec::new();
    for d in &pending {
        let payload = d["payload"].as_str().unwrap_or("{}");
        let sig = ledger_core::ext::hmac_sha256_hex(d["secret"].as_str().unwrap_or("").as_bytes(), payload.as_bytes());
        let r = ureq::post(d["url"].as_str().unwrap_or("")).header("content-type", "application/json").header("x-ledgercraft-signature", &sig)
            .config().timeout_global(Some(Duration::from_secs(10))).build().send(payload);
        results.push((d["id"].as_i64().unwrap_or(0), r.is_ok()));
    }
    let guard = st.org.lock().unwrap();
    let Some(org) = guard.as_ref() else { return 0 };
    let mut n = 0;
    for (id, ok) in results {
        if ok { n += 1; }
        let _ = org.ledger.conn.execute("UPDATE webhook_deliveries SET attempts = attempts + 1, status = CASE WHEN ?2 THEN 'DELIVERED' WHEN attempts + 1 >= 5 THEN 'FAILED' ELSE 'PENDING' END WHERE id = ?1", params![id, ok]);
    }
    n
}

/// Fetch the European Central Bank daily reference rates and store them for enabled currencies.
pub fn fetch_ecb_rates(st: &AppState) -> Result<Value, String> {
    let xml = ureq::get("https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml").config().timeout_global(Some(Duration::from_secs(15))).build()
        .call().map_err(|e| format!("couldn't reach the ECB: {e}"))?.body_mut().read_to_string().map_err(|e| e.to_string())?;
    let date = xml.split("time='").nth(1).and_then(|s| s.get(..10)).ok_or("unexpected ECB response")?.to_string();
    let mut eur: std::collections::HashMap<String, f64> = std::collections::HashMap::from([("EUR".to_string(), 1.0)]);
    for part in xml.split("currency='").skip(1) {
        let code = &part[..3];
        if let Some(r) = part.split("rate='").nth(1).and_then(|s| s.split('\'').next()).and_then(|s| s.parse::<f64>().ok()) { eur.insert(code.to_string(), r); }
    }
    let guard = st.org.lock().unwrap();
    let org = guard.as_ref().ok_or("no organisation open")?;
    let l = &org.ledger;
    let base = l.base_currency().map_err(|e| e.to_string())?;
    let base_per_eur = *eur.get(&base).ok_or(format!("the ECB doesn't publish a rate for {base}"))?;
    let enabled: Vec<String> = util::query_json(&l.conn, "SELECT code FROM currencies WHERE enabled = 1", []).unwrap_or_default().into_iter().filter_map(|c| c["code"].as_str().map(String::from)).collect();
    let mut updated = Vec::new();
    for c in enabled {
        if c == base { continue; }
        if let Some(r) = eur.get(&c) {
            let rate = r / base_per_eur; // foreign per 1 base
            let s = format!("{rate:.8}");
            if l.set_rate(&c, &date, &s, "ECB").is_ok() { updated.push(json!({"code": c, "rate": s})); }
        }
    }
    Ok(json!({"date": date, "updated": updated}))
}

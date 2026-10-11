//! Adviser tools: attachments, files inbox with bill capture (text extraction + optional OCR plugin),
//! accountant workpapers, assurance health checks.

use crate::{documents::{DocInput, LineInput}, error::invalid, money, statements::parse_flex_date, util, Error, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};

pub const MAX_ATTACHMENT: usize = 15 * 1024 * 1024;

/// Decode standard base64 (with or without data: URL prefix).
pub fn b64_decode(s: &str) -> Result<Vec<u8>> {
    let s = s.split_once(";base64,").map(|x| x.1).unwrap_or(s);
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32; let mut bits = 0;
    for c in s.bytes() {
        let v = match c { b'A'..=b'Z' => c - b'A', b'a'..=b'z' => c - b'a' + 26, b'0'..=b'9' => c - b'0' + 52, b'+' | b'-' => 62, b'/' | b'_' => 63, b'=' | b'\r' | b'\n' | b' ' => continue,
            _ => return Err(Error::Validation("invalid file data".into())) } as u32;
        buf = (buf << 6) | v; bits += 6;
        if bits >= 8 { bits -= 8; out.push((buf >> bits) as u8); buf &= (1 << bits) - 1; }
    }
    Ok(out)
}

/// Pull readable text out of a PDF (handles Flate-compressed content streams) or plain text.
pub fn extract_text(bytes: &[u8], mime: &str) -> String {
    if mime.starts_with("text/") { return String::from_utf8_lossy(bytes).to_string(); }
    if !(mime == "application/pdf" || bytes.starts_with(b"%PDF")) { return String::new(); }
    let mut chunks: Vec<Vec<u8>> = Vec::new();
    let mut i = 0;
    while let Some(p) = find(&bytes[i..], b"stream") {
        let mut start = i + p + 6;
        if bytes.get(start) == Some(&b'\r') { start += 1; }
        if bytes.get(start) == Some(&b'\n') { start += 1; }
        let Some(e) = find(&bytes[start..], b"endstream") else { break };
        let data = &bytes[start..start + e];
        match miniz_oxide::inflate::decompress_to_vec_zlib(data) { Ok(d) => chunks.push(d), Err(_) => chunks.push(data.to_vec()) }
        i = start + e + 9;
    }
    let mut text = String::new();
    for c in chunks {
        let s = String::from_utf8_lossy(&c);
        if !s.contains("Tj") && !s.contains("TJ") { continue; }
        // collect (...) string literals inside text objects
        let mut in_str = 0; let mut cur = String::new(); let mut esc = false;
        for ch in s.chars() {
            if in_str > 0 {
                if esc { cur.push(match ch { 'n' => '\n', 'r' => ' ', 't' => ' ', c => c }); esc = false; continue; }
                match ch { '\\' => esc = true, '(' => { in_str += 1; cur.push(ch) } ')' => { in_str -= 1; if in_str == 0 { text.push_str(&cur); cur.clear(); } else { cur.push(ch) } } c => cur.push(c) }
            } else if ch == '(' { in_str = 1; } else if ch == 'T' || ch == '\'' { } else if ch == '\n' { text.push('\n'); } else if ch == ']' { text.push(' '); }
        }
        text.push('\n');
    }
    text
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> { h.windows(n.len()).position(|w| w == n) }

/// Heuristic field extraction from bill/receipt text.
pub fn parse_bill_text(text: &str) -> Value {
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let supplier = lines.iter().find(|l| l.chars().filter(|c| c.is_alphabetic()).count() > 3 && !l.to_lowercase().contains("invoice")).map(|s| s.to_string()).unwrap_or_default();
    let amount_in = |l: &str| -> Option<i64> {
        l.split(|c: char| c.is_whitespace() || c == '$' || c == '£' || c == '€').filter_map(|t| { let t = t.trim_matches(|c: char| !c.is_ascii_digit() && c != '.' && c != ','); if t.contains('.') { money::parse_fixed(t, 2) } else { None } }).last()
    };
    let mut total = None; let mut tax = None; let mut date = None; let mut number = None; let mut abn = None;
    for l in &lines {
        let low = l.to_lowercase();
        if total.is_none() && (low.contains("total") || low.contains("amount due") || low.contains("balance due")) && !low.contains("subtotal") { total = amount_in(l); }
        if tax.is_none() && (low.contains("gst") || low.contains("vat") || low.contains("tax")) && !low.contains("total incl") { tax = amount_in(l).filter(|v| *v > 0); }
        if date.is_none() { for tok in l.split_whitespace().collect::<Vec<_>>().windows(1) { if let Some(d) = parse_flex_date(tok[0], "DMY") { if d.as_str() > "2000-01-01" { date = Some(d); break; } } }
            if date.is_none() { if let Some(d) = parse_flex_date(l, "DMY") { if d.as_str() > "2000-01-01" { date = Some(d); } } } }
        if number.is_none() && (low.contains("invoice no") || low.contains("invoice #") || low.contains("invoice number") || low.starts_with("inv")) {
            number = l.split(|c: char| c == ':' || c == '#').nth(1).map(|s| s.trim().split_whitespace().next().unwrap_or("").to_string()).filter(|s| !s.is_empty());
        }
        if abn.is_none() && (low.contains("abn") || low.contains("vat no") || low.contains("gst no")) { abn = Some(l.chars().filter(|c| c.is_ascii_digit()).collect::<String>()).filter(|s| s.len() >= 8); }
    }
    if total.is_none() { total = lines.iter().filter_map(|l| amount_in(l)).max(); }
    json!({"supplier": supplier, "total": total, "tax": tax, "date": date, "number": number, "tax_number": abn})
}

impl Ledger {
    pub fn add_attachment(&self, entity: &str, entity_id: Option<i64>, filename: &str, mime: &str, data_b64: &str) -> Result<i64> {
        let bytes = b64_decode(data_b64)?;
        if bytes.len() > MAX_ATTACHMENT { return invalid("files must be 15 MB or smaller"); }
        let name: String = filename.chars().filter(|c| !matches!(c, '/' | '\\' | '\0')).collect();
        self.conn.execute("INSERT INTO attachments(entity, entity_id, filename, mime, size, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![entity, entity_id, name, mime, bytes.len() as i64, bytes])?;
        let id = self.conn.last_insert_rowid();
        if let Some(eid) = entity_id { self.add_note(entity, eid, "HISTORY", &format!("Attached {name}"))?; }
        Ok(id)
    }

    pub fn attachment(&self, id: i64) -> Result<(String, String, Vec<u8>)> {
        Ok(self.conn.query_row("SELECT filename, mime, data FROM attachments WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?)
    }

    pub fn delete_attachment(&self, id: i64) -> Result<()> { self.conn.execute("DELETE FROM attachments WHERE id = ?1", [id])?; Ok(()) }

    pub fn list_attachments(&self, entity: &str, id: i64) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT id, filename, mime, size, created_at FROM attachments WHERE entity = ?1 AND entity_id = ?2", params![entity, id])
    }

    // ---------------- Files inbox ----------------
    pub fn inbox_upload(&self, filename: &str, mime: &str, data_b64: &str) -> Result<i64> {
        self.atomic(|| {
            let aid = self.add_attachment("inbox", None, filename, mime, data_b64)?;
            let (_, _, bytes) = self.attachment(aid)?;
            let mut text = extract_text(&bytes, mime);
            if text.trim().is_empty() && mime.starts_with("image/") { text = self.ocr_image(&bytes).unwrap_or_default(); }
            let fields = parse_bill_text(&text);
            let extracted = json!({"fields": fields, "text_preview": text.chars().take(1500).collect::<String>(), "method": if text.trim().is_empty() { "none" } else if mime.starts_with("image/") { "ocr" } else { "text" }});
            self.conn.execute("INSERT INTO file_inbox(attachment_id, extracted) VALUES (?1, ?2)", params![aid, extracted.to_string()])?;
            Ok(self.conn.last_insert_rowid())
        })
    }

    /// Optional OCR plugin: run the command in setting `ocr_command` (e.g. a Tesseract install) on an image.
    fn ocr_image(&self, bytes: &[u8]) -> Option<String> {
        let cmd = self.setting("ocr_command").ok().flatten().filter(|s| !s.trim().is_empty())?;
        let tmp = std::env::temp_dir().join(format!("lc-ocr-{}.png", util::random_token(8)));
        std::fs::write(&tmp, bytes).ok()?;
        let out = std::process::Command::new(cmd.trim()).arg(&tmp).arg("stdout").output().ok();
        let _ = std::fs::remove_file(&tmp);
        out.filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).to_string())
    }

    pub fn list_inbox(&self) -> Result<Vec<Value>> {
        let mut v = util::query_json(&self.conn, "SELECT f.*, a.filename, a.mime, a.size FROM file_inbox f JOIN attachments a ON a.id = f.attachment_id WHERE f.status <> 'ARCHIVED' ORDER BY f.id DESC", [])?;
        for f in v.iter_mut() { f["extracted"] = serde_json::from_str(f["extracted"].as_str().unwrap_or("{}")).unwrap_or(json!({})); }
        Ok(v)
    }

    /// Create a draft bill from an inbox file using the extracted fields (user reviews before approving).
    pub fn inbox_to_bill(&self, id: i64, overrides: &Value) -> Result<i64> {
        let (aid, extracted): (i64, String) = self.conn.query_row("SELECT attachment_id, extracted FROM file_inbox WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let ex: Value = serde_json::from_str(&extracted)?;
        let f = &ex["fields"];
        let pick = |k: &str| overrides[k].as_str().filter(|s| !s.is_empty()).map(String::from).or_else(|| f[k].as_str().filter(|s| !s.is_empty()).map(String::from));
        let supplier = pick("supplier").unwrap_or_else(|| "Unknown supplier".into());
        let date = pick("date").unwrap_or_else(util::today);
        let total = overrides["total"].as_str().and_then(|s| money::parse_fixed(s, 2)).or(f["total"].as_i64()).unwrap_or(0);
        self.atomic(|| {
            let cid = self.find_or_create_contact(&supplier)?;
            let acct: Option<i64> = self.conn.query_row("SELECT purchase_account_id FROM contacts WHERE id = ?1", [cid], |r| r.get(0))?;
            let did = self.save_document(None, &DocInput { kind: "ACCPAY".into(), contact_id: Some(cid), number: pick("number"), date, line_amount_type: Some("INCLUSIVE".into()),
                lines: vec![LineInput { description: format!("Bill from {supplier}"), quantity: Some("1".into()), unit_price: Some(money::fmt_money(total)), account_id: overrides["account_id"].as_i64().or(acct), ..Default::default() }],
                ..Default::default() })?;
            self.conn.execute("UPDATE attachments SET entity = 'document', entity_id = ?2 WHERE id = ?1", params![aid, did])?;
            self.conn.execute("UPDATE file_inbox SET status = 'PROCESSED', document_id = ?2 WHERE id = ?1", params![id, did])?;
            self.add_note("document", did, "HISTORY", "Created from Files inbox")?;
            Ok(did)
        })
    }

    pub fn archive_inbox(&self, id: i64) -> Result<()> { self.conn.execute("UPDATE file_inbox SET status = 'ARCHIVED' WHERE id = ?1", [id])?; Ok(()) }

    // ---------------- Workpapers ----------------
    pub fn workpapers(&self, period_end: &str) -> Result<Vec<Value>> {
        util::query_json(&self.conn,
            "SELECT a.id AS account_id, a.code, a.name, a.type,
               COALESCE((SELECT SUM(jl.amount) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id WHERE jl.account_id = a.id AND j.posted = 1 AND j.date <= ?1), 0) AS balance,
               w.note, w.reviewed, w.reviewed_at, u.name AS reviewed_by,
               (SELECT COUNT(*) FROM attachments x WHERE x.entity = 'workpaper' AND x.entity_id = a.id) AS files
             FROM accounts a LEFT JOIN workpapers w ON w.account_id = a.id AND w.period_end = ?1 LEFT JOIN users u ON u.id = w.reviewed_by
             WHERE a.class IN ('ASSET','LIABILITY','EQUITY') AND a.status = 'ACTIVE' ORDER BY a.code", [period_end])
    }

    pub fn save_workpaper(&self, account_id: i64, period_end: &str, note: &str, reviewed: bool) -> Result<()> {
        self.conn.execute("INSERT INTO workpapers(account_id, period_end, note, reviewed, reviewed_by, reviewed_at) VALUES (?1, ?2, ?3, ?4, CASE WHEN ?4 = 1 THEN ?5 END, CASE WHEN ?4 = 1 THEN datetime('now') END)
            ON CONFLICT(account_id, period_end) DO UPDATE SET note = excluded.note, reviewed = excluded.reviewed, reviewed_by = excluded.reviewed_by, reviewed_at = excluded.reviewed_at",
            params![account_id, period_end, note, reviewed as i64, self.user_id])?;
        Ok(())
    }

    // ---------------- Assurance ----------------
    pub fn assurance_health(&self) -> Result<Value> {
        let journals: i64 = self.conn.query_row("SELECT COUNT(*) FROM journals WHERE posted = 1", [], |r| r.get(0))?;
        let edits: i64 = self.conn.query_row("SELECT COUNT(*) FROM journals WHERE reverses_journal_id IS NOT NULL AND created_at >= datetime('now','-30 days')", [], |r| r.get(0))?;
        let stale: i64 = self.conn.query_row("SELECT COUNT(*) FROM statement_lines WHERE status = 'UNRECONCILED' AND date < date('now','-30 days')", [], |r| r.get(0))?;
        let mut flags = Vec::new();
        let late: i64 = self.conn.query_row("SELECT COUNT(*) FROM journals WHERE posted = 1 AND date < date(created_at, '-90 days') AND created_at >= datetime('now','-30 days')", [], |r| r.get(0))?;
        if late > 0 { flags.push(format!("{late} transaction(s) entered in the last 30 days are dated more than 90 days in the past")); }
        let bank_edits: i64 = self.conn.query_row("SELECT COUNT(*) FROM audit_log WHERE entity = 'contact' AND action = 'UPDATE' AND after LIKE '%bank%' AND at >= datetime('now','-30 days')", [], |r| r.get(0))?;
        if bank_edits > 0 { flags.push(format!("{bank_edits} contact change(s) in the last 30 days — check supplier bank details before paying")); }
        let lock = self.setting("lock_date_others")?.unwrap_or_default();
        if lock.is_empty() { flags.push("No lock date is set. Lock periods you've already reported on (Settings → Financial settings).".into()); }
        let neg_bank: i64 = self.conn.query_row("SELECT COUNT(*) FROM (SELECT jl.account_id, SUM(jl.amount) s FROM journal_lines jl JOIN accounts a ON a.id = jl.account_id JOIN journals j ON j.id = jl.journal_id WHERE a.type = 'BANK' AND a.bank_type = 'BANK' AND j.posted = 1 GROUP BY jl.account_id HAVING s < 0)", [], |r| r.get(0))?;
        if neg_bank > 0 { flags.push(format!("{neg_bank} bank account(s) have a negative balance in the ledger")); }
        let dup: i64 = self.conn.query_row("SELECT COUNT(*) FROM (SELECT contact_id, number FROM documents WHERE type = 'ACCPAY' AND number <> '' AND status <> 'DELETED' GROUP BY contact_id, number HAVING COUNT(*) > 1)", [], |r| r.get(0))?;
        if dup > 0 { flags.push(format!("{dup} possible duplicate bill(s) (same supplier and number)")); }
        Ok(json!({"imbalance": self.ledger_imbalance()?, "journals": journals, "edits_30d": edits, "stale_unreconciled": stale, "flags": flags}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn b64() { assert_eq!(b64_decode("aGVsbG8=").unwrap(), b"hello"); assert_eq!(b64_decode("data:text/plain;base64,aGk=").unwrap(), b"hi"); }
    #[test]
    fn bill_fields() {
        let t = "Officeworks Pty Ltd\nABN 36 004 763 526\nTax Invoice\nInvoice No: OW-12345\nDate 12/03/2026\nPaper  20.00\nGST 2.00\nTotal $22.00\n";
        let f = parse_bill_text(t);
        assert_eq!(f["supplier"], "Officeworks Pty Ltd");
        assert_eq!(f["total"], 2200);
        assert_eq!(f["tax"], 200);
        assert_eq!(f["date"], "2026-03-12");
        assert_eq!(f["number"], "OW-12345");
    }
}

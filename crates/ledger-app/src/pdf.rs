//! Minimal PDF writer (PDF 1.4, standard Helvetica fonts, WinAnsi text) and document layouts.
//! No external files or fonts are needed; output opens in any PDF viewer and prints cleanly.

use serde_json::Value;

/// Helvetica advance widths (1/1000 em) for ASCII 32..=126.
const HELV: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556,
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556,
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

/// Width of `s` in points at `size` (bold text is a little wider).
pub fn width(s: &str, size: f32, bold: bool) -> f32 {
    // Helvetica-Bold letters are a little wider; its digits and punctuation match the regular face.
    let units: f32 = s.chars().map(|c| {
        let i = c as u32;
        let w = if (32..=126).contains(&i) { HELV[(i - 32) as usize] as f32 } else { 556.0 };
        if bold && c.is_alphabetic() { w * 1.08 } else { w }
    }).sum();
    units / 1000.0 * size
}

/// Escape text for a PDF string; characters outside WinAnsi become '?'.
fn esc(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '(' | ')' | '\\' => { o.push('\\'); o.push(c); }
            '–' | '—' => o.push('-'),
            '‘' | '’' => o.push('\''),
            '“' | '”' => o.push('"'),
            '…' => o.push_str("..."),
            c if (c as u32) >= 32 && (c as u32) <= 126 => o.push(c),
            c if (c as u32) >= 160 && (c as u32) <= 255 => o.push_str(&format!("\\{:03o}", c as u32)),
            _ => o.push('?'),
        }
    }
    o
}

pub struct Pdf { pub w: f32, pub h: f32, pages: Vec<String>, cur: String }

impl Pdf {
    pub fn new(letter: bool) -> Pdf { let (w, h) = if letter { (612.0, 792.0) } else { (595.28, 841.89) }; Pdf { w, h, pages: vec![], cur: String::new() } }
    pub fn new_page(&mut self) { if !self.cur.is_empty() { self.pages.push(std::mem::take(&mut self.cur)); } }
    /// Text with its baseline at `y` (measured from the top of the page).
    pub fn text(&mut self, x: f32, y: f32, size: f32, bold: bool, rgb: (f32, f32, f32), s: &str) {
        self.cur += &format!("BT /{} {size:.1} Tf {:.3} {:.3} {:.3} rg {x:.2} {:.2} Td ({}) Tj ET\n", if bold { "F2" } else { "F1" }, rgb.0, rgb.1, rgb.2, self.h - y, esc(s));
    }
    pub fn text_right(&mut self, right: f32, y: f32, size: f32, bold: bool, rgb: (f32, f32, f32), s: &str) {
        let x = right - width(s, size, bold);
        self.text(x, y, size, bold, rgb, s);
    }
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, rgb: (f32, f32, f32)) {
        self.cur += &format!("{:.3} {:.3} {:.3} rg {x:.2} {:.2} {w:.2} {h:.2} re f\n", rgb.0, rgb.1, rgb.2, self.h - y - h);
    }
    pub fn line(&mut self, x1: f32, y: f32, x2: f32, w: f32, rgb: (f32, f32, f32)) {
        self.cur += &format!("{:.3} {:.3} {:.3} RG {w:.2} w {x1:.2} {:.2} m {x2:.2} {:.2} l S\n", rgb.0, rgb.1, rgb.2, self.h - y, self.h - y);
    }

    /// Serialise the document.
    pub fn finish(mut self) -> Vec<u8> {
        self.new_page();
        if self.pages.is_empty() { self.pages.push(String::new()); }
        let n = self.pages.len();
        // Objects: 1 catalog, 2 pages, 3 F1, 4 F2, then per page: page, content.
        let mut objs: Vec<String> = vec![];
        objs.push("<< /Type /Catalog /Pages 2 0 R >>".into());
        let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 5 + i * 2)).collect();
        objs.push(format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")));
        objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".into());
        objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>".into());
        for (i, content) in self.pages.iter().enumerate() {
            objs.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>", self.w, self.h, 6 + i * 2));
            objs.push(format!("<< /Length {} >>\nstream\n{}endstream", content.len(), content));
        }
        let mut out: Vec<u8> = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let mut offsets = vec![];
        for (i, o) in objs.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
        }
        let xref = out.len();
        let mut x = format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1);
        for off in offsets { x += &format!("{off:010} 00000 n \n"); }
        x += &format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1);
        out.extend_from_slice(x.as_bytes());
        out
    }
}

/// Word-wrap `s` to fit `max` points.
fn wrap(s: &str, size: f32, max: f32) -> Vec<String> {
    let mut lines = vec![];
    for para in s.split('\n') {
        let mut cur = String::new();
        for word in para.split_whitespace() {
            let cand = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            if width(&cand, size, false) > max && !cur.is_empty() { lines.push(std::mem::take(&mut cur)); cur = word.to_string(); } else { cur = cand; }
        }
        lines.push(cur);
    }
    lines
}

fn money(v: i64) -> String { crate::ui::money(v) }
fn i(v: &Value) -> i64 { v.as_i64().unwrap_or(0) }
fn st<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }

const INK: (f32, f32, f32) = (0.13, 0.14, 0.16);
const MUTED: (f32, f32, f32) = (0.42, 0.44, 0.48);
const RULE: (f32, f32, f32) = (0.82, 0.84, 0.87);
const BAND: (f32, f32, f32) = (0.95, 0.96, 0.97);

/// Apply a form template (Lists ▸ Templates): the document's own template, else the default.
/// Sets the printed title and colour, adds the template's terms and payment instructions, and
/// hides the tax number when the template says so.
pub fn apply_template(doc: &mut Value, settings: &mut Value, themes: &Value, title: &mut String, accent: &mut (f32, f32, f32)) {
    let list = themes.as_array().cloned().unwrap_or_default();
    let Some(t) = list.iter().find(|t| doc["branding_theme_id"].as_i64().is_some() && t["id"] == doc["branding_theme_id"])
        .or_else(|| list.iter().find(|t| t["is_default"].as_i64() == Some(1))).or(list.first()) else { return };
    let field = match doc["type"].as_str().unwrap_or("") { "ACCREC" => "invoice_title", "ACCRECCREDIT" => "credit_title", "QUOTE" => "quote_title", "PO" => "po_title", _ => "" };
    if let Some(s) = t[field].as_str().map(str::trim).filter(|s| !s.is_empty()) { *title = s.to_string(); }
    if let Some(h) = t["accent"].as_str().map(|s| s.trim().trim_start_matches('#')).filter(|h| h.len() == 6).and_then(|h| u32::from_str_radix(h, 16).ok()) {
        *accent = (((h >> 16) & 255) as f32 / 255.0, ((h >> 8) & 255) as f32 / 255.0, (h & 255) as f32 / 255.0);
    }
    let mut terms = doc["terms"].as_str().unwrap_or("").trim().to_string();
    if terms.is_empty() { terms = t["terms"].as_str().unwrap_or("").trim().to_string(); }
    let pay = t["payment_instructions"].as_str().unwrap_or("").trim();
    if !pay.is_empty() && matches!(doc["type"].as_str(), Some("ACCREC")) { if !terms.is_empty() { terms.push_str("\n\n"); } terms.push_str(pay); }
    doc["terms"] = Value::String(terms);
    if t["show_tax_number"].as_i64() == Some(0) { settings["tax_number"] = Value::String(String::new()); }
}

/// Render an invoice, bill, quote/estimate, credit note or purchase order.
/// `title` is the printed heading; `fmt_date` formats ISO dates in the company's style.
pub fn document(doc: &Value, settings: &Value, title: &str, fmt_date: &dyn Fn(&str) -> String, accent: (f32, f32, f32)) -> Vec<u8> {
    documents(&[(doc.clone(), title.to_string())], settings, fmt_date, accent)
}

/// Several forms in one PDF, each starting on a new page (File ▸ Print Forms).
pub fn documents(docs: &[(Value, String)], settings: &Value, fmt_date: &dyn Fn(&str) -> String, accent: (f32, f32, f32)) -> Vec<u8> {
    let letter = settings["country"] == "US" || settings["country"] == "CA";
    let mut p = Pdf::new(letter);
    for (doc, title) in docs {
        p.new_page();
        draw_document(&mut p, doc, settings, title, fmt_date, accent);
    }
    p.finish()
}

fn draw_document(p: &mut Pdf, doc: &Value, settings: &Value, title: &str, fmt_date: &dyn Fn(&str) -> String, accent: (f32, f32, f32)) {
    let (l, r) = (48.0, p.w - 48.0);
    let mut y = 64.0;
    // Company block (left) and title block (right).
    p.text(l, y, 16.0, true, INK, st(settings, "org_name"));
    let mut yy = y + 16.0;
    for line in st(settings, "address").lines().filter(|x| !x.trim().is_empty()) { p.text(l, yy, 9.5, false, MUTED, line); yy += 12.0; }
    for k in ["phone", "email", "website"] { if !st(settings, k).is_empty() { p.text(l, yy, 9.5, false, MUTED, st(settings, k)); yy += 12.0; } }
    if !st(settings, "tax_number").is_empty() {
        let lbl = match st(settings, "country") { "AU" => "ABN", "NZ" => "GST No.", "GB" | "IE" => "VAT No.", _ => "Tax ID" };
        p.text(l, yy, 9.5, false, MUTED, &format!("{lbl} {}", st(settings, "tax_number"))); yy += 12.0;
    }
    p.text_right(r, y + 4.0, 22.0, true, accent, &title.to_uppercase());
    let mut ry = y + 24.0;
    let kind = st(doc, "type");
    let due_label = match kind { "QUOTE" => "Expires", "PO" => "Delivery", "ACCRECCREDIT" | "ACCPAYCREDIT" => "", _ => "Due date" };
    let due = doc["due_date"].as_str().or(doc["expiry_date"].as_str()).or(doc["delivery_date"].as_str()).unwrap_or("");
    let mut meta: Vec<(&str, String)> = vec![("Number", st(doc, "number").to_string()), ("Date", fmt_date(st(doc, "date")))];
    if !due_label.is_empty() && !due.is_empty() { meta.push((due_label, fmt_date(due))); }
    if !st(doc, "reference").is_empty() { meta.push(("Reference", st(doc, "reference").to_string())); }
    for (k, v) in meta {
        p.text_right(r - 110.0, ry, 9.5, false, MUTED, k);
        p.text_right(r, ry, 9.5, true, INK, &v);
        ry += 13.0;
    }
    y = yy.max(ry) + 18.0;
    // Bill to.
    let to_label = match kind { "ACCPAY" | "ACCPAYCREDIT" | "PO" => "FROM / SUPPLIER", _ => "BILL TO" };
    p.text(l, y, 8.5, true, MUTED, to_label);
    y += 14.0;
    p.text(l, y, 11.0, true, INK, st(doc, "contact_name"));
    y += 13.0;
    for line in st(doc, "contact_address").lines().filter(|x| !x.trim().is_empty()) { p.text(l, y, 9.5, false, MUTED, line); y += 12.0; }
    y += 18.0;

    // Lines table.
    let cols = [(l + 8.0, "DESCRIPTION", false), (r - 230.0, "QTY", true), (r - 150.0, "RATE", true), (r - 70.0, "TAX", true), (r - 8.0, "AMOUNT", true)];
    let header = |p: &mut Pdf, y: f32| {
        p.rect(l, y - 12.0, r - l, 20.0, BAND);
        for (x, h, right) in cols { if right { p.text_right(x, y + 2.0, 8.0, true, MUTED, h); } else { p.text(x, y + 2.0, 8.0, true, MUTED, h); } }
    };
    header(p, y);
    y += 22.0;
    let bottom = p.h - 72.0;
    for line in doc["lines"].as_array().cloned().unwrap_or_default() {
        let desc = { let d = st(&line, "description"); if d.is_empty() { st(&line, "item_code").to_string() } else { d.to_string() } };
        let wrapped = wrap(&desc, 9.5, r - 230.0 - 50.0 - (l + 8.0));
        let need = 13.0 * wrapped.len() as f32 + 8.0;
        if y + need > bottom { p.new_page(); y = 64.0; header(p, y); y += 22.0; }
        for (n, w) in wrapped.iter().enumerate() { p.text(l + 8.0, y + n as f32 * 13.0, 9.5, false, INK, w); }
        let qty = { let q = i(&line["qty_e4"]); let s = format!("{:.4}", q as f64 / 10000.0); s.trim_end_matches('0').trim_end_matches('.').to_string() };
        let rate = { let u = i(&line["unit_e2"]); if u % 100 == 0 { money(u / 100) } else { format!("{:.4}", u as f64 / 10000.0) } };
        p.text_right(r - 230.0, y, 9.5, false, INK, &qty);
        p.text_right(r - 150.0, y, 9.5, false, INK, &rate);
        p.text_right(r - 70.0, y, 9.5, false, MUTED, &money(i(&line["tax_amount"])));
        p.text_right(r - 8.0, y, 9.5, false, INK, &money(i(&line["line_amount"])));
        y += need - 2.0;
        p.line(l, y - 9.0, r, 0.5, RULE);
    }
    // Totals.
    y += 10.0;
    if y + 120.0 > bottom { p.new_page(); y = 64.0; }
    let lx = r - 220.0;
    let total_row = |p: &mut Pdf, y: &mut f32, label: &str, v: i64, bold: bool, size: f32| {
        p.text(lx, *y, size, bold, if bold { INK } else { MUTED }, label);
        p.text_right(r - 8.0, *y, size, bold, INK, &money(v));
        *y += size + 7.0;
    };
    total_row(p, &mut y, "Subtotal", i(&doc["subtotal"]), false, 9.5);
    for t in doc["tax_summary"].as_array().cloned().unwrap_or_default() {
        let lbl = format!("{} {}", if doc["line_amount_type"] == "INCLUSIVE" { "Includes" } else { "Total" }, st(&t, "name"));
        total_row(p, &mut y, &lbl, i(&t["tax"]), false, 9.5);
    }
    p.line(lx, y - 7.0, r, 1.0, INK);
    y += 4.0;
    total_row(p, &mut y, &format!("TOTAL {}", st(doc, "currency")), i(&doc["total"]), true, 11.5);
    let ledger = matches!(kind, "ACCREC" | "ACCPAY" | "ACCRECCREDIT" | "ACCPAYCREDIT");
    if ledger {
        for pmt in doc["payments"].as_array().cloned().unwrap_or_default() { total_row(p, &mut y, &format!("Payment {}", fmt_date(st(&pmt, "date"))), -i(&pmt["amount"]), false, 9.5); }
        for a in doc["allocations"].as_array().cloned().unwrap_or_default() { total_row(p, &mut y, &format!("Credit {}", st(&a, "credit_number")), -i(&a["amount"]), false, 9.5); }
        p.rect(lx - 8.0, y - 13.0, r - lx + 8.0, 22.0, BAND);
        total_row(p, &mut y, &format!("{} {}", if matches!(kind, "ACCRECCREDIT" | "ACCPAYCREDIT") { "CREDIT REMAINING" } else { "AMOUNT DUE" }, st(doc, "currency")), i(&doc["amount_due"]), true, 12.0);
    }
    // Notes / terms.
    if !st(doc, "terms").is_empty() {
        y += 16.0;
        p.text(l, y, 8.5, true, MUTED, "NOTES");
        y += 13.0;
        for w in wrap(st(doc, "terms"), 9.5, r - l) { if y > bottom { p.new_page(); y = 64.0; } p.text(l, y, 9.5, false, INK, &w); y += 12.5; }
    }
    p.text(l, p.h - 36.0, 7.5, false, MUTED, "Generated by LedgerCraft");
}

/// Flatten a report into rows of (label, values, level) where level 0 = row, 1 = child, 2 = section title, 3 = total.
fn report_rows(rep: &Value, relabel: &dyn Fn(&str) -> String) -> Vec<(String, Vec<Value>, u8)> {
    let mut out = vec![];
    for s in rep["sections"].as_array().cloned().unwrap_or_default() {
        if let Some(t) = s["title"].as_str().filter(|x| !x.is_empty()) { out.push((relabel(t), vec![], 2)); }
        for r in s["rows"].as_array().cloned().unwrap_or_default() {
            let date = r["date"].as_str().map(|d| format!("{d}  ")).unwrap_or_default();
            out.push((format!("{date}{}", relabel(st(&r, "label"))), r["values"].as_array().cloned().unwrap_or_default(), 0));
            for c in r["children"].as_array().cloned().unwrap_or_default() {
                let date = c["date"].as_str().map(|d| format!("{d}  ")).unwrap_or_default();
                out.push((format!("{date}{}", st(&c, "label")), c["values"].as_array().cloned().unwrap_or_default(), 1));
            }
        }
        if !s["total"].is_null() { out.push((relabel(s["total"]["label"].as_str().unwrap_or("Total")), s["total"]["values"].as_array().cloned().unwrap_or_default(), 3)); }
    }
    out
}

fn cell(v: &Value, qty: bool) -> String {
    match v.as_i64() { None => String::new(), Some(x) if qty => { let s = format!("{:.2}", x as f64 / 10000.0); s.trim_end_matches('0').trim_end_matches('.').to_string() } Some(x) => money(x) }
}

/// A voucher check (QuickBooks Desktop style, US Letter): the check in the top third, then two
/// stubs (payee's and yours) with the expense lines. `c`: payee, date (ISO), number, amount
/// (cents), words, memo, bank, lines [{account, memo, amount}].
pub fn check(c: &Value, settings: &Value, fmt_date: &dyn Fn(&str) -> String) -> Vec<u8> {
    let mut p = Pdf::new(true);
    let (l, r) = (36.0, p.w - 36.0);
    let s = |k: &str| c[k].as_str().unwrap_or("").to_string();
    let amount = c["amount"].as_i64().unwrap_or(0);
    // --- the check ---
    p.text(l, 50.0, 12.0, true, INK, st(settings, "org_name"));
    let mut y = 62.0;
    for line in st(settings, "address").lines().filter(|x| !x.trim().is_empty()).take(3) { p.text(l, y, 8.5, false, MUTED, line); y += 10.0; }
    p.text_right(r, 50.0, 10.0, true, INK, &s("number"));
    p.text(r - 150.0, 82.0, 8.0, false, MUTED, "DATE");
    p.text(r - 110.0, 82.0, 10.5, false, INK, &fmt_date(&s("date")));
    p.line(r - 115.0, 85.0, r, 0.6, RULE);
    p.text(l, 118.0, 7.5, false, MUTED, "PAY TO THE");
    p.text(l, 127.0, 7.5, false, MUTED, "ORDER OF");
    p.text(l + 62.0, 126.0, 11.0, true, INK, &s("payee"));
    p.line(l + 60.0, 130.0, r - 130.0, 0.6, RULE);
    p.text(r - 120.0, 126.0, 11.0, false, INK, "$");
    p.rect(r - 108.0, 112.0, 108.0, 18.0, BAND);
    p.text_right(r - 6.0, 126.0, 11.0, true, INK, &format!("**{}", money(amount)));
    let words = s("words");
    p.text(l, 152.0, 10.0, false, INK, &format!("{words}{}", "*".repeat(90usize.saturating_sub(words.len()))));
    p.text_right(r, 152.0, 8.0, false, MUTED, "DOLLARS");
    p.line(l, 156.0, r, 0.6, RULE);
    p.text(l + 20.0, 178.0, 10.0, false, INK, &s("payee"));
    p.text(l, 232.0, 7.5, false, MUTED, "MEMO");
    p.text(l + 32.0, 232.0, 9.5, false, INK, &s("memo"));
    p.line(l + 30.0, 236.0, l + 250.0, 0.6, RULE);
    p.line(r - 200.0, 236.0, r, 0.6, RULE);
    p.text(r - 200.0, 246.0, 7.0, false, MUTED, "AUTHORIZED SIGNATURE");
    // --- two stubs ---
    for top in [276.0f32, 540.0] {
        p.line(l, top - 8.0, r, 0.4, RULE);
        p.text(l, top + 10.0, 10.0, true, INK, &s("payee"));
        p.text_right(r, top + 10.0, 10.0, true, INK, &format!("{}   {}", fmt_date(&s("date")), s("number")));
        p.text(l, top + 30.0, 8.0, true, MUTED, "ACCOUNT");
        p.text(l + 230.0, top + 30.0, 8.0, true, MUTED, "MEMO");
        p.text_right(r, top + 30.0, 8.0, true, MUTED, "AMOUNT");
        let mut yy = top + 46.0;
        for ln in c["lines"].as_array().cloned().unwrap_or_default().iter().take(10) {
            p.text(l, yy, 9.5, false, INK, ln["account"].as_str().unwrap_or(""));
            p.text(l + 230.0, yy, 9.5, false, INK, ln["memo"].as_str().unwrap_or(""));
            p.text_right(r, yy, 9.5, false, INK, &money(ln["amount"].as_i64().unwrap_or(0)));
            yy += 13.0;
        }
        p.line(r - 120.0, top + 200.0, r, 0.6, RULE);
        p.text(l, top + 214.0, 9.0, false, MUTED, &s("bank"));
        p.text_right(r, top + 214.0, 10.0, true, INK, &money(amount));
    }
    p.finish()
}

/// Payslips for a pay run: one page per employee — earnings, tax, deductions, retirement, net pay, leave.
pub fn payslips(run: &Value, settings: &Value, fmt_date: &dyn Fn(&str) -> String, retirement: &str) -> Vec<u8> {
    let mut p = Pdf::new(settings["country"] == "US" || settings["country"] == "CA");
    let (l, r) = (48.0, p.w - 48.0);
    for s in run["payslips"].as_array().cloned().unwrap_or_default() {
        p.new_page();
        p.text(l, 64.0, 16.0, true, INK, st(settings, "org_name"));
        p.text_right(r, 64.0, 18.0, true, INK, "Payslip");
        p.text(l, 92.0, 12.0, true, INK, st(&s, "employee"));
        p.text(l, 108.0, 9.5, false, MUTED, &format!("Pay period {} to {}", fmt_date(st(run, "period_start")), fmt_date(st(run, "period_end"))));
        p.text(l, 121.0, 9.5, false, MUTED, &format!("Paid on {}", fmt_date(st(run, "payment_date"))));
        p.text_right(r, 108.0, 9.5, false, MUTED, &format!("Pay run {}", st(run, "number")));
        let mut y = 156.0;
        p.rect(l, y - 12.0, r - l, 18.0, BAND);
        p.text(l + 6.0, y, 9.0, true, MUTED, "DESCRIPTION");
        p.text_right(r - 150.0, y, 9.0, true, MUTED, "HOURS / UNITS");
        p.text_right(r - 6.0, y, 9.0, true, MUTED, "AMOUNT");
        y += 20.0;
        // Earnings, deductions and reimbursements; tax and employer amounts are in the totals below.
        for ln in s["lines"].as_array().cloned().unwrap_or_default().into_iter().filter(|ln| matches!(ln["kind"].as_str(), Some("EARNINGS" | "DEDUCTION" | "REIMBURSEMENT"))) {
            let kind = st(&ln, "kind");
            let label = match kind { "DEDUCTION" => format!("{} (deduction)", st(&ln, "name")), "REIMBURSEMENT" => format!("{} (reimbursement)", st(&ln, "name")), _ => st(&ln, "name").to_string() };
            p.text(l + 6.0, y, 10.0, false, INK, &label);
            if let Some(u) = ln["units_e2"].as_i64().filter(|u| *u != 0) { p.text_right(r - 150.0, y, 10.0, false, INK, &format!("{:.2}", u as f64 / 100.0)); }
            let amt = i(&ln["amount"]);
            p.text_right(r - 6.0, y, 10.0, false, INK, &money(if kind == "DEDUCTION" { -amt } else { amt }));
            y += 15.0;
        }
        y += 8.0;
        p.line(r - 220.0, y - 10.0, r, 0.6, RULE);
        for (label, v, bold) in [("Gross pay", i(&s["gross"]), false), ("Tax withheld", -i(&s["tax"]), false), ("Deductions", -i(&s["deductions"]), false), ("Reimbursements", i(&s["reimbursements"]), false), ("Net pay", i(&s["net"]), true)] {
            if v == 0 && !bold { continue; }
            p.text(r - 220.0, y, if bold { 11.0 } else { 10.0 }, bold, INK, label);
            p.text_right(r - 6.0, y, if bold { 11.0 } else { 10.0 }, bold, INK, &money(v));
            y += 15.0;
        }
        if i(&s["super"]) != 0 { p.text(l, y + 10.0, 9.5, false, MUTED, &format!("{retirement} paid by the employer: {}", money(i(&s["super"])))); y += 14.0; }
        let leave = s["leave"].as_array().cloned().unwrap_or_default();
        if !leave.is_empty() {
            y += 22.0;
            p.text(l, y, 10.0, true, INK, "Leave this pay period");
            for lv in leave { y += 14.0; p.text(l + 6.0, y, 9.5, false, INK, st(&lv, "name")); p.text_right(r - 6.0, y, 9.5, false, INK, &format!("accrued {:.2} h · taken {:.2} h", i(&lv["accrued_e2"]) as f64 / 100.0, i(&lv["taken_e2"]) as f64 / 100.0)); }
        }
    }
    p.finish()
}

/// A report as a PDF table (landscape when there are many columns).
pub fn report(rep: &Value, title: &str, letter: bool, relabel: &dyn Fn(&str) -> String) -> Vec<u8> {
    let cols: Vec<String> = rep["columns"].as_array().cloned().unwrap_or_default().iter().map(|c| c.as_str().unwrap_or("").to_string()).collect();
    let qty: Vec<usize> = rep["qty_cols"].as_array().cloned().unwrap_or_default().iter().filter_map(|x| x.as_u64().map(|x| x as usize)).collect();
    let mut p = Pdf::new(letter);
    if cols.len() > 4 { std::mem::swap(&mut p.w, &mut p.h); }
    let (l, r) = (40.0, p.w - 40.0);
    let colw = ((r - l) * 0.62 / cols.len().max(1) as f32).min(95.0);
    let first_val = r - colw * cols.len() as f32;
    let header = |p: &mut Pdf, y: f32| {
        p.text(l, 40.0, 9.0, false, MUTED, st(rep, "org"));
        p.text(l, 58.0, 15.0, true, INK, title);
        p.text(l, 74.0, 9.0, false, MUTED, &format!("{}{}", st(rep, "subtitle"), rep["basis"].as_str().map(|b| format!(" - {b} basis")).unwrap_or_default()));
        p.rect(l, y - 12.0, r - l, 18.0, BAND);
        for (n, c) in cols.iter().enumerate() { p.text_right(first_val + colw * (n as f32 + 1.0) - 4.0, y + 1.0, 7.5, true, MUTED, &c.to_uppercase()); }
    };
    let mut y = 100.0;
    header(&mut p, y);
    y += 20.0;
    for (label, values, level) in report_rows(rep, relabel) {
        if y > p.h - 50.0 { p.new_page(); y = 100.0; header(&mut p, y); y += 20.0; }
        let bold = level >= 2;
        if level == 3 { p.line(l, y - 10.0, r, 0.5, RULE); }
        let indent = if level == 1 { 14.0 } else { 0.0 };
        let mut lbl = label.clone();
        while width(&lbl, 9.0, bold) > first_val - l - indent - 8.0 && lbl.chars().count() > 4 { lbl.pop(); }
        p.text(l + 4.0 + indent, y, 9.0, bold, if level == 1 { MUTED } else { INK }, &lbl);
        for (n, v) in values.iter().enumerate().take(cols.len()) { p.text_right(first_val + colw * (n as f32 + 1.0) - 4.0, y, 9.0, level == 3, INK, &cell(v, qty.contains(&n))); }
        y += if level == 2 { 16.0 } else { 13.0 };
        if level == 3 { y += 4.0; }
    }
    p.text(l, p.h - 24.0, 7.0, false, MUTED, "Generated by LedgerCraft");
    p.finish()
}

/// A report as CSV (opens in Excel, Numbers, LibreOffice).
pub fn report_csv(rep: &Value, relabel: &dyn Fn(&str) -> String) -> String {
    let q = |s: &str| if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() };
    let cols: Vec<String> = rep["columns"].as_array().cloned().unwrap_or_default().iter().map(|c| q(c.as_str().unwrap_or(""))).collect();
    let qty: Vec<usize> = rep["qty_cols"].as_array().cloned().unwrap_or_default().iter().filter_map(|x| x.as_u64().map(|x| x as usize)).collect();
    let mut out = format!("{}\n{}\n\n,{}\n", q(st(rep, "title")), q(st(rep, "subtitle")), cols.join(","));
    for (label, values, level) in report_rows(rep, relabel) {
        let lbl = if level == 1 { format!("  {label}") } else { label };
        let vals: Vec<String> = values.iter().enumerate().map(|(n, v)| cell(v, qty.contains(&n)).replace(',', "")).collect();
        out += &format!("{},{}\n", q(&lbl), vals.join(","));
    }
    out
}

/// Where printed documents are saved: Documents\LedgerCraft (falls back to the temp folder).
pub fn output_dir() -> std::path::PathBuf {
    // Tests never write into the user's Documents folder.
    if cfg!(test) { let d = std::env::temp_dir().join("lc-test-output"); let _ = std::fs::create_dir_all(&d); return d; }
    let base = std::env::var("USERPROFILE").map(|h| std::path::PathBuf::from(h).join("Documents")).ok().filter(|p| p.exists()).unwrap_or_else(std::env::temp_dir);
    let dir = base.join("LedgerCraft");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Open a file with the system's default app (the PDF viewer, for printing).
pub fn open_file(path: &std::path::Path) {
    #[cfg(windows)]
    { let _ = std::process::Command::new("cmd").args(["/C", "start", ""]).arg(path).spawn(); }
    #[cfg(target_os = "macos")]
    { let _ = std::process::Command::new("open").arg(path).spawn(); }
    #[cfg(all(unix, not(target_os = "macos")))]
    { let _ = std::process::Command::new("xdg-open").arg(path).spawn(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Cross-reference table offsets must point at the objects, or viewers reject the file.
    fn check_structure(bytes: &[u8]) {
        let find = |hay: &[u8], needle: &[u8]| hay.windows(needle.len()).rposition(|w| w == needle);
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(bytes.ends_with(b"%%EOF
"));
        let at = find(bytes, b"startxref
").unwrap() + 10;
        let tail = std::str::from_utf8(&bytes[at..]).unwrap();
        let sx: usize = tail.lines().next().unwrap().parse().unwrap();
        assert!(bytes[sx..].starts_with(b"xref"));
        let xref = std::str::from_utf8(&bytes[sx..]).unwrap();
        for (n, line) in xref.lines().skip(3).take_while(|l| l.ends_with(" n ")).enumerate() {
            let off: usize = line[..10].parse().unwrap();
            assert!(bytes[off..].starts_with(format!("{} 0 obj", n + 1).as_bytes()), "object {} offset", n + 1);
        }
    }

    #[test]
    fn invoice_pdf_is_well_formed_and_paginates() {
        let lines: Vec<_> = (0..60).map(|n| json!({"description": format!("Line {n} — design work with a fairly long description that has to wrap onto a second line"), "qty_e4": 10000, "unit_e2": 1450000, "line_amount": 14500, "tax_amount": 1450})).collect();
        let doc = json!({"type": "ACCREC", "number": "INV-0001", "date": "2026-10-12", "due_date": "2026-10-26", "contact_name": "Acme (Pty) Ltd", "contact_address": "1 Main St\nSydney",
            "lines": lines, "subtotal": 870000, "tax_summary": [{"name": "GST", "tax": 87000}], "total": 957000, "amount_due": 957000, "currency": "AUD", "terms": "Pay within 14 days."});
        let settings = json!({"org_name": "Harbourside Design Co", "address": "1 Harbour St\nSydney NSW", "country": "AU", "tax_number": "12 345 678 901"});
        let pdf = document(&doc, &settings, "Tax Invoice", &|d| d.to_string(), (0.1, 0.5, 0.3));
        check_structure(&pdf);
        let s = String::from_utf8_lossy(&pdf);
        let pages: usize = s.split("/Count ").nth(1).unwrap().split(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap();
        assert!((2..=6).contains(&pages), "60 wrapped lines should span several pages, got {pages}");
        assert!(s.contains("Acme \\(Pty\\) Ltd"), "parentheses escaped");
        assert!(s.contains("9,570.00"));
    }

    #[test]
    fn report_exports() {
        let rep = json!({"title": "Profit and Loss", "org": "Co", "subtitle": "Oct", "columns": ["Oct 2026", "Sep 2026"], "sections": [
            {"title": "Income", "rows": [{"label": "Sales, retail", "values": [123456, 100]}], "total": {"label": "Total Income", "values": [123456, 100]}},
            {"title": "", "rows": [], "total": {"label": "Net Profit", "values": [-5, null]}}]});
        let pdf = report(&rep, "Profit and Loss", false, &|s| s.to_string());
        check_structure(&pdf);
        let csv = report_csv(&rep, &|s| s.to_string());
        assert!(csv.contains("\"Sales, retail\",1234.56,1.00"), "{csv}");
        assert!(csv.contains("Net Profit,-0.05,"), "{csv}");
    }

    #[test]
    fn text_width_and_escaping() {
        assert!((width("0000", 10.0, false) - 22.24).abs() < 0.01);
        assert_eq!(esc("a(b)\\c"), "a\\(b\\)\\\\c");
        assert_eq!(esc("é"), "\\351");
        assert_eq!(esc("漢"), "?");
    }

    /// Writes a sample invoice from the demo company to $LC_PDF_OUT for visual review.
    #[test]
    #[ignore]
    fn sample_invoice_pdf() {
        let out = std::env::var("LC_PDF_OUT").expect("set LC_PDF_OUT");
        let dir = std::env::temp_dir().join(format!("lc-pdf-sample-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let be = crate::backend::Backend::new(dir.clone());
        be.call_now("orgs.create", json!({"org_name": "Harbourside Design Co", "country": "AU", "demo": true, "owner_name": "Alex"})).unwrap();
        let list = be.call_now("docs.list", json!({"type": "ACCREC", "tab": "awaiting"})).unwrap();
        let id = list["rows"][0]["id"].as_i64().unwrap();
        let d = be.call_now("docs.get", json!({"id": id})).unwrap();
        let st = be.call_now("settings.get", json!({})).unwrap();
        std::fs::write(&out, document(&d, &st, "Tax Invoice", &|x| crate::ui::date(x, crate::ui::DateStyle::Intl), (0.16, 0.6, 0.27))).unwrap();
        let rep = be.call_now("reports.run", json!({"name": "profit-and-loss", "params": {"from": "2026-07-01", "to": "2026-09-30"}})).unwrap();
        std::fs::write(format!("{out}.report.pdf"), report(&rep, "Profit and Loss", false, &|s| s.to_string())).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

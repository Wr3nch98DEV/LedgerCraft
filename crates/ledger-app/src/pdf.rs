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

/// Render an invoice, bill, quote/estimate, credit note or purchase order.
/// `title` is the printed heading; `fmt_date` formats ISO dates in the company's style.
pub fn document(doc: &Value, settings: &Value, title: &str, fmt_date: &dyn Fn(&str) -> String, accent: (f32, f32, f32)) -> Vec<u8> {
    let letter = settings["country"] == "US" || settings["country"] == "CA";
    let mut p = Pdf::new(letter);
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
    header(&mut p, y);
    y += 22.0;
    let bottom = p.h - 72.0;
    for line in doc["lines"].as_array().cloned().unwrap_or_default() {
        let desc = { let d = st(&line, "description"); if d.is_empty() { st(&line, "item_code").to_string() } else { d.to_string() } };
        let wrapped = wrap(&desc, 9.5, r - 230.0 - 50.0 - (l + 8.0));
        let need = 13.0 * wrapped.len() as f32 + 8.0;
        if y + need > bottom { p.new_page(); y = 64.0; header(&mut p, y); y += 22.0; }
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
    total_row(&mut p, &mut y, "Subtotal", i(&doc["subtotal"]), false, 9.5);
    for t in doc["tax_summary"].as_array().cloned().unwrap_or_default() {
        let lbl = format!("{} {}", if doc["line_amount_type"] == "INCLUSIVE" { "Includes" } else { "Total" }, st(&t, "name"));
        total_row(&mut p, &mut y, &lbl, i(&t["tax"]), false, 9.5);
    }
    p.line(lx, y - 7.0, r, 1.0, INK);
    y += 4.0;
    total_row(&mut p, &mut y, &format!("TOTAL {}", st(doc, "currency")), i(&doc["total"]), true, 11.5);
    let ledger = matches!(kind, "ACCREC" | "ACCPAY" | "ACCRECCREDIT" | "ACCPAYCREDIT");
    if ledger {
        for pmt in doc["payments"].as_array().cloned().unwrap_or_default() { total_row(&mut p, &mut y, &format!("Payment {}", fmt_date(st(&pmt, "date"))), -i(&pmt["amount"]), false, 9.5); }
        for a in doc["allocations"].as_array().cloned().unwrap_or_default() { total_row(&mut p, &mut y, &format!("Credit {}", st(&a, "credit_number")), -i(&a["amount"]), false, 9.5); }
        p.rect(lx - 8.0, y - 13.0, r - lx + 8.0, 22.0, BAND);
        total_row(&mut p, &mut y, &format!("{} {}", if matches!(kind, "ACCRECCREDIT" | "ACCPAYCREDIT") { "CREDIT REMAINING" } else { "AMOUNT DUE" }, st(doc, "currency")), i(&doc["amount_due"]), true, 12.0);
    }
    // Notes / terms.
    if !st(doc, "terms").is_empty() {
        y += 16.0;
        p.text(l, y, 8.5, true, MUTED, "NOTES");
        y += 13.0;
        for w in wrap(st(doc, "terms"), 9.5, r - l) { if y > bottom { p.new_page(); y = 64.0; } p.text(l, y, 9.5, false, INK, &w); y += 12.5; }
    }
    p.text(l, p.h - 36.0, 7.5, false, MUTED, "Generated by LedgerCraft");
    p.finish()
}

/// Where printed documents are saved: Documents\LedgerCraft (falls back to the temp folder).
pub fn output_dir() -> std::path::PathBuf {
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
        std::fs::write(out, document(&d, &st, "Tax Invoice", &|x| crate::ui::date(x, crate::ui::DateStyle::Intl), (0.16, 0.6, 0.27))).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

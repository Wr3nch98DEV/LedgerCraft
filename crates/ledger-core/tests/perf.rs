//! Performance check with ~100k transactions. Run with:
//!   cargo test -p ledger-core --release --test perf -- --ignored --nocapture

use ledger_core::{banking::BankTxInput, documents::{DocInput, LineInput}, reports::ReportParams, seed::SetupInput, Ledger};
use std::time::Instant;

#[test]
#[ignore]
fn hundred_thousand_transactions() {
    let dir = tempfile::tempdir().unwrap();
    let mut l = Ledger::open(&dir.path().join("perf.lcdb")).unwrap();
    l.setup_org(&SetupInput { org_name: "Perf".into(), country: "AU".into(), base_currency: None, fy_end_month: None, tax_basis: None, tax_registered: Some(true),
        tax_number: None, tax_period: None, industry: None, owner_name: None, owner_email: None, conversion_date: None }).unwrap();
    let sales = l.account_by_code("200").unwrap().unwrap();
    let exp = l.account_by_code("429").unwrap().unwrap();
    let bank = l.account_by_code("090").unwrap().unwrap();
    let gst: i64 = l.conn.query_row("SELECT id FROM tax_rates WHERE name = 'GST on Income'", [], |r| r.get(0)).unwrap();
    let customers: Vec<i64> = (0..200).map(|i| l.find_or_create_contact(&format!("Customer {i}")).unwrap()).collect();
    let t = Instant::now();
    l.conn.execute_batch("BEGIN").unwrap();
    let n_docs: i64 = std::env::var("PERF_N").ok().and_then(|v| v.parse().ok()).unwrap_or(50_000);
    for i in 0..n_docs {
        let day = 1 + (i % 28);
        let month = 1 + (i / 4200) % 12;
        let date = format!("2025-{month:02}-{day:02}");
        l.save_document(None, &DocInput { kind: "ACCREC".into(), contact_id: Some(customers[(i % 200) as usize]), date: date.clone(), line_amount_type: Some("EXCLUSIVE".into()),
            lines: vec![LineInput { description: "Work".into(), quantity: Some("1".into()), unit_price: Some(format!("{}.00", 50 + i % 900)), account_id: Some(sales), tax_rate_id: Some(gst), ..Default::default() }],
            status: Some("AUTHORISED".into()), ..Default::default() }).unwrap();
        l.save_bank_transaction(None, &BankTxInput { kind: "SPEND".into(), account_id: bank, date, line_amount_type: Some("NOTAX".into()),
            lines: vec![LineInput { amount: Some(format!("{}.50", 5 + i % 300)), account_id: Some(exp), ..Default::default() }], ..Default::default() }).unwrap();
    }
    l.conn.execute_batch("COMMIT").unwrap();
    let n: i64 = l.conn.query_row("SELECT COUNT(*) FROM journals", [], |r| r.get(0)).unwrap();
    println!("inserted {n} journals in {:.1}s", t.elapsed().as_secs_f64());
    let time = |name: &str, f: &dyn Fn()| { let t = Instant::now(); f(); let ms = t.elapsed().as_millis(); println!("{name:<28} {ms:>6} ms"); ms };
    let p = ReportParams { from: Some("2025-01-01".into()), to: Some("2025-12-31".into()), ..Default::default() };
    let mut worst = 0;
    worst = worst.max(time("profit and loss", &|| { l.profit_and_loss(&p).unwrap(); }));
    worst = worst.max(time("P&L monthly (12 cols)", &|| { l.profit_and_loss(&ReportParams { monthly: Some(true), ..p.clone() }).unwrap(); }));
    worst = worst.max(time("balance sheet", &|| { l.balance_sheet(&ReportParams { date: Some("2025-12-31".into()), ..Default::default() }).unwrap(); }));
    worst = worst.max(time("trial balance", &|| { l.trial_balance(&ReportParams { date: Some("2025-12-31".into()), ..Default::default() }).unwrap(); }));
    worst = worst.max(time("aged receivables", &|| { l.aged(true, &ReportParams { date: Some("2025-12-31".into()), ..Default::default() }).unwrap(); }));
    worst = worst.max(time("tax reconciliation", &|| { l.tax_report(&p).unwrap(); }));
    worst = worst.max(time("invoice list (500)", &|| { l.list_documents("ACCREC", "all", "", None, 500, 0).unwrap(); }));
    worst = worst.max(time("search", &|| { l.search("Customer 42", 20).unwrap(); }));
    worst = worst.max(time("dashboard", &|| { ledger_core::dashboard::dashboard(&l).unwrap(); }));
    assert_eq!(l.ledger_imbalance().unwrap(), 0);
    println!("slowest: {worst} ms");
}

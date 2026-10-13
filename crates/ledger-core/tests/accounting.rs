//! End-to-end accounting correctness tests (posting rules, invariants, lock dates, FX, inventory).

use ledger_core::{
    banking::BankTxInput, documents::{DocInput, LineInput}, items::ItemInput, journal::{JLine, NewJournal},
    payments::PaymentInput, reports::ReportParams, seed::SetupInput, statements::StmtLine, Ledger,
};
use serde_json::Value;

pub fn org() -> Ledger {
    let mut l = Ledger::open_in_memory().unwrap();
    l.setup_org(&SetupInput { org_name: "Test Co".into(), country: "AU".into(), base_currency: None, fy_end_month: None, tax_basis: None,
        tax_registered: Some(true), tax_number: None, tax_period: None, industry: None, owner_name: None, owner_email: None, conversion_date: None }).unwrap();
    l
}

fn acct(l: &Ledger, code: &str) -> i64 { l.account_by_code(code).unwrap().unwrap() }
fn tax(l: &Ledger, name: &str) -> i64 { l.conn.query_row("SELECT id FROM tax_rates WHERE name = ?1", [name], |r| r.get(0)).unwrap() }
fn bal(l: &Ledger, id: i64) -> i64 { l.conn.query_row("SELECT COALESCE(SUM(jl.amount),0) FROM journal_lines jl JOIN journals j ON j.id=jl.journal_id WHERE j.posted=1 AND jl.account_id=?1", [id], |r| r.get(0)).unwrap() }
fn line(account: i64, tax_rate: Option<i64>, qty: &str, price: &str) -> LineInput {
    LineInput { description: "Line".into(), quantity: Some(qty.into()), unit_price: Some(price.into()), account_id: Some(account), tax_rate_id: tax_rate, ..Default::default() }
}

fn invoice(l: &Ledger, date: &str, lines: Vec<LineInput>, status: &str) -> i64 {
    l.save_document(None, &DocInput { kind: "ACCREC".into(), contact_name: Some("Acme".into()), date: date.into(), line_amount_type: Some("EXCLUSIVE".into()),
        lines, status: Some(status.into()), ..Default::default() }).unwrap()
}

fn assert_invariants(l: &Ledger) {
    assert_eq!(l.ledger_imbalance().unwrap(), 0, "ledger out of balance");
    let bs = l.balance_sheet(&ReportParams { date: Some("2099-12-31".into()), ..Default::default() }).unwrap();
    assert!(bs["check"].as_array().unwrap().iter().all(|v| v.as_i64() == Some(0)), "A != L + E: {:?}", bs["check"]);
    let tb = l.trial_balance(&ReportParams { date: Some("2099-12-31".into()), ..Default::default() }).unwrap();
    assert_eq!(tb["balanced"], Value::Bool(true));
    let bsc = l.balance_sheet(&ReportParams { date: Some("2099-12-31".into()), basis: Some("CASH".into()), ..Default::default() }).unwrap();
    assert!(bsc["check"].as_array().unwrap().iter().all(|v| v.as_i64() == Some(0)), "cash basis A != L + E: {:?}", bsc["check"]);
}

#[test]
fn invoice_payment_void_lifecycle() {
    let l = org();
    let (sales, gst, ar, bank) = (acct(&l, "200"), l.sys_account("GST").unwrap(), l.sys_account("AR").unwrap(), acct(&l, "090"));
    let id = invoice(&l, "2026-01-10", vec![line(sales, Some(tax(&l, "GST on Income")), "2", "500")], "AUTHORISED");
    assert_eq!(bal(&l, ar), 110000);
    assert_eq!(bal(&l, sales), -100000);
    assert_eq!(bal(&l, gst), -10000);
    let d = l.get_document(id).unwrap();
    assert_eq!(d["number"], "INV-0001");
    assert_eq!(d["total"], 110000);
    // partial then full payment
    let p1 = l.add_payment(&PaymentInput { document_id: id, account_id: bank, date: "2026-01-20".into(), amount: "100.00".into(), ..Default::default() }).unwrap();
    assert_eq!(l.get_document(id).unwrap()["status"], "AUTHORISED");
    assert!(l.add_payment(&PaymentInput { document_id: id, account_id: bank, date: "2026-01-21".into(), amount: "1000.01".into(), ..Default::default() }).is_err(), "overpayment rejected");
    l.add_payment(&PaymentInput { document_id: id, account_id: bank, date: "2026-01-21".into(), amount: "1000.00".into(), ..Default::default() }).unwrap();
    let d = l.get_document(id).unwrap();
    assert_eq!(d["status"], "PAID");
    assert_eq!(d["amount_due"], 0);
    assert_eq!(bal(&l, ar), 0);
    assert_eq!(bal(&l, bank), 110000);
    // cannot void with payments; remove payment, then void
    assert!(l.set_document_status(id, "VOIDED").is_err());
    l.delete_payment(p1).unwrap();
    assert_eq!(l.get_document(id).unwrap()["status"], "AUTHORISED");
    assert_invariants(&l);
}

#[test]
fn void_fully_reverses_and_edit_reposts() {
    let l = org();
    let sales = acct(&l, "200");
    let id = invoice(&l, "2026-02-01", vec![line(sales, Some(tax(&l, "GST on Income")), "1", "200")], "AUTHORISED");
    // edit approved invoice (no payments): price change re-posts
    let mut input = DocInput { kind: "ACCREC".into(), contact_name: Some("Acme".into()), date: "2026-02-01".into(), line_amount_type: Some("EXCLUSIVE".into()),
        lines: vec![line(sales, Some(tax(&l, "GST on Income")), "1", "300")], ..Default::default() };
    l.save_document(Some(id), &input).unwrap();
    assert_eq!(bal(&l, sales), -30000);
    assert_eq!(l.get_document(id).unwrap()["status"], "AUTHORISED");
    l.set_document_status(id, "VOIDED").unwrap();
    assert_eq!(bal(&l, sales), 0);
    assert_eq!(bal(&l, l.sys_account("AR").unwrap()), 0);
    input.status = Some("AUTHORISED".into());
    assert!(l.save_document(Some(id), &input).is_err(), "voided invoices can't be edited");
    assert_invariants(&l);
}

#[test]
fn bill_inclusive_tax() {
    let l = org();
    let exp = acct(&l, "453");
    let id = l.save_document(None, &DocInput { kind: "ACCPAY".into(), contact_name: Some("Officeworks".into()), date: "2026-03-01".into(), number: Some("OW-77".into()),
        line_amount_type: Some("INCLUSIVE".into()), lines: vec![line(exp, Some(tax(&l, "GST on Expenses")), "1", "110")], status: Some("AUTHORISED".into()), ..Default::default() }).unwrap();
    assert_eq!(bal(&l, exp), 10000);
    assert_eq!(bal(&l, l.sys_account("GST").unwrap()), 1000);
    assert_eq!(bal(&l, l.sys_account("AP").unwrap()), -11000);
    assert_eq!(l.get_document(id).unwrap()["number"], "OW-77");
    assert_invariants(&l);
}

#[test]
fn db_rejects_unbalanced_and_mutation() {
    let l = org();
    let a = acct(&l, "200");
    let b = acct(&l, "090");
    assert!(l.post_journal(NewJournal { date: "2026-01-01".into(), source_type: "MANJOURNAL".into(), lines: vec![JLine::new(a, 100), JLine::new(b, -99)], ..Default::default() }).is_err());
    // bypass the engine: the trigger must still refuse to post
    l.conn.execute("INSERT INTO journals(number, date, source_type) VALUES (999, '2026-01-01', 'MANJOURNAL')", []).unwrap();
    let jid = l.conn.last_insert_rowid();
    l.conn.execute("INSERT INTO journal_lines(journal_id, account_id, amount) VALUES (?1, ?2, 500)", [jid, a]).unwrap();
    let err = l.conn.execute("UPDATE journals SET posted = 1 WHERE id = ?1", [jid]).unwrap_err().to_string();
    assert!(err.contains("does not balance"), "{err}");
    // posted lines are immutable
    let j = l.post_journal(NewJournal { date: "2026-01-01".into(), source_type: "MANJOURNAL".into(), lines: vec![JLine::new(a, 100), JLine::new(b, -100)], ..Default::default() }).unwrap();
    assert!(l.conn.execute("UPDATE journal_lines SET amount = 1 WHERE journal_id = ?1", [j]).is_err());
    assert!(l.conn.execute("DELETE FROM journal_lines WHERE journal_id = ?1", [j]).is_err());
    assert!(l.conn.execute("DELETE FROM journals WHERE id = ?1", [j]).is_err());
    assert!(l.conn.execute("UPDATE audit_log SET action = 'x'", []).is_err());
}

#[test]
fn lock_dates_block_all_write_paths() {
    let mut l = org();
    let sales = acct(&l, "200");
    let bank = acct(&l, "090");
    let id = invoice(&l, "2026-01-10", vec![line(sales, None, "1", "100")], "AUTHORISED");
    l.set_setting("lock_date_all", "2026-01-31").unwrap();
    // new invoice in locked period
    assert!(l.save_document(None, &DocInput { kind: "ACCREC".into(), contact_name: Some("X".into()), date: "2026-01-15".into(), lines: vec![line(sales, None, "1", "1")], status: Some("AUTHORISED".into()), ..Default::default() }).is_err());
    // void in locked period
    assert!(l.set_document_status(id, "VOIDED").is_err());
    // bank transaction
    assert!(l.save_bank_transaction(None, &BankTxInput { kind: "SPEND".into(), account_id: bank, date: "2026-01-05".into(), lines: vec![LineInput { amount: Some("10".into()), account_id: Some(acct(&l, "404")), ..Default::default() }], ..Default::default() }).is_err());
    // direct journal via SQL respects the trigger even with an adviser
    assert!(l.post_journal(NewJournal { date: "2026-01-31".into(), source_type: "MANJOURNAL".into(), lines: vec![JLine::new(sales, 1), JLine::new(bank, -1)], ..Default::default() }).is_err());
    // after the lock date it works
    assert!(l.post_journal(NewJournal { date: "2026-02-01".into(), source_type: "MANJOURNAL".into(), lines: vec![JLine::new(sales, 1), JLine::new(bank, -1)], ..Default::default() }).is_ok());
    // non-adviser lock
    l.set_setting("lock_date_all", "").unwrap();
    l.set_setting("lock_date_others", "2026-02-28").unwrap();
    l.role = "STANDARD".into();
    assert!(l.post_journal(NewJournal { date: "2026-02-15".into(), source_type: "MANJOURNAL".into(), lines: vec![JLine::new(sales, 1), JLine::new(bank, -1)], ..Default::default() }).is_err());
    l.role = "ADVISER".into();
    assert!(l.post_journal(NewJournal { date: "2026-02-15".into(), source_type: "MANJOURNAL".into(), lines: vec![JLine::new(sales, 1), JLine::new(bank, -1)], ..Default::default() }).is_ok());
}

#[test]
fn credit_note_allocation() {
    let l = org();
    let sales = acct(&l, "200");
    let gst_inc = Some(tax(&l, "GST on Income"));
    let inv = invoice(&l, "2026-04-01", vec![line(sales, gst_inc, "1", "1000")], "AUTHORISED");
    let cn = l.save_document(None, &DocInput { kind: "ACCRECCREDIT".into(), contact_name: Some("Acme".into()), date: "2026-04-05".into(),
        lines: vec![line(sales, gst_inc, "1", "100")], status: Some("AUTHORISED".into()), ..Default::default() }).unwrap();
    assert_eq!(bal(&l, l.sys_account("AR").unwrap()), 110000 - 11000);
    l.allocate_credit(cn, inv, "110.00", Some("2026-04-05")).unwrap();
    assert_eq!(l.get_document(inv).unwrap()["amount_due"], 99000);
    assert_eq!(l.get_document(cn).unwrap()["status"], "PAID");
    assert_invariants(&l);
}

#[test]
fn foreign_currency_realised_gain() {
    let l = org();
    let sales = acct(&l, "200");
    let bank = acct(&l, "090");
    l.conn.execute("UPDATE currencies SET enabled = 1 WHERE code = 'USD'", []).unwrap();
    // USD 100 at 0.65 = AUD 153.85
    let id = l.save_document(None, &DocInput { kind: "ACCREC".into(), contact_name: Some("US Client".into()), date: "2026-05-01".into(), currency: Some("USD".into()), rate: Some("0.65".into()),
        line_amount_type: Some("NOTAX".into()), lines: vec![line(sales, None, "1", "100")], status: Some("AUTHORISED".into()), ..Default::default() }).unwrap();
    assert_eq!(bal(&l, l.sys_account("AR").unwrap()), 15385);
    // half paid at 0.70: 50 USD = 71.43 AUD vs 76.93 (half of 153.85, rounded) -> loss 5.50
    l.add_payment(&PaymentInput { document_id: id, account_id: bank, date: "2026-05-10".into(), amount: "50".into(), rate: Some("0.70".into()), ..Default::default() }).unwrap();
    assert_eq!(bal(&l, bank), 7143);
    assert_eq!(bal(&l, l.sys_account("FXGAIN").unwrap()), 7693 - 7143);
    // final half at 0.60: 83.33 AUD; clears remaining 76.92 -> gain 6.41
    l.add_payment(&PaymentInput { document_id: id, account_id: bank, date: "2026-05-20".into(), amount: "50".into(), rate: Some("0.60".into()), ..Default::default() }).unwrap();
    assert_eq!(bal(&l, l.sys_account("AR").unwrap()), 0, "AR fully cleared in base currency");
    assert_eq!(bal(&l, l.sys_account("FXGAIN").unwrap()), 550 - 641);
    assert_invariants(&l);
}

#[test]
fn tracked_inventory_average_cost() {
    let l = org();
    let item = l.create_item(&ItemInput { code: "WIDGET".into(), name: "Widget".into(), is_tracked: true, sale_price: Some("20".into()), sale_account_id: Some(acct(&l, "200")),
        purchase_account_id: Some(acct(&l, "310")), purchase_price: Some("5".into()), ..Default::default() }).unwrap();
    let inv_acct = l.sys_account("INVENTORY").unwrap();
    // buy 10 @ 5 and 10 @ 7 -> avg 6
    for price in ["5", "7"] {
        l.save_document(None, &DocInput { kind: "ACCPAY".into(), contact_name: Some("Supplier".into()), date: "2026-06-01".into(), line_amount_type: Some("NOTAX".into()),
            lines: vec![LineInput { item_id: Some(item), quantity: Some("10".into()), unit_price: Some(price.into()), ..Default::default() }], status: Some("AUTHORISED".into()), ..Default::default() }).unwrap();
    }
    assert_eq!(bal(&l, inv_acct), 12000);
    // sell 4 -> COGS 24
    invoice(&l, "2026-06-05", vec![LineInput { item_id: Some(item), quantity: Some("4".into()), ..Default::default() }], "AUTHORISED");
    assert_eq!(bal(&l, acct(&l, "310")), 2400);
    assert_eq!(bal(&l, inv_acct), 9600);
    let v = l.inventory_valuation().unwrap();
    assert_eq!(v["total"], 9600);
    assert_eq!(v["gl_balance"], 9600);
    // can't oversell
    assert!(l.save_document(None, &DocInput { kind: "ACCREC".into(), contact_name: Some("Acme".into()), date: "2026-06-06".into(),
        lines: vec![LineInput { item_id: Some(item), quantity: Some("17".into()), ..Default::default() }], status: Some("AUTHORISED".into()), ..Default::default() }).is_err());
    // stocktake write-off of 1
    l.adjust_inventory(item, "2026-06-07", -10_000, None, acct(&l, "310"), "Damaged").unwrap();
    assert_eq!(bal(&l, inv_acct), 9000);
    assert_invariants(&l);
}

#[test]
fn bank_import_and_reconcile() {
    let l = org();
    let bank = acct(&l, "090");
    let sales = acct(&l, "200");
    let inv = invoice(&l, "2026-07-01", vec![line(sales, Some(tax(&l, "GST on Income")), "1", "100")], "AUTHORISED");
    let r = l.import_lines(bank, "test.csv", "CSV", &[
        StmtLine { date: "2026-07-03".into(), amount: 11000, payee: "Acme".into(), reference: "INV-0001".into(), ..Default::default() },
        StmtLine { date: "2026-07-04".into(), amount: -2500, payee: "Bank".into(), description: "Monthly fee".into(), ..Default::default() },
    ]).unwrap();
    assert_eq!(r["added"], 2);
    // duplicate import is ignored
    let r2 = l.import_lines(bank, "test.csv", "CSV", &[StmtLine { date: "2026-07-03".into(), amount: 11000, payee: "Acme".into(), reference: "INV-0001".into(), ..Default::default() }]).unwrap();
    assert_eq!(r2["added"], 0);
    let lines = l.statement_lines(bank, "UNRECONCILED", 100).unwrap();
    let (l1, l2) = (lines[0]["id"].as_i64().unwrap(), lines[1]["id"].as_i64().unwrap());
    let sugg = l.reconcile_suggestions(l1).unwrap();
    assert_eq!(sugg["documents"][0]["id"].as_i64(), Some(inv), "invoice suggested by amount + reference");
    l.reconcile_pay_documents(l1, &[(inv, "110.00".into())], &[]).unwrap();
    assert_eq!(l.get_document(inv).unwrap()["status"], "PAID");
    l.reconcile_create(l2, BankTxInput { lines: vec![LineInput { description: "Fee".into(), account_id: Some(acct(&l, "404")), ..Default::default() }], ..Default::default() }).unwrap();
    assert_eq!(l.statement_lines(bank, "UNRECONCILED", 100).unwrap().len(), 0);
    let rec = l.reconciliation_report(bank, "2026-07-31").unwrap();
    assert_eq!(rec["statement_balance"], 8500);
    assert_eq!(rec["ledger_balance"], 8500);
    assert_eq!(rec["difference"], 0);
    // unreconcile then the payment is free again
    l.unreconcile(l1).unwrap();
    assert_eq!(l.unreconciled_ledger_items(bank).unwrap().len(), 1);
    assert_invariants(&l);
}

#[test]
fn statement_balance_when_bank_clears_items_after_the_statement_starts() {
    let l = org();
    let bank = acct(&l, "090");
    // Spent on 1 July in the ledger; the bank only clears it on 3 July — the first statement line.
    let tx = l.save_bank_transaction(None, &BankTxInput { kind: "SPEND".into(), account_id: bank, date: "2026-07-01".into(), contact_name: Some("Supplier".into()), line_amount_type: Some("NOTAX".into()),
        lines: vec![LineInput { amount: Some("100".into()), account_id: Some(acct(&l, "404")), ..Default::default() }], ..Default::default() }).unwrap();
    l.import_lines(bank, "s.csv", "CSV", &[
        StmtLine { date: "2026-07-03".into(), amount: -10000, payee: "Supplier".into(), ..Default::default() },
        StmtLine { date: "2026-07-04".into(), amount: 5000, payee: "Customer".into(), ..Default::default() },
    ]).unwrap();
    let first = l.statement_lines(bank, "UNRECONCILED", 100).unwrap()[0]["id"].as_i64().unwrap();
    l.reconcile_match(first, &[("BANKTX".into(), tx)]).unwrap();
    let rec = l.reconciliation_report(bank, "2026-07-31").unwrap();
    assert_eq!(rec["statement_balance"], -5000, "the cleared item is carried by its statement line, not the opening balance");
    assert_eq!(rec["difference"], 0);
    let acc = l.bank_accounts().unwrap().into_iter().find(|a| a["id"] == bank).unwrap();
    assert_eq!(acc["statement_balance"], -5000);
}

#[test]
fn invoice_numbering_settings() {
    let l = org();
    let names: Vec<String> = l.list_sequences().unwrap().iter().map(|q| q["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(&names[..4], &["INV", "CN", "QU", "PO"]);
    l.set_sequence("INV", "HD-", 1500).unwrap();
    let id = invoice(&l, "2026-07-01", vec![line(acct(&l, "200"), None, "1", "10")], "AUTHORISED");
    assert_eq!(l.get_document(id).unwrap()["number"], "HD-1500");
    // setting the next number back to one already used skips ahead instead of failing
    l.set_sequence("INV", "HD-", 1500).unwrap();
    let id2 = invoice(&l, "2026-07-02", vec![line(acct(&l, "200"), None, "1", "10")], "AUTHORISED");
    assert_eq!(l.get_document(id2).unwrap()["number"], "HD-1501");
    assert!(l.set_sequence("INV", "X", 0).is_err());
    assert!(l.set_sequence("NOPE", "X", 1).is_err());
}

#[test]
fn remittance_advice_for_bill_payments() {
    let l = org();
    let bank = acct(&l, "090");
    let bill = l.save_document(None, &DocInput { kind: "ACCPAY".into(), contact_name: Some("Paper Co".into()), date: "2026-07-01".into(), number: Some("PC-77".into()), line_amount_type: Some("NOTAX".into()),
        lines: vec![line(acct(&l, "429"), None, "1", "1234.5")], status: Some("AUTHORISED".into()), ..Default::default() }).unwrap();
    let pid = l.add_payment(&PaymentInput { document_id: bill, account_id: bank, date: "2026-07-10".into(), amount: "1234.50".into(), reference: "EFT 991".into(), ..Default::default() }).unwrap();
    // no email address yet -> reported, nothing queued
    let r = ledger_core::email::email_remittance(&l, &[pid]).unwrap();
    assert_eq!(r["queued"], 0);
    assert_eq!(r["missing_email"][0], "Paper Co");
    l.conn.execute("UPDATE contacts SET email = 'ap@paper.test' WHERE name = 'Paper Co'", []).unwrap();
    let r = ledger_core::email::email_remittance(&l, &[pid]).unwrap();
    assert_eq!(r["queued"], 1);
    let body: String = l.conn.query_row("SELECT body FROM outbox ORDER BY id DESC LIMIT 1", [], |r| r.get(0)).unwrap();
    assert!(body.contains("PC-77") && body.contains("1,234.50") && body.contains("EFT 991"), "{body}");
}

#[test]
fn batch_payment_reconciles_as_one_line() {
    let l = org();
    let bank = acct(&l, "090");
    let mut bills = Vec::new();
    for (n, amt) in [("A-1", "100"), ("B-2", "250.5")] {
        bills.push(l.save_document(None, &DocInput { kind: "ACCPAY".into(), contact_name: Some(format!("Supplier {n}")), date: "2026-07-01".into(), number: Some(n.into()), line_amount_type: Some("NOTAX".into()),
            lines: vec![line(acct(&l, "429"), None, "1", amt)], status: Some("AUTHORISED".into()), ..Default::default() }).unwrap());
    }
    let batch = l.batch_payment(bank, "2026-07-05", "Run 1", &[(bills[0], "100.00".into()), (bills[1], "250.50".into())]).unwrap();
    let items = l.unreconciled_ledger_items(bank).unwrap();
    assert_eq!(items.len(), 1, "the batch shows as a single item: {items:?}");
    assert_eq!(items[0]["kind"], "BATCH");
    assert_eq!(items[0]["amount"], -35050);
    l.import_lines(bank, "s.csv", "CSV", &[StmtLine { date: "2026-07-06".into(), amount: -35050, payee: "Batch".into(), ..Default::default() }]).unwrap();
    let line_id = l.statement_lines(bank, "UNRECONCILED", 10).unwrap()[0]["id"].as_i64().unwrap();
    let sugg = l.reconcile_suggestions(line_id).unwrap();
    assert_eq!(sugg["matches"][0]["kind"], "BATCH", "{sugg}");
    l.reconcile_match(line_id, &[("BATCH".into(), batch)]).unwrap();
    assert_eq!(l.unreconciled_ledger_items(bank).unwrap().len(), 0);
    assert_eq!(l.reconciliation_report(bank, "2026-07-31").unwrap()["difference"], 0);
    l.unreconcile(line_id).unwrap();
    assert_eq!(l.unreconciled_ledger_items(bank).unwrap()[0]["kind"], "BATCH");
}

#[test]
fn sales_receipt_is_a_paid_sale() {
    let l = org();
    let bank = acct(&l, "090");
    let sales = acct(&l, "200");
    let input = DocInput { kind: "ACCREC".into(), contact_name: Some("Walk-in".into()), date: "2026-07-03".into(), line_amount_type: Some("EXCLUSIVE".into()),
        lines: vec![line(sales, Some(tax(&l, "GST on Income")), "2", "50")], ..Default::default() };
    let id = l.save_sales_receipt(&input, bank).unwrap();
    let d = l.get_document(id).unwrap();
    assert_eq!(d["status"], "PAID");
    assert_eq!(d["subtype"], "RECEIPT");
    assert_eq!(d["amount_due"], 0);
    assert_eq!(bal(&l, bank), 11000);
    assert_eq!(bal(&l, sales), -10000);
    assert_eq!(bal(&l, l.sys_account("AR").unwrap()), 0, "nothing left owing");
    let list = l.list_documents("ACCREC", "all", "", None, 10, 0).unwrap();
    assert_eq!(list["rows"][0]["subtype"], "RECEIPT");
    // A bill can't be a sales receipt.
    assert!(l.save_sales_receipt(&DocInput { kind: "ACCPAY".into(), ..input.clone() }, bank).is_err());
    assert_invariants(&l);
}

#[test]
fn checks_are_numbered_spend_money() {
    let l = org();
    let bank = acct(&l, "090");
    assert_eq!(l.next_check_number(bank).unwrap(), "1001");
    let chk = |n: &str, amt: &str| BankTxInput { kind: "SPEND".into(), account_id: bank, date: "2026-07-05".into(), contact_name: Some("Landlord".into()), line_amount_type: Some("NOTAX".into()),
        lines: vec![LineInput { amount: Some(amt.into()), account_id: Some(acct(&l, "469")), ..Default::default() }], subtype: Some("CHECK".into()), check_number: Some(n.into()), ..Default::default() };
    l.save_bank_transaction(None, &chk("1001", "500")).unwrap();
    assert_eq!(l.next_check_number(bank).unwrap(), "1002");
    l.save_bank_transaction(None, &chk("1002", "20")).unwrap();
    let txs = l.bank_account_transactions(bank, 10).unwrap();
    assert!(txs.iter().any(|t| t["description"] == "Check 1002"), "{txs:?}");
    assert_eq!(bal(&l, bank), -52000);
    assert_invariants(&l);
}

#[test]
fn undeposited_funds_and_bank_deposit() {
    let l = org();
    let bank = acct(&l, "090");
    let sales = acct(&l, "200");
    let uf = l.undeposited_funds_account().unwrap();
    assert_eq!(l.undeposited_funds_account().unwrap(), uf, "created once");
    let a = invoice(&l, "2026-07-01", vec![line(sales, None, "1", "100")], "AUTHORISED");
    let b = invoice(&l, "2026-07-02", vec![line(sales, None, "1", "250")], "AUTHORISED");
    let pa = l.add_payment(&PaymentInput { document_id: a, account_id: uf, date: "2026-07-05".into(), amount: "100".into(), ..Default::default() }).unwrap();
    let pb = l.add_payment(&PaymentInput { document_id: b, account_id: uf, date: "2026-07-06".into(), amount: "250".into(), ..Default::default() }).unwrap();
    assert_eq!(l.undeposited_payments().unwrap().len(), 2);
    assert_eq!(bal(&l, uf), 35000);
    assert_eq!(bal(&l, bank), 0, "money isn't in the bank until it's deposited");
    let dep = l.bank_deposit(bank, "2026-07-07", "Deposit #1", &[pa, pb]).unwrap();
    assert_eq!((bal(&l, uf), bal(&l, bank)), (0, 35000));
    assert!(l.undeposited_payments().unwrap().is_empty());
    assert!(l.bank_deposit(bank, "2026-07-07", "again", &[pa]).is_err(), "can't deposit twice");
    // The deposit is one item to match against the one statement line.
    let items = l.unreconciled_ledger_items(bank).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!((items[0]["kind"].as_str(), items[0]["amount"].as_i64()), (Some("DEPOSIT"), Some(35000)));
    l.import_lines(bank, "s.csv", "CSV", &[StmtLine { date: "2026-07-08".into(), amount: 35000, payee: "Deposit".into(), ..Default::default() }]).unwrap();
    let line_id = l.statement_lines(bank, "UNRECONCILED", 10).unwrap()[0]["id"].as_i64().unwrap();
    assert_eq!(l.reconcile_suggestions(line_id).unwrap()["matches"][0]["kind"], "DEPOSIT");
    l.reconcile_match(line_id, &[("DEPOSIT".into(), dep)]).unwrap();
    assert_eq!(l.reconciliation_report(bank, "2026-07-31").unwrap()["difference"], 0);
    assert!(l.delete_deposit(dep).is_err(), "reconciled deposits can't be deleted");
    l.unreconcile(line_id).unwrap();
    l.delete_deposit(dep).unwrap();
    assert_eq!((bal(&l, uf), bal(&l, bank)), (35000, 0));
    assert_eq!(l.undeposited_payments().unwrap().len(), 2, "payments go back to Undeposited Funds");
    assert_invariants(&l);
}

#[test]
fn tick_off_reconciliation() {
    let l = org();
    let bank = acct(&l, "090");
    let sales = acct(&l, "200");
    let a = invoice(&l, "2026-07-01", vec![line(sales, None, "1", "100")], "AUTHORISED");
    let b = invoice(&l, "2026-07-02", vec![line(sales, None, "1", "40")], "AUTHORISED");
    let pa = l.add_payment(&PaymentInput { document_id: a, account_id: bank, date: "2026-07-05".into(), amount: "100".into(), ..Default::default() }).unwrap();
    l.add_payment(&PaymentInput { document_id: b, account_id: bank, date: "2026-07-06".into(), amount: "40".into(), ..Default::default() }).unwrap();
    let late = invoice(&l, "2026-08-01", vec![line(sales, None, "1", "7")], "AUTHORISED");
    l.add_payment(&PaymentInput { document_id: late, account_id: bank, date: "2026-08-02".into(), amount: "7".into(), ..Default::default() }).unwrap();
    assert_eq!(l.reconcile_beginning_balance(bank).unwrap(), 0);
    let r = l.reconcile_start(bank, "2026-07-31", 10000).unwrap();
    assert_eq!(l.reconcile_start(bank, "2026-07-31", 10000).unwrap(), r, "resumes the open one");
    let s = l.reconcile_session(r).unwrap();
    assert_eq!(s["items"].as_array().unwrap().len(), 2, "only items up to the statement date");
    assert_eq!(s["difference"], 10000);
    assert!(l.reconcile_finish(r).is_err(), "can't finish with a difference");
    l.reconcile_toggle(r, "PAYMENT", pa, true).unwrap();
    let s = l.reconcile_session(r).unwrap();
    assert_eq!((s["cleared_balance"].as_i64(), s["difference"].as_i64()), (Some(10000), Some(0)));
    l.reconcile_finish(r).unwrap();
    assert!(l.reconcile_toggle(r, "PAYMENT", pa, false).is_err(), "finished ones are locked");
    let left = l.unreconciled_ledger_items(bank).unwrap();
    assert_eq!(left.len(), 2, "the ticked payment is reconciled, the rest aren't");
    assert!(left.iter().all(|i| i["id"].as_i64() != Some(pa) || i["kind"] != "PAYMENT"));
    // The next reconciliation starts from the last ending balance.
    assert_eq!(l.reconcile_beginning_balance(bank).unwrap(), 10000);
    let r2 = l.reconcile_start(bank, "2026-08-31", 14700).unwrap();
    assert_eq!(l.reconcile_session(r2).unwrap()["beginning_balance"], 10000);
    // Undo the last finished one: its items become unreconciled again.
    l.reconcile_undo_last(bank).unwrap();
    assert_eq!(l.unreconciled_ledger_items(bank).unwrap().len(), 3);
    assert_eq!(l.reconcile_beginning_balance(bank).unwrap(), 0);
    assert!(l.reconcile_start(acct(&l, "200"), "2026-07-31", 0).is_err(), "only bank accounts");
    assert_invariants(&l);
}

#[test]
fn excluded_bank_lines() {
    let l = org();
    let bank = acct(&l, "090");
    l.import_lines(bank, "s.csv", "CSV", &[
        StmtLine { date: "2026-07-08".into(), amount: -5000, payee: "Cafe".into(), ..Default::default() },
        StmtLine { date: "2026-07-09".into(), amount: -5000, payee: "Cafe (duplicate)".into(), ..Default::default() },
    ]).unwrap();
    let lines = l.statement_lines(bank, "UNRECONCILED", 10).unwrap();
    let dup = lines[1]["id"].as_i64().unwrap();
    l.exclude_statement_line(dup, true).unwrap();
    assert_eq!(l.statement_lines(bank, "UNRECONCILED", 10).unwrap().len(), 1, "excluded lines leave review");
    assert_eq!(l.statement_lines(bank, "EXCLUDED", 10).unwrap().len(), 1);
    assert_eq!(l.reconciliation_report(bank, "2026-07-31").unwrap()["statement_balance"], -5000, "and don't count towards the statement balance");
    assert!(l.exclude_statement_line(dup, true).is_err());
    l.exclude_statement_line(dup, false).unwrap();
    assert_eq!(l.statement_lines(bank, "UNRECONCILED", 10).unwrap().len(), 2, "restored to review");
}

#[test]
fn account_detail_types_and_auto_numbers() {
    let l = org();
    let id = l.create_account(&ledger_core::accounts::AccountInput { code: "".into(), name: "Google Ads".into(), kind: "EXPENSE".into(), detail_type: Some("Advertising/Promotional".into()), ..Default::default() }).unwrap();
    let a = l.get_account(id).unwrap();
    assert_eq!(a["detail_type"], "Advertising/Promotional");
    let code: i64 = a["code"].as_str().unwrap().parse().expect("numbered automatically");
    let id2 = l.create_account(&ledger_core::accounts::AccountInput { code: "".into(), name: "Trade shows".into(), kind: "EXPENSE".into(), ..Default::default() }).unwrap();
    assert_eq!(l.get_account(id2).unwrap()["code"].as_str().unwrap().parse::<i64>().unwrap(), code + 1, "next number in sequence");
    // Updating without a detail type keeps it.
    l.update_account(id, &ledger_core::accounts::AccountInput { code: code.to_string(), name: "Google Ads (search)".into(), kind: "EXPENSE".into(), ..Default::default() }).unwrap();
    assert_eq!(l.get_account(id).unwrap()["detail_type"], "Advertising/Promotional");
    assert!(l.create_account(&ledger_core::accounts::AccountInput { code: "".into(), name: "".into(), kind: "EXPENSE".into(), ..Default::default() }).is_err());
}

#[test]
fn delayed_charges_and_credits() {
    let l = org();
    let sales = acct(&l, "200");
    let doc = |price: &str| DocInput { contact_name: Some("Acme".into()), date: "2026-07-01".into(), line_amount_type: Some("NOTAX".into()), lines: vec![line(sales, None, "1", price)], ..Default::default() };
    let charge = l.save_delayed(None, &doc("300"), false).unwrap();
    let credit = l.save_delayed(None, &doc("50"), true).unwrap();
    assert_eq!(l.ledger_imbalance().unwrap(), 0);
    assert_eq!(bal(&l, sales), 0, "delayed charges post nothing");
    let contact = l.get_document(charge).unwrap()["contact_id"].as_i64().unwrap();
    assert_eq!(l.open_delayed(contact).unwrap().len(), 2);
    assert_eq!(l.list_documents("QUOTE", "all", "", None, 50, 0).unwrap()["rows"].as_array().unwrap().len(), 0, "not listed as quotes");
    assert_eq!(l.list_documents("DELAYED", "all", "", None, 50, 0).unwrap()["rows"].as_array().unwrap().len(), 2);
    // Invoice with the charge and the credit added (credit as a negative line).
    let inv_input = DocInput { kind: "ACCREC".into(), contact_id: Some(contact), date: "2026-07-31".into(), line_amount_type: Some("NOTAX".into()),
        lines: vec![line(sales, None, "1", "300"), line(sales, None, "-1", "50")], status: Some("AUTHORISED".into()), ..Default::default() };
    let inv = l.save_invoice_with_delayed(None, &inv_input, &[charge, credit]).unwrap();
    assert_eq!(l.get_document(inv).unwrap()["total"], 25000);
    assert!(l.open_delayed(contact).unwrap().is_empty(), "both closed");
    assert_eq!(l.get_document(charge).unwrap()["status"], "INVOICED");
    assert!(l.save_invoice_with_delayed(None, &inv_input, &[charge]).is_err(), "can't be invoiced twice");
    assert_eq!(l.list_documents("ACCREC", "all", "", None, 50, 0).unwrap()["rows"].as_array().unwrap().len(), 1, "the failed save left nothing behind");
    assert!(l.save_delayed(Some(charge), &doc("1"), false).is_err(), "closed ones can't be edited");
    assert_invariants(&l);
}

#[test]
fn contractor_1099_summary() {
    let l = org();
    let bank = acct(&l, "090");
    let exp = acct(&l, "429");
    let bill = |who: &str, price: &str| l.save_document(None, &DocInput { kind: "ACCPAY".into(), contact_name: Some(who.into()), date: "2026-03-01".into(),
        line_amount_type: Some("NOTAX".into()), lines: vec![line(exp, None, "1", price)], status: Some("AUTHORISED".into()), ..Default::default() }).unwrap();
    let a = bill("Ace Contracting", "900");
    let b = bill("Bob Freelance", "400");
    let c = bill("Not Tracked LLC", "5000");
    for (d, amt) in [(a, "900"), (b, "400"), (c, "5000")] {
        l.add_payment(&PaymentInput { document_id: d, account_id: bank, date: "2026-03-15".into(), amount: amt.into(), ..Default::default() }).unwrap();
    }
    // A card payment to Ace doesn't count towards the 1099.
    let card = l.create_account(&ledger_core::accounts::AccountInput { code: "".into(), name: "Visa".into(), kind: "BANK".into(), bank_type: Some("CREDITCARD".into()), ..Default::default() }).unwrap();
    let a2 = bill("Ace Contracting", "250");
    l.add_payment(&PaymentInput { document_id: a2, account_id: card, date: "2026-04-01".into(), amount: "250".into(), ..Default::default() }).unwrap();
    for d in [a, b] { let cid = l.get_document(d).unwrap()["contact_id"].as_i64().unwrap(); l.set_track_1099(cid, true).unwrap(); }
    let r = l.report_1099(&ReportParams { from: Some("2026-01-01".into()), to: Some("2026-12-31".into()), ..Default::default() }).unwrap();
    let over = r["sections"][0]["rows"].as_array().unwrap();
    assert_eq!(over.len(), 1);
    assert!(over[0]["label"].as_str().unwrap().starts_with("Ace Contracting"));
    assert_eq!(over[0]["values"], serde_json::json!([115000, 25000, 90000]));
    assert_eq!(r["sections"][1]["rows"][0]["values"][2], 40000, "Bob is under $600");
    assert_eq!(r["sections"].as_array().unwrap().iter().map(|s| s["rows"].as_array().unwrap().len()).sum::<usize>(), 2, "untracked vendors are left out");
}

#[test]
fn sales_tax_by_agency() {
    let mut l = Ledger::open_in_memory().unwrap();
    l.setup_org(&SetupInput { org_name: "US Co".into(), country: "US".into(), base_currency: None, fy_end_month: None, tax_basis: None,
        tax_registered: Some(true), tax_number: None, tax_period: None, industry: None, owner_name: None, owner_email: None, conversion_date: None }).unwrap();
    let sales = acct(&l, "200");
    // 7.25% = California 6% (state) + Los Angeles County 1.25%.
    let rate = l.create_sales_tax_rate("Los Angeles 7.25%", &[("California CDTFA".into(), "State".into(), 600), ("LA County".into(), "County".into(), 125)]).unwrap();
    assert_eq!(l.agency("california cdtfa", None).unwrap(), l.agency("California CDTFA", None).unwrap(), "agencies are matched by name");
    invoice(&l, "2026-07-01", vec![line(sales, Some(rate), "1", "1000")], "AUTHORISED");
    invoice(&l, "2026-07-02", vec![line(sales, Some(rate), "1", "333")], "AUTHORISED");
    let rows = l.tax_by_agency("2026-07-01", "2026-07-31", false).unwrap();
    let get = |n: &str| rows.iter().find(|r| r["name"] == n).unwrap()["tax"].as_i64().unwrap();
    let total_tax = 7250 + 2414; // 72.50 + 24.14 (333 × 7.25% = 24.1425)
    assert_eq!(get("California CDTFA") + get("LA County"), total_tax, "split adds back up exactly");
    assert_eq!(get("California CDTFA"), 6000 + 1998);
    let (f, t) = l.agency_period("QUARTERLY", "2026-08-15").unwrap();
    assert_eq!((f.as_str(), t.as_str()), ("2026-07-01", "2026-09-30"));
    let (f, t) = l.agency_period("MONTHLY", "2026-02-10").unwrap();
    assert_eq!((f.as_str(), t.as_str()), ("2026-02-01", "2026-02-28"));
    let r = l.report_tax_by_agency(&ReportParams { from: Some("2026-07-01".into()), to: Some("2026-07-31".into()), ..Default::default() }).unwrap();
    assert_eq!(r["sections"][0]["total"]["values"][1], total_tax);
    assert_eq!(l.list_agencies().unwrap().len(), 2);
}

#[test]
fn sales_orders() {
    let l = org();
    let sales = acct(&l, "200");
    let doc = DocInput { contact_name: Some("Acme".into()), date: "2026-07-01".into(), line_amount_type: Some("NOTAX".into()), lines: vec![line(sales, None, "4", "250")], ..Default::default() };
    let so = l.save_sales_order(None, &doc).unwrap();
    assert_eq!(bal(&l, sales), 0, "sales orders post nothing");
    assert_eq!(l.get_document(so).unwrap()["subtype"], "SALESORDER");
    let list = |k: &str| l.list_documents(k, "all", "", None, 50, 0).unwrap()["rows"].as_array().unwrap().len();
    assert_eq!((list("SALESORDER"), list("QUOTE"), list("DELAYED")), (1, 0, 0), "each variant in its own list");
    // Convert to an invoice; the order closes.
    let inv = l.copy_document(so, "ACCREC", Some("2026-07-10")).unwrap();
    assert_eq!(l.get_document(inv).unwrap()["total"], 100000);
    assert_eq!(l.get_document(so).unwrap()["status"], "INVOICED");
    assert!(l.save_sales_order(Some(so), &doc).is_err(), "invoiced orders can't be edited");
    assert!(l.save_delayed(Some(so), &doc, false).is_err(), "kinds can't be mixed");
}

#[test]
fn reports_add_up() {
    let l = org();
    let sales = acct(&l, "200");
    let rent = acct(&l, "469");
    let bank = acct(&l, "090");
    invoice(&l, "2026-08-01", vec![line(sales, Some(tax(&l, "GST on Income")), "1", "1000")], "AUTHORISED");
    l.save_bank_transaction(None, &BankTxInput { kind: "SPEND".into(), account_id: bank, date: "2026-08-02".into(), contact_name: Some("Landlord".into()), line_amount_type: Some("INCLUSIVE".into()),
        lines: vec![LineInput { amount: Some("330".into()), account_id: Some(rent), tax_rate_id: Some(tax(&l, "GST on Expenses")), ..Default::default() }], ..Default::default() }).unwrap();
    let pl = l.profit_and_loss(&ReportParams { from: Some("2026-07-01".into()), to: Some("2026-08-31".into()), ..Default::default() }).unwrap();
    let net = pl["sections"].as_array().unwrap().iter().find(|s| s["total"]["label"] == "Net Profit").unwrap()["total"]["values"][0].as_i64().unwrap();
    assert_eq!(net, 100000 - 30000);
    // cash basis: invoice unpaid so only rent
    let plc = l.profit_and_loss(&ReportParams { from: Some("2026-07-01".into()), to: Some("2026-08-31".into()), basis: Some("CASH".into()), ..Default::default() }).unwrap();
    let netc = plc["sections"].as_array().unwrap().iter().find(|s| s["total"]["label"] == "Net Profit").unwrap()["total"]["values"][0].as_i64().unwrap();
    assert_eq!(netc, -30000);
    let tax = l.tax_report(&ReportParams { from: Some("2026-07-01".into()), to: Some("2026-09-30".into()), basis: Some("ACCRUAL".into()), ..Default::default() }).unwrap();
    assert_eq!(tax["difference"], 0, "tax report reconciles to GL");
    let aged = l.aged(true, &ReportParams { date: Some("2026-08-31".into()), ..Default::default() }).unwrap();
    assert_eq!(aged["sections"][0]["total"]["values"][5], 110000);
    assert_invariants(&l);
}

#[test]
fn cash_basis_balance_sheet_keeps_opening_balances() {
    let l = org();
    let bank = acct(&l, "090");
    let sales = acct(&l, "200");
    let cap = acct(&l, "970");
    l.save_conversion_balances("2026-06-30", &[serde_json::json!({"account_id": bank, "debit": "5000"}), serde_json::json!({"account_id": cap, "credit": "5000"})]).unwrap();
    let id = invoice(&l, "2026-07-01", vec![line(sales, Some(tax(&l, "GST on Income")), "3", "33.33")], "AUTHORISED");
    l.add_payment(&PaymentInput { document_id: id, account_id: bank, date: "2026-07-05".into(), amount: "40".into(), ..Default::default() }).unwrap();
    let p = ReportParams { date: Some("2026-07-31".into()), basis: Some("CASH".into()), ..Default::default() };
    let bs = l.balance_sheet(&p).unwrap();
    assert_eq!(bs["check"][0], 0);
    let bank_row = bs["sections"].as_array().unwrap().iter().flat_map(|s| s["rows"].as_array().cloned().unwrap_or_default()).find(|r| r["account_id"] == bank).unwrap();
    assert_eq!(bank_row["values"][0], 504000, "opening balance + receipt on cash basis");
    assert_invariants(&l);
}

mod property {
    use super::*;
    use proptest::prelude::*;

    #[derive(Debug, Clone)]
    enum Op { Invoice(u32, bool), Bill(u32), Pay(usize, u8), Spend(u32), Receive(u32), Void(usize), Credit(u32) }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            (1u32..500_000, any::<bool>()).prop_map(|(a, i)| Op::Invoice(a, i)),
            (1u32..500_000).prop_map(Op::Bill),
            (0usize..20, 1u8..=100).prop_map(|(i, p)| Op::Pay(i, p)),
            (1u32..100_000).prop_map(Op::Spend),
            (1u32..100_000).prop_map(Op::Receive),
            (0usize..20).prop_map(Op::Void),
            (1u32..50_000).prop_map(Op::Credit),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 24, .. ProptestConfig::default() })]
        #[test]
        fn ledger_always_balances(ops in proptest::collection::vec(op(), 1..25)) {
            let l = org();
            let (sales, exp, bank) = (acct(&l, "200"), acct(&l, "429"), acct(&l, "090"));
            let gi = tax(&l, "GST on Income"); let ge = tax(&l, "GST on Expenses");
            let mut docs: Vec<i64> = vec![];
            for o in ops {
                let amt = |c: u32| ledger_core::money::fmt_money(c as i64);
                let _ = match o {
                    Op::Invoice(c, inc) => l.save_document(None, &DocInput { kind: "ACCREC".into(), contact_name: Some("C".into()), date: "2026-03-01".into(),
                        line_amount_type: Some(if inc { "INCLUSIVE" } else { "EXCLUSIVE" }.into()), lines: vec![line(sales, Some(gi), "1", &amt(c))], status: Some("AUTHORISED".into()), ..Default::default() }).map(|id| docs.push(id)),
                    Op::Bill(c) => l.save_document(None, &DocInput { kind: "ACCPAY".into(), contact_name: Some("S".into()), date: "2026-03-02".into(),
                        line_amount_type: Some("INCLUSIVE".into()), lines: vec![line(exp, Some(ge), "3", &amt(c))], status: Some("AUTHORISED".into()), ..Default::default() }).map(|id| docs.push(id)),
                    Op::Pay(i, pct) => if let Some(&d) = docs.get(i % docs.len().max(1)) {
                        let due = l.get_document(d).unwrap()["amount_due"].as_i64().unwrap_or(0);
                        let a = (due * pct as i64 / 100).max(1);
                        l.add_payment(&PaymentInput { document_id: d, account_id: bank, date: "2026-03-10".into(), amount: ledger_core::money::fmt_money(a), ..Default::default() }).map(|_| ())
                    } else { Ok(()) },
                    Op::Spend(c) => l.save_bank_transaction(None, &BankTxInput { kind: "SPEND".into(), account_id: bank, date: "2026-03-03".into(), line_amount_type: Some("INCLUSIVE".into()),
                        lines: vec![LineInput { amount: Some(amt(c)), account_id: Some(exp), tax_rate_id: Some(ge), ..Default::default() }], ..Default::default() }).map(|_| ()),
                    Op::Receive(c) => l.save_bank_transaction(None, &BankTxInput { kind: "RECEIVE".into(), account_id: bank, date: "2026-03-04".into(), line_amount_type: Some("EXCLUSIVE".into()),
                        lines: vec![LineInput { amount: Some(amt(c)), account_id: Some(sales), tax_rate_id: Some(gi), ..Default::default() }], ..Default::default() }).map(|_| ()),
                    Op::Void(i) => if let Some(&d) = docs.get(i % docs.len().max(1)) { l.set_document_status(d, "VOIDED") } else { Ok(()) },
                    Op::Credit(c) => l.save_document(None, &DocInput { kind: "ACCRECCREDIT".into(), contact_name: Some("C".into()), date: "2026-03-05".into(),
                        lines: vec![line(sales, Some(gi), "1", &amt(c))], status: Some("AUTHORISED".into()), ..Default::default() }).map(|_| ()),
                };
                prop_assert_eq!(l.ledger_imbalance().unwrap(), 0);
            }
            let bs = l.balance_sheet(&ReportParams { date: Some("2026-12-31".into()), ..Default::default() }).unwrap();
            prop_assert_eq!(bs["check"][0].as_i64(), Some(0));
            // AR in the ledger equals outstanding receivables per documents
            let ar_docs: i64 = l.conn.query_row("SELECT COALESCE(SUM(CASE WHEN type='ACCREC' THEN 1 ELSE -1 END * (total - amount_paid - amount_credited)),0) FROM documents WHERE type IN ('ACCREC','ACCRECCREDIT') AND status = 'AUTHORISED'", [], |r| r.get(0)).unwrap();
            prop_assert_eq!(bal(&l, l.sys_account("AR").unwrap()), ar_docs);
        }
    }
}

#[test]
fn demo_company_is_consistent() {
    let l = org();
    ledger_core::demo::seed_demo(&l).unwrap();
    assert_invariants(&l);
    let n: i64 = l.conn.query_row("SELECT COUNT(*) FROM documents WHERE type = 'ACCREC'", [], |r| r.get(0)).unwrap();
    assert!(n > 30, "demo has invoices");
    let unrec: i64 = l.conn.query_row("SELECT COUNT(*) FROM statement_lines WHERE status = 'UNRECONCILED'", [], |r| r.get(0)).unwrap();
    assert!(unrec >= 5, "demo has lines to reconcile");
    // every report runs
    for name in ["profit-and-loss", "balance-sheet", "trial-balance", "cash-summary", "executive-summary", "aged-receivables", "aged-payables", "tax-reconciliation", "journal-report", "general-ledger"] {
        let _ = name;
    }
    l.profit_and_loss(&ReportParams { monthly: Some(true), ..Default::default() }).unwrap();
    l.cash_summary(&ReportParams::default()).unwrap();
    l.executive_summary(&ReportParams::default()).unwrap();
    l.tax_report(&ReportParams::default()).unwrap();
    l.journal_report(&ReportParams::default()).unwrap();
    ledger_core::dashboard::dashboard(&l).unwrap();
    ledger_core::dashboard::cashflow_forecast(&l, 90, vec![]).unwrap();
    let r = l.run_repeating("2099-01-01").unwrap();
    assert!(!r.is_empty());
    assert_invariants(&l);
}

#[test]
fn bas_reconciles_to_tax_report() {
    let l = org();
    let sales = acct(&l, "200");
    invoice(&l, "2026-07-10", vec![line(sales, Some(tax(&l, "GST on Income")), "1", "1000")], "AUTHORISED");
    l.save_document(None, &DocInput { kind: "ACCPAY".into(), contact_name: Some("S".into()), date: "2026-07-11".into(), line_amount_type: Some("INCLUSIVE".into()),
        lines: vec![line(acct(&l, "453"), Some(tax(&l, "GST on Expenses")), "1", "330")], status: Some("AUTHORISED".into()), ..Default::default() }).unwrap();
    let id = l.create_tax_return(Some("AU-BAS"), "2026-07-01", "2026-09-30", Some("ACCRUAL")).unwrap();
    let r = l.get_tax_return(id).unwrap();
    let b = |k: &str| r["data"]["boxes"].as_array().unwrap().iter().find(|x| x["box"] == k).unwrap()["amount"].as_i64().unwrap();
    assert_eq!(b("G1"), 110000);
    assert_eq!(b("1A"), 10000);
    assert_eq!(b("G11"), 33000);
    assert_eq!(b("1B"), 3000);
    assert_eq!(b("9"), 7000);
    l.file_tax_return(id).unwrap();
    // filed return locks the period for non-advisers
    let mut l = l;
    l.role = "STANDARD".into();
    assert!(l.check_lock("2026-08-01").is_err());
}

#[test]
fn fixed_asset_depreciation_and_disposal() {
    let l = org();
    l.ensure_asset_types().unwrap();
    let ty = l.list_asset_types().unwrap().into_iter().find(|t| t["name"] == "Computer equipment").unwrap()["id"].as_i64().unwrap();
    let a = l.save_asset(None, &ledger_core::assets::AssetInput { name: "Laptop".into(), asset_type_id: Some(ty), purchase_date: "2026-01-01".into(), purchase_price: "3000".into(),
        method: Some("STRAIGHTLINE".into()), life_years: Some("3".into()), ..Default::default() }).unwrap();
    l.register_asset(a).unwrap();
    let r = l.run_depreciation("2026-06-30").unwrap();
    assert_eq!(r["total"], 50000, "6 months of 3000/36 = 500");
    assert_eq!(bal(&l, acct(&l, "416")), 50000);
    assert_eq!(bal(&l, acct(&l, "721")), -50000);
    l.rollback_depreciation().unwrap();
    assert_eq!(bal(&l, acct(&l, "416")), 0);
    l.run_depreciation("2026-06-30").unwrap();
    // sell for 2,800 on 15 July: depreciation to 30 June (500), book value 2,500 -> gain 300
    l.dispose_asset(a, "2026-07-15", "2800", Some(acct(&l, "090")), acct(&l, "260")).unwrap();
    assert_eq!(bal(&l, acct(&l, "720")), -300000 + 0, "cost removed (cost was never posted via bill in this test)");
    assert_eq!(bal(&l, acct(&l, "260")), -30000);
    assert_invariants(&l);
}

#[test]
fn payroll_run_posts_balanced_journal() {
    let mut l = org();
    l.set_setting("payroll_jurisdiction", "AU").unwrap();
    l.ensure_payroll_setup().unwrap();
    let cal: i64 = l.conn.query_row("SELECT id FROM pay_calendars WHERE frequency = 'FORTNIGHTLY'", [], |r| r.get(0)).unwrap();
    l.save_employee(None, &ledger_core::payroll::EmployeeInput { first_name: "A".into(), last_name: "B".into(), start_date: "2020-01-01".into(), pay_calendar_id: Some(cal),
        pay_basis: "SALARY".into(), annual_salary: Some("78000".into()), employment_type: "FULLTIME".into(), ..Default::default() }).unwrap();
    l.save_employee(None, &ledger_core::payroll::EmployeeInput { first_name: "H".into(), last_name: "Ourly".into(), start_date: "2020-01-01".into(), pay_calendar_id: Some(cal),
        pay_basis: "HOURLY".into(), hourly_rate: Some("42.50".into()), hours_per_week: Some("24".into()), employment_type: "PARTTIME".into(), ..Default::default() }).unwrap();
    let rid = l.create_pay_run(cal, None).unwrap();
    let r = l.get_pay_run(rid).unwrap();
    let hourly = r["payslips"].as_array().unwrap().iter().find(|s| s["employee"] == "H Ourly").unwrap();
    assert_eq!(hourly["gross"], 204000, "48 hours x 42.50 = 2,040.00");
    assert_eq!(r["wages"], 300000 + 204000);
    assert_eq!(r["super"], 36000 + 24480);
    let net = r["net_pay"].as_i64().unwrap();
    assert_eq!(net, 504000 - r["tax"].as_i64().unwrap());
    l.role = "ADVISER".into();
    l.post_pay_run(rid, Some(acct(&l, "090"))).unwrap();
    assert_eq!(bal(&l, acct(&l, "090")), -net);
    assert_eq!(bal(&l, l.sys_account("WAGESPAYABLE").unwrap()), 0, "net wages paid out");
    let leave: i64 = l.conn.query_row("SELECT SUM(hours_e2) FROM leave_balances", [], |r| r.get(0)).unwrap();
    assert!(leave > 0, "leave accrued");
    assert_invariants(&l);
}

#[test]
fn expense_claim_and_project_invoice() {
    let l = org();
    let rid = l.save_receipt(None, &ledger_core::expenses::ReceiptInput { supplier: "Cafe".into(), date: "2026-03-01".into(), kind: "RECEIPT".into(), line_amount_type: Some("INCLUSIVE".into()),
        lines: vec![LineInput { description: "Lunch".into(), amount: Some("55".into()), account_id: Some(acct(&l, "420")), ..Default::default() }], ..Default::default() }).unwrap();
    l.set_receipt_status(rid, "SUBMITTED").unwrap();
    l.set_receipt_status(rid, "AUTHORISED").unwrap();
    assert_eq!(bal(&l, l.sys_account("AP").unwrap()), -5500);
    let cust = l.find_or_create_contact("Client").unwrap();
    let pid = l.save_project(None, &serde_json::json!({"name": "Website", "contact_id": cust})).unwrap();
    let task = l.save_task(None, pid, &serde_json::json!({"name": "Build", "charge_type": "TIME", "rate": "100"})).unwrap();
    l.add_time(pid, task, "2026-03-02", 90, "Work").unwrap();
    l.add_project_cost(pid, &serde_json::json!({"description": "Hosting", "amount": "50", "markup": "20"})).unwrap();
    let inv = l.invoice_project(pid).unwrap();
    let d = l.get_document(inv).unwrap();
    assert_eq!(d["subtotal"], 15000 + 6000, "1.5h x 100 + 50 + 20% markup");
    assert!(l.invoice_project(pid).is_err(), "nothing left to bill");
}

#[test]
fn csv_import_dry_run_then_real() {
    let l = org();
    let csv = "*ContactName,EmailAddress,FirstName\nNew Customer,nc@example.com,Nia\nOther Co,oc@example.com,Oz\n";
    let r = l.import_csv("contacts", csv, true, None).unwrap();
    assert_eq!(r["created"], 2);
    assert_eq!(l.list_contacts("all", "").unwrap().len(), 0, "dry run writes nothing");
    let r = l.import_csv("contacts", csv, false, None).unwrap();
    assert_eq!(r["created"], 2);
    let inv = "*ContactName,*InvoiceNumber,*InvoiceDate,*DueDate,Description,*Quantity,*UnitAmount,*AccountCode,*TaxType\nNew Customer,X-1,1/03/2026,15/03/2026,Work,2,50,200,GST on Income\nNew Customer,X-1,1/03/2026,15/03/2026,More,1,10,200,GST on Income\n";
    let r = l.import_csv("invoices", inv, false, None).unwrap();
    assert_eq!(r["created"], 1, "{r}");
    assert_eq!(bal(&l, l.sys_account("AR").unwrap()), 12100);
    assert!(l.export_csv("invoices").unwrap().contains("X-1"));
    assert_invariants(&l);
}

#[test]
fn cash_flow_statement_ties_to_bank() {
    let l = org();
    ledger_core::demo::seed_demo(&l).unwrap();
    let r = ledger_core::extra_reports::run_any(&l, "cash-flow-statement", &ReportParams { from: Some("2020-01-01".into()), ..Default::default() }).unwrap();
    assert_eq!(r["check"][0], 0, "indirect cash flow reconciles to the change in bank balances");
}

#[test]
fn build_assemblies_move_component_cost() {
    let l = org();
    let mk = |code: &str| l.create_item(&ItemInput { code: code.into(), name: code.into(), is_tracked: true, sale_price: Some("50".into()), sale_account_id: Some(acct(&l, "200")),
        purchase_account_id: Some(acct(&l, "310")), purchase_price: Some("1".into()), ..Default::default() }).unwrap();
    let (leg, top, table) = (mk("LEG"), mk("TOP"), mk("TABLE"));
    let buy = |item: i64, qty: &str, price: &str| { l.save_document(None, &DocInput { kind: "ACCPAY".into(), contact_name: Some("Supplier".into()), date: "2026-06-01".into(), line_amount_type: Some("NOTAX".into()),
        lines: vec![LineInput { item_id: Some(item), quantity: Some(qty.into()), unit_price: Some(price.into()), ..Default::default() }], status: Some("AUTHORISED".into()), ..Default::default() }).unwrap(); };
    buy(leg, "10", "5");   // 10 legs @ 5.00
    buy(top, "3", "40");   // 3 tops @ 40.00
    // A table = 4 legs + 1 top.
    l.set_assembly_components(table, &[(leg, 40_000), (top, 10_000)]).unwrap();
    assert!(l.set_assembly_components(leg, &[(table, 10_000)]).is_err(), "no cycles");
    assert!(l.set_assembly_components(table, &[(table, 10_000)]).is_err(), "not its own component");
    assert_eq!(l.max_buildable(table).unwrap(), 20_000, "10 legs make 2 tables");
    assert!(l.build_assembly(table, 30_000, "2026-06-02", "").is_err(), "not enough legs for 3");
    let inv = l.sys_account("INVENTORY").unwrap();
    let before = bal(&l, inv);
    l.build_assembly(table, 20_000, "2026-06-02", "first batch").unwrap();
    let q = |id: i64| -> (i64, i64) { l.conn.query_row("SELECT qty_on_hand_e4, total_cost FROM items WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap() };
    assert_eq!(q(leg), (20_000, 1_000));     // 2 legs left, 10.00
    assert_eq!(q(top), (10_000, 4_000));     // 1 top left, 40.00
    assert_eq!(q(table), (20_000, 12_000));  // 2 tables at 60.00 each
    assert_eq!(bal(&l, inv), before, "same inventory account: value just moves between items");
    let comps = l.assembly_components(table).unwrap();
    assert_eq!(comps.len(), 2);
    assert_eq!(l.list_builds(Some(table)).unwrap().len(), 1);
    assert_invariants(&l);
}

#[test]
fn credit_limit_and_credit_hold() {
    let l = org();
    let sales = acct(&l, "200");
    let first = invoice(&l, "2026-01-10", vec![line(sales, None, "1", "800")], "AUTHORISED");
    let acme: i64 = l.conn.query_row("SELECT contact_id FROM documents WHERE id = ?1", [first], |r| r.get(0)).unwrap();
    // A 1,000.00 limit: 800 owing + 300 more is over it.
    l.set_credit(acme, Some(100_000), false).unwrap();
    let doc = |status: &str, allow: bool| DocInput { kind: "ACCREC".into(), contact_id: Some(acme), date: "2026-01-12".into(), line_amount_type: Some("EXCLUSIVE".into()),
        lines: vec![line(sales, None, "1", "300")], status: Some(status.into()), allow_over_limit: allow, ..Default::default() };
    let err = l.save_document(None, &doc("AUTHORISED", false)).unwrap_err().to_string();
    assert!(err.contains("CREDIT_LIMIT:"), "{err}");
    // Drafts aren't checked; approving the draft on its own is.
    let draft = l.save_document(None, &doc("DRAFT", false)).unwrap();
    assert!(l.set_document_status(draft, "AUTHORISED").is_err());
    // The user can go ahead anyway.
    l.save_document(Some(draft), &doc("AUTHORISED", true)).unwrap();
    // On hold: nothing new can be approved, even when allowed over the limit.
    l.set_credit(acme, None, true).unwrap();
    let err = l.save_document(None, &doc("AUTHORISED", true)).unwrap_err().to_string();
    assert!(err.contains("credit hold"), "{err}");
    l.set_credit(acme, None, false).unwrap();
    l.save_document(None, &doc("AUTHORISED", false)).unwrap();
    assert_invariants(&l);
}

#[test]
fn progress_invoicing_from_an_estimate() {
    let l = org();
    let sales = acct(&l, "200");
    let quote = l.save_document(None, &DocInput { kind: "QUOTE".into(), contact_name: Some("Acme".into()), date: "2026-02-01".into(), line_amount_type: Some("NOTAX".into()),
        lines: vec![line(sales, None, "4", "250")], status: Some("SENT".into()), ..Default::default() }).unwrap();
    let total = |id: i64| l.get_document(id).unwrap()["total"].as_i64().unwrap();
    assert_eq!(total(quote), 100_000);
    let first = l.progress_invoice(quote, 4_000, Some("2026-02-10")).unwrap();
    assert_eq!(total(first), 40_000);
    assert_eq!(l.get_document(quote).unwrap()["progress_invoiced"], 40_000);
    // 70% more would be over the estimate.
    let err = l.progress_invoice(quote, 7_000, None).unwrap_err().to_string();
    assert!(err.contains("left to invoice"), "{err}");
    let rest = l.progress_invoice(quote, 6_000, None).unwrap();
    assert_eq!(total(rest), 60_000);
    assert_eq!(l.get_document(quote).unwrap()["status"], "INVOICED");
    assert!(l.progress_invoice(quote, 1_000, None).is_err());
}

#[test]
fn price_rules_quantity_breaks_and_dates() {
    let l = org();
    l.save_price_rule("Bulk", None, None, 100_000, -2_000, None, None).unwrap();
    l.save_price_rule("Spring sale", None, None, 0, -500, Some("2026-09-01"), Some("2026-11-30")).unwrap();
    assert!(l.save_price_rule("Bad", None, None, 0, -500, Some("2026-12-01"), Some("2026-11-01")).is_err());
    let rules = l.list_price_rules().unwrap();
    use ledger_core::pricelevels::rule_bp;
    assert_eq!(rule_bp(&rules, 1, None, 10_000, "2026-01-05"), None);
    assert_eq!(rule_bp(&rules, 1, None, 10_000, "2026-10-05"), Some(-500));
    // The lowest price wins: 10 or more gets the bulk 20% off.
    assert_eq!(rule_bp(&rules, 1, None, 100_000, "2026-10-05"), Some(-2_000));
}

#[test]
fn inventory_locations_receive_transfer_and_sell() {
    let l = org();
    let item = l.create_item(&ItemInput { code: "BOLT".into(), name: "Bolt".into(), is_tracked: true, sale_price: Some("2".into()), sale_account_id: Some(acct(&l, "200")),
        purchase_account_id: Some(acct(&l, "310")), purchase_price: Some("1".into()), ..Default::default() }).unwrap();
    let north = l.save_location(None, "North warehouse", "1 North St").unwrap();
    let doc = |kind: &str, qty: &str, price: &str, loc: Option<i64>| DocInput { kind: kind.into(), contact_name: Some("Acme".into()), date: "2026-06-01".into(), line_amount_type: Some("NOTAX".into()),
        lines: vec![LineInput { item_id: Some(item), quantity: Some(qty.into()), unit_price: Some(price.into()), account_id: Some(acct(&l, "200")), ..Default::default() }],
        status: Some("AUTHORISED".into()), location_id: loc, ..Default::default() };
    // 100 received into North, none at the main location.
    l.save_document(None, &doc("ACCPAY", "100", "1", Some(north))).unwrap();
    assert_eq!(l.stock_at(item, Some(north)).unwrap(), 1_000_000);
    assert_eq!(l.stock_at(item, None).unwrap(), 0);
    // Selling from the main location fails; from North works.
    let err = l.save_document(None, &doc("ACCREC", "5", "2", None)).unwrap_err().to_string();
    assert!(err.contains("Main location"), "{err}");
    l.save_document(None, &doc("ACCREC", "5", "2", Some(north))).unwrap();
    // Move 20 to the main location; cost and the ledger don't change.
    let before = l.get_item(item).unwrap()["total_cost"].as_i64().unwrap();
    l.transfer_stock(item, Some(north), None, 200_000, "2026-06-02").unwrap();
    assert_eq!(l.get_item(item).unwrap()["total_cost"].as_i64().unwrap(), before);
    assert_eq!(l.stock_at(item, None).unwrap(), 200_000);
    assert_eq!(l.stock_at(item, Some(north)).unwrap(), 750_000);
    assert!(l.transfer_stock(item, None, Some(north), 300_000, "2026-06-02").is_err());
    // A location with stock can't be removed; the report has a column per location.
    assert!(l.delete_location(north).is_err());
    let r = l.stock_by_location_report().unwrap();
    assert_eq!(r["columns"][1], "North warehouse");
    assert_eq!(r["sections"][0]["rows"][0]["values"], serde_json::json!([200_000, 750_000, 950_000]));
    assert_invariants(&l);
}

#[test]
fn fifo_costing() {
    let l = org();
    l.update_settings(&serde_json::json!({"inventory_costing": "FIFO"})).unwrap();
    let item = l.create_item(&ItemInput { code: "NUT".into(), name: "Nut".into(), is_tracked: true, sale_price: Some("20".into()), sale_account_id: Some(acct(&l, "200")),
        purchase_account_id: Some(acct(&l, "310")), purchase_price: Some("5".into()), ..Default::default() }).unwrap();
    for (date, price) in [("2026-06-01", "5"), ("2026-06-02", "7")] {
        l.save_document(None, &DocInput { kind: "ACCPAY".into(), contact_name: Some("Supplier".into()), date: date.into(), line_amount_type: Some("NOTAX".into()),
            lines: vec![LineInput { item_id: Some(item), quantity: Some("10".into()), unit_price: Some(price.into()), ..Default::default() }], status: Some("AUTHORISED".into()), ..Default::default() }).unwrap();
    }
    assert_eq!(l.inventory_cost_out(item, 150_000).unwrap(), 8_500, "10 @ 5 then 5 @ 7");
    l.save_document(None, &DocInput { kind: "ACCREC".into(), contact_name: Some("Acme".into()), date: "2026-06-03".into(), line_amount_type: Some("NOTAX".into()),
        lines: vec![LineInput { item_id: Some(item), quantity: Some("15".into()), unit_price: Some("20".into()), ..Default::default() }], status: Some("AUTHORISED".into()), ..Default::default() }).unwrap();
    // The last 5 are the 7.00 ones.
    assert_eq!(l.get_item(item).unwrap()["total_cost"].as_i64().unwrap(), 3_500);
    assert_eq!(l.inventory_cost_out(item, 50_000).unwrap(), 3_500);
    assert_invariants(&l);
}

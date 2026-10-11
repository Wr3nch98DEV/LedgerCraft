//! Demo company: twelve months of realistic activity so every screen has data to explore.
//! All SQL is parameterised; `.format()` here is chrono date formatting for labels.

use crate::{
    advanced::{ManualJournalInput, MjLine}, banking::{BankRuleInput, BankTxInput, RuleCondition, RuleLine, TransferInput},
    contacts::ContactInput, documents::{DocInput, LineInput}, items::ItemInput, money, payments::PaymentInput,
    statements::StmtLine, util, Ledger, Result,
};
use rusqlite::params;
use serde_json::json;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); self.0 >> 33 }
    fn range(&mut self, lo: i64, hi: i64) -> i64 { lo + (self.next() % ((hi - lo + 1) as u64)) as i64 }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T { &v[(self.next() % v.len() as u64) as usize] }
}

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

pub fn seed_demo(l: &Ledger) -> Result<()> {
    let mut rng = Rng(42);
    let today = util::parse_date(&util::today())?;
    let today_s = util::fmt_date(today);
    let start = util::add_months(today, -12);
    let span = (today - start).num_days();
    let d = |days: i64| util::fmt_date(start + chrono::Duration::days(days));
    let acct = |code: &str| l.account_by_code(code).ok().flatten().unwrap();
    let tax_by = |rt: &str| -> Option<i64> { l.conn.query_row("SELECT id FROM tax_rates WHERE report_type = ?1 AND rate_bp > 0 ORDER BY id LIMIT 1", [rt], |r| r.get(0)).ok() };
    let (t_out, t_in) = (tax_by("OUTPUT"), tax_by("INPUT"));
    let bank = acct("090");
    let savings = acct("091");
    l.atomic(|| {
        l.conn.execute("UPDATE accounts SET name = 'Business Everyday', bank_account_number = '062-000 1234 5678' WHERE id = ?1", [bank])?;
        l.conn.execute("UPDATE accounts SET name = 'Business Saver', bank_account_number = '062-000 8765 4321' WHERE id = ?1", [savings])?;
        l.set_setting("address", "Level 2, 18 Harbour Street\nSydney NSW 2000")?;
        l.set_setting("phone", "02 9000 1234")?;
        l.set_setting("email", "accounts@harbourside.example")?;
        if l.setting_or("tax_number", "")?.is_empty() { l.set_setting("tax_number", "12 345 678 901")?; }
        l.save_tracking_category(None, "Region", &["Sydney".into(), "Melbourne".into(), "Online".into()])?;
        let regions: Vec<i64> = { let mut st = l.conn.prepare("SELECT id FROM tracking_options ORDER BY id")?; st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()? };
        l.save_conversion_balances(&util::add_days(&d(0), -1)?, &[
            json!({"account_id": bank, "debit": "18450.00"}), json!({"account_id": savings, "debit": "25000.00"}),
            json!({"account_id": acct("720"), "debit": "8400.00"}), json!({"account_id": acct("721"), "credit": "2100.00"}),
            json!({"account_id": acct("970"), "credit": "10000.00"}), json!({"account_id": acct("960"), "credit": "39750.00"}),
        ])?;
        let customers = [("Bluewater Cafe", "Mia", "mia@bluewater.example"), ("Northside Physio", "Liam", "accounts@northsidephysio.example"),
            ("Pinecrest Realty", "Ava", "ava@pinecrest.example"), ("Sunrise Bakery", "Noah", "hello@sunrisebakery.example"),
            ("Coastal Yoga Studio", "Isla", "isla@coastalyoga.example"), ("Metro Legal Partners", "Oliver", "finance@metrolegal.example"),
            ("Greenleaf Nursery", "Chloe", "chloe@greenleaf.example"), ("Ironbark Builders", "Jack", "ap@ironbark.example")];
        let streets = ["George", "Pitt", "King", "Elizabeth", "Oxford"];
        let mut cust_ids = Vec::new();
        for (n, f, e) in customers.iter() {
            let street = rng.pick(&streets).to_string();
            cust_ids.push(l.create_contact(&ContactInput { name: (*n).into(), first_name: (*f).into(), email: (*e).into(), is_customer: Some(true),
                phone: String::from("02 9") + &rng.range(100, 999).to_string() + " " + &rng.range(1000, 9999).to_string(),
                postal_address: rng.range(1, 200).to_string() + " " + &street + " Street\nSydney NSW 2000", ..Default::default() })?);
        }
        let suppliers = [("Officeworks", "453"), ("Telstra", "489"), ("AGL Energy", "445"), ("Adobe Systems", "485"),
            ("Harbour Property Group", "469"), ("Allianz Insurance", "433"), ("PrintCo Australia", "461"), ("Paper & Ink Wholesale", "310")];
        let mut sup_ids = Vec::new();
        for (n, exp) in suppliers.iter() {
            let slug: String = n.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
            sup_ids.push(l.create_contact(&ContactInput { name: (*n).into(), is_supplier: Some(true), purchase_account_id: Some(acct(exp)),
                email: String::from("billing@") + &slug + ".example", ..Default::default() })?);
        }
        let design = l.create_item(&ItemInput { code: "DES-HR".into(), name: "Design services (per hour)".into(), sales_description: "Graphic design services".into(), sale_price: Some("145".into()),
            sale_account_id: Some(acct("200")), sale_tax_rate_id: t_out, is_purchased: Some(false), ..Default::default() })?;
        let logo = l.create_item(&ItemInput { code: "LOGO-PKG".into(), name: "Logo package".into(), sales_description: "Logo design package incl. 3 concepts and brand guide".into(),
            sale_price: Some("1850".into()), sale_account_id: Some(acct("200")), sale_tax_rate_id: t_out, is_purchased: Some(false), ..Default::default() })?;
        let web = l.create_item(&ItemInput { code: "WEB-MAINT".into(), name: "Website maintenance (monthly)".into(), sales_description: "Website hosting & maintenance".into(),
            sale_price: Some("220".into()), sale_account_id: Some(acct("200")), sale_tax_rate_id: t_out, is_purchased: Some(false), ..Default::default() })?;
        let cards = l.create_item(&ItemInput { code: "BC-500".into(), name: "Business cards (box of 500)".into(), sales_description: "Premium business cards, 500 pack".into(),
            sale_price: Some("89".into()), sale_account_id: Some(acct("200")), sale_tax_rate_id: t_out, purchase_price: Some("32".into()),
            purchase_account_id: Some(acct("310")), purchase_tax_rate_id: t_in, is_tracked: true, reorder_level: Some("20".into()), ..Default::default() })?;
        let b = l.save_document(None, &DocInput { kind: "ACCPAY".into(), contact_id: Some(sup_ids[7]), number: Some("PW-1001".into()), date: d(3), line_amount_type: Some("EXCLUSIVE".into()),
            lines: vec![LineInput { item_id: Some(cards), quantity: Some("120".into()), unit_price: Some("32".into()), tax_rate_id: t_in, ..Default::default() }], status: Some("AUTHORISED".into()), ..Default::default() })?;
        let bt = l.get_document(b)?["total"].as_i64().unwrap();
        let bp = l.add_payment(&PaymentInput { document_id: b, account_id: bank, date: d(20), amount: money::fmt_money(bt), ..Default::default() })?;
        let mut stmt: Vec<StmtLine> = vec![StmtLine { date: d(20), amount: -bt, payee: "PAPER & INK WHOLESALE".into(), description: "BPAY".into(), fitid: Some(String::from("PAY") + &bp.to_string()), ..Default::default() }];
        let mut unpaid_recent = Vec::new();
        for m in 0..12i64 {
            let base = m * 30 + 2;
            for k in 0..rng.range(3, 6) {
                let c = *rng.pick(&cust_ids);
                let off = base + k * 5 + rng.range(0, 3);
                if off > span { continue; }
                let date = d(off);
                let mut lines = vec![LineInput { item_id: Some(design), quantity: Some(rng.range(4, 24).to_string()), tracking1: Some(*rng.pick(&regions)), ..Default::default() }];
                if rng.range(0, 3) == 0 { lines.push(LineInput { item_id: Some(logo), quantity: Some("1".into()), tracking1: Some(*rng.pick(&regions)), ..Default::default() }); }
                if rng.range(0, 2) == 0 { lines.push(LineInput { item_id: Some(cards), quantity: Some(rng.range(1, 4).to_string()), ..Default::default() }); }
                let inv = l.save_document(None, &DocInput { kind: "ACCREC".into(), contact_id: Some(c), date: date.clone(), reference: String::from("PO-") + &rng.range(1000, 9999).to_string(),
                    line_amount_type: Some("EXCLUSIVE".into()), lines, status: Some("AUTHORISED".into()), ..Default::default() });
                let Ok(inv) = inv else { continue };
                let doc = l.get_document(inv)?;
                let total = doc["total"].as_i64().unwrap();
                let number = doc["number"].as_str().unwrap_or("").to_string();
                let payee = doc["contact_name"].as_str().unwrap_or("").to_uppercase();
                let pay_day = off + rng.range(7, 35);
                if pay_day < span - 3 && rng.range(0, 9) > 0 {
                    let pd = d(pay_day);
                    if pay_day < span - 20 {
                        let pid = l.add_payment(&PaymentInput { document_id: inv, account_id: bank, date: pd.clone(), amount: money::fmt_money(total), reference: number.clone(), ..Default::default() })?;
                        stmt.push(StmtLine { date: pd, amount: total, payee, description: String::from("DIRECT CREDIT ") + &number, reference: number.clone(), fitid: Some(String::from("PAY") + &pid.to_string()), ..Default::default() });
                    } else {
                        // received in the bank but not yet matched in LedgerCraft: try Reconcile
                        stmt.push(StmtLine { date: pd, amount: total, payee, description: String::from("DIRECT CREDIT ") + &number, reference: number.clone(), fitid: Some(String::from("UNREC") + &inv.to_string()), ..Default::default() });
                    }
                } else { unpaid_recent.push(inv); }
            }
            for (si, (sname, exp)) in suppliers.iter().enumerate().take(7) {
                let amount = match si { 0 => rng.range(80, 400), 1 => rng.range(120, 160), 2 => rng.range(180, 420), 3 => 89, 4 => 2400, 5 => if m % 3 == 0 { 640 } else { continue }, _ => if m % 2 == 0 { rng.range(150, 900) } else { continue } };
                let off = base + 1 + si as i64 * 2;
                if off > span { continue; }
                let date = d(off);
                let dd = util::parse_date(&date)?;
                let desc = String::from(*sname) + " - " + MONTHS[chrono::Datelike::month0(&dd) as usize] + " " + &chrono::Datelike::year(&dd).to_string();
                let bill = l.save_document(None, &DocInput { kind: "ACCPAY".into(), contact_id: Some(sup_ids[si]), number: Some(sname[..3].to_uppercase() + "-" + &(2000 + m * 10 + si as i64).to_string()),
                    date: date.clone(), line_amount_type: Some("INCLUSIVE".into()),
                    lines: vec![LineInput { description: desc, quantity: Some("1".into()), unit_price: Some(amount.to_string()), account_id: Some(acct(exp)), tax_rate_id: t_in, ..Default::default() }],
                    status: Some("AUTHORISED".into()), ..Default::default() })?;
                let pd = off + rng.range(5, 25);
                if pd < span - 5 {
                    let pid = l.add_payment(&PaymentInput { document_id: bill, account_id: bank, date: d(pd), amount: amount.to_string() + ".00", ..Default::default() })?;
                    stmt.push(StmtLine { date: d(pd), amount: -amount * 100, payee: sname.to_uppercase(), description: "BPAY / DIRECT DEBIT".into(), fitid: Some(String::from("PAY") + &pid.to_string()), ..Default::default() });
                }
            }
            let fee_day = base + 27;
            if fee_day < span {
                let t = l.save_bank_transaction(None, &BankTxInput { kind: "SPEND".into(), account_id: bank, date: d(fee_day), contact_name: Some("Commonwealth Bank".into()), reference: "Monthly account fee".into(),
                    line_amount_type: Some("NOTAX".into()), lines: vec![LineInput { description: "Account keeping fee".into(), amount: Some("10.00".into()), account_id: Some(acct("404")), ..Default::default() }], ..Default::default() })?;
                stmt.push(StmtLine { date: d(fee_day), amount: -1000, payee: "COMMONWEALTH BANK".into(), description: "ACCOUNT FEE".into(), fitid: Some(String::from("TX") + &t.to_string()), ..Default::default() });
                let interest = rng.range(4000, 9000);
                l.save_bank_transaction(None, &BankTxInput { kind: "RECEIVE".into(), account_id: savings, date: d(fee_day), contact_name: Some("Commonwealth Bank".into()), reference: "Interest".into(),
                    line_amount_type: Some("NOTAX".into()), lines: vec![LineInput { description: "Interest earned".into(), amount: Some(money::fmt_money(interest)), account_id: Some(acct("270")), ..Default::default() }], ..Default::default() })?;
            }
            let wage_day = base + 25;
            if wage_day < span {
                let wages = rng.range(520000, 560000);
                let tw = l.save_bank_transaction(None, &BankTxInput { kind: "SPEND".into(), account_id: bank, date: d(wage_day), contact_name: Some("Staff wages".into()), reference: "Payroll".into(),
                    line_amount_type: Some("NOTAX".into()), lines: vec![LineInput { description: "Wages".into(), amount: Some(money::fmt_money(wages)), account_id: Some(acct("477")), ..Default::default() }], ..Default::default() })?;
                stmt.push(StmtLine { date: d(wage_day), amount: -wages, payee: "PAYROLL".into(), description: "WAGES".into(), fitid: Some(String::from("TX") + &tw.to_string()), ..Default::default() });
            }
        }
        l.import_lines(bank, "demo-feed.ofx", "OFX", &stmt)?;
        let lines: Vec<(i64, String)> = { let mut st = l.conn.prepare("SELECT id, dedupe_key FROM statement_lines WHERE account_id = ?1")?; st.query_map([bank], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()? };
        for (lid, key) in lines {
            if let Some(pid) = key.strip_prefix("F:PAY").and_then(|x| x.parse::<i64>().ok()) { let _ = l.reconcile_match(lid, &[("PAYMENT".into(), pid)]); }
            if let Some(tid) = key.strip_prefix("F:TX").and_then(|x| x.parse::<i64>().ok()) { let _ = l.reconcile_match(lid, &[("BANKTX".into(), tid)]); }
        }
        let recent = |n: i64| util::fmt_date(today - chrono::Duration::days(n));
        l.import_lines(bank, "recent.csv", "CSV", &[
            StmtLine { date: recent(6), amount: -4590, payee: "OFFICEWORKS 0423".into(), description: "Stationery".into(), ..Default::default() },
            StmtLine { date: recent(5), amount: -1250, payee: "SQ *HARBOUR COFFEE".into(), description: "Client meeting".into(), ..Default::default() },
            StmtLine { date: recent(4), amount: -6800, payee: "UBER *TRIP".into(), description: "Travel".into(), ..Default::default() },
            StmtLine { date: recent(3), amount: -500000, payee: "TRANSFER TO SAVER".into(), description: "Internal transfer".into(), ..Default::default() },
            StmtLine { date: recent(2), amount: -3300, payee: "GOOGLE WORKSPACE".into(), description: "Subscription".into(), ..Default::default() },
        ])?;
        l.save_bank_rule(None, &BankRuleInput { name: "Coffee meetings".into(), kind: "SPEND".into(), match_mode: "ANY".into(),
            conditions: vec![RuleCondition { field: "any".into(), op: "contains".into(), value: "coffee".into() }],
            percent_lines: vec![RuleLine { account_id: Some(acct("420")), percent: Some("100".into()), description: "Client meeting coffee".into(), ..Default::default() }], ..Default::default() })?;
        l.save_bank_rule(None, &BankRuleInput { name: "Rideshare".into(), kind: "SPEND".into(), match_mode: "ALL".into(),
            conditions: vec![RuleCondition { field: "payee".into(), op: "contains".into(), value: "uber".into() }],
            percent_lines: vec![RuleLine { account_id: Some(acct("493")), tax_rate_id: t_in, percent: Some("100".into()), description: "Rideshare".into(), ..Default::default() }], ..Default::default() })?;
        l.save_transfer(&TransferInput { from_account_id: bank, to_account_id: savings, date: recent(3), amount: "5000.00".into(), reference: "To saver".into(), ..Default::default() })?;
        l.save_document(None, &DocInput { kind: "QUOTE".into(), contact_id: Some(cust_ids[2]), date: recent(4), expiry_date: Some(util::fmt_date(today + chrono::Duration::days(26))),
            title: "Brand refresh".into(), summary: "Refresh of logo, stationery and website templates".into(), line_amount_type: Some("EXCLUSIVE".into()),
            lines: vec![LineInput { item_id: Some(logo), quantity: Some("1".into()), ..Default::default() }, LineInput { item_id: Some(design), quantity: Some("30".into()), ..Default::default() }], status: Some("SENT".into()), ..Default::default() })?;
        l.save_document(None, &DocInput { kind: "PO".into(), contact_id: Some(sup_ids[7]), date: recent(2), delivery_date: Some(util::fmt_date(today + chrono::Duration::days(7))),
            delivery_address: "Level 2, 18 Harbour Street, Sydney NSW 2000".into(), line_amount_type: Some("EXCLUSIVE".into()),
            lines: vec![LineInput { item_id: Some(cards), quantity: Some("100".into()), unit_price: Some("31.50".into()), tax_rate_id: t_in, ..Default::default() }], status: Some("AUTHORISED".into()), ..Default::default() })?;
        l.save_document(None, &DocInput { kind: "ACCREC".into(), contact_id: Some(cust_ids[4]), date: recent(1), line_amount_type: Some("EXCLUSIVE".into()),
            lines: vec![LineInput { item_id: Some(design), quantity: Some("6".into()), ..Default::default() }], status: Some("DRAFT".into()), ..Default::default() })?;
        l.save_repeating(None, &json!({"kind": "ACCREC", "start_date": util::fmt_date(today + chrono::Duration::days(5)), "period_unit": "MONTHLY", "period_count": 1, "save_as": "AUTHORISED", "due_terms_days": 14,
            "template": {"contact_id": cust_ids[1], "reference": "Website care [Month] [Year]", "line_amount_type": "EXCLUSIVE", "lines": [{"item_id": web, "quantity": "1"}]}}))?;
        if let Some(&inv) = unpaid_recent.last() {
            let doc = l.get_document(inv)?;
            if doc["status"] == "AUTHORISED" {
                let cn = l.save_document(None, &DocInput { kind: "ACCRECCREDIT".into(), contact_id: doc["contact_id"].as_i64(), date: today_s.clone(), reference: String::from("Discount on ") + doc["number"].as_str().unwrap_or(""),
                    line_amount_type: Some("EXCLUSIVE".into()), lines: vec![LineInput { description: "Goodwill discount".into(), quantity: Some("1".into()), unit_price: Some("50".into()), account_id: Some(acct("200")), tax_rate_id: t_out, ..Default::default() }],
                    status: Some("AUTHORISED".into()), ..Default::default() })?;
                let total = l.get_document(cn)?["total"].as_i64().unwrap_or(0);
                l.allocate_credit(cn, inv, &money::fmt_money(total), Some(&today_s))?;
            }
        }
        let prev_month_end = util::fmt_date(chrono::NaiveDate::from_ymd_opt(chrono::Datelike::year(&today), chrono::Datelike::month(&today), 1).unwrap() - chrono::Duration::days(1));
        l.save_manual_journal(None, &ManualJournalInput { date: prev_month_end.clone(), narration: "Accrued accounting fees".into(),
            line_amount_type: Some("NOTAX".into()), auto_reverse_date: Some(util::add_days(&prev_month_end, 1)?), lines: vec![
                MjLine { description: "Accrued accounting fees".into(), account_id: Some(acct("412")), debit: Some("750".into()), ..Default::default() },
                MjLine { description: "Accrued accounting fees".into(), account_id: Some(acct("830")), credit: Some("750".into()), ..Default::default() }], status: Some("POSTED".into()), ..Default::default() })?;
        let fy_d = util::parse_date(&l.fy_start(&today_s)?)?;
        let mut bl = Vec::new();
        for m in 0..12 {
            let x = util::add_months(fy_d, m);
            let p = x.format("%Y-%m").to_string();
            for (code, amt) in [("200", "14000"), ("469", "2400"), ("477", "5500"), ("489", "150"), ("445", "300")] {
                bl.push(json!({"account_id": acct(code), "period": p, "amount": amt}));
            }
        }
        l.save_budget(None, "Annual budget", None, &bl)?;
        l.conn.execute("UPDATE accounts SET dashboard_watch = 1 WHERE code IN ('200','477','469')", params![])?;
        Ok(())
    })?;
    crate::modules::seed_demo_extras(l)?;
    Ok(())
}

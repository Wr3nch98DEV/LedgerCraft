//! Headless UI tests: render the real views, click widgets by their text with iced's simulator,
//! and check the messages produced and the resulting state. No window or real mouse involved.

use crate::app::{App, Lookups, Message, Route};
use crate::theme::Skin;
use iced_test::simulator;
use serde_json::{Value, json};

/// A booted app on a fresh demo company, with session and reference data loaded.
fn demo_app(tag: &str, skin: Skin) -> (App, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("lc-ui-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (mut app, _) = App::boot(dir.clone(), None, Some(skin));
    app.be.call_now("orgs.create", json!({"org_name": "Demo Co", "country": "US", "demo": true, "owner_name": "Alex Morgan"})).unwrap();
    let session = app.be.call_now("session.get", json!({})).unwrap();
    let _ = app.update(Message::Session(Ok(session)));
    let get = |m: &str, p: Value| app.be.call_now(m, p).unwrap().as_array().cloned().unwrap();
    let lk = Lookups { accounts: get("accounts.list", json!({})), tax_rates: get("taxRates.list", json!({})), items: get("items.list", json!({})), contacts: get("contacts.list", json!({"filter": "all"})), price_levels: get("priceLevels.list", json!({})), templates: get("branding.list", json!({})), price_rules: get("priceRules.list", json!({})), locations: get("locations.list", json!({})) };
    let _ = app.update(Message::LookupsLoaded(Box::new(lk)));
    (app, dir)
}

/// iced's headless renderer isn't safe to run from several test threads at once, so
/// simulator use is serialised (the app itself only ever has one renderer).
static SIM: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn sim_lock() -> std::sync::MutexGuard<'static, ()> { SIM.lock().unwrap_or_else(|e| e.into_inner()) }

/// Click the widget showing `label` and return the messages it produced.
fn click(app: &App, label: &str) -> Vec<Message> {
    let _g = sim_lock();
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 1400.0), app.view());
    ui.click(label).unwrap_or_else(|e| panic!("couldn't click {label:?}: {e:?}"));
    ui.into_messages().collect()
}

#[test]
fn quickbooks_new_menu_opens_invoice_editor() {
    let (mut app, dir) = demo_app("newmenu", Skin::QuickBooks);
    assert_eq!(app.route, Route::Dashboard);
    let msgs = click(&app, "New");
    assert!(matches!(msgs.as_slice(), [Message::ToggleNew]), "{msgs:?}");
    let _ = app.update(Message::ToggleNew);
    assert!(app.new_menu);
    // QuickBooks groups the menu under Customers / Vendors / Employees / Other.
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for heading in ["CUSTOMERS", "VENDORS", "EMPLOYEES", "OTHER"] { assert!(ui.find(heading).is_ok(), "missing {heading}"); }
    }
    let msgs = click(&app, "Invoice");
    let Some(Message::Nav(r)) = msgs.into_iter().next() else { panic!("no navigation") };
    assert_eq!(r, Route::Doc { kind: "ACCREC".into(), id: None });
    let _ = app.update(Message::Nav(r));
    assert!(!app.new_menu, "menu closes on navigation");
    assert!(app.doc.is_some(), "editor opened");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn invoice_editor_validates_and_adds_lines() {
    let (mut app, dir) = demo_app("editor", Skin::QuickBooks);
    let _ = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None }));
    let lines_before = app.doc.as_ref().unwrap().lines.len();
    let msgs = click(&app, "Add lines");
    for m in msgs { let _ = app.update(m); }
    assert_eq!(app.doc.as_ref().unwrap().lines.len(), lines_before + 1);
    // Saving with no customer is refused with a clear message, before anything reaches the engine.
    let msgs = click(&app, "Save and close");
    assert!(matches!(msgs.first(), Some(Message::Doc(crate::doc::Msg::Save(true)))), "{msgs:?}");
    for m in msgs { let _ = app.update(m); }
    assert_eq!(app.doc.as_ref().unwrap().error.as_deref(), Some("Choose a customer"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sidebar_and_gear_menu_navigation() {
    let (mut app, dir) = demo_app("nav", Skin::QuickBooks);
    let msgs = click(&app, "Banking");
    assert!(matches!(msgs.first(), Some(Message::Nav(Route::Banking(None)))), "{msgs:?}");
    let msgs = click(&app, "Reports");
    assert!(matches!(msgs.first(), Some(Message::Nav(Route::Reports))), "{msgs:?}");
    // Gear menu → switch layout keeps the same company and route.
    let _ = app.update(Message::ToggleGear);
    let msgs = click(&app, "Switch to Xero-style layout");
    assert!(matches!(msgs.first(), Some(Message::SetSkin(Skin::Xero))), "{msgs:?}");
    let _ = app.update(Message::SetSkin(Skin::Xero));
    assert_eq!(app.skin, Skin::Xero);
    assert_eq!(app.route, Route::Dashboard);
    // The Xero-style frame has its own top menu.
    let msgs = click(&app, "Accounting");
    assert!(matches!(msgs.first(), Some(Message::OpenMenu(Some("accounting")))), "{msgs:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn bank_feed_match_button_reconciles() {
    let (mut app, dir) = demo_app("bank", Skin::QuickBooks);
    let _ = app.update(Message::Nav(Route::Banking(None)));
    // Load the page data the way the screen does, synchronously.
    let acc = app.be.call_now("accounts.bank", json!({})).unwrap()[0]["id"].as_i64().unwrap();
    let mut lines = app.be.call_now("statements.lines", json!({"account_id": acc})).unwrap();
    for l in lines.as_array_mut().unwrap() { l["suggest"] = app.be.call_now("rec.suggestions", json!({"id": l["id"]})).unwrap(); }
    let n = lines.as_array().unwrap().len();
    let banks = app.be.call_now("accounts.bank", json!({})).unwrap();
    app.data = Some(json!({"banks": banks, "lines": lines, "account": acc}));
    let msgs = click(&app, "Match");
    let Some(Message::Bank(m)) = msgs.into_iter().next() else { panic!("Match produced nothing") };
    let (method, params) = crate::banking::match_command(&m).expect("a match action");
    app.be.call_now(method, params).unwrap();
    assert_eq!(app.be.call_now("statements.lines", json!({"account_id": acc})).unwrap().as_array().unwrap().len(), n - 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn password_protected_company_shows_sign_in() {
    let (mut app, dir) = demo_app("login", Skin::QuickBooks);
    let user = app.be.call_now("session.get", json!({})).unwrap()["user"].clone();
    app.be.call_now("users.setPassword", json!({"id": user["id"], "password": "correct horse battery"})).unwrap();
    // Re-open the app: the session now requires signing in.
    let (mut app2, _) = App::boot(dir.clone(), None, Some(Skin::QuickBooks));
    let r = app2.be.call_now("session.get", json!({}));
    let _ = app2.update(Message::Session(r));
    assert_eq!(app2.route, Route::Login);
    let msgs = click(&app2, "Sign in");
    assert!(matches!(msgs.first(), Some(Message::Setup(crate::setup::Msg::SignIn))), "{msgs:?}");
    // Wrong password is refused; the right one signs in.
    assert!(app2.be.call_now("auth.login", json!({"email": user["email"], "password": "nope"})).is_err());
    let ok = app2.be.call_now("auth.login", json!({"email": user["email"], "password": "correct horse battery"}));
    let _ = app2.update(Message::Setup(crate::setup::Msg::SignedIn(ok)));
    assert!(app2.be.token.is_some());
    assert!(app2.be.call_now("session.get", json!({})).is_ok());
    let _ = &mut app;
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn run_and_submit_payroll() {
    let (mut app, dir) = demo_app("payroll", Skin::QuickBooks);
    let _ = app.update(Message::Nav(Route::Payroll));
    app.data = Some(json!({"employees": app.be.call_now("payroll.employees", json!({})).unwrap(), "runs": app.be.call_now("payroll.payRuns", json!({})).unwrap(),
        "settings": app.be.call_now("payroll.settings", json!({})).unwrap()}));
    let msgs = click(&app, "Run payroll");
    assert!(matches!(msgs.first(), Some(Message::People(crate::people::Msg::RunPayroll))), "{msgs:?}");
    // What RunPayroll sends:
    let cal = app.data.as_ref().unwrap()["settings"]["calendars"][0]["id"].as_i64().unwrap();
    let id = app.be.call_now("payroll.createPayRun", json!({"calendar_id": cal})).unwrap()["id"].as_i64().unwrap();
    let _ = app.update(Message::Nav(Route::PayRun(id)));
    app.data = Some(app.be.call_now("payroll.payRun", json!({"id": id})).unwrap());
    assert!(app.people.bank.is_some(), "a bank account is preselected");
    let msgs = click(&app, "Submit payroll");
    assert!(matches!(msgs.first(), Some(Message::People(crate::people::Msg::Submit(x))) if *x == id), "{msgs:?}");
    app.be.call_now("payroll.post", json!({"id": id, "bank_account_id": app.people.bank.as_ref().unwrap().id})).unwrap();
    assert_eq!(app.be.call_now("payroll.payRun", json!({"id": id})).unwrap()["status"], "POSTED");
    let tb = app.be.call_now("reports.run", json!({"name": "trial-balance", "params": {"date": "2099-12-31"}})).unwrap();
    assert_eq!(tb["balanced"], true);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn global_search_opens_contact() {
    let (mut app, dir) = demo_app("search", Skin::QuickBooks);
    let _ = app.update(Message::Search("pinecrest".into()));
    let r = app.be.call_now("search", json!({"q": "pinecrest", "limit": 12}));
    let _ = app.update(Message::SearchResults("pinecrest".into(), r));
    assert!(!app.search_results.is_empty());
    let msgs = click(&app, "Pinecrest Realty");
    let Some(Message::OpenResult(entity, id)) = msgs.into_iter().next() else { panic!("no result click") };
    assert_eq!(entity, "contact");
    let _ = app.update(Message::OpenResult(entity, id));
    assert_eq!(app.route, Route::Contact(id));
    assert!(app.search.is_empty() && app.search_results.is_empty(), "search closes after opening a result");
    // Stale replies (for text the user has since changed) are ignored.
    let _ = app.update(Message::Search("zz".into()));
    let _ = app.update(Message::SearchResults("pinecrest".into(), Ok(json!([{"entity": "contact", "id": 1, "title": "x"}]))));
    assert!(app.search_results.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn edit_company_details() {
    let (mut app, dir) = demo_app("settings", Skin::QuickBooks);
    let _ = app.update(Message::Nav(Route::Settings));
    let msgs = click(&app, "Edit");
    assert!(matches!(msgs.first(), Some(Message::Settings(crate::settings::Msg::Edit))), "{msgs:?}");
    let _ = app.update(Message::Settings(crate::settings::Msg::Edit));
    assert_eq!(app.settings.values[0], "Demo Co");
    let _ = app.update(Message::Settings(crate::settings::Msg::Field(4, "555 0100".into())));
    let msgs = click(&app, "Save");
    assert!(matches!(msgs.first(), Some(Message::Settings(crate::settings::Msg::Save))), "{msgs:?}");
    // What Save sends:
    let mut p = serde_json::Map::new();
    for ((k, _), v) in crate::settings::FIELDS.iter().zip(app.settings.values.iter()) { p.insert(k.to_string(), json!(v)); }
    app.be.call_now("settings.update", Value::Object(p)).unwrap();
    assert_eq!(app.be.call_now("settings.get", json!({})).unwrap()["phone"], "555 0100");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn xero_reconcile_ok_button() {
    let (mut app, dir) = demo_app("xerorec", Skin::Xero);
    let _ = app.update(Message::Nav(Route::Banking(None)));
    let acc = app.be.call_now("accounts.bank", json!({})).unwrap()[0]["id"].as_i64().unwrap();
    let mut lines = app.be.call_now("statements.lines", json!({"account_id": acc})).unwrap();
    for l in lines.as_array_mut().unwrap() { l["suggest"] = app.be.call_now("rec.suggestions", json!({"id": l["id"]})).unwrap(); }
    let n = lines.as_array().unwrap().len();
    let banks = app.be.call_now("accounts.bank", json!({})).unwrap();
    app.data = Some(json!({"banks": banks, "lines": lines, "account": acc}));
    let msgs = click(&app, "OK");
    let Some(Message::Bank(m)) = msgs.into_iter().next() else { panic!("OK produced nothing") };
    match &m {
        crate::banking::Msg::MatchDoc(..) | crate::banking::Msg::MatchTx(..) => { let (method, p) = crate::banking::match_command(&m).unwrap(); app.be.call_now(method, p).unwrap(); }
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(app.be.call_now("statements.lines", json!({"account_id": acc})).unwrap().as_array().unwrap().len(), n - 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn transfer_and_save_and_send() {
    let (mut app, dir) = demo_app("transfer", Skin::QuickBooks);
    let _ = app.update(Message::Nav(Route::Transfer));
    let f = app.form.as_ref().unwrap();
    assert!(f.from.is_some() && f.to.is_some() && f.from != f.to, "two different bank accounts preselected");
    let _ = app.update(Message::Form(crate::forms::Msg::Amount("125.00".into())));
    let msgs = click(&app, "Save");
    assert!(matches!(msgs.first(), Some(Message::Form(crate::forms::Msg::Save))), "{msgs:?}");
    let f = app.form.as_ref().unwrap();
    let date = crate::ui::to_iso(&f.date, app.date_style()).unwrap();
    app.be.call_now("transfers.save", json!({"from_account_id": f.from.as_ref().unwrap().id, "to_account_id": f.to.as_ref().unwrap().id, "date": date, "amount": "125.00", "reference": ""})).unwrap();
    // A new invoice offers Save and send (QuickBooks' main action).
    let _ = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None }));
    let msgs = click(&app, "Save and send");
    assert!(matches!(msgs.first(), Some(Message::Doc(crate::doc::Msg::SaveAndSend))), "{msgs:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sales_receipt_from_new_menu() {
    let (mut app, dir) = demo_app("receipt", Skin::QuickBooks);
    let _ = app.update(Message::ToggleNew);
    let msgs = click(&app, "Sales receipt");
    let Some(Message::Nav(r)) = msgs.into_iter().next() else { panic!("Sales receipt is not enabled in + New") };
    let _ = app.update(Message::Nav(r));
    {
        let e = app.doc.as_mut().unwrap();
        assert_eq!(e.kind, "SALESRECEIPT");
        assert!(e.pay_account.is_some(), "a deposit account is preselected");
        e.contact_text = "Walk-in customer".into();
        e.lines[0].description = "Logo tweak".into();
        e.lines[0].price = "200".into();
    }
    let msgs = click(&app, "Save and close");
    assert!(matches!(msgs.first(), Some(Message::Doc(crate::doc::Msg::Save(true)))), "{msgs:?}");
    // What saving sends:
    let e = app.doc.as_ref().unwrap();
    let mut p = e.payload(&app, "AUTHORISED").unwrap();
    p["account_id"] = json!(e.pay_account.as_ref().unwrap().id);
    let id = app.be.call_now("docs.salesReceipt", p).unwrap()["id"].as_i64().unwrap();
    let d = app.be.call_now("docs.get", json!({"id": id})).unwrap();
    assert_eq!((d["status"].as_str(), d["subtype"].as_str(), d["amount_due"].as_i64()), (Some("PAID"), Some("RECEIPT"), Some(0)));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn write_a_check() {
    let (mut app, dir) = demo_app("check", Skin::QuickBooks);
    let _ = app.update(Message::ToggleNew);
    let msgs = click(&app, "Check");
    let Some(Message::Nav(r)) = msgs.into_iter().next() else { panic!("Check is not enabled in + New") };
    let _ = app.update(Message::Nav(r));
    let acc = app.pay.as_ref().unwrap().account.as_ref().unwrap().id;
    let next = app.be.call_now("banktx.nextCheck", json!({"account_id": acc}));
    let _ = app.update(Message::Pay(crate::payments::Msg::NextCheck(next)));
    assert_eq!(app.pay.as_ref().unwrap().check_no, "1001");
    let rent = app.lk.account_opts(|a| a["code"] == "469").remove(0);
    let _ = app.update(Message::Pay(crate::payments::Msg::LineAccount(0, rent.clone())));
    let _ = app.update(Message::Pay(crate::payments::Msg::LineAmount(0, "1,200.00".into())));
    let msgs = click(&app, "Save and close");
    assert!(matches!(msgs.first(), Some(Message::Pay(crate::payments::Msg::Save))), "{msgs:?}");
    // What Save sends for a check:
    let tx = json!({"type": "SPEND", "account_id": acc, "date": "2026-10-12", "reference": "", "line_amount_type": "INCLUSIVE", "subtype": "CHECK", "check_number": "1001",
        "lines": [{"account_id": rent.id, "description": "", "amount": "1200.00", "tax_rate_id": null}]});
    app.be.call_now("banktx.save", json!({"tx": tx})).unwrap();
    let txs = app.be.call_now("bank.transactions", json!({"id": acc, "limit": 50})).unwrap();
    assert!(txs.as_array().unwrap().iter().any(|t| t["description"] == "Check 1001"));
    assert_eq!(app.be.call_now("banktx.nextCheck", json!({"account_id": acc})).unwrap()["number"], "1002");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn undeposited_funds_then_bank_deposit() {
    let (mut app, dir) = demo_app("deposit", Skin::QuickBooks);
    // QuickBooks companies get Undeposited Funds, and it's offered as a "Deposit to" account.
    let uf = app.be.call_now("accounts.undeposited", json!({})).unwrap()["id"].as_i64().unwrap();
    let r = app.load_lookups(); let _ = r;
    let get = |m: &str, p: Value| app.be.call_now(m, p).unwrap().as_array().cloned().unwrap();
    app.lk.accounts = get("accounts.list", json!({}));
    assert!(app.lk.bank_opts().iter().any(|o| o.id == uf && o.label == "Undeposited Funds"));
    // Receive two invoice payments into Undeposited Funds.
    let open = app.be.call_now("docs.list", json!({"type": "ACCREC", "tab": "awaiting"})).unwrap();
    let rows = open["rows"].as_array().unwrap();
    for r in rows.iter().take(2) {
        app.be.call_now("payments.add", json!({"document_id": r["id"], "account_id": uf, "date": "2026-10-12", "amount": crate::ui::plain(r["amount_due"].as_i64().unwrap()), "reference": ""})).unwrap();
    }
    let expected: i64 = rows.iter().take(2).map(|r| r["amount_due"].as_i64().unwrap()).sum();
    // + New → Bank deposit.
    let _ = app.update(Message::ToggleNew);
    let msgs = click(&app, "Bank deposit");
    let Some(Message::Nav(r)) = msgs.into_iter().next() else { panic!("Bank deposit is not enabled in + New") };
    let _ = app.update(Message::Nav(r));
    let und = app.be.call_now("deposits.undeposited", json!({}));
    let _ = app.update(Message::Pay(crate::payments::Msg::Undeposited(und)));
    let s = app.pay.as_ref().unwrap();
    assert_eq!(s.docs.len(), 2);
    assert!(s.docs.iter().all(|d| d.pick), "all ticked by default");
    let bank = s.account.as_ref().unwrap().id;
    assert_ne!(bank, uf, "deposits go to a real bank account");
    let msgs = click(&app, "Save and close");
    assert!(matches!(msgs.first(), Some(Message::Pay(crate::payments::Msg::Save))), "{msgs:?}");
    let ids: Vec<i64> = s.docs.iter().map(|d| d.id).collect();
    app.be.call_now("deposits.create", json!({"account_id": bank, "date": "2026-10-12", "reference": "", "payment_ids": ids})).unwrap();
    let items = app.be.call_now("rec.unreconciled", json!({"account_id": bank})).unwrap();
    assert!(items.as_array().unwrap().iter().any(|i| i["kind"] == "DEPOSIT" && i["amount"].as_i64() == Some(expected)));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn tick_off_reconcile() {
    use crate::reconcile::Msg as R;
    let (mut app, dir) = demo_app("tickoff", Skin::QuickBooks);
    // Accounting ▸ Reconcile.
    let _ = app.update(Message::Nav(Route::Chart));
    let msgs = click(&app, "Reconcile");
    assert!(matches!(msgs.as_slice(), [Message::Nav(Route::Reconcile)]), "{msgs:?}");
    let _ = app.update(Message::Nav(Route::Reconcile));
    let acc = app.rec.account.as_ref().expect("a bank account is preselected").id;
    let begin = app.be.call_now("tickoff.beginning", json!({"account_id": acc})).unwrap()["beginning_balance"].as_i64().unwrap();
    let hist = app.be.call_now("tickoff.history", json!({"account_id": acc})).unwrap();
    app.data = Some(json!({"account": acc, "beginning_balance": begin, "history": hist}));
    // Starting without an ending balance is refused before reaching the engine.
    let msgs = click(&app, "Start reconciling");
    for m in msgs { let _ = app.update(m); }
    assert!(app.rec.session.is_none() && app.toasts.iter().any(|t| t.2), "missing ending balance is flagged");
    // Peek at what's uncleared, then enter a statement balance that covers exactly the first item.
    let today = crate::ui::today_iso();
    let probe = app.be.call_now("tickoff.start", json!({"account_id": acc, "statement_date": today, "ending_balance": 0})).unwrap();
    let item = probe["items"].as_array().unwrap().iter().find(|i| !i["contact"].as_str().unwrap_or("").is_empty()).cloned().expect("demo has uncleared items");
    let ending = begin + item["amount"].as_i64().unwrap();
    let _ = app.update(Message::Rec(R::Ending(crate::ui::plain(ending))));
    let _ = app.update(Message::Rec(R::Start));
    let started = app.be.call_now("tickoff.start", json!({"account_id": acc, "statement_date": today, "ending_balance": ending}));
    let _ = app.update(Message::Rec(R::Session(started)));
    assert_eq!(app.rec.session.as_ref().unwrap()["difference"], item["amount"]);
    // Clicking the row ticks it; the difference drops to zero and Finish becomes available.
    let msgs = click(&app, item["contact"].as_str().unwrap());
    let Some(Message::Rec(R::Toggle(kind, id, true))) = msgs.into_iter().next() else { panic!("row click doesn't tick") };
    let _ = app.update(Message::Rec(R::Toggle(kind.clone(), id, true)));
    assert_eq!(app.rec.session.as_ref().unwrap()["difference"], 0, "ticking updates the totals at once");
    let rid = app.rec.session.as_ref().unwrap()["id"].clone();
    let reply = app.be.call_now("tickoff.toggle", json!({"id": rid, "kind": kind, "target_id": id, "on": true}));
    let _ = app.update(Message::Rec(R::Session(reply)));
    let msgs = click(&app, "Finish now");
    assert!(matches!(msgs.as_slice(), [Message::Rec(R::Finish)]), "{msgs:?}");
    let _ = app.update(Message::Rec(R::Finish));
    let done = app.be.call_now("tickoff.finish", json!({"id": rid}));
    let _ = app.update(Message::Rec(R::Finished(done)));
    assert!(app.rec.session.is_none());
    let left = app.be.call_now("tickoff.history", json!({"account_id": acc})).unwrap();
    assert_eq!(left[0]["status"], "FINISHED");
    assert_eq!(app.be.call_now("tickoff.beginning", json!({"account_id": acc})).unwrap()["beginning_balance"], ending, "next one starts from this ending balance");
    let _ = std::fs::remove_dir_all(dir);
}

/// Renders the tick-off screen to `%TEMP%\lc-reconcile.png` for a visual check (run with --ignored).
#[test]
#[ignore]
fn render_reconcile_screen() {
    let (mut app, dir) = demo_app("tickoff-shot", Skin::QuickBooks);
    let _ = app.update(Message::Nav(Route::Reconcile));
    let acc = app.rec.account.as_ref().unwrap().id;
    let s = app.be.call_now("tickoff.start", json!({"account_id": acc, "statement_date": crate::ui::today_iso(), "ending_balance": 2_000_000})).unwrap();
    let first = s["items"][0].clone();
    let s = app.be.call_now("tickoff.toggle", json!({"id": s["id"], "kind": first["kind"], "target_id": first["id"], "on": true})).unwrap();
    app.rec.session = Some(s);
    let out = std::env::temp_dir().join("lc-reconcile.png");
    let _ = std::fs::remove_file(&out);
    let _g = sim_lock();
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 900.0), app.view());
    ui.snapshot(&app.theme()).unwrap().matches_image(&out).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn exclude_bank_line_and_restore() {
    use crate::banking::{Msg as B, Tab};
    let (mut app, dir) = demo_app("exclude", Skin::QuickBooks);
    let _ = app.update(Message::Nav(Route::Banking(None)));
    let acc = app.be.call_now("accounts.bank", json!({})).unwrap()[0]["id"].as_i64().unwrap();
    let lines = app.be.call_now("statements.lines", json!({"account_id": acc})).unwrap();
    let n = lines.as_array().unwrap().len();
    let id = lines[0]["id"].as_i64().unwrap();
    let banks = app.be.call_now("accounts.bank", json!({})).unwrap();
    app.data = Some(json!({"banks": banks, "lines": lines, "account": acc}));
    let _ = app.update(Message::Bank(B::Expand(id)));
    let msgs = click(&app, "Exclude");
    assert!(matches!(msgs.as_slice(), [Message::Bank(B::Exclude(x, true))] if *x == id), "{msgs:?}");
    app.be.call_now("statements.exclude", json!({"id": id, "on": true})).unwrap();
    assert_eq!(app.be.call_now("statements.lines", json!({"account_id": acc})).unwrap().as_array().unwrap().len(), n - 1);
    // Excluded tab → Undo puts it back in review.
    let _ = app.update(Message::Bank(B::Tab(Tab::Excluded)));
    let excluded = app.be.call_now("statements.lines", json!({"account_id": acc, "status": "EXCLUDED"})).unwrap();
    app.data = Some(json!({"banks": banks, "lines": excluded, "account": acc}));
    let msgs = click(&app, "Undo");
    assert!(matches!(msgs.as_slice(), [Message::Bank(B::Exclude(x, false))] if *x == id), "{msgs:?}");
    app.be.call_now("statements.exclude", json!({"id": id, "on": false})).unwrap();
    assert_eq!(app.be.call_now("statements.lines", json!({"account_id": acc})).unwrap().as_array().unwrap().len(), n);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn layout_is_remembered_per_user() {
    let dir = std::env::temp_dir().join(format!("lc-ui-peruser-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (mut app, _) = App::boot(dir.clone(), None, None);
    app.be.call_now("orgs.create", json!({"org_name": "Demo Co", "country": "US", "demo": false, "owner_name": "Alex Morgan"})).unwrap();
    let session = app.be.call_now("session.get", json!({})).unwrap();
    let _ = app.update(Message::Session(Ok(session.clone())));
    assert_eq!(app.skin, Skin::QuickBooks);
    let _ = app.update(Message::SetSkin(Skin::Xero));
    // Someone else used this computer in QuickBooks layout since then (the computer default changes)…
    let prefs = dir.join("ui-preferences.json");
    let mut p: Value = serde_json::from_str(&std::fs::read_to_string(&prefs).unwrap()).unwrap();
    p["layout"] = json!("quickbooks");
    std::fs::write(&prefs, p.to_string()).unwrap();
    // …but when this user opens the company again, they get their own layout back.
    let (mut again, _) = App::boot(dir.clone(), None, None);
    assert_eq!(again.skin, Skin::QuickBooks, "before sign-in: the computer's default");
    let _ = again.update(Message::Session(Ok(session)));
    assert_eq!(again.skin, Skin::Xero, "after sign-in: this user's layout");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn customize_report_columns_and_filter() {
    use crate::reports::{Columns, Msg as Rp, Negative};
    let (mut app, dir) = demo_app("customize", Skin::QuickBooks);
    let _ = app.update(Message::Nav(Route::Report("profit-and-loss".into())));
    let msgs = click(&app, "Customize");
    assert!(matches!(msgs.as_slice(), [Message::Report(Rp::Customize)]), "{msgs:?}");
    let _ = app.update(Message::Report(Rp::Customize));
    assert!(app.report.customize);
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["Customize report", "Display columns by", "Negative numbers", "Run report"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    // Custom dates over a year, by month, negatives in parentheses.
    let _ = app.update(Message::Report(Rp::From("01/01/2026".into())));
    let _ = app.update(Message::Report(Rp::To("12/31/2026".into())));
    let _ = app.update(Message::Report(Rp::Columns(Columns::Months)));
    let _ = app.update(Message::Report(Rp::Negative(Negative::Parentheses)));
    let p = crate::reports::params(&app);
    assert_eq!((p["from"].as_str(), p["to"].as_str(), p["monthly"].as_bool()), (Some("2026-01-01"), Some("2026-12-31"), Some(true)));
    let rep = app.be.call_now("reports.run", json!({"name": "profit-and-loss", "params": p})).unwrap();
    assert_eq!(rep["columns"].as_array().unwrap().len(), 12, "one column per month");
    // Previous year comparison.
    let _ = app.update(Message::Report(Rp::Columns(Columns::PreviousYear)));
    let rep = app.be.call_now("reports.run", json!({"name": "profit-and-loss", "params": crate::reports::params(&app)})).unwrap();
    assert_eq!(rep["columns"].as_array().unwrap().len(), 2);
    // Bad custom dates are caught before running.
    let _ = app.update(Message::Report(Rp::To("2025-01-01".into())));
    let _ = app.update(Message::Report(Rp::Run));
    assert!(app.toasts.iter().any(|t| t.2), "start after end is flagged");
    // A/R aging filtered to one customer.
    let _ = app.update(Message::Nav(Route::Report("aged-receivables".into())));
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    let _ = app.update(Message::Report(Rp::Contact(Some(cust.clone()))));
    assert_eq!(crate::reports::params(&app)["contact_id"], cust.id);
    let _ = std::fs::remove_dir_all(dir);
}

/// Renders the P&L with the Customize panel to `%TEMP%\lc-customize-wgpu.png` (run with --ignored).
#[test]
#[ignore]
fn render_customize_panel() {
    use crate::reports::{Columns, Msg as Rp, Negative};
    let (mut app, dir) = demo_app("customize-shot", Skin::QuickBooks);
    let _ = app.update(Message::Nav(Route::Report("profit-and-loss".into())));
    let _ = app.update(Message::Report(Rp::Customize));
    let _ = app.update(Message::Report(Rp::Columns(Columns::PreviousYear)));
    let _ = app.update(Message::Report(Rp::Negative(Negative::Parentheses)));
    app.data = Some(app.be.call_now("reports.run", json!({"name": "profit-and-loss", "params": crate::reports::params(&app)})).unwrap());
    let out = std::env::temp_dir().join("lc-customize.png");
    let _ = std::fs::remove_file(std::env::temp_dir().join("lc-customize-wgpu.png"));
    let _g = sim_lock();
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 900.0), app.view());
    ui.snapshot(&app.theme()).unwrap().matches_image(&out).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn settings_users_and_email() {
    use crate::settings::{Msg as S, RoleOpt, Security};
    let (mut app, dir) = demo_app("settings-users", Skin::QuickBooks);
    let _ = app.update(Message::Nav(Route::Settings));
    let users = app.be.call_now("users.list", json!({}));
    let _ = app.update(Message::Settings(S::Users(users)));
    // Account and settings ▸ Users.
    let msgs = click(&app, "Users");
    assert!(matches!(msgs.as_slice(), [Message::Settings(S::Cat("Users"))]), "{msgs:?}");
    let _ = app.update(Message::Settings(S::Cat("Users")));
    // Add a user.
    let msgs = click(&app, "Add user");
    assert!(matches!(msgs.as_slice(), [Message::Settings(S::AddUser)]), "{msgs:?}");
    let _ = app.update(Message::Settings(S::AddUser));
    let _ = app.update(Message::Settings(S::UName("Sam Lee".into())));
    let _ = app.update(Message::Settings(S::UEmail("not-an-email".into())));
    let _ = app.update(Message::Settings(S::SaveUser));
    assert!(app.toasts.iter().any(|t| t.2), "bad email is flagged");
    let _ = app.update(Message::Settings(S::UEmail("sam@example.test".into())));
    let _ = app.update(Message::Settings(S::URole(RoleOpt { key: "INVOICE_ONLY", label: "Invoice only" })));
    let msgs = click(&app, "Save");
    assert!(matches!(msgs.as_slice(), [Message::Settings(S::SaveUser)]), "{msgs:?}");
    let saved = app.be.call_now("users.create", json!({"name": "Sam Lee", "email": "sam@example.test", "role": app.settings.u_role.unwrap()}));
    let _ = app.update(Message::Settings(S::UserSaved(saved)));
    let _ = app.update(Message::Settings(S::Users(app.be.call_now("users.list", json!({})))));
    let sam = app.settings.users.iter().find(|u| u["email"] == "sam@example.test").cloned().expect("user added");
    assert_eq!(sam["role"], "INVOICE_ONLY");
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["Manage users", "Invoice only", "Primary admin", "Deactivate"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    // Email settings: saved with the engine's security value; the password is never shown back.
    let _ = app.update(Message::Settings(S::EditSmtp));
    for (n, v) in ["smtp.example.test", "465", "books@example.test", "secret", "books@example.test"].iter().enumerate() { let _ = app.update(Message::Settings(S::Smtp(n, v.to_string()))); }
    let _ = app.update(Message::Settings(S::SmtpTls(Security::Tls)));
    let _ = app.update(Message::Settings(S::SaveSmtp));
    let mut p = serde_json::Map::new();
    for ((k, _), v) in crate::settings::SMTP.iter().zip(app.settings.smtp.iter()) { p.insert(k.to_string(), json!(v)); }
    p.insert("smtp_tls".into(), json!(app.settings.smtp_tls.key()));
    app.be.call_now("settings.update", Value::Object(p)).unwrap();
    let st = app.be.call_now("settings.get", json!({})).unwrap();
    assert_eq!((st["smtp_host"].as_str(), st["smtp_port"].as_str(), st["smtp_tls"].as_str()), (Some("smtp.example.test"), Some("465"), Some("TLS")));
    assert!(st.get("smtp_pass").is_none(), "password isn't readable");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn new_account_with_detail_type() {
    use crate::forms::Msg as F;
    let (mut app, dir) = demo_app("newaccount", Skin::QuickBooks);
    let _ = app.update(Message::Nav(Route::Chart));
    app.data = Some(app.be.call_now("accounts.list", json!({})).unwrap());
    let _ = app.update(Message::Nav(Route::NewAccount));
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["New account", "Account type", "Detail type", "Number"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    // Credit card type → BANK account flagged as a credit card.
    let _ = app.update(Message::Form(F::AcctType("Credit Card".into())));
    assert_eq!(app.form.as_ref().unwrap().detail.as_deref(), Some("Credit Card"));
    let _ = app.update(Message::Form(F::AcctType("Other Expense".into())));
    let _ = app.update(Message::Form(F::Detail("Penalties & Settlements".into())));
    let _ = app.update(Message::Form(F::Name("Parking fines".into())));
    let msgs = click(&app, "Save");
    assert!(matches!(msgs.as_slice(), [Message::Form(F::Save)]), "{msgs:?}");
    let _ = app.update(Message::Form(F::Save));
    assert!(app.form.as_ref().unwrap().busy, "sent to the engine");
    // The engine call the form makes:
    let id = app.be.call_now("accounts.create", json!({"code": "", "name": "Parking fines", "type": "OVERHEADS", "detail_type": "Penalties & Settlements", "description": "", "tax_rate_id": null, "bank_type": null})).unwrap()["id"].as_i64().unwrap();
    let a = app.be.call_now("accounts.get", json!({"id": id})).unwrap();
    assert_eq!((a["type"].as_str(), a["detail_type"].as_str()), (Some("OVERHEADS"), Some("Penalties & Settlements")));
    assert!(!a["code"].as_str().unwrap().is_empty(), "numbered automatically");
    assert_eq!(crate::coa::qb_type("OVERHEADS", ""), "Other Expense");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn delayed_charge_added_to_invoice() {
    use crate::doc::Msg as D;
    let (mut app, dir) = demo_app("delayed", Skin::QuickBooks);
    // + New → Delayed charge.
    let _ = app.update(Message::ToggleNew);
    let msgs = click(&app, "Delayed charge");
    let Some(Message::Nav(r)) = msgs.into_iter().next() else { panic!("Delayed charge isn't enabled in + New") };
    assert_eq!(r, Route::Doc { kind: "DELAYEDCHARGE".into(), id: None });
    let _ = app.update(Message::Nav(r));
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    {
        let e = app.doc.as_mut().unwrap();
        e.contact = Some(cust.clone());
        e.lines[0].description = "Extra site visit".into();
        e.lines[0].price = "180".into();
    }
    let p = app.doc.as_ref().unwrap().payload(&app, "SENT").unwrap();
    assert_eq!((p["doc"]["type"].as_str(), p["credit"].as_bool()), (Some("QUOTE"), Some(false)));
    let charge = app.be.call_now("docs.saveDelayed", p).unwrap()["id"].as_i64().unwrap();
    // A new invoice for that customer offers it; Add copies its lines in.
    let _ = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None }));
    let _ = app.update(Message::Doc(D::Contact(cust.clone())));
    let open = app.be.call_now("docs.openDelayed", json!({"contact_id": cust.id}));
    let _ = app.update(Message::Doc(D::Delayed(open)));
    let msgs = click(&app, "Add");
    assert!(matches!(msgs.as_slice(), [Message::Doc(D::AddDelayed(x))] if *x == charge), "{msgs:?}");
    let _ = app.update(Message::Doc(D::AddDelayed(charge)));
    let e = app.doc.as_ref().unwrap();
    assert!(e.lines.iter().any(|l| l.description == "Extra site visit"));
    assert!(e.delayed.is_empty() && e.added == vec![charge]);
    let p = e.payload(&app, "AUTHORISED").unwrap();
    assert_eq!(p["close_delayed"], json!([charge]));
    let inv = app.be.call_now("docs.save", p).unwrap()["id"].as_i64().unwrap();
    assert_eq!(app.be.call_now("docs.get", json!({"id": charge})).unwrap()["status"], "INVOICED");
    assert!(app.be.call_now("docs.get", json!({"id": inv})).unwrap()["total"].as_i64().unwrap() >= 18000);
    assert_eq!(crate::vocab::status(Skin::QuickBooks, "DELAYEDCHARGE", "INVOICED", false), "Closed");
    let _ = std::fs::remove_dir_all(dir);
}

/// Renders a new invoice with an open delayed charge to `%TEMP%\lc-delayed-wgpu.png` (run with --ignored).
#[test]
#[ignore]
fn render_add_to_invoice() {
    use crate::doc::Msg as D;
    let (mut app, dir) = demo_app("delayed-shot", Skin::QuickBooks);
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    app.be.call_now("docs.saveDelayed", json!({"doc": {"contact_id": cust.id, "date": "2026-10-05", "line_amount_type": "NOTAX", "lines": [{"description": "Extra site visit", "quantity": "1", "unit_price": "180"}]}, "credit": false})).unwrap();
    let _ = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None }));
    let _ = app.update(Message::Doc(D::Contact(cust.clone())));
    let open = app.be.call_now("docs.openDelayed", json!({"contact_id": cust.id}));
    let _ = app.update(Message::Doc(D::Delayed(open)));
    let _ = std::fs::remove_file(std::env::temp_dir().join("lc-delayed-wgpu.png"));
    let _g = sim_lock();
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 900.0), app.view());
    ui.snapshot(&app.theme()).unwrap().matches_image(std::env::temp_dir().join("lc-delayed.png")).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn track_vendor_for_1099() {
    let (mut app, dir) = demo_app("t1099", Skin::QuickBooks);
    // New vendor form (US company) offers 1099 tracking.
    let _ = app.update(Message::Nav(Route::NewContact(false)));
    let msgs = click(&app, "Track payments for 1099");
    assert!(matches!(msgs.as_slice(), [Message::Form(crate::forms::Msg::Track1099(true))]), "{msgs:?}");
    // Vendor page toggle.
    let vendor = app.lk.contact_opts(false).into_iter().next().unwrap();
    let _ = app.update(Message::Nav(Route::Contact(vendor.id)));
    let c = app.be.call_now("contacts.get", json!({"id": vendor.id})).unwrap();
    app.data = Some(json!({"contact": c, "sales": [], "bills": []}));
    if app.data.as_ref().unwrap()["contact"]["is_customer"] == 1 { app.data.as_mut().unwrap()["contact"]["is_customer"] = json!(0); }
    let msgs = click(&app, "Track payments for 1099");
    assert!(matches!(msgs.as_slice(), [Message::Set1099(x, true)] if *x == vendor.id), "{msgs:?}");
    let _ = app.update(Message::Set1099(vendor.id, true));
    app.be.call_now("contacts.set1099", json!({"id": vendor.id, "on": true})).unwrap();
    // The 1099 summary lists them and is in the US report centre.
    let rep = app.be.call_now("reports.run", json!({"name": "contractor-1099", "params": {"from": "2026-01-01", "to": "2026-12-31"}})).unwrap();
    let listed: usize = rep["sections"].as_array().unwrap().iter().map(|s| s["rows"].as_array().unwrap().len()).sum();
    assert_eq!(listed, 1);
    let _ = app.update(Message::Nav(Route::Reports));
    app.data = Some(app.be.call_now("reports.catalogue", json!({})).unwrap());
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        assert!(ui.find("1099 Contractor Summary").is_ok());
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sales_tax_rate_by_agency() {
    use crate::people::Msg as P;
    let (mut app, dir) = demo_app("agency", Skin::QuickBooks);
    let _ = app.update(Message::Nav(Route::Taxes));
    let mut v = app.be.call_now("taxReturns.list", json!({})).unwrap();
    v["agencies"] = app.be.call_now("agencies.list", json!({})).unwrap();
    app.data = Some(v);
    let msgs = click(&app, "Add sales tax rate");
    assert!(matches!(msgs.as_slice(), [Message::People(P::AddRate)]), "{msgs:?}");
    let _ = app.update(Message::People(P::AddRate));
    let _ = app.update(Message::People(P::PartAgency(0, "California CDTFA".into())));
    let _ = app.update(Message::People(P::PartRate(0, "6".into())));
    let _ = app.update(Message::People(P::AddPart));
    let _ = app.update(Message::People(P::PartAgency(1, "LA County".into())));
    let _ = app.update(Message::People(P::PartRate(1, "abc".into())));
    let _ = app.update(Message::People(P::SaveRate));
    assert!(app.toasts.iter().any(|t| t.2) && !app.people.busy, "bad rate is flagged");
    let _ = app.update(Message::People(P::PartRate(1, "1.25".into())));
    let msgs = click(&app, "Save");
    assert!(matches!(msgs.as_slice(), [Message::People(P::SaveRate)]), "{msgs:?}");
    let _ = app.update(Message::People(P::SaveRate));
    assert!(app.people.busy, "sent to the engine");
    // What the form sends:
    let r = app.be.call_now("taxRates.createSales", json!({"name": "Sales Tax 7.25%", "parts": [{"agency": "California CDTFA", "name": "California CDTFA", "rate_bp": 600}, {"agency": "LA County", "name": "LA County", "rate_bp": 125}]})).unwrap();
    let _ = app.update(Message::People(P::Created(Ok(r), "rate")));
    assert!(!app.people.adding_rate);
    let ag = app.be.call_now("agencies.list", json!({})).unwrap();
    assert_eq!(ag.as_array().unwrap().len(), 2);
    let rates = app.be.call_now("taxRates.list", json!({})).unwrap();
    assert!(rates.as_array().unwrap().iter().any(|t| t["name"] == "Sales Tax 7.25%" && t["rate_bp"] == 725));
    let _ = std::fs::remove_dir_all(dir);
}

/// Renders the US Taxes screen with agencies to `%TEMP%\lc-agencies-wgpu.png` (run with --ignored).
#[test]
#[ignore]
fn render_agencies() {
    let (mut app, dir) = demo_app("agency-shot", Skin::QuickBooks);
    app.be.call_now("taxRates.createSales", json!({"name": "Los Angeles 7.25%", "parts": [{"agency": "California CDTFA", "name": "State", "rate_bp": 600}, {"agency": "LA County", "name": "County", "rate_bp": 125}]})).unwrap();
    let _ = app.update(Message::Nav(Route::Taxes));
    let mut v = app.be.call_now("taxReturns.list", json!({})).unwrap();
    v["agencies"] = app.be.call_now("agencies.list", json!({})).unwrap();
    app.data = Some(v);
    let _ = app.update(Message::People(crate::people::Msg::AddRate));
    let _ = std::fs::remove_file(std::env::temp_dir().join("lc-agencies-wgpu.png"));
    let _g = sim_lock();
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 900.0), app.view());
    ui.snapshot(&app.theme()).unwrap().matches_image(std::env::temp_dir().join("lc-agencies.png")).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

/// A booted app in the QuickBooks Desktop layout on a demo company.
fn desktop_app(tag: &str) -> (App, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("lc-ui-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (mut app, _) = App::boot_layout(dir.clone(), None, Some(crate::theme::Layout::Desktop));
    app.be.call_now("orgs.create", json!({"org_name": "Demo Co", "country": "US", "demo": true, "owner_name": "Alex Morgan"})).unwrap();
    let session = app.be.call_now("session.get", json!({})).unwrap();
    let _ = app.update(Message::Session(Ok(session)));
    let get = |m: &str, p: Value| app.be.call_now(m, p).unwrap().as_array().cloned().unwrap();
    let lk = Lookups { accounts: get("accounts.list", json!({})), tax_rates: get("taxRates.list", json!({})), items: get("items.list", json!({})), contacts: get("contacts.list", json!({"filter": "all"})), price_levels: get("priceLevels.list", json!({})), templates: get("branding.list", json!({})), price_rules: get("priceRules.list", json!({})), locations: get("locations.list", json!({})) };
    let _ = app.update(Message::LookupsLoaded(Box::new(lk)));
    (app, dir)
}

#[test]
fn desktop_menus_windows_and_shortcuts() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("desk");
    assert!(app.desktop());
    assert_eq!(app.desk.windows.len(), 1, "Home is open");
    // Lists ▸ Chart of Accounts, then Customers ▸ Create Invoices (from a window without its own Create Invoices icon).
    let _ = app.update(Message::Nav(Route::Chart));
    app.data = Some(app.be.call_now("accounts.list", json!({})).unwrap());
    let msgs = click(&app, "Customers");
    assert!(matches!(msgs.as_slice(), [Message::OpenMenu(Some("Customers"))]), "{msgs:?}");
    let _ = app.update(Message::OpenMenu(Some("Customers")));
    let msgs = click(&app, "Create Invoices");
    let Some(Message::Nav(r)) = msgs.into_iter().next() else { panic!("no navigation") };
    let _ = app.update(Message::Nav(r));
    assert_eq!(app.desk.windows.len(), 3);
    // Type something, open another window, come back: the unsaved invoice is still there.
    app.doc.as_mut().unwrap().reference = "PO-778".into();
    let _ = app.update(Message::Nav(Route::Products));
    assert_eq!(app.desk.windows.len(), 4);
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["Home", "Create Invoices", "Chart of Accounts", "Item List"] { assert!(ui.find(t).is_ok(), "missing tab {t}"); }
    }
    let _ = app.update(Message::Desk(W::Activate(2)));
    assert_eq!(app.doc.as_ref().unwrap().reference, "PO-778", "unsaved form kept with its window");
    // Opening the same screen again brings its window forward instead of a duplicate.
    let _ = app.update(Message::Nav(Route::Chart));
    let _ = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None }));
    assert_eq!(app.desk.windows.len(), 4);
    assert_eq!(app.doc.as_ref().unwrap().reference, "PO-778");
    // Cancel closes the window.
    let _ = app.update(Message::Back);
    assert_eq!(app.desk.windows.len(), 3);
    // Ctrl+W opens Write Checks.
    let key = iced::keyboard::Event::KeyPressed { key: iced::keyboard::Key::Character("w".into()), modified_key: iced::keyboard::Key::Character("w".into()),
        physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::KeyW), location: iced::keyboard::Location::Standard, modifiers: iced::keyboard::Modifiers::CTRL, text: None, repeat: false };
    let _ = app.update(Message::Key(key));
    assert_eq!(app.route, Route::Check(None));
    // Close All leaves Home.
    let _ = app.update(Message::Desk(W::CloseAll));
    assert_eq!(app.desk.windows.len(), 1);
    assert_eq!(app.route, Route::Dashboard);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_customer_center() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("center");
    let _ = app.update(Message::Nav(Route::Customers));
    let list = app.be.call_now("contacts.list", json!({"filter": "customers"})).unwrap();
    let first = list[0]["id"].as_i64().unwrap();
    let second = list[1].clone();
    let _ = app.update(Message::Loaded(app.page_gen, Ok(list.clone())));
    assert_eq!(app.desk.sel, Some(first), "first name selected automatically");
    // Clicking a name selects it; its details arrive and show with its transactions.
    let msgs = click(&app, second["name"].as_str().unwrap());
    let sid = second["id"].as_i64().unwrap();
    assert!(matches!(msgs.as_slice(), [Message::Desk(W::Select(x))] if *x == sid), "{msgs:?}");
    let _ = app.update(Message::Desk(W::Select(sid)));
    let c = app.be.call_now("contacts.get", json!({"id": sid})).unwrap();
    let rows = app.be.call_now("docs.list", json!({"type": "SALES", "contact_id": sid})).unwrap()["rows"].clone();
    let _ = app.update(Message::Desk(W::Detail(sid, Ok(json!({"contact": c, "rows": rows})))));
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["Customer Information", "Customers & Jobs", "BALANCE TOTAL", "New Customer & Job"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    // Opening a contact (e.g. from search) lands in the Center with that name selected.
    let _ = app.update(Message::Nav(Route::Contact(first)));
    assert_eq!((app.route.clone(), app.desk.sel), (Route::Customers, Some(first)));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_create_invoices_save_and_close() {
    use crate::doc::Msg as D;
    let (mut app, dir) = desktop_app("deskinv");
    let _ = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None }));
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["CUSTOMER:JOB", "BILL TO", "INVOICE #", "Save & Close", "Save & New", "CUSTOMER MESSAGE", "Receive Payments"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    let _ = app.update(Message::Doc(D::Contact(cust)));
    let _ = app.update(Message::Doc(D::Desc(0, "Design work".into())));
    let _ = app.update(Message::Doc(D::Price(0, "250".into())));
    let windows = app.desk.windows.len();
    let msgs = click(&app, "Save & Close");
    assert!(matches!(msgs.as_slice(), [Message::Doc(D::SaveClose)]), "{msgs:?}");
    let _ = app.update(Message::Doc(D::SaveClose));
    let p = app.doc.as_ref().unwrap().payload(&app, "AUTHORISED").unwrap();
    let saved = app.be.call_now("docs.save", p);
    let _ = app.update(Message::Doc(D::Saved(saved, false)));
    assert_eq!(app.desk.windows.len(), windows - 1, "the window closes once saved");
    // Bills: the Expenses tab takes an amount directly.
    let _ = app.update(Message::Nav(Route::Doc { kind: "ACCPAY".into(), id: None }));
    let _ = app.update(Message::Doc(D::Amount(0, "99.50".into())));
    assert_eq!(app.doc.as_ref().unwrap().totals(&app).0, 9950);
    let _ = app.update(Message::Doc(D::BillTab(true)));
    assert!(app.doc.as_ref().unwrap().lines.iter().any(|l| l.on_items), "Items tab gets its own line");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_money_forms() {
    use crate::payments::Msg as P;
    let (mut app, dir) = desktop_app("deskpay");
    let _ = app.update(Message::Nav(Route::Check(None)));
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["PAY TO THE", "ORDER OF", "DOLLARS", "MEMO", "ENDING BALANCE", "Save & New", "Save & Close"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    let _ = app.update(Message::Pay(P::LineAmount(0, "1234.56".into())));
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        let w = crate::paydesk::words(123456);
        assert_eq!(w, "One thousand two hundred thirty-four and 56/100");
        let line = format!("{w}{}", "*".repeat(60usize.saturating_sub(w.len())));
        assert!(ui.find(line.as_str()).is_ok(), "amount in words on the check");
    }
    // Save & New keeps the window open and starts a fresh check.
    let windows = app.desk.windows.len();
    let _ = app.update(Message::Pay(P::SaveNew));
    assert!(app.pay.as_ref().unwrap().new_after);
    let _ = app.update(Message::Pay(P::Saved(Ok(json!({"id": 1})))));
    assert_eq!(app.desk.windows.len(), windows);
    // Save & Close closes it.
    let _ = app.update(Message::Pay(P::Save));
    let _ = app.update(Message::Pay(P::Saved(Ok(json!({"id": 1})))));
    assert_eq!(app.desk.windows.len(), windows - 1);
    for (r, want) in [(Route::ReceivePayment(None), "Customer Payment"), (Route::PayBills, "Select Bills to Be Paid"), (Route::Deposit, "Deposit Total")] {
        let _ = app.update(Message::Nav(r));
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        assert!(ui.find(want).is_ok(), "missing {want}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_check_register() {
    let (mut app, dir) = desktop_app("register");
    let _ = app.update(Message::Nav(Route::Register(None)));
    let banks = app.be.call_now("accounts.bank", json!({})).unwrap();
    let acc = banks[0]["id"].as_i64().unwrap();
    let rows = app.be.call_now("bank.transactions", json!({"id": acc, "limit": 1000})).unwrap();
    let bal = banks[0]["ledger_balance_fx"].as_i64().unwrap();
    app.data = Some(json!({"banks": banks, "account": acc, "balance": bal, "rows": rows}));
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["PAYMENT", "DEPOSIT", "BALANCE", "ENDING BALANCE", "Go to"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
        assert!(ui.find(crate::ui::money(bal).as_str()).is_ok(), "top row shows the ledger balance");
    }
    assert_eq!(crate::desktop::title(&app, &Route::Register(None)), banks[0]["name"].as_str().unwrap(), "window titled with the account");
    let _ = std::fs::remove_dir_all(dir);
}

fn desktop_rec_session(tag: &str) -> (App, std::path::PathBuf) {
    let (mut app, dir) = desktop_app(tag);
    let _ = app.update(Message::Nav(Route::Reconcile));
    let acc = app.rec.account.as_ref().unwrap().id;
    let s = app.be.call_now("tickoff.start", json!({"account_id": acc, "statement_date": crate::ui::today_iso(), "ending_balance": 2_000_000})).unwrap();
    app.rec.session = Some(s);
    (app, dir)
}

#[test]
fn desktop_reconcile_two_columns() {
    use crate::reconcile::Msg as R;
    let (mut app, dir) = desktop_rec_session("deskrec");
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["Checks and Payments", "Deposits and Other Credits", "Mark All", "Unmark All", "Leave", "Reconcile Now", "Difference"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    let msgs = click(&app, "Mark All");
    assert!(matches!(msgs.as_slice(), [Message::Rec(R::TickAll(true))]), "{msgs:?}");
    let _ = app.update(Message::Rec(R::TickAll(true)));
    let s = app.rec.session.as_ref().unwrap();
    assert!(s["items"].as_array().unwrap().iter().all(|i| i["cleared"] == true), "everything ticked");
    let _ = std::fs::remove_dir_all(dir);
}

/// Renders the Desktop reconcile window to `%TEMP%\lc-deskrec-wgpu.png` (run with --ignored).
#[test]
#[ignore]
fn render_desktop_reconcile() {
    let (app, dir) = desktop_rec_session("deskrec-shot");
    let _ = std::fs::remove_file(std::env::temp_dir().join("lc-deskrec-wgpu.png"));
    let _g = sim_lock();
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 900.0), app.view());
    ui.snapshot(&app.theme()).unwrap().matches_image(std::env::temp_dir().join("lc-deskrec.png")).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_lists_and_report_window() {
    use crate::reports::Msg as Rp;
    let (mut app, dir) = desktop_app("desklists");
    let _ = app.update(Message::Nav(Route::Chart));
    app.data = Some(app.be.call_now("accounts.list", json!({})).unwrap());
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["Accounts Receivable", "Other Current Liability", "BALANCE TOTAL", "Account: New", "Use Register"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    let _ = app.update(Message::Nav(Route::Products));
    app.data = Some(app.be.call_now("items.list", json!({})).unwrap());
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["Item List", "Item: New", "DESCRIPTION"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    let _ = app.update(Message::Nav(Route::Report("profit-and-loss".into())));
    assert_eq!(crate::desktop::title(&app, &app.route.clone()), "Profit & Loss");
    let p = crate::reports::params(&app);
    app.data = Some(app.be.call_now("reports.run", json!({"name": "profit-and-loss", "params": p})).unwrap());
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["Customize Report", "Memorize", "Hide Header", "Refresh", "Dates", "Columns"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    let _ = app.update(Message::Report(Rp::HideHeader));
    assert!(app.report.hide_header);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_report_center() {
    let (mut app, dir) = desktop_app("deskrc");
    let _ = app.update(Message::Nav(Route::Reports));
    app.data = Some(app.be.call_now("reports.catalogue", json!({})).unwrap());
    let msgs = click(&app, "Vendors & Payables");
    assert!(matches!(msgs.as_slice(), [Message::Desk(crate::desktop::Msg::ReportCat("Vendors & Payables"))]), "{msgs:?}");
    let _ = app.update(Message::Desk(crate::desktop::Msg::ReportCat("Vendors & Payables")));
    let _g = sim_lock();
    let mut ui = simulator(app.view());
    for t in ["A/P Aging Summary", "1099 Contractor Summary", "Standard", "Memorized"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    drop(ui);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_income_and_bill_trackers() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("tracker");
    let _ = app.update(Message::Nav(Route::AllSales));
    let mut rows = vec![];
    for k in ["SALES", "QUOTE", "DELAYED"] { rows.extend(app.be.call_now("docs.list", json!({"type": k, "limit": 1000})).unwrap()["rows"].as_array().cloned().unwrap()); }
    app.data = Some(json!({"rows": rows}));
    assert_eq!(crate::desktop::title(&app, &Route::AllSales), "Income Tracker");
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["CUSTOMER:JOB", "OPEN BALANCE", "Manage Transactions", "Receive Payment"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    // Clicking a box filters; clicking it again clears the filter.
    let _ = app.update(Message::Desk(W::Tracker("overdue")));
    assert_eq!(app.desk.tracker, "overdue");
    let _ = app.update(Message::Desk(W::Tracker("overdue")));
    assert_eq!(app.desk.tracker, "all");
    let _ = app.update(Message::Nav(Route::Bills));
    assert_eq!(crate::desktop::title(&app, &Route::Bills), "Bill Tracker");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_small_forms() {
    use crate::forms::Msg as F;
    let (mut app, dir) = desktop_app("deskforms");
    // New Customer: name, person and address reach the engine; Save & Close closes the window.
    let _ = app.update(Message::Nav(Route::NewContact(true)));
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["CUSTOMER NAME", "Address Info", "FULL NAME", "INVOICE/BILL TO", "Save & Close"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    for m in [F::Name("Hilltop Cafe".into()), F::First("Sam".into()), F::Last("Lee".into()), F::Address("1 Hill St".into())] { let _ = app.update(Message::Form(m)); }
    let windows = app.desk.windows.len();
    let _ = app.update(Message::Form(F::Save));
    let r = app.be.call_now("contacts.create", json!({"name": "Hilltop Cafe", "first_name": "Sam", "last_name": "Lee", "postal_address": "1 Hill St", "is_customer": true}));
    let id = r.as_ref().unwrap()["id"].as_i64().unwrap();
    let _ = app.update(Message::Form(F::Saved(r)));
    assert_eq!(app.desk.windows.len(), windows - 1);
    let c = app.be.call_now("contacts.get", json!({"id": id})).unwrap();
    assert_eq!((c["first_name"].as_str(), c["postal_address"].as_str()), (Some("Sam"), Some("1 Hill St")));
    // Journal entry and transfer screens.
    for (r, want) in [(Route::Journal, "ENTRY NO."), (Route::Transfer, "TRANSFER FUNDS FROM"), (Route::NewProduct, "ITEM NAME/NUMBER")] {
        let _ = app.update(Message::Nav(r));
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        assert!(ui.find(want).is_ok(), "missing {want}");
    }
    // Save & New on the journal keeps the window.
    let _ = app.update(Message::Nav(Route::Journal));
    let windows = app.desk.windows.len();
    let _ = app.update(Message::Form(F::SaveNew));
    let _ = app.update(Message::Form(F::Saved(Ok(json!({"id": 1})))));
    assert_eq!(app.desk.windows.len(), windows);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_employee_center() {
    let (mut app, dir) = desktop_app("deskemp");
    let _ = app.update(Message::Nav(Route::Payroll));
    let emps = app.be.call_now("payroll.employees", json!({})).unwrap();
    let runs = app.be.call_now("payroll.payRuns", json!({})).unwrap();
    app.data = Some(json!({"employees": emps, "runs": runs, "settings": {}}));
    assert_eq!(crate::desktop::title(&app, &Route::Payroll), "Employee Center");
    let second = &emps[1];
    let name = format!("{} {}", second["first_name"].as_str().unwrap(), second["last_name"].as_str().unwrap());
    let msgs = click(&app, &name);
    let id = second["id"].as_i64().unwrap();
    assert!(matches!(msgs.as_slice(), [Message::Desk(crate::desktop::Msg::Employee(x))] if *x == id), "{msgs:?}");
    let _ = app.update(Message::Desk(crate::desktop::Msg::Employee(id)));
    let _g = sim_lock();
    let mut ui = simulator(app.view());
    for t in ["Employee Information", "Hire Date", "Pay Employees", "Transactions"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    drop(ui);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_preferences() {
    use crate::settings::Msg as S;
    let (mut app, dir) = desktop_app("deskprefs");
    let _ = app.update(Message::Nav(Route::Settings));
    assert_eq!(crate::desktop::title(&app, &Route::Settings), "Preferences");
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["My Preferences", "Company Preferences", "Desktop View", "QuickBooks Desktop-style", "Send Forms"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    let msgs = click(&app, "Send Forms");
    assert!(matches!(msgs.as_slice(), [Message::Settings(S::Cat("Send Forms"))]), "{msgs:?}");
    let _ = app.update(Message::Settings(S::Cat("Send Forms")));
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        assert!(ui.find("Email (outgoing mail)").is_ok());
    }
    // Switching layout from Preferences works and the window list goes away with Desktop.
    let msgs = click(&app, "Desktop View");
    for m in msgs { let _ = app.update(m); }
    let msgs = click(&app, "Xero-style");
    assert!(matches!(msgs.as_slice(), [Message::SetLayout(crate::theme::Layout::Xero)]), "{msgs:?}");
    let _ = app.update(Message::SetLayout(crate::theme::Layout::Xero));
    assert!(!app.desktop() && app.desk.windows.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_doc_lists() {
    let (mut app, dir) = desktop_app("desklistdocs");
    for (r, t, new) in [(Route::Quotes, "QUOTE", "New Estimate"), (Route::CreditNotes, "ACCRECCREDIT", "New Credit Memo"), (Route::PurchaseOrders, "PO", "New Purchase Order")] {
        let _ = app.update(Message::Nav(r.clone()));
        app.data = Some(app.be.call_now("docs.list", json!({"type": t})).unwrap());
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        assert!(ui.find(new).is_ok(), "missing {new}");
        assert!(ui.find("NUM").is_ok());
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_form_history_panel() {
    use crate::doc::Msg as D;
    let (mut app, dir) = desktop_app("deskhist");
    let _ = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None }));
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    let _ = app.update(Message::Doc(D::Contact(cust.clone())));
    let rows = app.be.call_now("docs.list", json!({"type": "SALES", "contact_id": cust.id, "limit": 6}));
    let _ = app.update(Message::Doc(D::History(rows)));
    assert!(!app.doc.as_ref().unwrap().history.is_empty());
    let _g = sim_lock();
    let mut ui = simulator(app.view());
    for t in ["SUMMARY", "RECENT TRANSACTIONS", "Open balance", "Edit / QuickReport"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    drop(ui);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_calendar() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("deskcal");
    let _ = app.update(Message::Nav(Route::Calendar));
    let mut rows = vec![];
    for k in ["ACCREC", "ACCPAY"] { rows.extend(app.be.call_now("docs.list", json!({"type": k, "tab": "awaiting"})).unwrap()["rows"].as_array().cloned().unwrap()); }
    app.data = Some(json!({"rows": rows}));
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["SUN", "SAT", "UPCOMING: NEXT 7 DAYS", "DUE: PAST DUE", "Today"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    let _ = app.update(Message::Desk(W::CalMonth(1)));
    let _ = app.update(Message::Desk(W::CalMonth(1)));
    assert_eq!(app.desk.cal_offset, 2);
    let _ = app.update(Message::Desk(W::CalMonth(0)));
    assert_eq!(app.desk.cal_offset, 0, "Today goes back to this month");
    let d = chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    assert_eq!(crate::ui::add_months_date(d, 2), chrono::NaiveDate::from_ymd_opt(2027, 1, 1).unwrap());
    assert_eq!(crate::ui::add_months_date(d, -11), chrono::NaiveDate::from_ymd_opt(2025, 12, 1).unwrap());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_create_statements() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("deskstmt");
    let _ = app.update(Message::Nav(Route::Statements));
    let list = app.be.call_now("contacts.list", json!({"filter": "customers"})).unwrap();
    app.data = Some(list.clone());
    let owing: Vec<i64> = list.as_array().unwrap().iter().filter(|c| c["receivable"].as_i64().unwrap_or(0) > 0).map(|c| c["id"].as_i64().unwrap()).collect();
    assert!(!owing.is_empty());
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["SELECT CUSTOMERS", "All Customers (with an open balance)", "Print", "E-mail"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    // The statements: one section per customer, totals equal what they owe.
    let rep = crate::desktop::statement_report(&app.be, &owing, &crate::ui::today_iso(), crate::ui::DateStyle::Us).unwrap();
    let secs = rep["sections"].as_array().unwrap();
    assert_eq!(secs.len(), owing.len());
    for (s, id) in secs.iter().zip(&owing) {
        let want = list.as_array().unwrap().iter().find(|c| c["id"].as_i64() == Some(*id)).unwrap()["receivable"].as_i64().unwrap();
        let credits: i64 = app.be.call_now("docs.list", json!({"type": "ACCRECCREDIT", "contact_id": id, "tab": "awaiting"})).unwrap()["rows"].as_array().unwrap().iter().map(|r| r["amount_due"].as_i64().unwrap_or(0)).sum();
        assert_eq!(s["total"]["values"][0].as_i64().unwrap(), want - credits);
    }
    let pdf = crate::pdf::report(&rep, "Statement", true, &|s: &str| s.to_string());
    assert!(pdf.starts_with(b"%PDF"));
    // One customer.
    let one = crate::app::Opt { id: owing[0], label: "x".into() };
    let _ = app.update(Message::Desk(W::StmtOne(Some(one))));
    assert_eq!(app.desk.stmt_one.as_ref().unwrap().id, owing[0]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_memorize_transaction() {
    use crate::doc::Msg as D;
    let (mut app, dir) = desktop_app("deskmemo");
    let _ = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None }));
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    let _ = app.update(Message::Doc(D::Contact(cust)));
    let _ = app.update(Message::Doc(D::Desc(0, "Monthly retainer".into())));
    let _ = app.update(Message::Doc(D::Price(0, "500".into())));
    let msgs = click(&app, "Memorize");
    assert!(matches!(msgs.as_slice(), [Message::Doc(D::Memorize)]), "{msgs:?}");
    let _ = app.update(Message::Doc(D::Memorize));
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["Memorize Transaction", "HOW OFTEN", "NEXT DATE"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    // Schedule it for today so it can be entered now.
    let _ = app.update(Message::Doc(D::MemoNext(crate::ui::date(&crate::ui::today_iso(), app.date_style()))));
    let _ = app.update(Message::Doc(D::MemoFreq("Quarterly")));
    let _ = app.update(Message::Doc(D::MemoAuto(true)));
    let _ = app.update(Message::Doc(D::MemoSave));
    // The call the panel makes:
    let e = app.doc.as_ref().unwrap();
    let p = e.payload(&app, "AUTHORISED").unwrap();
    assert_eq!(crate::doc::memo_schedule("Quarterly"), ("MONTHLY", 3));
    app.be.call_now("repeating.save", json!({"repeating": {"kind": "ACCREC", "template": p["doc"], "period_unit": "MONTHLY", "period_count": 3,
        "start_date": crate::ui::today_iso(), "next_date": crate::ui::today_iso(), "save_as": "AUTHORISED"}})).unwrap();
    let _ = app.update(Message::Doc(D::Memorized(Ok(json!({"id": 1})))));
    assert!(!app.doc.as_ref().unwrap().memo_open);
    // Memorized Transaction List shows it; Enter All Due Now creates the invoice.
    let _ = app.update(Message::Nav(Route::Memorized));
    app.data = Some(app.be.call_now("repeating.list", json!({"kind": "ALL"})).unwrap());
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["Quarterly", "Enter All Due Now", "Invoice"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    let before = app.be.call_now("docs.list", json!({"type": "ACCREC", "limit": 1000})).unwrap()["rows"].as_array().unwrap().len();
    let ran = app.be.call_now("repeating.run", json!({})).unwrap();
    assert!(ran.as_array().unwrap().iter().any(|x| x["kind"] == "ACCREC"));
    let after = app.be.call_now("docs.list", json!({"type": "ACCREC", "limit": 1000})).unwrap()["rows"].as_array().unwrap().len();
    assert!(after > before, "the memorized invoice was entered");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_apply_credit_to_invoice() {
    use crate::doc::Msg as D;
    let (mut app, dir) = desktop_app("deskapply");
    // An open invoice and a credit memo for the same customer.
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    let mk = |kind: &str, price: &str| app.be.call_now("docs.save", json!({"doc": {"type": kind, "contact_id": cust.id, "date": crate::ui::today_iso(), "line_amount_type": "NOTAX", "status": "AUTHORISED",
        "lines": [{"description": "x", "quantity": "1", "unit_price": price, "account_id": app.lk.accounts.iter().find(|a| a["code"] == "200").unwrap()["id"]}]}})).unwrap()["id"].as_i64().unwrap();
    let inv = mk("ACCREC", "300");
    let cm = mk("ACCRECCREDIT", "120");
    let _ = app.update(Message::Nav(Route::Doc { kind: "ACCRECCREDIT".into(), id: Some(cm) }));
    let d = app.be.call_now("docs.get", json!({"id": cm}));
    let _ = app.update(Message::Doc(D::Loaded(d)));
    let msgs = click(&app, "Apply to Invoice");
    assert!(matches!(msgs.as_slice(), [Message::Doc(D::ApplyOpen)]), "{msgs:?}");
    let _ = app.update(Message::Doc(D::ApplyOpen));
    let open = app.be.call_now("docs.list", json!({"type": "ACCREC", "contact_id": cust.id, "tab": "awaiting", "limit": 200}));
    let _ = app.update(Message::Doc(D::ApplyDocs(open)));
    // Only apply to our new invoice.
    let list = app.doc.as_ref().unwrap().apply.clone().unwrap();
    for (n, (d, _)) in list.iter().enumerate() { let v = if d["id"].as_i64() == Some(inv) { "120.00" } else { "" }; let _ = app.update(Message::Doc(D::ApplyAmt(n, v.into()))); }
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        assert!(ui.find("Apply credit to Invoices").is_ok());
    }
    let _ = app.update(Message::Doc(D::ApplySave));
    app.be.call_now("credits.allocate", json!({"credit_id": cm, "document_id": inv, "amount": "120.00"})).unwrap();
    let _ = app.update(Message::Doc(D::Applied(Ok(json!({"applied": 1})))));
    assert!(app.doc.as_ref().map(|e| e.apply.is_none()).unwrap_or(true));
    assert_eq!(app.be.call_now("docs.get", json!({"id": inv})).unwrap()["amount_due"], 18000);
    assert_eq!(app.be.call_now("docs.get", json!({"id": cm})).unwrap()["amount_due"], 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_give_refund() {
    use crate::doc::Msg as D;
    let (mut app, dir) = desktop_app("deskrefund");
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    let cm = app.be.call_now("docs.save", json!({"doc": {"type": "ACCRECCREDIT", "contact_id": cust.id, "date": crate::ui::today_iso(), "line_amount_type": "NOTAX", "status": "AUTHORISED",
        "lines": [{"description": "Returned goods", "quantity": "1", "unit_price": "75", "account_id": app.lk.accounts.iter().find(|a| a["code"] == "200").unwrap()["id"]}]}})).unwrap()["id"].as_i64().unwrap();
    let _ = app.update(Message::Nav(Route::Doc { kind: "ACCRECCREDIT".into(), id: Some(cm) }));
    let d = app.be.call_now("docs.get", json!({"id": cm}));
    let _ = app.update(Message::Doc(D::Loaded(d)));
    let msgs = click(&app, "Give Refund");
    assert!(matches!(msgs.as_slice(), [Message::Doc(D::RefundOpen)]), "{msgs:?}");
    let _ = app.update(Message::Doc(D::RefundOpen));
    assert_eq!(app.doc.as_ref().unwrap().pay_amount, "75.00", "refund defaults to the credit left");
    let _ = app.update(Message::Doc(D::AddPayment));
    let e = app.doc.as_ref().unwrap();
    let r = app.be.call_now("payments.add", json!({"document_id": cm, "account_id": e.pay_account.as_ref().unwrap().id, "date": crate::ui::today_iso(), "amount": "75.00", "reference": ""}));
    assert!(r.is_ok(), "{r:?}");
    assert_eq!(app.be.call_now("docs.get", json!({"id": cm})).unwrap()["status"], "PAID");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_register_entry_row() {
    use crate::desktop::{Msg as W, RegField as F};
    let (mut app, dir) = desktop_app("regentry");
    let _ = app.update(Message::Nav(Route::Register(None)));
    let banks = app.be.call_now("accounts.bank", json!({})).unwrap();
    let acc = banks[0]["id"].as_i64().unwrap();
    app.data = Some(json!({"banks": banks, "account": acc, "balance": banks[0]["ledger_balance_fx"], "rows": app.be.call_now("bank.transactions", json!({"id": acc})).unwrap()}));
    // Missing account → refused before reaching the engine.
    let _ = app.update(Message::Desk(W::Reg(F::Payment, "45.50".into())));
    let _ = app.update(Message::Desk(W::RegRecord));
    assert!(app.toasts.iter().any(|t| t.2) && !app.desk.reg.busy);
    // Payment and deposit are either/or.
    let _ = app.update(Message::Desk(W::Reg(F::Deposit, "10".into())));
    assert!(app.desk.reg.payment.is_empty());
    let _ = app.update(Message::Desk(W::Reg(F::Payment, "45.50".into())));
    assert!(app.desk.reg.deposit.is_empty());
    let expense = app.lk.account_opts(|a| a["code"] == "429").into_iter().next().unwrap();
    let _ = app.update(Message::Desk(W::RegAccount(expense)));
    let _ = app.update(Message::Desk(W::Reg(F::Number, "1050".into())));
    let _ = app.update(Message::Desk(W::Reg(F::Payee, "Corner Hardware".into())));
    let _ = app.update(Message::Desk(W::RegRecord));
    assert!(app.desk.reg.busy, "sent to the engine");
    // What the row sends: a check (it has a number) for 45.50.
    let tx = json!({"tx": {"type": "SPEND", "account_id": acc, "date": crate::ui::today_iso(), "reference": "", "line_amount_type": "INCLUSIVE", "subtype": "CHECK", "check_number": "1050",
        "contact_name": "Corner Hardware", "lines": [{"account_id": app.desk.reg.account.as_ref().unwrap().id, "description": "", "amount": "45.50"}]}});
    let r = app.be.call_now("banktx.save", tx);
    assert!(r.is_ok(), "{r:?}");
    let _ = app.update(Message::Desk(W::RegSaved(r)));
    assert!(app.desk.reg.payment.is_empty() && app.desk.reg.account.is_none(), "row cleared for the next entry");
    let rows = app.be.call_now("bank.transactions", json!({"id": acc})).unwrap();
    assert!(rows.as_array().unwrap().iter().any(|r| r["description"] == "Check 1050"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn report_quickzoom_to_account() {
    use crate::reports::Msg as Rp;
    let (mut app, dir) = desktop_app("zoom");
    let _ = app.update(Message::Nav(Route::Report("profit-and-loss".into())));
    let _ = app.update(Message::Report(Rp::Period(crate::reports::Period::Last12Months)));
    let p = crate::reports::params(&app);
    let rep = app.be.call_now("reports.run", json!({"name": "profit-and-loss", "params": p})).unwrap();
    let row = rep["sections"].as_array().unwrap().iter().flat_map(|s| s["rows"].as_array().cloned().unwrap_or_default()).find(|r| r["account_id"].is_i64()).unwrap();
    app.data = Some(rep.clone());
    let label = row["label"].as_str().unwrap().to_string();
    let msgs = click(&app, &label);
    let acc = row["account_id"].as_i64().unwrap();
    assert!(matches!(msgs.as_slice(), [Message::Report(Rp::Zoom(x))] if *x == acc), "{msgs:?}");
    let _ = app.update(Message::Report(Rp::Zoom(acc)));
    assert_eq!(app.route, Route::Report("account-transactions".into()));
    let p = crate::reports::params(&app);
    assert_eq!(p["account_id"], acc);
    let detail = app.be.call_now("reports.run", json!({"name": "account-transactions", "params": p})).unwrap();
    assert_eq!(detail["sections"].as_array().unwrap().len(), 1, "just that account");
    assert_eq!(detail["sections"][0]["account_id"], acc);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_center_transactions_tab() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("centertx");
    let _ = app.update(Message::Nav(Route::Customers));
    app.data = Some(app.be.call_now("contacts.list", json!({"filter": "customers"})).unwrap());
    let msgs = click(&app, "Transactions");
    assert!(matches!(msgs.as_slice(), [Message::Desk(W::TxType(Some("Invoices")))]), "{msgs:?}");
    let _ = app.update(Message::Desk(W::TxType(Some("Invoices"))));
    let rows = app.be.call_now("docs.list", json!({"type": "ACCREC", "limit": 1000}));
    let _ = app.update(Message::Desk(W::TxRows("Invoices", rows)));
    assert!(!app.desk.tx_rows.is_empty());
    assert!(app.desk.tx_rows.iter().all(|r| r["subtype"] != "RECEIPT"), "sales receipts have their own entry");
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["Estimates", "Statement Charges", "Sales Receipts", "Credit Memos", "OPEN BALANCE"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    // Back to the names tab.
    let _ = app.update(Message::Desk(W::TxType(None)));
    assert!(app.desk.tx_type.is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_one_window_view() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("onewin");
    let _ = app.update(Message::Nav(Route::Chart));
    let _ = app.update(Message::Nav(Route::Products));
    assert_eq!(app.desk.windows.len(), 3);
    let _ = app.update(Message::Desk(W::OneWindow(true)));
    assert_eq!(app.desk.windows.len(), 1, "only the current window stays");
    assert_eq!(app.desk.windows[0].route, Route::Products);
    let _ = app.update(Message::Nav(Route::Customers));
    assert_eq!(app.desk.windows.len(), 1);
    assert_eq!(app.desk.windows[0].route, Route::Customers);
    let _ = app.update(Message::Desk(W::OneWindow(false)));
    let _ = app.update(Message::Nav(Route::Chart));
    assert_eq!(app.desk.windows.len(), 2, "multiple windows again");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_pay_sales_tax() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("paytax");
    // A rate with one agency, and an invoice that charges it today.
    let rate = app.be.call_now("taxRates.createSales", json!({"name": "State 6%", "parts": [{"agency": "State Tax Dept", "name": "State", "rate_bp": 600}]})).unwrap()["id"].as_i64().unwrap();
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    let sales = app.lk.accounts.iter().find(|a| a["code"] == "200").unwrap()["id"].clone();
    app.be.call_now("docs.save", json!({"doc": {"type": "ACCREC", "contact_id": cust.id, "date": crate::ui::today_iso(), "line_amount_type": "EXCLUSIVE", "status": "AUTHORISED",
        "lines": [{"description": "x", "quantity": "1", "unit_price": "1000", "account_id": sales, "tax_rate_id": rate}]}})).unwrap();
    let _ = app.update(Message::Nav(Route::PaySalesTax));
    let ag = app.be.call_now("agencies.list", json!({})).unwrap();
    app.data = Some(ag.clone());
    let agency = ag.as_array().unwrap().iter().find(|a| a["name"] == "State Tax Dept").unwrap().clone();
    assert!(agency["tax"].as_i64().unwrap() >= 6000);
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["PAY FROM ACCOUNT", "CHECK DATE", "State Tax Dept", "OK"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    let id = agency["id"].as_i64().unwrap();
    let _ = app.update(Message::Desk(W::TaxPick(id, true)));
    assert_eq!(app.desk.tax_pay[0].2, crate::ui::plain(agency["tax"].as_i64().unwrap()), "defaults to what was collected");
    let _ = app.update(Message::Desk(W::TaxAmt(id, "60.00".into())));
    let liab = app.lk.accounts.iter().find(|a| a["system_account"] == "GST").unwrap()["id"].as_i64().unwrap();
    let bal = |app: &App| app.be.call_now("accounts.list", json!({})).unwrap().as_array().unwrap().iter().find(|a| a["id"].as_i64() == Some(liab)).unwrap()["ytd_balance"].as_i64().unwrap();
    let before = bal(&app);
    let _ = app.update(Message::Desk(W::TaxPay));
    assert!(app.desk.tax_busy);
    // The payment the window records:
    let bank = crate::payments::bank_only(&app)[0].id;
    app.be.call_now("banktx.save", json!({"tx": {"type": "SPEND", "account_id": bank, "date": crate::ui::today_iso(), "reference": "Sales tax payment", "contact_name": "State Tax Dept",
        "line_amount_type": "NOTAX", "lines": [{"account_id": liab, "description": "Sales tax payment — State Tax Dept", "amount": "60.00"}]}})).unwrap();
    assert_eq!(bal(&app) - before, 6000, "liability (a credit balance) reduced by 60.00");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_adjust_inventory_and_receive_items() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("adjinv");
    let _ = app.update(Message::Nav(Route::AdjustInventory));
    let items = app.be.call_now("items.list", json!({})).unwrap();
    app.data = Some(items.clone());
    let tracked = items.as_array().unwrap().iter().find(|x| x["is_tracked"].as_i64() == Some(1)).expect("demo has a tracked item").clone();
    let id = tracked["id"].as_i64().unwrap();
    let cur = tracked["qty_on_hand_e4"].as_i64().unwrap();
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["ADJUSTMENT ACCOUNT", "NEW QUANTITY", "QTY DIFFERENCE", tracked["name"].as_str().unwrap()] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    let new_qty = format!("{}", (cur - 10000) as f64 / 10000.0);
    let _ = app.update(Message::Desk(W::AdjQty(id, new_qty)));
    let _ = app.update(Message::Desk(W::AdjSave));
    assert!(app.desk.adj_busy, "sent to the engine");
    let acc = app.lk.account_opts(|a| a["type"] == "DIRECTCOSTS")[0].id;
    app.be.call_now("items.adjust", json!({"id": id, "date": crate::ui::today_iso(), "quantity": "-1", "account_id": acc, "reason": ""})).unwrap();
    let after = app.be.call_now("items.list", json!({})).unwrap().as_array().unwrap().iter().find(|x| x["id"].as_i64() == Some(id)).unwrap()["qty_on_hand_e4"].as_i64().unwrap();
    assert_eq!(after, cur - 10000);
    // Receive Items opens the item receipt window (stock in now, bill later).
    let _ = app.update(Message::ReceiveItems);
    assert_eq!(app.route, Route::ItemReceipts);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn back_up_company_to_file() {
    let (app, dir) = desktop_app("backup");
    let target = dir.join("my backup.lcdb");
    ledger_server::jobs::backup_to(&app.be.st, &target).unwrap();
    assert!(target.exists());
    // The copy is a complete company file.
    let copy = ledger_core::Ledger::open(&target).unwrap();
    let n: i64 = copy.conn.query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0)).unwrap();
    assert!(n > 10);
    // Backing up again over the same file replaces it.
    drop(copy);
    ledger_server::jobs::backup_to(&app.be.st, &target).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn restore_backup_as_new_company() {
    let (app, dir) = desktop_app("restore");
    let backup = dir.join("demo backup.lcdb");
    ledger_server::jobs::backup_to(&app.be.st, &backup).unwrap();
    let before = app.be.st.list_orgs().len();
    let file = app.be.st.restore_backup(&backup).unwrap();
    assert!(file.contains("restored"), "{file}");
    assert_eq!(app.be.st.list_orgs().len(), before + 1, "added alongside, nothing replaced");
    let s = app.be.call_now("session.get", json!({})).unwrap();
    assert_eq!(s["org"]["name"], "Demo Co", "the restored company is open");
    // Restoring the same file again makes another copy rather than overwriting.
    let again = app.be.st.restore_backup(&backup).unwrap();
    assert_ne!(again, file);
    // Not a company file → refused.
    let junk = dir.join("junk.lcdb");
    std::fs::write(&junk, b"not a database").unwrap();
    assert!(app.be.st.restore_backup(&junk).is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn help_window_and_alt_t() {
    let (mut app, dir) = desktop_app("help");
    let key = |c: &str, m: iced::keyboard::Modifiers, code: iced::keyboard::key::Code, named: Option<iced::keyboard::key::Named>| iced::keyboard::Event::KeyPressed {
        key: named.map(iced::keyboard::Key::Named).unwrap_or(iced::keyboard::Key::Character(c.into())), modified_key: iced::keyboard::Key::Character(c.into()),
        physical_key: iced::keyboard::key::Physical::Code(code), location: iced::keyboard::Location::Standard, modifiers: m, text: None, repeat: false };
    let _ = app.update(Message::Key(key("", iced::keyboard::Modifiers::empty(), iced::keyboard::key::Code::F1, Some(iced::keyboard::key::Named::F1))));
    assert_eq!(app.route, Route::Help);
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["Keyboard shortcuts", "Where things are", "About LedgerCraft"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    let dark = app.dark;
    let _ = app.update(Message::Key(key("t", iced::keyboard::Modifiers::ALT, iced::keyboard::key::Code::KeyT, None)));
    assert_ne!(app.dark, dark, "Alt+T toggles light/dark");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_sales_orders() {
    use crate::doc::Msg as D;
    let (mut app, dir) = desktop_app("salesorder");
    let _ = app.update(Message::Nav(Route::Doc { kind: "SALESORDER".into(), id: None }));
    assert_eq!(crate::desktop::title(&app, &app.route.clone()), "Create Sales Orders");
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["Sales Order", "S.O. NO.", "Save & Close"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    let _ = app.update(Message::Doc(D::Contact(cust)));
    let _ = app.update(Message::Doc(D::Desc(0, "40 branded mugs".into())));
    let _ = app.update(Message::Doc(D::Price(0, "640".into())));
    let p = app.doc.as_ref().unwrap().payload(&app, "SENT").unwrap();
    assert_eq!(p["doc"]["type"], "QUOTE");
    let so = app.be.call_now("docs.saveSalesOrder", p).unwrap()["id"].as_i64().unwrap();
    // Income Tracker: in the Sales Orders box, not Estimates.
    let _ = app.update(Message::Nav(Route::AllSales));
    let mut rows = vec![];
    for k in ["SALES", "QUOTE", "SALESORDER", "DELAYED"] { rows.extend(app.be.call_now("docs.list", json!({"type": k, "limit": 1000})).unwrap()["rows"].as_array().cloned().unwrap()); }
    app.data = Some(json!({"rows": rows}));
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        assert!(ui.find("1 SALES ORDERS").is_ok());
    }
    // Create Invoice from the order closes it.
    let inv = app.be.call_now("docs.copy", json!({"id": so, "to": "ACCREC"})).unwrap()["id"].as_i64().unwrap();
    assert!(inv > 0);
    let d = app.be.call_now("docs.get", json!({"id": so})).unwrap();
    assert_eq!(crate::vocab::status(Skin::QuickBooks, "SALESORDER", d["status"].as_str().unwrap(), false), "Closed");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn attach_file_to_document() {
    let (mut app, dir) = desktop_app("attach");
    assert_eq!(crate::doc::b64(b"Man"), "TWFu");
    assert_eq!(crate::doc::b64(b"Ma"), "TWE=");
    assert_eq!(crate::doc::b64(b"M"), "TQ==");
    assert_eq!(crate::doc::mime_for("Receipt.PDF"), "application/pdf");
    let inv = app.be.call_now("docs.list", json!({"type": "ACCREC", "limit": 1})).unwrap()["rows"][0]["id"].as_i64().unwrap();
    let data = b"%PDF-1.4 receipt scan";
    app.be.call_now("attachments.add", json!({"entity": "document", "entity_id": inv, "filename": "receipt.pdf", "mime": "application/pdf", "data": crate::doc::b64(data)})).unwrap();
    let _ = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: Some(inv) }));
    let d = app.be.call_now("docs.get", json!({"id": inv}));
    let _ = app.update(Message::Doc(crate::doc::Msg::Loaded(d)));
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        assert!(ui.find("receipt.pdf").is_ok(), "attachment listed on the form");
        assert!(ui.find("ATTACHMENTS").is_ok());
    }
    // The stored bytes round-trip.
    let aid = app.doc.as_ref().unwrap().doc.as_ref().unwrap()["attachments"][0]["id"].as_i64().unwrap();
    let (_, _, bytes) = app.be.st.org.lock().unwrap().as_ref().unwrap().ledger.attachment(aid).unwrap();
    assert_eq!(bytes, data);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_doc_center() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("doccenter");
    let text_doc = "Officeworks\nTax Invoice 4471\nDate 03/10/2026\nTotal $123.45\n";
    app.be.call_now("inbox.upload", json!({"filename": "officeworks.txt", "mime": "text/plain", "data": crate::doc::b64(text_doc.as_bytes())})).unwrap();
    let _ = app.update(Message::Nav(Route::DocCenter));
    let list = app.be.call_now("inbox.list", json!({})).unwrap();
    app.data = Some(list.clone());
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["Add a Document", "officeworks.txt", "Create Bill", "View"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    let id = list[0]["id"].as_i64().unwrap();
    let msgs = click(&app, "Create Bill");
    assert!(matches!(msgs.as_slice(), [Message::Desk(W::DocToBill(x))] if *x == id), "{msgs:?}");
    let bill = app.be.call_now("inbox.toBill", json!({"id": id, "overrides": {}}));
    assert!(bill.is_ok(), "{bill:?}");
    let _ = app.update(Message::Desk(W::DocBill(bill)));
    assert!(matches!(app.route, Route::Doc { ref kind, id: Some(_) } if kind == "ACCPAY"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_pay_liabilities() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("payliab");
    // Make sure some withholding is owed: wages expense / withholding payable.
    let all = app.be.call_now("accounts.list", json!({})).unwrap();
    let wid = all.as_array().unwrap().iter().find(|a| a["system_account"] == "PAYGWITHHOLDING").unwrap()["id"].clone();
    let exp = all.as_array().unwrap().iter().find(|a| a["system_account"] == "WAGESEXPENSE").unwrap()["id"].clone();
    app.be.call_now("mj.save", json!({"id": null, "journal": {"date": crate::ui::today_iso(), "narration": "Withholding", "line_amount_type": "NOTAX", "status": "POSTED",
        "lines": [{"account_id": exp, "description": "", "debit": "500", "credit": ""}, {"account_id": wid, "description": "", "debit": "", "credit": "500"}]}})).unwrap();
    let _ = app.update(Message::Nav(Route::PayLiabilities));
    let accts = app.be.call_now("accounts.list", json!({})).unwrap();
    app.data = Some(accts.clone());
    let w = accts.as_array().unwrap().iter().find(|a| a["system_account"] == "PAYGWITHHOLDING").unwrap().clone();
    let owed = -w["ytd_balance"].as_i64().unwrap();
    assert!(owed >= 50000, "withholding is owed");
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["Pay Liabilities", "PAYABLE TO", "Internal Revenue Service", "Create Payment"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    let id = w["id"].as_i64().unwrap();
    let _ = app.update(Message::Desk(W::LiabPick(id, true)));
    assert_eq!(app.desk.liab_pay[0].2, crate::ui::plain(owed), "defaults to the balance owed");
    let _ = app.update(Message::Desk(W::LiabPay));
    assert!(app.desk.tax_busy);
    let bank = crate::payments::bank_only(&app)[0].id;
    app.be.call_now("banktx.save", json!({"tx": {"type": "SPEND", "account_id": bank, "date": crate::ui::today_iso(), "reference": "Payroll liability payment", "contact_name": "Internal Revenue Service",
        "line_amount_type": "NOTAX", "lines": [{"account_id": id, "description": "Payroll liability payment", "amount": crate::ui::plain(owed)}]}})).unwrap();
    let after = app.be.call_now("accounts.list", json!({})).unwrap().as_array().unwrap().iter().find(|a| a["id"].as_i64() == Some(id)).unwrap()["ytd_balance"].as_i64().unwrap();
    assert_eq!(after, 0, "liability cleared");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_class_list_and_enter_time() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("classtime");
    // Class List: add a class.
    let _ = app.update(Message::Nav(Route::ClassList));
    app.data = Some(app.be.call_now("tracking.list", json!({})).unwrap());
    let _ = app.update(Message::Desk(W::ClassNew("Northern Region".into())));
    let _ = app.update(Message::Desk(W::ClassAdd));
    let cats = app.data.as_ref().unwrap().as_array().unwrap().clone();
    let (id, name, mut opts) = match cats.first() {
        Some(c) => (c["id"].clone(), c["name"].as_str().unwrap().to_string(), c["options"].as_array().unwrap().iter().map(|o| o["name"].as_str().unwrap().to_string()).collect::<Vec<_>>()),
        None => (json!(null), "Class".into(), vec![]),
    };
    opts.push("Northern Region".into());
    let r = app.be.call_now("tracking.save", json!({"id": id, "name": name, "options": opts}));
    let _ = app.update(Message::Desk(W::ClassDone(r)));
    let after = app.be.call_now("tracking.list", json!({})).unwrap();
    assert!(after.to_string().contains("Northern Region"));
    // Enter Time: durations and saving against a project task.
    assert_eq!(crate::desktop::parse_duration("1:30"), Some(90));
    assert_eq!(crate::desktop::parse_duration("1.5"), Some(90));
    assert_eq!(crate::desktop::parse_duration("45m"), Some(45));
    assert_eq!(crate::desktop::parse_duration("0:75"), None);
    let _ = app.update(Message::Nav(Route::EnterTime));
    let projects = app.be.call_now("projects.list", json!({"status": "INPROGRESS"})).unwrap();
    let p = projects["projects"][0].clone();
    let full = app.be.call_now("projects.get", json!({"id": p["id"]})).unwrap();
    app.data = Some(json!({"projects": [{"id": p["id"], "name": p["name"], "contact": p["contact_name"], "tasks": full["tasks"]}]}));
    let _ = app.update(Message::Desk(W::TimeProject(crate::app::Opt { id: p["id"].as_i64().unwrap(), label: "p".into() })));
    let _ = app.update(Message::Desk(W::TimeTask(crate::app::Opt { id: full["tasks"][0]["id"].as_i64().unwrap(), label: "t".into() })));
    let _ = app.update(Message::Desk(W::TimeField(1, "2:15".into())));
    let _ = app.update(Message::Desk(W::TimeSave));
    assert!(app.desk.time_busy);
    let r = app.be.call_now("projects.addTime", json!({"project_id": p["id"], "task_id": full["tasks"][0]["id"], "date": crate::ui::today_iso(), "minutes": 135, "description": ""}));
    assert!(r.is_ok(), "{r:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn memorized_reports() {
    use crate::reports::{Columns, Msg as Rp, Period};
    let (mut app, dir) = desktop_app("memrep");
    let _ = app.update(Message::Nav(Route::Report("profit-and-loss".into())));
    app.data = Some(json!({"title": "Profit and Loss", "columns": [], "sections": []}));
    let _ = app.update(Message::Report(Rp::Period(Period::LastYear)));
    let _ = app.update(Message::Report(Rp::Columns(Columns::Months)));
    let _ = app.update(Message::Report(Rp::Basis(true)));
    app.data = Some(json!({"title": "Profit and Loss", "columns": [], "sections": []}));
    let msgs = click(&app, "Memorize");
    assert!(matches!(msgs.as_slice(), [Message::Report(Rp::Memorize)]), "{msgs:?}");
    let _ = app.update(Message::Report(Rp::Memorize));
    let _ = app.update(Message::Report(Rp::MemoName("Last year by month (cash)".into())));
    let _ = app.update(Message::Report(Rp::MemoSave));
    assert_eq!(app.memorized_reports().len(), 1);
    // Change everything, then run the memorized one: its settings come back.
    let _ = app.update(Message::Report(Rp::Period(Period::ThisMonth)));
    let _ = app.update(Message::Report(Rp::Columns(Columns::Total)));
    let _ = app.update(Message::Report(Rp::Basis(false)));
    let _ = app.update(Message::Nav(Route::MemorizedReports));
    {
        let _g = sim_lock();
        let mut ui = simulator(app.view());
        for t in ["Last year by month (cash)", "Profit & Loss", "Last year"] { assert!(ui.find(t).is_ok(), "missing {t}"); }
    }
    let _ = app.update(Message::Report(Rp::RunMemorized(0)));
    assert_eq!(app.route, Route::Report("profit-and-loss".into()));
    assert_eq!((app.report.period, app.report.columns, app.report.cash), (Period::LastYear, Columns::Months, true));
    let _ = app.update(Message::Nav(Route::MemorizedReports));
    let _ = app.update(Message::Report(Rp::DeleteMemorized(0)));
    assert!(app.memorized_reports().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_view_choices_are_remembered() {
    use crate::desktop::{Msg as W, Pane};
    let (mut app, dir) = desktop_app("deskprefs2");
    let _ = app.update(Message::Desk(W::OneWindow(true)));
    let _ = app.update(Message::Desk(W::Pane(Pane::Balances)));
    let (again, _) = App::boot_layout(dir.clone(), None, Some(crate::theme::Layout::Desktop));
    assert!(again.desk.one_window);
    assert_eq!(again.desk.pane, Pane::Balances);
    let _ = std::fs::remove_dir_all(dir);
}

/// Run a task's work (engine calls) and feed the resulting messages back into the app, like the
/// real runtime does. Timers (toast dismissal) are cut short.
fn settle(app: &mut App, task: iced::Task<Message>) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
    let mut pending = vec![task];
    let mut rounds = 0;
    while let Some(t) = pending.pop() {
        rounds += 1;
        if rounds > 200 { break; }
        let Some(stream) = iced_runtime::task::into_stream(t) else { continue };
        // Keep every result that arrives; stop waiting once nothing more comes within 1.5 s (timers).
        let actions: Vec<iced_runtime::Action<Message>> = rt.block_on(async {
            use iced::futures::StreamExt;
            let mut stream = stream;
            let mut out = vec![];
            while let Ok(Some(a)) = tokio::time::timeout(std::time::Duration::from_millis(1500), stream.next()).await { out.push(a); }
            out
        });
        for a in actions {
            if let iced_runtime::Action::Output(m) = a { pending.push(app.update(m)); }
        }
    }
}

/// Every enabled Desktop menu item opens a real screen that renders with its data (no placeholders, no panics).
#[test]
fn desktop_every_menu_item_opens_a_screen() {
    let (mut app, dir) = desktop_app("sweep");
    let mut opened = 0;
    for menu in crate::desktop::MENUS {
        for (label, _, msg) in crate::desktop::menu_items(&app, menu) {
            let Some(Message::Nav(r)) = msg else { continue };
            if matches!(r, Route::Setup) { continue; }
            let t = app.update(Message::Nav(r.clone()));
            settle(&mut app, t);
            assert!(app.error.is_none(), "{menu} ▸ {label}: {:?}", app.error);
            // Render with the data the screen loads (or while loading).
            let _g = sim_lock();
            let _ui = simulator(app.view());
            opened += 1;
        }
    }
    assert!(opened > 50, "only {opened} menu items opened");
    let _ = std::fs::remove_dir_all(dir);
}

/// Every screen, in every layout, loads its real data without an error and renders.
#[test]
fn every_screen_in_every_layout_loads() {
    use crate::theme::Layout;
    let routes = vec![Route::Dashboard, Route::AllSales, Route::Invoices, Route::Quotes, Route::CreditNotes, Route::Customers, Route::Products, Route::Expenses, Route::Bills,
        Route::Vendors, Route::PurchaseOrders, Route::Contacts, Route::Banking(None), Route::Chart, Route::Reports, Route::Report("profit-and-loss".into()),
        Route::Report("balance-sheet".into()), Route::Report("bank-reconciliation".into()), Route::Settings, Route::Doc { kind: "ACCREC".into(), id: None },
        Route::Doc { kind: "ACCPAY".into(), id: None }, Route::Doc { kind: "QUOTE".into(), id: None }, Route::Doc { kind: "SALESRECEIPT".into(), id: None },
        Route::ReceivePayment(None), Route::PayBills, Route::Spend(None), Route::Check(None), Route::Deposit, Route::Transfer, Route::Journal,
        Route::NewContact(true), Route::NewProduct, Route::NewAccount, Route::Payroll, Route::Taxes, Route::Projects, Route::Mileage, Route::Reconcile,
        Route::Register(None), Route::Calendar, Route::Statements, Route::Memorized, Route::PaySalesTax, Route::AdjustInventory, Route::Help, Route::DocCenter,
        Route::PayLiabilities, Route::ClassList, Route::EnterTime, Route::MemorizedReports, Route::PayrollItems, Route::OtherNames, Route::FinanceCharges, Route::Templates, Route::PriceLevels, Route::PrintForms, Route::PayrollForms, Route::CustomizeFavorites, Route::BuildAssemblies, Route::InventoryCenter, Route::BatchInvoices, Route::Reminders, Route::Find];
    for layout in Layout::ALL {
        let (mut app, dir) = desktop_app(&format!("all-{}", layout.key()));
        let t = app.update(Message::SetLayout(layout));
        settle(&mut app, t);
        for r in &routes {
            let t = app.update(Message::Nav(r.clone()));
            settle(&mut app, t);
            assert!(app.error.is_none(), "{} ▸ {r:?}: {:?}", layout.label(), app.error);
            assert!(app.toasts.iter().all(|t| !t.2), "{} ▸ {r:?}: error toast {:?}", layout.label(), app.toasts);
            let _g = sim_lock();
            let _ui = simulator(app.view());
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Click a widget and run everything it triggers (engine calls included).
fn press(app: &mut App, label: &str) {
    let msgs = click(app, label);
    assert!(!msgs.is_empty(), "{label} did nothing");
    for m in msgs { let t = app.update(m); settle(app, t); }
}

fn count(app: &App, kind: &str) -> usize { app.be.call_now("docs.list", json!({"type": kind, "limit": 5000})).unwrap()["rows"].as_array().unwrap().len() }

#[test]
fn end_to_end_desktop_daily_work() {
    use crate::doc::Msg as D;
    use crate::payments::Msg as P;
    let (mut app, dir) = desktop_app("e2e");
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    // 1. Create Invoices → Save & Close: invoice saved, window closed.
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None })); settle(&mut app, t);
    let windows = app.desk.windows.len();
    let invoices = count(&app, "ACCREC");
    for m in [D::Contact(cust.clone()), D::Desc(0, "Website redesign".into()), D::Price(0, "1200".into())] { let t = app.update(Message::Doc(m)); settle(&mut app, t); }
    press(&mut app, "Save & Close");
    assert_eq!(count(&app, "ACCREC"), invoices + 1, "invoice saved");
    assert_eq!(app.desk.windows.len(), windows - 1, "window closed");
    let inv = app.be.call_now("docs.list", json!({"type": "ACCREC", "contact_id": cust.id, "limit": 1})).unwrap()["rows"][0].clone();
    let due = inv["amount_due"].as_i64().unwrap();
    assert!(due > 0);
    // 2. Receive Payments for that invoice only → Save & Close.
    let t = app.update(Message::Nav(Route::ReceivePayment(Some(cust.id)))); settle(&mut app, t);
    let docs = app.pay.as_ref().unwrap().docs.clone();
    for (n, d) in docs.iter().enumerate() { let t = app.update(Message::Pay(P::Pick(n, d.id == inv["id"].as_i64().unwrap()))); settle(&mut app, t); }
    press(&mut app, "Save & Close");
    assert_eq!(app.be.call_now("docs.get", json!({"id": inv["id"]})).unwrap()["status"], "PAID", "payment applied");
    // 3. Write Checks → Save & Close: a check appears in the register.
    let t = app.update(Message::Nav(Route::Check(None))); settle(&mut app, t);
    let acct = app.lk.account_opts(|a| a["code"] == "429").into_iter().next().unwrap();
    for m in [P::ContactText("Corner Hardware".into()), P::LineAccount(0, acct), P::LineAmount(0, "86.40".into())] { let t = app.update(Message::Pay(m)); settle(&mut app, t); }
    let no = app.pay.as_ref().unwrap().check_no.clone();
    press(&mut app, "Save & Close");
    let bank = crate::payments::bank_only(&app)[0].id;
    let reg = app.be.call_now("bank.transactions", json!({"id": bank})).unwrap();
    assert!(reg.as_array().unwrap().iter().any(|r| r["description"] == format!("Check {no}")), "check {no} in the register");
    // 4. Make General Journal Entries → Save & Close.
    let t = app.update(Message::Nav(Route::Journal)); settle(&mut app, t);
    let a1 = app.lk.account_opts(|a| a["code"] == "429").into_iter().next().unwrap();
    let a2 = app.lk.account_opts(|a| a["code"] == "200").into_iter().next().unwrap();
    use crate::forms::Msg as F;
    for m in [F::JAccount(0, a1), F::JDebit(0, "50".into()), F::JAccount(1, a2), F::JCredit(1, "50".into())] { let t = app.update(Message::Form(m)); settle(&mut app, t); }
    let before = app.be.call_now("reports.run", json!({"name": "journal-report", "params": {"from": "2000-01-01", "to": "2100-01-01"}})).unwrap().to_string().len();
    press(&mut app, "Save & Close");
    let after = app.be.call_now("reports.run", json!({"name": "journal-report", "params": {"from": "2000-01-01", "to": "2100-01-01"}})).unwrap().to_string().len();
    assert!(after > before, "journal posted");
    assert!(app.toasts.iter().all(|t| !t.2), "no errors: {:?}", app.toasts);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn end_to_end_desktop_bills_orders_register_reconcile() {
    use crate::doc::Msg as D;
    use crate::payments::Msg as P;
    let (mut app, dir) = desktop_app("e2e2");
    // Enter Bills (Expenses tab) → Save & Close.
    let vendor = app.lk.contact_opts(false).into_iter().next().unwrap();
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCPAY".into(), id: None })); settle(&mut app, t);
    let acct = app.lk.account_opts(|a| a["code"] == "429").into_iter().next().unwrap();
    for m in [D::Contact(vendor.clone()), D::Account(0, acct), D::Amount(0, "310.00".into())] { let t = app.update(Message::Doc(m)); settle(&mut app, t); }
    let bills = count(&app, "ACCPAY");
    press(&mut app, "Save & Close");
    assert_eq!(count(&app, "ACCPAY"), bills + 1);
    let bill = app.be.call_now("docs.list", json!({"type": "ACCPAY", "contact_id": vendor.id, "limit": 1})).unwrap()["rows"][0].clone();
    // Pay Bills: only that bill → Pay Selected Bills.
    let t = app.update(Message::Nav(Route::PayBills)); settle(&mut app, t);
    let docs = app.pay.as_ref().unwrap().docs.clone();
    for (n, d) in docs.iter().enumerate() { let t = app.update(Message::Pay(P::Pick(n, d.id == bill["id"].as_i64().unwrap()))); settle(&mut app, t); }
    press(&mut app, "Pay Selected Bills");
    assert_eq!(app.be.call_now("docs.get", json!({"id": bill["id"]})).unwrap()["status"], "PAID");
    // Sales order → Save & Close → reopen → Create Invoice.
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    let t = app.update(Message::Nav(Route::Doc { kind: "SALESORDER".into(), id: None })); settle(&mut app, t);
    for m in [D::Contact(cust), D::Desc(0, "12 chairs".into()), D::Price(0, "95".into())] { let t = app.update(Message::Doc(m)); settle(&mut app, t); }
    press(&mut app, "Save & Close");
    let so = app.be.call_now("docs.list", json!({"type": "SALESORDER", "limit": 1})).unwrap()["rows"][0]["id"].as_i64().unwrap();
    let t = app.update(Message::Nav(Route::Doc { kind: "SALESORDER".into(), id: Some(so) })); settle(&mut app, t);
    let invoices = count(&app, "ACCREC");
    press(&mut app, "Create Invoice");
    assert_eq!(count(&app, "ACCREC"), invoices + 1, "invoice created from the order");
    assert_eq!(app.be.call_now("docs.get", json!({"id": so})).unwrap()["status"], "INVOICED");
    // Register entry row: a deposit typed straight in.
    let t = app.update(Message::Nav(Route::Register(None))); settle(&mut app, t);
    let income = app.lk.account_opts(|a| a["code"] == "260").into_iter().next().unwrap();
    use crate::desktop::{Msg as W, RegField};
    for m in [W::RegAccount(income), W::Reg(RegField::Payee, "Cash sale".into()), W::Reg(RegField::Deposit, "42.00".into())] { let t = app.update(Message::Desk(m)); settle(&mut app, t); }
    press(&mut app, "Record");
    let bank = app.data.as_ref().unwrap()["account"].as_i64().unwrap();
    assert!(app.be.call_now("bank.transactions", json!({"id": bank})).unwrap().as_array().unwrap().iter().any(|r| r["contact"] == "Cash sale"));
    // Reconcile: statement balance = cleared items → Mark All → Reconcile Now.
    let t = app.update(Message::Nav(Route::Reconcile)); settle(&mut app, t);
    let acc = app.rec.account.as_ref().unwrap().id;
    let probe = app.be.call_now("tickoff.start", json!({"account_id": acc, "statement_date": crate::ui::today_iso(), "ending_balance": 0})).unwrap();
    let all: i64 = probe["items"].as_array().unwrap().iter().map(|i| i["amount"].as_i64().unwrap()).sum();
    let ending = probe["beginning_balance"].as_i64().unwrap() + all;
    let _ = app.update(Message::Rec(crate::reconcile::Msg::Ending(crate::ui::plain(ending))));
    press(&mut app, "Continue");
    press(&mut app, "Mark All");
    assert_eq!(app.rec.session.as_ref().unwrap()["difference"], 0);
    press(&mut app, "Reconcile Now");
    assert!(app.rec.session.is_none(), "reconciliation finished");
    let hist = app.be.call_now("tickoff.history", json!({"account_id": acc})).unwrap();
    assert_eq!(hist[0]["status"], "FINISHED");
    assert!(app.toasts.iter().all(|t| !t.2), "no errors: {:?}", app.toasts);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn end_to_end_desktop_money_screens() {
    use crate::desktop::Msg as W;
    use crate::doc::Msg as D;
    use crate::payments::Msg as P;
    let (mut app, dir) = desktop_app("e2e3");
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    let sales = app.lk.accounts.iter().find(|a| a["code"] == "200").unwrap()["id"].clone();
    let mk = |app: &App, kind: &str, price: &str| app.be.call_now("docs.save", json!({"doc": {"type": kind, "contact_id": cust.id, "date": crate::ui::today_iso(), "line_amount_type": "NOTAX", "status": "AUTHORISED",
        "lines": [{"description": "x", "quantity": "1", "unit_price": price, "account_id": sales}]}})).unwrap()["id"].as_i64().unwrap();
    // Receive a payment into Undeposited Funds, then Make Deposits.
    let inv = mk(&app, "ACCREC", "200");
    let uf = app.be.call_now("accounts.undeposited", json!({})).unwrap()["id"].as_i64().unwrap();
    app.be.call_now("payments.add", json!({"document_id": inv, "account_id": uf, "date": crate::ui::today_iso(), "amount": "200", "reference": ""})).unwrap();
    let t = app.update(Message::Nav(Route::Deposit)); settle(&mut app, t);
    assert!(!app.pay.as_ref().unwrap().docs.is_empty(), "waiting payments listed");
    press(&mut app, "Save & Close");
    assert!(app.be.call_now("deposits.undeposited", json!({})).unwrap()["payments"].as_array().unwrap().is_empty(), "deposited");
    // Apply a credit memo to an invoice from the credit memo.
    let inv2 = mk(&app, "ACCREC", "300");
    let cm = mk(&app, "ACCRECCREDIT", "100");
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCRECCREDIT".into(), id: Some(cm) })); settle(&mut app, t);
    press(&mut app, "Apply to Invoice");
    let list = app.doc.as_ref().unwrap().apply.clone().unwrap();
    for (n, (d, _)) in list.iter().enumerate() { let v = if d["id"].as_i64() == Some(inv2) { "100.00" } else { "" }; let _ = app.update(Message::Doc(D::ApplyAmt(n, v.into()))); }
    press(&mut app, "Done");
    assert_eq!(app.be.call_now("docs.get", json!({"id": inv2})).unwrap()["amount_due"], 20000);
    // Give Refund on another credit memo.
    let cm2 = mk(&app, "ACCRECCREDIT", "40");
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCRECCREDIT".into(), id: Some(cm2) })); settle(&mut app, t);
    press(&mut app, "Give Refund");
    press(&mut app, "OK");
    assert_eq!(app.be.call_now("docs.get", json!({"id": cm2})).unwrap()["status"], "PAID", "refunded");
    // Transfer Funds.
    let t = app.update(Message::Nav(Route::Transfer)); settle(&mut app, t);
    let _ = app.update(Message::Form(crate::forms::Msg::Amount("25".into())));
    press(&mut app, "Save & Close");
    // Pay Sales Tax (needs an agency with tax).
    let rate = app.be.call_now("taxRates.createSales", json!({"name": "State 5%", "parts": [{"agency": "State Dept", "name": "State", "rate_bp": 500}]})).unwrap()["id"].as_i64().unwrap();
    app.be.call_now("docs.save", json!({"doc": {"type": "ACCREC", "contact_id": cust.id, "date": crate::ui::today_iso(), "line_amount_type": "EXCLUSIVE", "status": "AUTHORISED",
        "lines": [{"description": "x", "quantity": "1", "unit_price": "100", "account_id": sales, "tax_rate_id": rate}]}})).unwrap();
    let t = app.update(Message::Nav(Route::PaySalesTax)); settle(&mut app, t);
    let agency = app.data.as_ref().unwrap().as_array().unwrap().iter().find(|a| a["name"] == "State Dept").unwrap()["id"].as_i64().unwrap();
    let _ = app.update(Message::Desk(W::TaxPick(agency, true)));
    let spends_before = app.be.call_now("bank.transactions", json!({"id": crate::payments::bank_only(&app)[0].id})).unwrap().as_array().unwrap().len();
    press(&mut app, "OK");
    let spends_after = app.be.call_now("bank.transactions", json!({"id": crate::payments::bank_only(&app)[0].id})).unwrap().as_array().unwrap().len();
    assert_eq!(spends_after, spends_before + 1, "sales tax paid");
    // Adjust inventory down by one.
    let t = app.update(Message::Nav(Route::AdjustInventory)); settle(&mut app, t);
    let item = app.data.as_ref().unwrap().as_array().unwrap().iter().find(|x| x["is_tracked"].as_i64() == Some(1)).unwrap().clone();
    let id = item["id"].as_i64().unwrap();
    let cur = item["qty_on_hand_e4"].as_i64().unwrap();
    let _ = app.update(Message::Desk(W::AdjQty(id, format!("{}", (cur - 10000) as f64 / 10000.0))));
    press(&mut app, "Save & Close");
    let now = app.be.call_now("items.list", json!({})).unwrap().as_array().unwrap().iter().find(|x| x["id"].as_i64() == Some(id)).unwrap()["qty_on_hand_e4"].as_i64().unwrap();
    assert_eq!(now, cur - 10000);
    // Enter Time.
    let t = app.update(Message::Nav(Route::EnterTime)); settle(&mut app, t);
    let p = app.data.as_ref().unwrap()["projects"][0].clone();
    let _ = app.update(Message::Desk(W::TimeProject(crate::app::Opt { id: p["id"].as_i64().unwrap(), label: "p".into() })));
    let _ = app.update(Message::Desk(W::TimeTask(crate::app::Opt { id: p["tasks"][0]["id"].as_i64().unwrap(), label: "t".into() })));
    let _ = app.update(Message::Desk(W::TimeField(1, "1:15".into())));
    press(&mut app, "Save & New");
    // New Class.
    let t = app.update(Message::Nav(Route::ClassList)); settle(&mut app, t);
    let _ = app.update(Message::Desk(W::ClassNew("Retail".into())));
    press(&mut app, "New Class");
    assert!(app.be.call_now("tracking.list", json!({})).unwrap().to_string().contains("Retail"));
    let errors: Vec<_> = app.toasts.iter().filter(|t| t.2).collect();
    assert!(errors.is_empty(), "errors: {errors:?}");
    let _ = (P::Save, D::Save(true));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn end_to_end_online_and_xero() {
    use crate::doc::Msg as D;
    use crate::payments::Msg as P;
    for (layout, save, cust_label) in [(crate::theme::Layout::Online, "Save and close", "Customer"), (crate::theme::Layout::Xero, "Approve", "Customer")] {
        let (mut app, dir) = desktop_app(&format!("e2e-{}", layout.key()));
        let t = app.update(Message::SetLayout(layout)); settle(&mut app, t);
        let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
        // Invoice
        let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None })); settle(&mut app, t);
        for m in [D::Contact(cust.clone()), D::Desc(0, "Consulting".into()), D::Price(0, "450".into())] { let t = app.update(Message::Doc(m)); settle(&mut app, t); }
        let before = count(&app, "ACCREC");
        press(&mut app, save);
        assert_eq!(count(&app, "ACCREC"), before + 1, "{}: invoice saved", layout.label());
        // Spend money / expense
        let t = app.update(Message::Nav(Route::Spend(None))); settle(&mut app, t);
        let acct = app.lk.account_opts(|a| a["code"] == "429").into_iter().next().unwrap();
        for m in [P::ContactText("Stationery Co".into()), P::LineAccount(0, acct), P::LineAmount(0, "19.95".into())] { let t = app.update(Message::Pay(m)); settle(&mut app, t); }
        press(&mut app, "Save and close");
        let bank = crate::payments::bank_only(&app)[0].id;
        assert!(app.be.call_now("bank.transactions", json!({"id": bank})).unwrap().as_array().unwrap().iter().any(|r| r["contact"] == "Stationery Co"), "{}: expense saved", layout.label());
        let errors: Vec<_> = app.toasts.iter().filter(|t| t.2).collect();
        assert!(errors.is_empty(), "{}: {errors:?}", layout.label());
        let _ = cust_label;
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Renders the Desktop Insights tab to `%TEMP%\lc-insights-wgpu.png` (run with --ignored).
#[test]
#[ignore]
fn render_insights() {
    let (mut app, dir) = desktop_app("insights-shot");
    let t = app.update(Message::Desk(crate::desktop::Msg::Insights(true))); settle(&mut app, t);
    let t = app.update(Message::Nav(Route::Dashboard)); settle(&mut app, t);
    let _ = std::fs::remove_file(std::env::temp_dir().join("lc-insights-wgpu.png"));
    let _g = sim_lock();
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 900.0), app.view());
    ui.snapshot(&app.theme()).unwrap().matches_image(std::env::temp_dir().join("lc-insights.png")).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_insights_loads_pl_and_top_customers() {
    let (mut app, dir) = desktop_app("insights");
    let t = app.update(Message::Desk(crate::desktop::Msg::Insights(true))); settle(&mut app, t);
    let d = app.data.clone().expect("insights data");
    assert!(d["pl"]["sections"].is_array(), "monthly P&L loaded");
    assert!(d["top"].is_object(), "sales by customer loaded");
    assert!(d["counts"]["awaiting"].is_number(), "invoice counts loaded");
    // Back to the Home Page tab loads the ordinary home data again.
    let t = app.update(Message::Desk(crate::desktop::Msg::Insights(false))); settle(&mut app, t);
    assert!(app.data.as_ref().map_or(true, |d| !d["pl"].is_object()));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn end_to_end_payroll_items_other_names_finance_charges() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("e2e-lists3");
    // Payroll Item List → New Item (a deduction).
    let t = app.update(Message::Nav(Route::PayrollItems)); settle(&mut app, t);
    assert!(app.data.as_ref().unwrap()["pay_items"].as_array().map_or(0, |a| a.len()) > 0);
    let _ = app.update(Message::Desk(W::PiNew("Uniform deduction".into())));
    let _ = app.update(Message::Desk(W::PiKind("Deduction")));
    press(&mut app, "New Item");
    let items = app.be.call_now("payroll.settings", json!({})).unwrap()["pay_items"].clone();
    let it = items.as_array().unwrap().iter().find(|i| i["name"] == "Uniform deduction").expect("pay item saved").clone();
    assert_eq!(it["kind"], "DEDUCTION");
    assert!(it["liability_account_id"].is_number(), "posts like the other deductions");
    // Other Names List → New Name shows in the list.
    let t = app.update(Message::Nav(Route::OtherNames)); settle(&mut app, t);
    let _ = app.update(Message::Desk(W::OnNew("Jordan Owner".into())));
    press(&mut app, "New Name");
    assert!(app.data.as_ref().unwrap().to_string().contains("Jordan Owner"));
    // Assess Finance Charges: one invoice per overdue customer, posted to an income account.
    let t = app.update(Message::Nav(Route::FinanceCharges)); settle(&mut app, t);
    let due = crate::desktop::finance_charges_due(&app);
    assert!(!due.is_empty() && due.iter().all(|c| c.3 > 0), "demo data has overdue customers: {due:?}");
    // Leave the first customer out.
    let _ = app.update(Message::Desk(W::FcPick(due[0].0, false)));
    let before = count(&app, "ACCREC");
    press(&mut app, "Assess Charges");
    assert_eq!(count(&app, "ACCREC"), before + due.len() - 1);
    // Assessing again the same day charges nothing more for those customers (only the one left out).
    let again = crate::desktop::finance_charges_due(&app);
    assert_eq!(again.iter().filter(|c| c.3 > 0).map(|c| c.0).collect::<Vec<_>>(), vec![due[0].0], "{again:?}");
    let errors: Vec<_> = app.toasts.iter().filter(|t| t.2).collect();
    assert!(errors.is_empty(), "errors: {errors:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn finance_charge_math() {
    let (mut app, dir) = desktop_app("fc-math");
    app.route = Route::FinanceCharges;
    app.data = Some(json!({"rows": [
        {"status": "AUTHORISED", "due_date": "2026-01-01", "amount_due": 100000, "contact_id": 1, "contact_name": "A"},
        {"status": "AUTHORISED", "due_date": "2026-03-01", "amount_due": 1000, "contact_id": 2, "contact_name": "B"},
    ]}));
    app.desk.fc_date = app.d("2026-01-31");
    app.desk.fc_rate = "36.5".into();
    // 30 days at 0.1%/day on 1,000.00 = 30.00; B isn't overdue yet.
    assert_eq!(crate::desktop::finance_charges_due(&app), vec![(1, "A".to_string(), 100000, 3000)]);
    app.desk.fc_grace = "10".into();
    assert_eq!(crate::desktop::finance_charges_due(&app)[0].3, 2000);
    app.desk.fc_min = "25".into();
    assert_eq!(crate::desktop::finance_charges_due(&app)[0].3, 2500);
    // Charged on 2026-01-21 already: only the 10 days since count; earlier charges aren't charged on.
    app.desk.fc_grace.clear(); app.desk.fc_min.clear();
    app.data.as_mut().unwrap()["charged"] = json!([{"reference": "Finance charge", "status": "AUTHORISED", "date": "2026-01-21", "contact_id": 1}]);
    app.data.as_mut().unwrap()["rows"].as_array_mut().unwrap().push(json!({"status": "AUTHORISED", "reference": "Finance charge", "due_date": "2026-01-01", "amount_due": 5000, "contact_id": 1, "contact_name": "A"}));
    assert_eq!(crate::desktop::finance_charges_due(&app), vec![(1, "A".to_string(), 100000, 1000)]);
    let _ = std::fs::remove_dir_all(dir);
}

/// Snapshot any Desktop screen: LC_ROUTE=financecharges cargo test -p ledger-app render_route -- --ignored
#[test]
#[ignore]
fn render_route() {
    let key = std::env::var("LC_ROUTE").unwrap_or_else(|_| "dashboard".into());
    let (mut app, dir) = desktop_app("route-shot");
    if std::env::var("LC_LIGHT").is_ok() { app.dark = false; }
    let route = crate::app::parse_route(&key).expect("route key");
    let t = app.update(Message::Nav(route)); settle(&mut app, t);
    // LC_MENU=Customers LC_MENU_SEL=2: an open menu with a keyboard highlight.
    if let Ok(m) = std::env::var("LC_MENU") {
        let m: &'static str = crate::desktop::MENUS.iter().find(|x| **x == m).copied().expect("menu name");
        let _ = app.update(Message::OpenMenu(Some(m)));
        app.desk.menu_sel = std::env::var("LC_MENU_SEL").ok().and_then(|v| v.parse().ok());
    }
    let _ = std::fs::remove_file(std::env::temp_dir().join("lc-route-wgpu.png"));
    let _g = sim_lock();
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 900.0), app.view());
    ui.snapshot(&app.theme()).unwrap().matches_image(std::env::temp_dir().join("lc-route.png")).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn end_to_end_templates() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("e2e-templates");
    let t = app.update(Message::Nav(Route::Templates)); settle(&mut app, t);
    let n0 = app.data.as_ref().unwrap().as_array().unwrap().len();
    assert!(n0 >= 1);
    // Edit the default straight away (no click on it first) and save.
    let _ = app.update(Message::Desk(W::TplField("invoice_title", "Tax Invoice (Custom)".into())));
    press(&mut app, "Save Template");
    let list = app.be.call_now("branding.list", json!({})).unwrap();
    assert_eq!(list[0]["invoice_title"], "Tax Invoice (Custom)");
    // A bad colour is refused.
    let _ = app.update(Message::Desk(W::TplField("accent", "blue".into())));
    let _ = app.update(Message::Desk(W::TplSave));
    assert!(app.toasts.iter().any(|t| t.2), "bad colour refused");
    app.toasts.clear();
    let _ = app.update(Message::Desk(W::TplField("accent", "#2f6fb5".into())));
    // New Template copies it; make it the default.
    press(&mut app, "New Template");
    let _ = app.update(Message::Desk(W::TplField("name", "Blue".into())));
    let _ = app.update(Message::Desk(W::TplDefault(true)));
    press(&mut app, "Save Template");
    let list = app.be.call_now("branding.list", json!({})).unwrap();
    assert_eq!(list.as_array().unwrap().len(), n0 + 1);
    assert_eq!(list[0]["name"], "Blue", "new default listed first");
    assert_eq!(list[0]["accent"], "#2f6fb5");
    // The default can't be deleted; pick the old one and delete it.
    let _ = app.update(Message::Desk(W::TplDelete));
    assert!(app.toasts.iter().any(|t| t.2));
    app.toasts.clear();
    let old = list[1]["id"].as_i64().unwrap();
    let _ = app.update(Message::Desk(W::TplPick(old)));
    press(&mut app, "Delete");
    assert_eq!(app.be.call_now("branding.list", json!({})).unwrap().as_array().unwrap().len(), n0);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn end_to_end_price_levels() {
    use crate::desktop::Msg as W;
    use crate::doc::Msg as D;
    let (mut app, dir) = desktop_app("e2e-pricelevels");
    let t = app.update(Message::Nav(Route::PriceLevels)); settle(&mut app, t);
    // A 0% or blank level is refused.
    let _ = app.update(Message::Desk(W::PlField(0, "Wholesale".into())));
    let _ = app.update(Message::Desk(W::PlAdd));
    assert!(app.toasts.iter().any(|t| t.2));
    app.toasts.clear();
    let _ = app.update(Message::Desk(W::PlField(1, "-10".into())));
    press(&mut app, "New Price Level");
    let levels = app.be.call_now("priceLevels.list", json!({})).unwrap();
    assert_eq!(levels[0]["name"], "Wholesale");
    assert_eq!(levels[0]["percent_bp"], -1000);
    // Assign it to a customer.
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    let lvl = crate::app::Opt { id: levels[0]["id"].as_i64().unwrap(), label: "Wholesale".into() };
    let _ = app.update(Message::Desk(W::PlCust(cust.clone())));
    let _ = app.update(Message::Desk(W::PlLevel(lvl)));
    press(&mut app, "Assign");
    assert_eq!(app.lk.price_level_bp(cust.id), Some(-1000));
    assert_eq!(app.be.call_now("priceLevels.list", json!({})).unwrap()[0]["customers"], 1);
    // An invoice for that customer prices items 10% lower.
    let item = app.lk.items.iter().find(|i| i["sale_unit_e2"].as_i64().unwrap_or(0) > 0).cloned().expect("priced item");
    let base = item["sale_unit_e2"].as_i64().unwrap();
    let iopt = crate::app::Opt { id: item["id"].as_i64().unwrap(), label: "i".into() };
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None })); settle(&mut app, t);
    let t = app.update(Message::Doc(D::Contact(cust.clone()))); settle(&mut app, t);
    let t = app.update(Message::Doc(D::Item(0, iopt.clone()))); settle(&mut app, t);
    let price = crate::doc::parse_e4(&app.doc.as_ref().unwrap().lines[0].price).unwrap();
    assert_eq!(price, ledger_core::pricelevels::apply_level(base, -1000));
    assert!(price < base);
    // Another customer pays the standard price.
    let other = app.lk.contact_opts(true).into_iter().nth(1).unwrap();
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None })); settle(&mut app, t);
    let t = app.update(Message::Doc(D::Contact(other))); settle(&mut app, t);
    let t = app.update(Message::Doc(D::Item(0, iopt))); settle(&mut app, t);
    assert_eq!(crate::doc::parse_e4(&app.doc.as_ref().unwrap().lines[0].price).unwrap(), base);
    // New Customer ▸ Price Level.
    let t = app.update(Message::Nav(Route::NewContact(true))); settle(&mut app, t);
    let _ = app.update(Message::Form(crate::forms::Msg::Name("Trade Buyer Pty".into())));
    let _ = app.update(Message::Form(crate::forms::Msg::PriceLevel(crate::app::Opt { id: levels[0]["id"].as_i64().unwrap(), label: "Wholesale".into() })));
    press(&mut app, "Save & Close");
    let tb = app.lk.contacts.iter().find(|c| c["name"] == "Trade Buyer Pty").expect("customer saved").clone();
    assert_eq!(tb["price_level_id"], levels[0]["id"]);
    // Deleting the level puts the customer back on standard prices.
    let t = app.update(Message::Nav(Route::PriceLevels)); settle(&mut app, t);
    let t = app.update(Message::Desk(W::PlDelete(levels[0]["id"].as_i64().unwrap()))); settle(&mut app, t);
    assert_eq!(app.lk.price_level_bp(cust.id), None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn end_to_end_print_forms() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("e2e-printforms");
    let t = app.update(Message::Nav(Route::PrintForms)); settle(&mut app, t);
    let unsent = |app: &App| app.be.call_now("docs.list", json!({"type": "ACCREC", "limit": 5000})).unwrap()["rows"].as_array().unwrap().iter()
        .filter(|d| matches!(d["status"].as_str(), Some("AUTHORISED" | "PAID")) && d["sent"].as_i64().unwrap_or(0) == 0).map(|d| d["id"].as_i64().unwrap()).collect::<Vec<_>>();
    let before = unsent(&app);
    assert!(before.len() >= 2, "demo data has unsent invoices");
    // Leave one out, print the rest into one PDF.
    let _ = app.update(Message::Desk(W::PfPick(before[0], false)));
    press(&mut app, "Print");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert_eq!(unsent(&app), vec![before[0]], "printed invoices are marked sent");
    let pdf = std::fs::read_dir(std::env::temp_dir().join("lc-test-output")).unwrap().filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("Invoices ")).max_by_key(|e| e.metadata().unwrap().modified().unwrap()).unwrap().path();
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    let pages = String::from_utf8_lossy(&bytes).matches("/Type /Page /").count();
    assert!(pages >= before.len() - 1, "one page per invoice ({pages})");
    let _ = std::fs::remove_file(pdf);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn end_to_end_payroll_forms() {
    let (mut app, dir) = desktop_app("e2e-payrollforms");
    // Run and post a pay run first.
    let cal = app.be.call_now("payroll.settings", json!({})).unwrap()["calendars"][0]["id"].as_i64().unwrap();
    let run = app.be.call_now("payroll.createPayRun", json!({"calendar_id": cal})).unwrap()["id"].as_i64().unwrap();
    let bank = app.lk.bank_opts()[0].id;
    app.be.call_now("payroll.post", json!({"id": run, "bank_account_id": bank})).unwrap();
    let t = app.update(Message::Nav(Route::PayrollForms)); settle(&mut app, t);
    let runs = app.data.clone().unwrap();
    let posted = runs.as_array().unwrap().iter().find(|r| r["status"] == "POSTED").expect("demo data has a posted pay run").clone();
    let id = posted["id"].as_i64().unwrap();
    let file = std::env::temp_dir().join("lc-test-output").join(format!("payroll-filing-{id}.json"));
    let _ = std::fs::remove_file(&file);
    let t = app.update(Message::Desk(crate::desktop::Msg::PrFile(id))); settle(&mut app, t);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert!(v["format"].is_string());
    let _ = std::fs::remove_file(file);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_customize_favorites() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("favorites");
    let t = app.update(Message::Nav(Route::CustomizeFavorites)); settle(&mut app, t);
    // Only Customize Favorites… to begin with.
    assert_eq!(crate::desktop::menu_items(&app, "Favorites").len(), 1);
    // Adding commands, then moving one up.
    let _ = app.update(Message::Desk(W::FavAdd("Banking ▸ Write Checks".into())));
    let _ = app.update(Message::Desk(W::FavAdd("Customers ▸ Create Invoices".into())));
    let _ = app.update(Message::Desk(W::FavUp(1)));
    assert_eq!(app.desk.favorites, vec!["Customers ▸ Create Invoices".to_string(), "Banking ▸ Write Checks".to_string()]);
    let menu = crate::desktop::menu_items(&app, "Favorites");
    assert_eq!(menu[0].0, "Create Invoices");
    assert!(menu[0].2.is_some() && menu[1].2.is_some());
    assert_eq!(menu[1].0, "Write Checks");
    // Remembered in preferences.
    app.desk.favorites.clear();
    app.load_desk_prefs();
    assert_eq!(app.desk.favorites.len(), 2);
    let _ = app.update(Message::Desk(W::FavRemove(0)));
    app.load_desk_prefs();
    assert_eq!(app.desk.favorites, vec!["Banking ▸ Write Checks".to_string()]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn print_uses_the_form_template() {
    let themes = json!([
        {"id": 1, "is_default": 1, "invoice_title": "Tax Invoice", "quote_title": "Estimate", "accent": "#ff0000", "terms": "Net 30", "payment_instructions": "Pay to BSB 000-000", "show_tax_number": 0},
        {"id": 2, "is_default": 0, "invoice_title": "Invoice (Blue)", "accent": "#0000ff", "terms": "", "payment_instructions": "", "show_tax_number": 1},
    ]);
    let mut st = json!({"tax_number": "12 345"});
    let mut d = json!({"type": "ACCREC", "terms": ""});
    let (mut title, mut accent) = ("Invoice".to_string(), (0.0, 0.0, 0.0));
    crate::pdf::apply_template(&mut d, &mut st, &themes, &mut title, &mut accent);
    assert_eq!(title, "Tax Invoice");
    assert_eq!(accent, (1.0, 0.0, 0.0));
    assert_eq!(d["terms"], "Net 30\n\nPay to BSB 000-000");
    assert_eq!(st["tax_number"], "", "tax number hidden");
    // A document's own template wins; quotes don't get payment instructions.
    let mut st = json!({"tax_number": "12 345"});
    let mut d = json!({"type": "ACCREC", "terms": "Due now", "branding_theme_id": 2});
    let (mut title, mut accent) = ("Invoice".to_string(), (0.0, 0.0, 0.0));
    crate::pdf::apply_template(&mut d, &mut st, &themes, &mut title, &mut accent);
    assert_eq!((title.as_str(), accent, d["terms"].as_str().unwrap(), st["tax_number"].as_str().unwrap()), ("Invoice (Blue)", (0.0, 0.0, 1.0), "Due now", "12 345"));
    let mut d = json!({"type": "QUOTE", "terms": ""});
    let mut title = "Quote".to_string();
    crate::pdf::apply_template(&mut d, &mut json!({}), &themes, &mut title, &mut accent);
    assert_eq!((title.as_str(), d["terms"].as_str().unwrap()), ("Estimate", "Net 30"));
}

#[test]
fn desktop_set_closing_date() {
    use crate::settings::Msg as S;
    let (mut app, dir) = desktop_app("closing");
    let t = app.update(Message::Nav(Route::Settings)); settle(&mut app, t);
    let _ = app.update(Message::Settings(S::Cat("Accounting")));
    press(&mut app, "Set Date…");
    let _ = app.update(Message::Settings(S::LockField(0, app.d("2026-01-31"))));
    press(&mut app, "OK");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert_eq!(app.setting("lock_date_all"), "2026-01-31");
    // A transaction on or before the closing date is refused; after it is fine.
    let cust = app.lk.contact_opts(true)[0].id;
    let acct = app.lk.account_opts(|a| a["type"] == "REVENUE")[0].id;
    let inv = |date: &str| json!({"doc": {"type": "ACCREC", "contact_id": cust, "date": date, "due_date": date, "line_amount_type": "NOTAX", "status": "AUTHORISED",
        "lines": [{"description": "x", "quantity": "1", "unit_price": "10", "account_id": acct}]}});
    assert!(app.be.call_now("docs.save", inv("2026-01-15")).is_err());
    assert!(app.be.call_now("docs.save", inv("2026-02-01")).is_ok());
    // The date for other users can't be earlier than the closing date.
    press(&mut app, "Set Date…");
    let _ = app.update(Message::Settings(S::LockField(1, app.d("2026-01-01"))));
    let _ = app.update(Message::Settings(S::LockSave));
    assert!(app.toasts.iter().any(|t| t.2));
    // Clearing removes it.
    let _ = app.update(Message::Settings(S::LockField(0, String::new())));
    let _ = app.update(Message::Settings(S::LockField(1, String::new())));
    press(&mut app, "OK");
    assert_eq!(app.setting("lock_date_all"), "");
    assert!(app.be.call_now("docs.save", inv("2026-01-15")).is_ok());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn end_to_end_build_assemblies() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("e2e-build");
    let adj = app.lk.account_opts(|a| a["type"] == "EXPENSE" || a["type"] == "DIRECTCOSTS")[0].id;
    let mk = |app: &App, code: &str, qty: &str, cost: &str| -> i64 {
        let id = app.be.call_now("items.create", json!({"code": code, "name": code, "is_tracked": true, "sale_price": "50"})).unwrap()["id"].as_i64().unwrap();
        if qty != "0" { app.be.call_now("items.adjust", json!({"id": id, "quantity": qty, "unit_cost": cost, "date": "2026-06-01", "account_id": adj, "reason": "Opening"})).unwrap(); }
        id
    };
    let (leg, top, table) = (mk(&app, "LEG", "10", "5"), mk(&app, "TOP", "3", "40"), mk(&app, "TABLE", "0", "0"));
    let t = app.update(Message::Nav(Route::BuildAssemblies)); settle(&mut app, t);
    let opt = |id: i64| crate::app::Opt { id, label: "x".into() };
    let t = app.update(Message::Desk(W::BaItem(opt(table)))); settle(&mut app, t);
    // No components yet: the component editor opens.
    assert!(app.desk.ba_edit.is_some());
    for m in [W::BaComp(0, opt(leg)), W::BaCompQty(0, "4".into()), W::BaCompAdd, W::BaComp(1, opt(top)), W::BaCompQty(1, "1".into())] { let _ = app.update(Message::Desk(m)); }
    press(&mut app, "Save Components");
    assert!(app.desk.ba_edit.is_none(), "{:?}", app.toasts);
    assert_eq!(app.desk.ba_data.as_ref().unwrap()["max_buildable"], 20000);
    // Build 2.
    let _ = app.update(Message::Desk(W::BaField(0, "2".into())));
    let _ = app.update(Message::Desk(W::BaField(1, app.d("2026-06-02"))));
    press(&mut app, "Build");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let item = app.be.call_now("items.get", json!({"id": table})).unwrap();
    assert_eq!(item["qty_on_hand_e4"], 20000);
    assert_eq!(item["total_cost"], 12000);
    assert_eq!(app.desk.ba_data.as_ref().unwrap()["builds"].as_array().unwrap().len(), 1);
    // Building more than the parts allow is refused.
    let _ = app.update(Message::Desk(W::BaField(0, "5".into())));
    press(&mut app, "Build");
    assert!(app.toasts.iter().any(|t| t.2));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[ignore]
fn render_build_assemblies() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("build-shot");
    let adj = app.lk.account_opts(|a| a["type"] == "EXPENSE" || a["type"] == "DIRECTCOSTS")[0].id;
    let mk = |app: &App, code: &str, name: &str, qty: &str, cost: &str| -> i64 {
        let id = app.be.call_now("items.create", json!({"code": code, "name": name, "is_tracked": true})).unwrap()["id"].as_i64().unwrap();
        if qty != "0" { app.be.call_now("items.adjust", json!({"id": id, "quantity": qty, "unit_cost": cost, "date": "2026-06-01", "account_id": adj, "reason": "Opening"})).unwrap(); }
        id
    };
    let (leg, top, table) = (mk(&app, "LEG", "Oak leg", "10", "5"), mk(&app, "TOP", "Oak table top", "3", "40"), mk(&app, "TABLE", "Oak dining table", "0", "0"));
    app.be.call_now("items.setComponents", json!({"id": table, "components": [{"item_id": leg, "quantity": "4"}, {"item_id": top, "quantity": "1"}]})).unwrap();
    let t = app.update(Message::Nav(Route::BuildAssemblies)); settle(&mut app, t);
    let t = app.update(Message::Desk(W::BaItem(crate::app::Opt { id: table, label: "TABLE — Oak dining table".into() }))); settle(&mut app, t);
    let _ = app.update(Message::Desk(W::BaField(0, "3".into())));
    let _ = std::fs::remove_file(std::env::temp_dir().join("lc-route-wgpu.png"));
    let _g = sim_lock();
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 900.0), app.view());
    ui.snapshot(&app.theme()).unwrap().matches_image(std::env::temp_dir().join("lc-route.png")).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

/// Snapshot many Desktop screens: LC_ROUTES="customers check calendar" LC_LIGHT=1 cargo test -p ledger-app render_routes -- --ignored
#[test]
#[ignore]
fn render_routes() {
    let keys = std::env::var("LC_ROUTES").unwrap_or_else(|_| "dashboard".into());
    for key in keys.split_whitespace() {
        let (mut app, dir) = desktop_app(&format!("routes-{}", key.replace(':', "-")));
        if std::env::var("LC_LIGHT").is_ok() { app.dark = false; }
        let t = app.update(Message::Nav(crate::app::parse_route(key).expect("route key"))); settle(&mut app, t);
        let name = format!("lc-shot-{}", key.replace(':', "-"));
        let _ = std::fs::remove_file(std::env::temp_dir().join(format!("{name}-wgpu.png")));
        {
            let _g = sim_lock();
            let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 900.0), app.view());
            ui.snapshot(&app.theme()).unwrap().matches_image(std::env::temp_dir().join(format!("{name}.png"))).unwrap();
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn invoice_template_choice_is_saved() {
    use crate::doc::Msg as D;
    let (mut app, dir) = desktop_app("tpl-choice");
    let blue = app.be.call_now("branding.save", json!({"theme": {"name": "Blue", "accent": "#2f6fb5", "invoice_title": "Invoice"}})).unwrap()["id"].as_i64().unwrap();
    let t = app.load_lookups(); settle(&mut app, t);
    assert!(app.lk.template_opts().len() >= 2);
    let cust = app.lk.contact_opts(true)[0].clone();
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None })); settle(&mut app, t);
    for m in [D::Contact(cust), D::Desc(0, "Work".into()), D::Price(0, "100".into()), D::Template(crate::app::Opt { id: blue, label: "Blue".into() })] { let t = app.update(Message::Doc(m)); settle(&mut app, t); }
    let id = { let p = app.doc.as_ref().unwrap().payload(&app, "AUTHORISED").unwrap(); app.be.call_now("docs.save", p).unwrap()["id"].as_i64().unwrap() };
    assert_eq!(app.be.call_now("docs.get", json!({"id": id})).unwrap()["branding_theme_id"], blue);
    // Reopened, the form shows it; switching back to the default is saved too.
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: Some(id) })); settle(&mut app, t);
    assert_eq!(app.doc.as_ref().unwrap().template.as_ref().map(|t| t.id), Some(blue));
    let std_id = app.lk.templates.iter().find(|t| t["name"] != "Blue").unwrap()["id"].as_i64().unwrap();
    let _ = app.update(Message::Doc(D::Template(crate::app::Opt { id: std_id, label: "Standard".into() })));
    let p = app.doc.as_ref().unwrap().payload(&app, "AUTHORISED").unwrap();
    app.be.call_now("docs.save", p).unwrap();
    assert_eq!(app.be.call_now("docs.get", json!({"id": id})).unwrap()["branding_theme_id"], std_id);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_customer_center_lists_jobs() {
    let (mut app, dir) = desktop_app("jobs");
    let t = app.update(Message::Nav(Route::Customers)); settle(&mut app, t);
    assert!(!app.desk.jobs.is_empty(), "demo projects load as jobs");
    let job = app.desk.jobs[0]["name"].as_str().unwrap().to_string();
    let msgs = click(&app, &format!("◦ {job}"));
    assert!(matches!(msgs.first(), Some(Message::Desk(crate::desktop::Msg::Select(_)))), "{msgs:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn quickbooks_date_keys() {
    use crate::ui::{date_keys, DateStyle::*};
    let k = |old: &str, new: &str| date_keys(old, new, Us);
    assert_eq!(k("10/13/2026", "10/13/2026+"), "10/14/2026");
    assert_eq!(k("10/13/2026", "10/13/2026-"), "10/12/2026");
    assert_eq!(k("10/13/2026", "+10/13/2026"), "10/14/2026", "typed anywhere in the field");
    assert_eq!(k("10/13/2026", "10/13/2026m"), "10/01/2026");
    assert_eq!(k("10/13/2026", "10/13/2026H"), "10/31/2026");
    assert_eq!(k("02/10/2028", "02/10/2028h"), "02/29/2028", "leap year");
    assert_eq!(k("10/13/2026", "10/13/2026y"), "01/01/2026");
    assert_eq!(k("10/13/2026", "10/13/2026r"), "12/31/2026");
    assert_eq!(k("10/13/2026", "10/13/2026w"), "10/11/2026", "Sunday");
    assert_eq!(k("10/13/2026", "10/13/2026k"), "10/17/2026", "Saturday");
    assert_eq!(k("10/13/2026", "10/13/2026t"), crate::ui::date(&crate::ui::today_iso(), Us));
    assert_eq!(date_keys("31/12/2026", "31/12/2026+", Intl), "01/01/2027");
    // Ordinary typing is untouched.
    assert_eq!(k("10/13/202", "10/13/2026"), "10/13/2026");
    assert_eq!(k("2026", "2026-"), "2026-", "ISO typing still works");
    assert_eq!(k("10/13/2026", "10/13/2026x"), "10/13/2026x");
    // Xero's "12 Oct 2026" style keeps letters for month names.
    assert_eq!(date_keys("12 O", "12 Oc", Long), "12 Oc");
    assert_eq!(date_keys("12 Oct 2026", "12 Oct 2026+", Long), "13 Oct 2026");
}

#[test]
fn date_keys_in_the_invoice_form() {
    let (mut app, dir) = desktop_app("datekeys");
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None })); settle(&mut app, t);
    let (d0, due0) = { let e = app.doc.as_ref().unwrap(); (e.date.clone(), e.due.clone()) };
    let _ = app.update(Message::Doc(crate::doc::Msg::Date(format!("{d0}+"))));
    let e = app.doc.as_ref().unwrap();
    let iso = |s: &str| crate::ui::to_iso(s, app.date_style()).unwrap();
    assert_eq!(iso(&e.date), crate::ui::add_days_iso(&iso(&d0), 1));
    assert_eq!(iso(&e.due), crate::ui::add_days_iso(&iso(&due0), 1), "terms keep the due date in step");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_form_keys_alt_a_and_alt_s() {
    use crate::doc::Msg as D;
    use iced::keyboard::{key::Code, Modifiers};
    let key = |c: &str, m: Modifiers, code: Code| iced::keyboard::Event::KeyPressed {
        key: iced::keyboard::Key::Character(c.into()), modified_key: iced::keyboard::Key::Character(c.into()),
        physical_key: iced::keyboard::key::Physical::Code(code), location: iced::keyboard::Location::Standard, modifiers: m, text: None, repeat: false };
    let (mut app, dir) = desktop_app("formkeys");
    let cust = app.lk.contact_opts(true)[0].clone();
    let fill = |app: &mut App| for m in [D::Contact(cust.clone()), D::Desc(0, "Keys".into()), D::Price(0, "10".into())] { let t = app.update(Message::Doc(m)); settle(app, t); };
    // Alt+S: Save & New — saved, and a fresh form in the same window.
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None })); settle(&mut app, t);
    fill(&mut app);
    let before = count(&app, "ACCREC");
    let t = app.update(Message::Key(key("s", Modifiers::ALT, Code::KeyS))); settle(&mut app, t);
    assert_eq!(count(&app, "ACCREC"), before + 1);
    assert!(app.doc.as_ref().is_some_and(|e| e.id.is_none()), "a new form follows");
    // Alt+A: Save & Close — saved, and the window closes.
    fill(&mut app);
    let windows = app.desk.windows.len();
    let t = app.update(Message::Key(key("a", Modifiers::ALT, Code::KeyA))); settle(&mut app, t);
    assert_eq!(count(&app, "ACCREC"), before + 2);
    assert_eq!(app.desk.windows.len(), windows - 1);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    // Away from a form, Alt+letter opens that menu (Alt+A = Favorites).
    let t = app.update(Message::Nav(Route::Dashboard)); settle(&mut app, t);
    let _ = app.update(Message::Key(key("u", Modifiers::ALT, Code::KeyU)));
    assert_eq!(app.menu, Some("Customers"));
    let _ = app.update(Message::Key(key("a", Modifiers::ALT, Code::KeyA)));
    assert_eq!(app.menu, Some("Favorites"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_menu_arrow_keys() {
    use iced::keyboard::{key::{Code, Named}, Modifiers};
    let named = |n: Named, code: Code| iced::keyboard::Event::KeyPressed {
        key: iced::keyboard::Key::Named(n), modified_key: iced::keyboard::Key::Named(n),
        physical_key: iced::keyboard::key::Physical::Code(code), location: iced::keyboard::Location::Standard, modifiers: Modifiers::empty(), text: None, repeat: false };
    let (mut app, dir) = desktop_app("menukeys");
    let _ = app.update(Message::OpenMenu(Some("Customers")));
    // Down highlights the first command (Customer Center); Down moves on, skipping separators.
    let _ = app.update(Message::Key(named(Named::ArrowDown, Code::ArrowDown)));
    assert_eq!(app.desk.menu_sel, Some(0));
    let _ = app.update(Message::Key(named(Named::ArrowDown, Code::ArrowDown)));
    let items = crate::desktop::menu_items(&app, "Customers");
    assert_eq!(items[app.desk.menu_sel.unwrap()].0, "Job Costing Center");
    let _ = app.update(Message::Key(named(Named::ArrowDown, Code::ArrowDown)));
    assert_eq!(items[app.desk.menu_sel.unwrap()].0, "Create Estimates", "the separator is skipped");
    // Up three times wraps past the top to the last command.
    for _ in 0..3 { let _ = app.update(Message::Key(named(Named::ArrowUp, Code::ArrowUp))); }
    assert!(items[app.desk.menu_sel.unwrap()].2.is_some());
    assert!(app.desk.menu_sel.unwrap() > 2);
    // Back to Customer Center and Enter opens it.
    let _ = app.update(Message::OpenMenu(None));
    let _ = app.update(Message::OpenMenu(Some("Customers")));
    let _ = app.update(Message::Key(named(Named::ArrowDown, Code::ArrowDown)));
    let t = app.update(Message::Key(named(Named::Enter, Code::Enter))); settle(&mut app, t);
    assert_eq!(app.route, Route::Customers);
    assert_eq!(app.menu, None);
    // Left from File wraps round to Help.
    let _ = app.update(Message::OpenMenu(Some("File")));
    let _ = app.update(Message::Key(named(Named::ArrowLeft, Code::ArrowLeft)));
    assert_eq!(app.menu, Some("Help"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[ignore]
fn render_invoice_quickfill() {
    use crate::doc::Msg as D;
    let (mut app, dir) = desktop_app("quickfill-shot");
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None })); settle(&mut app, t);
    let item = app.lk.item_opts()[0].clone();
    for m in [D::Contact(app.lk.contact_opts(true)[0].clone()), D::Item(0, item)] { let t = app.update(Message::Doc(m)); settle(&mut app, t); }
    assert_eq!(app.doc.as_ref().unwrap().item_boxes.len(), app.doc.as_ref().unwrap().lines.len());
    let _ = std::fs::remove_file(std::env::temp_dir().join("lc-route-wgpu.png"));
    let _g = sim_lock();
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 900.0), app.view());
    ui.snapshot(&app.theme()).unwrap().matches_image(std::env::temp_dir().join("lc-route.png")).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_inventory_center() {
    let (mut app, dir) = desktop_app("invcenter");
    let t = app.update(Message::Nav(Route::InventoryCenter)); settle(&mut app, t);
    let sel = app.desk.inv_sel.expect("first inventory item selected");
    let d = app.desk.inv_detail.clone().expect("details loaded");
    assert_eq!(d["id"].as_i64(), Some(sel));
    // A movement from a bill or invoice opens that transaction.
    let mv = d["movements"].as_array().unwrap().iter().find(|m| matches!(m["source_type"].as_str(), Some("ACCREC" | "ACCPAY"))).cloned().expect("demo stock has bills/invoices");
    let label = if mv["source_type"] == "ACCREC" { "Invoice" } else { "Bill" };
    let msgs = click(&app, label);
    assert!(msgs.iter().any(|m| matches!(m, Message::Nav(Route::Doc { id: Some(_), .. }))), "{msgs:?}");
    assert!(click(&app, "Inventory Valuation Summary").iter().any(|m| matches!(m, Message::Nav(Route::Report(_)))));
    let t = app.update(Message::Nav(Route::Report("inventory-summary".into()))); settle(&mut app, t);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn end_to_end_print_check() {
    use crate::payments::Msg as P;
    let (mut app, dir) = desktop_app("printcheck");
    let t = app.update(Message::Nav(Route::Check(None))); settle(&mut app, t);
    let vendor = app.lk.contact_opts(false)[0].clone();
    let acct = app.lk.account_opts(|a| a["type"] == "EXPENSE")[0].clone();
    let no = app.pay.as_ref().unwrap().check_no.clone();
    for m in [P::Contact(vendor), P::LineAccount(0, acct), P::LineAmount(0, "1234.56".into()), P::Reference("Office chairs".into())] { let t = app.update(Message::Pay(m)); settle(&mut app, t); }
    let file = std::env::temp_dir().join("lc-test-output").join(format!("Check {no}.pdf"));
    let _ = std::fs::remove_file(&file);
    press(&mut app, "Print Check");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let bytes = std::fs::read(&file).expect("check PDF written");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("**1,234.56"), "amount box");
    assert!(text.contains("One thousand two hundred thirty-four") || text.to_lowercase().contains("one thousand two hundred thirty-four"), "amount in words");
    // A fresh check follows, numbered after the one printed.
    let next = app.pay.as_ref().unwrap().check_no.clone();
    assert_ne!(next, no);
    let _ = std::fs::remove_file(file);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[ignore]
fn sample_check_pdf() {
    let c = json!({"payee": "Officeworks", "date": "2026-10-13", "number": "1001", "amount": 123456, "words": crate::paydesk::words(123456), "memo": "Office chairs",
        "bank": "Business Everyday", "lines": [{"account": "429 General Expenses", "memo": "Chairs x4", "amount": 100000}, {"account": "408 Cleaning", "memo": "Delivery", "amount": 23456}]});
    let st = json!({"org_name": "Demo Co", "address": "1 George St\nSydney NSW 2000", "country": "US"});
    std::fs::write(std::env::temp_dir().join("lc-check-sample.pdf"), crate::pdf::check(&c, &st, &|x| crate::ui::date(x, crate::ui::DateStyle::Us))).unwrap();
}

#[test]
fn desktop_edit_customer_keeps_other_fields() {
    use crate::forms::Msg as F;
    let (mut app, dir) = desktop_app("editcontact");
    let id = app.lk.contact_opts(true)[0].id;
    // Fields the Desktop form doesn't show.
    let mut c = app.be.call_now("contacts.get", json!({"id": id})).unwrap();
    let website = "https://example.test";
    let full = json!({"name": c["name"], "website": website, "notes": "VIP — call first", "email": c["email"], "phone": c["phone"],
        "people": [{"first_name": "Sam", "last_name": "Lee", "email": "sam@example.test", "include_in_emails": true}], "sales_terms_type": "DAYSAFTERBILLDATE", "sales_terms_days": 21});
    app.be.call_now("contacts.update", json!({"id": id, "contact": full})).unwrap();
    // Customer Center ▸ Edit Customer.
    let t = app.update(Message::Nav(Route::Customers)); settle(&mut app, t);
    let t = app.update(Message::Desk(crate::desktop::Msg::Select(id))); settle(&mut app, t);
    let msgs = click(&app, "Edit Customer");
    assert!(matches!(msgs.first(), Some(Message::Nav(Route::EditContact(x))) if *x == id), "{msgs:?}");
    let t = app.update(Message::Nav(Route::EditContact(id))); settle(&mut app, t);
    assert_eq!(crate::desktop::title(&app, &app.route), "Edit Customer");
    c = app.be.call_now("contacts.get", json!({"id": id})).unwrap();
    assert_eq!(app.form.as_ref().unwrap().name, c["name"].as_str().unwrap());
    let _ = app.update(Message::Form(F::Phone("02 5555 0000".into())));
    press(&mut app, "OK");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let after = app.be.call_now("contacts.get", json!({"id": id})).unwrap();
    assert_eq!(after["phone"], "02 5555 0000");
    assert_eq!(after["website"], website);
    assert_eq!(after["notes"], "VIP — call first");
    assert_eq!(after["sales_terms_days"], 21);
    assert_eq!(after["people"].as_array().unwrap().len(), 1);
    assert_eq!(after["is_customer"], 1);
    // Back in the Customer Center, the details show the change.
    assert_eq!(app.route, Route::Customers);
    assert_eq!(app.desk.detail.as_ref().map(|d| d["contact"]["phone"].clone()), Some(json!("02 5555 0000")));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_new_inventory_part_and_edit_item() {
    use crate::forms::Msg as F;
    let (mut app, dir) = desktop_app("edititem");
    // New Item ▸ Inventory Part.
    let t = app.update(Message::Nav(Route::NewProduct)); settle(&mut app, t);
    for m in [F::ItemType("Inventory Part".into()), F::Name("Oak stool".into()), F::Code("STOOL".into()), F::Price("120".into()), F::Cost("45".into()), F::Reorder("5".into())] { let _ = app.update(Message::Form(m)); }
    press(&mut app, "Save & Close");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let items = app.be.call_now("items.list", json!({})).unwrap();
    let it = items.as_array().unwrap().iter().find(|i| i["code"] == "STOOL").expect("item saved").clone();
    assert_eq!(it["is_tracked"], 1, "an inventory part");
    assert_eq!(it["reorder_level_e4"], 50000);
    let id = it["id"].as_i64().unwrap();
    // A purchase description set elsewhere is kept by Edit Item.
    let mut full = app.be.call_now("items.get", json!({"id": id})).unwrap();
    full["purchase_description"] = json!("Oak stool, flat-packed");
    app.be.call_now("items.update", json!({"id": id, "item": {"code": "STOOL", "name": "Oak stool", "sales_description": "Oak stool", "purchase_description": "Oak stool, flat-packed",
        "is_sold": true, "is_purchased": true, "sale_price": "120", "purchase_price": "45", "is_tracked": true, "reorder_level": "5",
        "sale_account_id": full["sale_account_id"], "purchase_account_id": full["purchase_account_id"]}})).unwrap();
    // Item List row → Edit Item.
    let t = app.update(Message::Nav(Route::Products)); settle(&mut app, t);
    let msgs = click(&app, "Oak stool");
    assert!(matches!(msgs.first(), Some(Message::Nav(Route::EditItem(x))) if *x == id), "{msgs:?}");
    let t = app.update(Message::Nav(Route::EditItem(id))); settle(&mut app, t);
    let s = app.form.as_ref().unwrap();
    assert_eq!((s.name.as_str(), s.price.as_str(), s.cost.as_str(), s.reorder.as_str(), s.tracked), ("Oak stool", "120.00", "45.00", "5", true));
    for m in [F::Price("135.50".into()), F::Reorder("8".into())] { let _ = app.update(Message::Form(m)); }
    press(&mut app, "OK");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let after = app.be.call_now("items.get", json!({"id": id})).unwrap();
    assert_eq!(after["sale_unit_e2"], 1_355_000);
    assert_eq!(after["reorder_level_e4"], 80000);
    assert_eq!(after["purchase_description"], "Oak stool, flat-packed", "kept");
    // Make Inactive.
    let t = app.update(Message::Nav(Route::EditItem(id))); settle(&mut app, t);
    let _ = app.update(Message::Form(F::Inactive(true)));
    press(&mut app, "OK");
    assert_ne!(app.be.call_now("items.get", json!({"id": id})).unwrap()["status"], "ACTIVE");
    assert!(app.lk.item_opts().iter().all(|o| o.id != id), "inactive items leave the pickers");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_edit_account_and_inactive() {
    use crate::forms::Msg as F;
    let (mut app, dir) = desktop_app("editacct");
    let t = app.update(Message::Nav(Route::Chart)); settle(&mut app, t);
    let acct = app.lk.accounts.iter().find(|a| a["type"] == "EXPENSE" && a["system_account"].is_null() && a["status"] == "ACTIVE").unwrap().clone();
    let id = acct["id"].as_i64().unwrap();
    let t = app.update(Message::Nav(Route::EditAccount(id))); settle(&mut app, t);
    assert_eq!(app.form.as_ref().unwrap().name, acct["name"].as_str().unwrap());
    for m in [F::Name("Staff amenities".into()), F::Memo("Coffee, tea, fruit".into())] { let _ = app.update(Message::Form(m)); }
    press(&mut app, "OK");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let after = app.be.call_now("accounts.get", json!({"id": id})).unwrap();
    assert_eq!((after["name"].as_str(), after["description"].as_str(), after["type"].as_str(), after["code"].as_str()),
        (Some("Staff amenities"), Some("Coffee, tea, fruit"), acct["type"].as_str(), acct["code"].as_str()));
    // Make inactive: gone from the chart and pickers; Include inactive brings it back to reactivate.
    let t = app.update(Message::Nav(Route::EditAccount(id))); settle(&mut app, t);
    let _ = app.update(Message::Form(F::Inactive(true)));
    press(&mut app, "OK");
    assert_eq!(app.be.call_now("accounts.get", json!({"id": id})).unwrap()["status"], "ARCHIVED");
    let t = app.load_lookups(); settle(&mut app, t);
    assert!(app.lk.account_opts(|_| true).iter().all(|o| o.id != id));
    // A system account can't be made inactive (the checkbox isn't offered).
    let sys = app.lk.accounts.iter().find(|a| a["system_account"].is_string()).unwrap()["id"].as_i64().unwrap();
    let t = app.update(Message::Nav(Route::EditAccount(sys))); settle(&mut app, t);
    assert!(click_fails(&app, "Account is inactive"));
    let _ = std::fs::remove_dir_all(dir);
}

fn click_fails(app: &App, label: &str) -> bool {
    let _g = sim_lock();
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 1400.0), app.view());
    ui.click(label).is_err()
}

#[test]
fn desktop_make_customer_inactive() {
    use crate::forms::Msg as F;
    let (mut app, dir) = desktop_app("custinactive");
    // One with nothing outstanding can be made inactive; one with open invoices can't.
    let list = app.be.call_now("contacts.list", json!({"filter": "customers"})).unwrap();
    let free = list.as_array().unwrap().iter().find(|c| c["receivable"] == 0).unwrap()["id"].as_i64().unwrap();
    let owing = list.as_array().unwrap().iter().find(|c| c["receivable"].as_i64().unwrap_or(0) > 0).unwrap()["id"].as_i64().unwrap();
    let t = app.update(Message::Nav(Route::EditContact(owing))); settle(&mut app, t);
    let _ = app.update(Message::Form(F::Inactive(true)));
    let _ = app.update(Message::Form(F::Save));
    assert!(app.toasts.iter().any(|t| t.2), "refused with open invoices");
    app.toasts.clear();
    let t = app.update(Message::Nav(Route::EditContact(free))); settle(&mut app, t);
    let _ = app.update(Message::Form(F::Inactive(true)));
    press(&mut app, "OK");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let active = app.be.call_now("contacts.list", json!({"filter": "customers"})).unwrap();
    assert!(active.as_array().unwrap().iter().all(|c| c["id"].as_i64() != Some(free)), "left the Active Customers list");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn end_to_end_batch_invoices() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("batchinv");
    let t = app.update(Message::Nav(Route::BatchInvoices)); settle(&mut app, t);
    let custs: Vec<i64> = app.data.as_ref().unwrap().as_array().unwrap().iter().take(3).filter_map(|c| c["id"].as_i64()).collect();
    for c in &custs { let _ = app.update(Message::Desk(W::BiCust(*c, true))); }
    let item = app.lk.item_opts()[0].clone();
    let _ = app.update(Message::Desk(W::BiItem(0, item.clone())));
    assert!(!app.desk.bi_lines[0].3.is_empty(), "rate filled from the item");
    let _ = app.update(Message::Desk(W::BiField(1, 0, "Monthly retainer".into())));
    let _ = app.update(Message::Desk(W::BiField(1, 2, "250".into())));
    let before = count(&app, "ACCREC");
    press(&mut app, "Create Invoices");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert_eq!(count(&app, "ACCREC"), before + custs.len());
    // Each customer got both lines.
    let rows = app.be.call_now("docs.list", json!({"type": "ACCREC", "limit": 5000})).unwrap()["rows"].as_array().unwrap().clone();
    for c in &custs {
        let inv = rows.iter().filter(|r| r["contact_id"].as_i64() == Some(*c)).max_by_key(|r| r["id"].as_i64()).unwrap();
        let full = app.be.call_now("docs.get", json!({"id": inv["id"]})).unwrap();
        assert_eq!(full["lines"].as_array().unwrap().len(), 2);
        assert_eq!(full["status"], "AUTHORISED");
    }
    assert!(app.desk.bi_custs.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_add_job() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("addjob");
    let t = app.update(Message::Nav(Route::Customers)); settle(&mut app, t);
    let id = app.desk.sel.expect("a customer is selected");
    let jobs_before = app.desk.jobs.len();
    press(&mut app, "Add Job");
    let _ = app.update(Message::Desk(W::JobNew(Some("Kitchen fit-out".into()))));
    let t = app.update(Message::Desk(W::JobSave(id))); settle(&mut app, t);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert_eq!(app.desk.jobs.len(), jobs_before + 1);
    assert!(app.desk.jobs.iter().any(|j| j["name"] == "Kitchen fit-out" && j["contact_id"].as_i64() == Some(id)));
    assert!(app.desk.job_new.is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn receive_payment_auto_applies_oldest_first() {
    use crate::payments::Msg as P;
    let (mut app, dir) = desktop_app("autoapply");
    // A customer with at least two open invoices.
    let list = app.be.call_now("contacts.list", json!({"filter": "customers"})).unwrap();
    let rows = app.be.call_now("docs.list", json!({"type": "ACCREC", "tab": "all", "limit": 5000})).unwrap()["rows"].as_array().unwrap().clone();
    let cust = list.as_array().unwrap().iter().find(|c| rows.iter().filter(|r| r["contact_id"] == c["id"] && r["status"] == "AUTHORISED").count() >= 2).expect("demo customer with 2 open invoices").clone();
    let t = app.update(Message::Nav(Route::ReceivePayment(cust["id"].as_i64()))); settle(&mut app, t);
    let mut docs = app.pay.as_ref().unwrap().docs.clone();
    docs.sort_by(|a, b| a.date.cmp(&b.date));
    let (oldest, next) = (docs[0].clone(), docs[1].clone());
    // Enough for the oldest plus 100.00 of the next.
    let amt = oldest.due + 10000;
    let _ = app.update(Message::Pay(P::PayTotal(crate::ui::plain(amt))));
    let s = app.pay.as_ref().unwrap();
    let get = |id: i64| s.docs.iter().find(|d| d.id == id).unwrap().clone();
    assert!(get(oldest.id).pick && crate::ui::parse_money(&get(oldest.id).amount) == Some(oldest.due));
    assert_eq!(crate::ui::parse_money(&get(next.id).amount), Some(10000));
    // Un-Apply clears it all.
    press(&mut app, "Un-Apply Payment");
    assert!(app.pay.as_ref().unwrap().docs.iter().all(|d| !d.pick && d.amount.is_empty()));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn pay_bills_select_all_and_clear() {
    let (mut app, dir) = desktop_app("paybills-all");
    let t = app.update(Message::Nav(Route::PayBills)); settle(&mut app, t);
    assert!(app.pay.as_ref().unwrap().docs.len() >= 2);
    press(&mut app, "Select All Bills");
    assert!(app.pay.as_ref().unwrap().docs.iter().all(|d| d.pick && crate::ui::parse_money(&d.amount) == Some(d.due)));
    press(&mut app, "Clear Selections");
    assert!(app.pay.as_ref().unwrap().docs.iter().all(|d| !d.pick));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn make_deposits_with_other_amounts() {
    use crate::payments::Msg as P;
    let (mut app, dir) = desktop_app("deposit-other");
    let t = app.update(Message::Nav(Route::Deposit)); settle(&mut app, t);
    let bank = app.pay.as_ref().unwrap().account.clone().unwrap();
    let equity = app.lk.account_opts(|a| a["type"] == "EQUITY")[0].clone();
    let bal = |app: &App| app.be.call_now("accounts.list", json!({})).unwrap().as_array().unwrap().iter().find(|a| a["id"].as_i64() == Some(bank.id)).unwrap()["ytd_balance"].as_i64().unwrap();
    let before = bal(&app);
    for m in [P::LineAccount(0, equity), P::LineDesc(0, "Owner contribution".into()), P::LineAmount(0, "5000".into())] { let t = app.update(Message::Pay(m)); settle(&mut app, t); }
    press(&mut app, "Save & Close");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert_eq!(bal(&app), before + 500_000);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn register_opens_edits_and_voids_a_check() {
    use crate::payments::Msg as P;
    let (mut app, dir) = desktop_app("edit-check");
    let t = app.update(Message::Nav(Route::Check(None))); settle(&mut app, t);
    let vendor = app.lk.contact_opts(false)[0].clone();
    let acct = app.lk.account_opts(|a| a["type"] == "EXPENSE")[0].clone();
    for m in [P::Contact(vendor.clone()), P::LineAccount(0, acct), P::LineAmount(0, "75.00".into()), P::Reference("Window cleaning".into())] { let t = app.update(Message::Pay(m)); settle(&mut app, t); }
    let bank = app.pay.as_ref().unwrap().account.clone().unwrap();
    press(&mut app, "Save & Close");
    // In the register, the check opens in Write Checks.
    let t = app.update(Message::Nav(Route::Register(Some(bank.id)))); settle(&mut app, t);
    let row = app.data.as_ref().unwrap()["rows"].as_array().unwrap().iter().find(|r| r["kind"] == "banktx" && r["reference"] == "Window cleaning").cloned().expect("check in register");
    let id = row["id"].as_i64().unwrap();
    let msgs = click(&app, "Window cleaning");
    assert!(msgs.iter().any(|m| matches!(m, Message::Nav(Route::BankTx(x)) if *x == id)), "{msgs:?}");
    let t = app.update(Message::Nav(Route::BankTx(id))); settle(&mut app, t);
    let s = app.pay.as_ref().unwrap();
    assert!(s.check);
    assert_eq!(s.contact.as_ref().map(|c| c.id), Some(vendor.id));
    assert_eq!(crate::ui::parse_money(&s.lines[0].amount), Some(7500));
    // Change the amount and save: the same check, updated.
    let t = app.update(Message::Pay(P::LineAmount(0, "80.00".into()))); settle(&mut app, t);
    press(&mut app, "Save & Close");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let tx = app.be.call_now("banktx.get", json!({"id": id})).unwrap();
    assert_eq!(tx["total"], 8000);
    // Void it.
    let t = app.update(Message::Nav(Route::BankTx(id))); settle(&mut app, t);
    press(&mut app, "Void Check");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert_ne!(app.be.call_now("banktx.get", json!({"id": id})).unwrap()["status"], "AUTHORISED");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_reminders() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("reminders");
    let t = app.update(Message::Nav(Route::Reminders)); settle(&mut app, t);
    assert!(app.data.as_ref().unwrap()["overdue"].as_array().map_or(0, |a| a.len()) > 0, "demo has overdue invoices");
    // The section header shows the count; opening it lists invoices that open their form.
    let n = app.data.as_ref().unwrap()["overdue"].as_array().unwrap().len();
    let label = format!("Overdue Invoices ({n})");
    press(&mut app, &label);
    assert!(app.desk.rem_open.contains(&"Overdue Invoices"));
    let first = app.data.as_ref().unwrap()["overdue"][0].clone();
    let text = format!("{} {}", first["contact_name"].as_str().unwrap(), first["number"].as_str().unwrap());
    let msgs = click(&app, &text);
    assert!(matches!(msgs.first(), Some(Message::Nav(Route::Doc { id: Some(_), .. }))), "{msgs:?}");
    let _ = app.update(Message::Desk(W::RemToggle("__none")));
    assert!(app.desk.rem_open.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_new_and_edit_employee() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("employee");
    let t = app.update(Message::Nav(Route::Payroll)); settle(&mut app, t);
    let before = app.be.call_now("payroll.employees", json!({})).unwrap().as_array().unwrap().len();
    press(&mut app, "New Employee");
    for (k, v) in [("first_name", "Riley"), ("last_name", "Nguyen"), ("email", "riley@example.test"), ("pay_basis", "HOURLY"), ("hourly_rate", "32.50"), ("hours_per_week", "30")] {
        let _ = app.update(Message::Desk(W::EmpField(k, v.into())));
    }
    press(&mut app, "OK");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let emps = app.be.call_now("payroll.employees", json!({})).unwrap();
    assert_eq!(emps.as_array().unwrap().len(), before + 1);
    let e = emps.as_array().unwrap().iter().find(|e| e["last_name"] == "Nguyen").unwrap().clone();
    let id = e["id"].as_i64().unwrap();
    let full = app.be.call_now("payroll.employee", json!({"id": id})).unwrap();
    assert_eq!(full["pay_basis"], "HOURLY");
    assert_eq!(full["hourly_rate_e2"], 325_000);
    assert_eq!(full["hours_per_week_e2"], 3000);
    // Edit: switch to salary.
    let t = app.update(Message::Desk(W::EmpEdit(id))); settle(&mut app, t);
    assert_eq!(app.desk.emp_form.as_ref().unwrap().1["hourly_rate"], "32.50");
    for (k, v) in [("pay_basis", "SALARY"), ("annual_salary", "72000")] { let _ = app.update(Message::Desk(W::EmpField(k, v.into()))); }
    let t = app.update(Message::Desk(W::EmpSave)); settle(&mut app, t);
    let full = app.be.call_now("payroll.employee", json!({"id": id})).unwrap();
    assert_eq!((full["pay_basis"].as_str(), full["annual_salary"].as_i64(), full["email"].as_str()), (Some("SALARY"), Some(7_200_000), Some("riley@example.test")));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[ignore]
fn render_employee_form() {
    let (mut app, dir) = desktop_app("emp-shot");
    let t = app.update(Message::Nav(Route::Payroll)); settle(&mut app, t);
    let _ = app.update(Message::Desk(crate::desktop::Msg::EmpNew));
    let _ = std::fs::remove_file(std::env::temp_dir().join("lc-route-wgpu.png"));
    let _g = sim_lock();
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 900.0), app.view());
    ui.snapshot(&app.theme()).unwrap().matches_image(std::env::temp_dir().join("lc-route.png")).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn desktop_find_window() {
    use crate::desktop::Msg as W;
    use iced::keyboard::{key::Code, Modifiers};
    let (mut app, dir) = desktop_app("find");
    // Ctrl+F opens Find.
    let ev = iced::keyboard::Event::KeyPressed { key: iced::keyboard::Key::Character("f".into()), modified_key: iced::keyboard::Key::Character("f".into()),
        physical_key: iced::keyboard::key::Physical::Code(Code::KeyF), location: iced::keyboard::Location::Standard, modifiers: Modifiers::CTRL, text: None, repeat: false };
    let t = app.update(Message::Key(ev)); settle(&mut app, t);
    assert_eq!(app.route, Route::Find);
    // By exact amount: a known invoice.
    let inv = app.be.call_now("docs.list", json!({"type": "ACCREC", "limit": 1})).unwrap()["rows"][0].clone();
    let _ = app.update(Message::Desk(W::FindType("Invoice")));
    let _ = app.update(Message::Desk(W::FindField(1, crate::ui::plain(inv["total"].as_i64().unwrap()))));
    let t = app.update(Message::Desk(W::FindRun)); settle(&mut app, t);
    let rows = app.data.as_ref().unwrap()["rows"].as_array().unwrap().clone();
    assert!(rows.iter().any(|r| r["id"] == inv["id"]), "found by amount");
    assert!(rows.iter().all(|r| r["total"] == inv["total"] && r["type"] == "ACCREC"));
    // By name across all types, limited by date.
    let _ = app.update(Message::Desk(W::FindReset));
    let _ = app.update(Message::Desk(W::FindType("All")));
    let _ = app.update(Message::Desk(W::FindField(0, inv["contact_name"].as_str().unwrap().into())));
    let _ = app.update(Message::Desk(W::FindField(2, app.d(inv["date"].as_str().unwrap()))));
    let _ = app.update(Message::Desk(W::FindField(3, app.d(inv["date"].as_str().unwrap()))));
    let t = app.update(Message::Desk(W::FindRun)); settle(&mut app, t);
    let rows = app.data.as_ref().unwrap()["rows"].as_array().unwrap().clone();
    assert!(!rows.is_empty() && rows.iter().all(|r| r["date"] == inv["date"]), "{:?} {:?} {:?} {:?}", inv["date"], app.desk.find_from, app.desk.find_text, rows.iter().map(|r| r["date"].clone()).collect::<Vec<_>>());
    let _ = std::fs::remove_dir_all(dir);
}

/// Step 1 of NEXT_STEPS_PLAN: every feature in the registry is reachable from each layout's own
/// navigation, and clicking it does what the registry says.
#[test]
fn every_feature_in_every_layout() {
    use crate::features::FEATURES;
    use crate::theme::Layout;
    let (mut app, dir) = desktop_app("registry");
    // QuickBooks Desktop: the named menu entry exists and is enabled.
    for f in FEATURES {
        let items = crate::desktop::menu_items(&app, f.desktop.0);
        let hit = items.iter().find(|(l, _, _)| l.trim() == f.desktop.1);
        assert!(hit.is_some_and(|h| h.2.is_some()), "Desktop: {} ▸ {} missing or disabled (feature {})", f.desktop.0, f.desktop.1, f.id);
    }
    let mut ids = std::collections::HashSet::new();
    for f in FEATURES { assert!(ids.insert(f.id), "duplicate feature id {}", f.id); }
    for layout in [Layout::Online, Layout::Xero] {
        let t = app.update(Message::SetLayout(layout)); settle(&mut app, t);
        for f in FEATURES {
            let (place, label) = if layout == Layout::Online { f.online } else { f.xero };
            // Open the place the feature lives in.
            app.new_menu = false; app.gear_menu = false; app.menu = None;
            app.route = Route::Dashboard; app.data = None;
            match (layout, place) {
                (_, "top") => {}
                (_, "new") => app.new_menu = true,
                (_, "settings") => app.gear_menu = true,
                (Layout::Online, p) if p.starts_with("new:") => app.new_menu = true,
                (Layout::Online, p) if p.starts_with("gear:") => app.gear_menu = true,
                (Layout::Online, sec) => app.route = match sec { "sales" => Route::AllSales, "expenses" => Route::Expenses, "payroll" => Route::Payroll, "taxes" => Route::Taxes,
                    "reports" => Route::Reports, "accounting" => Route::Chart, other => panic!("{}: unknown Online place {other}", f.id) },
                (_, m) => app.menu = crate::desktop::MENUS.iter().copied().chain(["business", "accounting", "payroll", "contacts"]).find(|x| *x == m),
            }
            let msgs = {
                let _g = sim_lock();
                let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1600.0, 1400.0), app.view());
                ui.click(label).unwrap_or_else(|e| panic!("{layout:?}: {place} ▸ {label} not found (feature {}): {e:?}", f.id));
                ui.into_messages().collect::<Vec<_>>()
            };
            assert!(!msgs.is_empty(), "{layout:?}: {place} ▸ {label} is disabled (feature {})", f.id);
            if place != "top" {
                let want = format!("{:?}", (f.go)());
                assert!(msgs.iter().any(|m| format!("{m:?}") == want), "{layout:?}: {place} ▸ {label} did {msgs:?}, expected {want}");
            }
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// Snapshot a layout with a menu open: LC_LAYOUT=xero LC_OPEN=business (or new / gear) LC_ROUTE=dashboard
#[test]
#[ignore]
fn render_layout_menu() {
    use crate::theme::Layout;
    let (mut app, dir) = desktop_app("menu-shot");
    let layout = Layout::from_key(&std::env::var("LC_LAYOUT").unwrap_or_default());
    let t = app.update(Message::SetLayout(layout)); settle(&mut app, t);
    if std::env::var("LC_LIGHT").is_ok() { app.dark = false; }
    let route = crate::app::parse_route(&std::env::var("LC_ROUTE").unwrap_or_else(|_| "dashboard".into())).unwrap();
    let t = app.update(Message::Nav(route)); settle(&mut app, t);
    match std::env::var("LC_OPEN").as_deref() {
        Ok("new") => app.new_menu = true,
        Ok("gear") => app.gear_menu = true,
        Ok(m) => app.menu = ["business", "accounting", "payroll", "contacts"].into_iter().chain(crate::acumatica::WORKSPACES.iter().map(|w| w.0)).chain(crate::sage::MENUS.iter().map(|w| w.0)).find(|x| *x == m),
        _ => {}
    }
    let _ = std::fs::remove_file(std::env::temp_dir().join("lc-route-wgpu.png"));
    let _g = sim_lock();
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 900.0), app.view());
    ui.snapshot(&app.theme()).unwrap().matches_image(std::env::temp_dir().join("lc-route.png")).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn xero_business_menu_order_lists_everything() {
    let order: Vec<&str> = crate::features::xero("business").into_iter().map(|x| x.0).collect();
    assert_eq!(&order[..3], &["Sales overview", "Invoices", "Quotes"]);
    // Every Business entry has a place in Xero's order (none fall to the end unsorted).
    let all = crate::features::FEATURES.iter().filter(|f| f.xero.0 == "business").count();
    assert_eq!(order.len(), all);
}

#[test]
fn end_to_end_fixed_assets() {
    use crate::extras::Msg as X;
    let (mut app, dir) = desktop_app("fixedassets");
    let t = app.update(Message::Nav(Route::FixedAssets)); settle(&mut app, t);
    press(&mut app, "New asset");
    for (k, v) in [("name", "Laptop"), ("purchase_price", "2400"), ("life_years", "3")] { let _ = app.update(Message::Extras(X::FaField(k, v.into()))); }
    let _ = app.update(Message::Extras(X::FaField("purchase_date", app.d("2026-01-01"))));
    let _ = app.update(Message::Extras(X::FaMethod("STRAIGHTLINE")));
    press(&mut app, "Save");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let asset = app.data.as_ref().unwrap()["assets"].as_array().unwrap().iter().find(|a| a["name"] == "Laptop").cloned().expect("draft saved");
    assert_eq!(asset["status"], "DRAFT");
    let id = asset["id"].as_i64().unwrap();
    let t = app.update(Message::Extras(X::FaRegister(id))); settle(&mut app, t);
    let _ = app.update(Message::Extras(X::FaDepDate(app.d("2026-06-30"))));
    let t = app.update(Message::Extras(X::FaRun)); settle(&mut app, t);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let a = app.be.call_now("assets.get", json!({"id": id})).unwrap();
    assert!(a["accum_dep"].as_i64().unwrap() > 0, "depreciated: {a}");
    // Dispose for 1,500.
    let t = app.update(Message::Extras(X::FaDisposeOpen(id))); settle(&mut app, t);
    let _ = app.update(Message::Extras(X::FaDisposeField(1, "1500".into())));
    let t = app.update(Message::Extras(X::FaDispose)); settle(&mut app, t);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert_eq!(app.be.call_now("assets.get", json!({"id": id})).unwrap()["status"], "DISPOSED");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn audit_log_and_currencies() {
    use crate::extras::Msg as X;
    let (mut app, dir) = desktop_app("audit-fx");
    let t = app.update(Message::Nav(Route::AuditLog)); settle(&mut app, t);
    assert!(!app.data.as_ref().unwrap().as_array().unwrap().is_empty(), "demo has history");
    let t = app.update(Message::Extras(X::AuditEntity("Contacts"))); settle(&mut app, t);
    assert!(app.data.as_ref().unwrap().as_array().unwrap().iter().all(|e| e["entity"] == "contact"));
    // Currencies: turn one on, set a rate, revalue.
    let t = app.update(Message::Nav(Route::Currencies)); settle(&mut app, t);
    let code = app.data.as_ref().unwrap().as_array().unwrap().iter().find(|c| c["enabled"].as_i64() != Some(1) && c["code"] != app.setting("base_currency").as_str()).unwrap()["code"].as_str().unwrap().to_string();
    let t = app.update(Message::Extras(X::FxEnable(code.clone(), true))); settle(&mut app, t);
    let _ = app.update(Message::Extras(X::FxRateOpen(code.clone())));
    let _ = app.update(Message::Extras(X::FxRateField(1, "0.61".into())));
    let t = app.update(Message::Extras(X::FxRateSave)); settle(&mut app, t);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let c = app.data.as_ref().unwrap().as_array().unwrap().iter().find(|c| c["code"] == code.as_str()).cloned().unwrap();
    assert_eq!(c["enabled"], 1);
    assert_eq!(c["latest_rate_e8"], 61_000_000);
    let t = app.update(Message::Extras(X::FxRevalue)); settle(&mut app, t);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn audit_details_are_readable() {
    use crate::extras::describe;
    assert_eq!(describe(&json!("{\"name\":\"Alex Morgan\"}")), "name: Alex Morgan");
    assert_eq!(describe(&json!({"status": "AUTHORISED", "total": 5000, "type": "ACCRECCREDIT"})), "status: AUTHORISED · total: 50.00 · type: ACCRECCREDIT");
    assert_eq!(describe(&serde_json::Value::Null), "");
}

#[test]
fn budgets_grid_edit_copy_across_and_new() {
    use crate::extras::Msg as X;
    let (mut app, dir) = desktop_app("budgets");
    let t = app.update(Message::Nav(Route::Budgets)); settle(&mut app, t);
    let b = app.data.as_ref().unwrap()["budget"].clone();
    assert!(b["id"].is_number(), "demo budget shown");
    let sales = app.lk.accounts.iter().find(|a| a["code"] == "200").unwrap()["id"].as_i64().unwrap();
    // The demo budget's first month for 200 Sales is 14,000; change it and copy across.
    let first = b["lines"].as_array().unwrap().iter().filter(|l| l["account_id"].as_i64() == Some(sales)).map(|l| l["period"].as_str().unwrap().to_string()).min().unwrap();
    let _ = app.update(Message::Extras(X::BudCell(sales, first.clone(), "15000".into())));
    let _ = app.update(Message::Extras(X::BudCopyAcross(sales)));
    press(&mut app, "Save");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let after = app.be.call_now("budgets.get", json!({"id": b["id"]})).unwrap();
    let sales_lines: Vec<i64> = after["lines"].as_array().unwrap().iter().filter(|l| l["account_id"].as_i64() == Some(sales)).map(|l| l["amount"].as_i64().unwrap()).collect();
    assert!(sales_lines.len() >= 12 && sales_lines.iter().all(|a| *a == 1_500_000), "{sales_lines:?}");
    // A new budget.
    let _ = app.update(Message::Extras(X::BudNew(Some("Stretch".into()))));
    let t = app.update(Message::Extras(X::BudCreate)); settle(&mut app, t);
    assert_eq!(app.data.as_ref().unwrap()["budget"]["name"], "Stretch");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn expense_claims_receipt_and_mileage() {
    use crate::extras::Msg as X;
    let (mut app, dir) = desktop_app("claims");
    let t = app.update(Message::Nav(Route::ExpenseClaims)); settle(&mut app, t);
    let bills_before = count(&app, "ACCPAY");
    // A receipt claim, submitted, then approved → a bill to the claimant.
    press(&mut app, "New receipt");
    for (k, v) in [("supplier", "Officeworks"), ("description", "Printer paper"), ("amount", "44.00")] { let _ = app.update(Message::Extras(X::ClField(k, v.into()))); }
    let acct = app.lk.account_opts(|a| a["class"] == "EXPENSE")[0].clone();
    let _ = app.update(Message::Extras(X::ClAccount(acct)));
    press(&mut app, "Submit for approval");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let claim = app.data.as_ref().unwrap().as_array().unwrap().iter().find(|r| r["supplier"] == "Officeworks").cloned().unwrap();
    assert_eq!(claim["status"], "SUBMITTED");
    let t = app.update(Message::Extras(X::ClStatus(claim["id"].as_i64().unwrap(), "AUTHORISED"))); settle(&mut app, t);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert_eq!(count(&app, "ACCPAY"), bills_before + 1, "approval creates the reimbursement bill");
    // Mileage: the Mileage screen shows trips only.
    let t = app.update(Message::Nav(Route::Mileage)); settle(&mut app, t);
    press(&mut app, "New trip");
    for (k, v) in [("purpose", "Client visit"), ("distance", "42")] { let _ = app.update(Message::Extras(X::ClField(k, v.into()))); }
    press(&mut app, "Save draft");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let trip = app.data.as_ref().unwrap().as_array().unwrap().iter().find(|r| r["reference"] == "Client visit").cloned().expect("trip saved");
    assert_eq!(trip["kind"], "MILEAGE");
    assert_eq!(trip["total"], if app.country() == "US" { 2814 } else { 3696 });
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn bank_rules_and_import_export() {
    use crate::extras::Msg as X;
    let (mut app, dir) = desktop_app("rules-ie");
    // Bank rule: money out whose payee contains "BP" → Motor Vehicle Expenses.
    let t = app.update(Message::Nav(Route::BankRules)); settle(&mut app, t);
    let before = app.data.as_ref().unwrap().as_array().unwrap().len();
    press(&mut app, "New rule");
    let _ = app.update(Message::Extras(X::BrField("field", "payee".into())));
    let _ = app.update(Message::Extras(X::BrField("value", "BP".into())));
    let motor = app.lk.account_opts(|a| a["type"] == "EXPENSE")[0].clone();
    let _ = app.update(Message::Extras(X::BrAccount(motor)));
    press(&mut app, "Save rule");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let rules = app.data.as_ref().unwrap().as_array().unwrap().clone();
    assert_eq!(rules.len(), before + 1);
    let r = rules.iter().find(|r| r["name"] == "Code BP").expect("named from the condition");
    let t = app.update(Message::Extras(X::BrDelete(r["id"].as_i64().unwrap()))); settle(&mut app, t);
    assert_eq!(app.data.as_ref().unwrap().as_array().unwrap().len(), before);
    // Import contacts: preview first, then import.
    let t = app.update(Message::Nav(Route::ImportExport)); settle(&mut app, t);
    let _ = app.update(Message::Extras(X::IeWhat("contacts")));
    let csv = "ContactName,EmailAddress\nHarbour Lights Pty Ltd,accounts@harbourlights.test\nSeaside Dental,admin@seaside.test\n";
    let t = app.update(Message::Extras(X::IePicked(Ok(Some(("contacts.csv".into(), csv.into())))))); settle(&mut app, t);
    let p = app.extras.ie_preview.clone().expect("preview");
    assert_eq!(p["created"], 2, "{p}");
    assert!(!app.be.call_now("contacts.list", json!({"filter": "all"})).unwrap().to_string().contains("Seaside Dental"), "nothing changed by the preview");
    let t = app.update(Message::Extras(X::IeRun)); settle(&mut app, t);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert!(app.be.call_now("contacts.list", json!({"filter": "all"})).unwrap().to_string().contains("Seaside Dental"));
    // Export.
    let file = std::env::temp_dir().join("lc-test-output").join("contacts.csv");
    let _ = std::fs::remove_file(&file);
    let t = app.update(Message::Extras(X::IeExport("contacts"))); settle(&mut app, t);
    assert!(std::fs::read_to_string(&file).unwrap().contains("Seaside Dental"));
    let _ = std::fs::remove_file(file);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn custom_report_builder_runs() {
    use crate::extras::Msg as X;
    let (mut app, dir) = desktop_app("custrep");
    let t = app.update(Message::Nav(Route::CustomReports)); settle(&mut app, t);
    press(&mut app, "New report");
    let _ = app.update(Message::Extras(X::CrName("Owner's summary".into())));
    let _ = app.update(Message::Extras(X::CrMonthly(false)));
    press(&mut app, "Save");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let id = app.data.as_ref().unwrap().as_array().unwrap().iter().find(|l| l["name"] == "Owner's summary").unwrap()["id"].as_i64().unwrap();
    let t = app.update(Message::Nav(Route::Report(format!("layout:{id}")))); settle(&mut app, t);
    let d = app.data.clone().expect("report ran");
    let rows = d["sections"][0]["rows"].as_array().unwrap().clone();
    assert_eq!(rows.len(), 3);
    let v = |n: usize| rows[n]["values"][0].as_i64().unwrap();
    assert_eq!(v(2), v(0) - v(1), "net = income - expenses");
    // It renders in the report window.
    let _ = click_fails(&app, "nothing");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[ignore]
fn render_custom_report() {
    use crate::extras::Msg as X;
    let (mut app, dir) = desktop_app("custrep-shot");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Xero)); settle(&mut app, t);
    let t = app.update(Message::Nav(Route::CustomReports)); settle(&mut app, t);
    let _ = app.update(Message::Extras(X::CrNew(true)));
    let shot = |app: &App, name: &str| {
        let _ = std::fs::remove_file(std::env::temp_dir().join(format!("{name}-wgpu.png")));
        let _g = sim_lock();
        let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 900.0), app.view());
        ui.snapshot(&app.theme()).unwrap().matches_image(std::env::temp_dir().join(format!("{name}.png"))).unwrap();
    };
    shot(&app, "lc-cr-builder");
    let _ = app.update(Message::Extras(X::CrName("Owner's summary".into())));
    let t = app.update(Message::Extras(X::CrSave)); settle(&mut app, t);
    let id = app.data.as_ref().unwrap().as_array().unwrap()[0]["id"].as_i64().unwrap();
    let t = app.update(Message::Nav(Route::Report(format!("layout:{id}")))); settle(&mut app, t);
    shot(&app, "lc-cr-report");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn new_user_with_password_and_tracked_item_online() {
    use crate::settings::Msg as S;
    let (mut app, dir) = desktop_app("user-pass");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Online)); settle(&mut app, t);
    let t = app.update(Message::Nav(Route::Settings)); settle(&mut app, t);
    let _ = app.update(Message::Settings(S::AddUser));
    for m in [S::UName("Jamie Bookkeeper".into()), S::UEmail("jamie@example.test".into()), S::UPassword("long enough pw".into())] { let _ = app.update(Message::Settings(m)); }
    let t = app.update(Message::Settings(S::SaveUser)); settle(&mut app, t);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let users = app.be.call_now("users.list", json!({})).unwrap();
    assert!(users.as_array().unwrap().iter().any(|u| u["email"] == "jamie@example.test"));
    assert!(app.be.token.is_some(), "this session stays signed in after the first password");
    assert!(app.be.call_now("session.get", json!({})).unwrap()["login_required"].is_null());
    // Online-style New item can track inventory now.
    let t = app.update(Message::Nav(Route::NewProduct)); settle(&mut app, t);
    for m in [crate::forms::Msg::Name("Notebook".into()), crate::forms::Msg::ItemType("Inventory Part".into()), crate::forms::Msg::Price("12".into())] { let _ = app.update(Message::Form(m)); }
    let t = app.update(Message::Form(crate::forms::Msg::Save)); settle(&mut app, t);
    let it = app.be.call_now("items.list", json!({})).unwrap().as_array().unwrap().iter().find(|i| i["name"] == "Notebook").cloned().expect("saved");
    assert_eq!(it["is_tracked"], 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn payroll_settings_schedules_and_leave() {
    use crate::extras::Msg as X;
    let (mut app, dir) = desktop_app("paysettings");
    let t = app.update(Message::Nav(Route::PayrollSettings)); settle(&mut app, t);
    let cals = app.data.as_ref().unwrap()["calendars"].as_array().unwrap().len();
    let leaves = app.data.as_ref().unwrap()["leave_types"].as_array().unwrap().len();
    let _ = app.update(Message::Extras(X::PsCalNew(true)));
    let _ = app.update(Message::Extras(X::PsCalField(0, "Weekly casuals".into())));
    let _ = app.update(Message::Extras(X::PsCalFreq("WEEKLY")));
    let t = app.update(Message::Extras(X::PsCalSave)); settle(&mut app, t);
    let _ = app.update(Message::Extras(X::PsLeaveNew(true)));
    let _ = app.update(Message::Extras(X::PsLeaveField(0, "Long service leave".into())));
    let _ = app.update(Message::Extras(X::PsLeaveField(1, "32.5".into())));
    let t = app.update(Message::Extras(X::PsLeaveSave)); settle(&mut app, t);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let d = app.data.as_ref().unwrap();
    assert_eq!(d["calendars"].as_array().unwrap().len(), cals + 1);
    assert!(d["calendars"].as_array().unwrap().iter().any(|c| c["name"] == "Weekly casuals" && c["frequency"] == "WEEKLY"));
    assert_eq!(d["leave_types"].as_array().unwrap().len(), leaves + 1);
    assert!(d["leave_types"].as_array().unwrap().iter().any(|l| l["name"] == "Long service leave" && l["accrual_hours_per_year_e2"] == 3250));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn payslips_and_bank_file() {
    use crate::extras::Msg as X;
    let (mut app, dir) = desktop_app("payslips");
    let cal = app.be.call_now("payroll.settings", json!({})).unwrap()["calendars"][0]["id"].as_i64().unwrap();
    let run = app.be.call_now("payroll.createPayRun", json!({"calendar_id": cal})).unwrap()["id"].as_i64().unwrap();
    app.be.call_now("payroll.post", json!({"id": run, "bank_account_id": app.lk.bank_opts()[0].id})).unwrap();
    let number = app.be.call_now("payroll.payRun", json!({"id": run})).unwrap()["number"].as_str().unwrap().to_string();
    let employees = app.be.call_now("payroll.payRun", json!({"id": run})).unwrap()["payslips"].as_array().unwrap().len();
    let t = app.update(Message::Nav(Route::PayRun(run))); settle(&mut app, t);
    press(&mut app, "Print payslips");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let file = std::env::temp_dir().join("lc-test-output").join(format!("Payslips {number}.pdf"));
    let pdf = std::fs::read(&file).expect("payslips written");
    assert_eq!(String::from_utf8_lossy(&pdf).matches("/Type /Page /").count(), employees, "a page per employee");
    let _ = std::fs::remove_file(file);
    // Bank file: written, or a clear message about missing bank details.
    app.toasts.clear();
    let t = app.update(Message::Extras(X::PayrunBankFile(run))); settle(&mut app, t);
    let toast = app.toasts.last().cloned().expect("a result");
    assert!(!toast.2 || toast.1.contains("bank account details"), "{toast:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[ignore]
fn sample_payslip_pdf() {
    let (app, dir) = desktop_app("payslip-sample");
    let cal = app.be.call_now("payroll.settings", json!({})).unwrap()["calendars"][0]["id"].as_i64().unwrap();
    let run = app.be.call_now("payroll.createPayRun", json!({"calendar_id": cal})).unwrap()["id"].as_i64().unwrap();
    let r = app.be.call_now("payroll.payRun", json!({"id": run})).unwrap();
    let st = app.be.call_now("settings.get", json!({})).unwrap();
    std::fs::write(std::env::temp_dir().join("lc-payslip-sample.pdf"), crate::pdf::payslips(&r, &st, &|x| crate::ui::date(x, crate::ui::DateStyle::Us), "401(k)")).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn projects_tasks_time_and_costs() {
    use crate::extras::Msg as X;
    let (mut app, dir) = desktop_app("projects");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Xero)); settle(&mut app, t);
    let t = app.update(Message::Nav(Route::Projects)); settle(&mut app, t);
    press(&mut app, "New project");
    let _ = app.update(Message::Extras(X::PjNewName("Website rebuild".into())));
    let _ = app.update(Message::Extras(X::PjNewCustomer(app.lk.contact_opts(true)[0].clone())));
    let t = app.update(Message::Extras(X::PjCreate)); settle(&mut app, t);
    let pid = app.data.as_ref().unwrap()["projects"].as_array().unwrap().iter().find(|p| p["name"] == "Website rebuild").unwrap()["id"].as_i64().unwrap();
    let msgs = click(&app, "Website rebuild");
    assert!(msgs.iter().any(|m| matches!(m, Message::Extras(X::PjSelect(x)) if *x == pid)), "{msgs:?}");
    let t = app.update(Message::Extras(X::PjSelect(pid))); settle(&mut app, t);
    // Task, time, expense.
    let _ = app.update(Message::Extras(X::PjTaskOpen(true)));
    let _ = app.update(Message::Extras(X::PjTaskField(0, "Design".into())));
    let _ = app.update(Message::Extras(X::PjTaskField(1, "150".into())));
    let t = app.update(Message::Extras(X::PjTaskSave)); settle(&mut app, t);
    let task = app.extras.pj_detail.as_ref().unwrap()["tasks"][0].clone();
    assert_eq!(task["name"], "Design");
    let _ = app.update(Message::Extras(X::PjTimeOpen(true)));
    let _ = app.update(Message::Extras(X::PjTimeTask(crate::app::Opt { id: task["id"].as_i64().unwrap(), label: "Design".into() })));
    let _ = app.update(Message::Extras(X::PjTimeField(1, "2:30".into())));
    let t = app.update(Message::Extras(X::PjTimeSave)); settle(&mut app, t);
    let _ = app.update(Message::Extras(X::PjCostOpen(true)));
    let _ = app.update(Message::Extras(X::PjCostField(1, "Stock photos".into())));
    let _ = app.update(Message::Extras(X::PjCostField(2, "80".into())));
    let t = app.update(Message::Extras(X::PjCostSave)); settle(&mut app, t);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let p = app.extras.pj_detail.clone().unwrap();
    assert_eq!(p["time"][0]["minutes"], 150);
    assert_eq!(p["costs"][0]["amount"], 8000);
    // 2.5 h × 150 + 80 chargeable = 455.00 to invoice.
    assert_eq!(p["summary"]["to_invoice"], 45500, "{}", p["summary"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn edit_screens_in_online_and_xero_and_search_opens_records() {
    let (mut app, dir) = desktop_app("edit-online");
    for layout in [crate::theme::Layout::Online, crate::theme::Layout::Xero] {
        let t = app.update(Message::SetLayout(layout)); settle(&mut app, t);
        // Item list row → Edit Item, with the item's details (not the journal form).
        let t = app.update(Message::Nav(Route::Products)); settle(&mut app, t);
        let item = app.data.as_ref().unwrap().as_array().unwrap()[0].clone();
        let msgs = click(&app, item["name"].as_str().unwrap());
        assert!(msgs.iter().any(|m| matches!(m, Message::Nav(Route::EditItem(_)))), "{layout:?}: {msgs:?}");
        let t = app.update(Message::Nav(Route::EditItem(item["id"].as_i64().unwrap()))); settle(&mut app, t);
        assert!(!click_fails(&app, "ITEM NAME/NUMBER"), "{layout:?}: item form shows");
        // Contact page → Edit.
        let cid = app.lk.contact_opts(true)[0].id;
        let t = app.update(Message::Nav(Route::Contact(cid))); settle(&mut app, t);
        assert!(click(&app, "Edit").iter().any(|m| matches!(m, Message::Nav(Route::EditContact(x)) if *x == cid)));
        // Chart → Edit account for a non-bank account.
        let acct = app.lk.accounts.iter().find(|a| a["code"] == "200").unwrap().clone();
        let t = app.update(Message::Nav(Route::Chart)); settle(&mut app, t);
        let msgs = click(&app, acct["code"].as_str().unwrap());
        assert!(msgs.iter().any(|m| matches!(m, Message::Nav(Route::EditAccount(_)))), "{layout:?} {msgs:?}");
    }
    // Search results open items and accounts.
    let t = app.update(Message::OpenResult("item".into(), 1)); settle(&mut app, t);
    assert!(matches!(app.route, Route::EditItem(1)));
    // A money-in bank transaction doesn't open on the check form.
    let rx = app.be.call_now("bank.transactions", json!({"id": app.lk.bank_opts()[0].id})).unwrap().as_array().unwrap().iter()
        .find(|r| r["kind"] == "banktx" && r["amount"].as_i64().unwrap_or(0) > 0).cloned();
    if let Some(rx) = rx {
        let t = app.update(Message::Nav(Route::BankTx(rx["id"].as_i64().unwrap()))); settle(&mut app, t);
        assert!(matches!(app.route, Route::Banking(_)), "{:?}", app.route);
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn two_step_verification_on_and_off() {
    use crate::settings::Msg as S;
    let (mut app, dir) = desktop_app("twofa");
    let t = app.update(Message::Nav(Route::Settings)); settle(&mut app, t);
    let _ = app.update(Message::Settings(S::Cat("Users & Roles")));
    press(&mut app, "Set up");
    let secret = app.settings.tfa.as_ref().expect("setup started").0.clone();
    assert!(!click_fails(&app, "Turn on"), "QR code and code box show");
    // A wrong code is refused; the right one turns it on.
    let _ = app.update(Message::Settings(S::TfaCode("000000".into())));
    let t = app.update(Message::Settings(S::TfaConfirm)); settle(&mut app, t);
    let counter = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() / 30;
    let code = ledger_core::users::totp_code(&ledger_core::users::base32_decode(&secret).unwrap(), counter);
    if code != "000000" { assert!(app.toasts.iter().any(|t| t.2), "wrong code refused"); }
    app.toasts.clear();
    let _ = app.update(Message::Settings(S::TfaCode(code)));
    let t = app.update(Message::Settings(S::TfaConfirm)); settle(&mut app, t);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let me = app.settings.users.iter().find(|u| u["id"] == 1).unwrap().clone();
    assert!(me["has_2fa"] == 1 || me["has_2fa"] == true, "{me}");
    press(&mut app, "Turn off");
    let me = app.settings.users.iter().find(|u| u["id"] == 1).unwrap().clone();
    assert!(me["has_2fa"] == 0 || me["has_2fa"] == false, "{me}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn csv_statement_columns_matched_by_hand() {
    use crate::banking::Msg as B;
    let (mut app, dir) = desktop_app("csvmap");
    let acc = app.lk.bank_opts()[0].id;
    let t = app.update(Message::Nav(Route::Banking(Some(acc)))); settle(&mut app, t);
    // A file whose columns can't be guessed goes to matching by hand rather than failing.
    let odd = "Posted,Details,Value\nOct 1,COFFEE HUT,-4.50\n";
    assert_eq!(crate::import::import_file(&app.be, acc, "odd.csv", odd, false).unwrap_err(), crate::import::NEEDS_MAPPING);
    let csv2 = "Posted,Details,Value\n01/10/2026,COFFEE HUT,-4.50\n02/10/2026,REFUND,12.00\n";
    let rows = crate::import::preview_rows(&app.be, "odd.csv", csv2).unwrap();
    let t = app.update(Message::Bank(B::MapStart(Ok(Some(("odd.csv".into(), csv2.into(), rows)))))); settle(&mut app, t);
    assert!(app.bank.map.as_ref().unwrap().has_header);
    assert!(!click_fails(&app, "Match the columns"));
    for m in [B::MapCol("date", Some(0)), B::MapCol("description", Some(1)), B::MapCol("amount", Some(2)), B::MapDate("DMY")] { let _ = app.update(Message::Bank(m)); }
    let lines_before = app.be.call_now("statements.lines", json!({"account_id": acc, "status": "ALL"})).map(|v| v.as_array().map(|a| a.len()).unwrap_or(0)).unwrap_or(0);
    let t = app.update(Message::Bank(B::MapImport)); settle(&mut app, t);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert!(app.toasts.iter().any(|t| t.1.contains("2 transaction(s) imported")), "{:?}", app.toasts);
    let lines_after = app.be.call_now("statements.lines", json!({"account_id": acc, "status": "ALL"})).map(|v| v.as_array().map(|a| a.len()).unwrap_or(0)).unwrap_or(0);
    assert!(lines_after >= lines_before);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn second_tracking_category() {
    use crate::desktop::Msg as W;
    let (mut app, dir) = desktop_app("tracking2");
    let t = app.update(Message::Nav(Route::ClassList)); settle(&mut app, t);
    let cats = app.data.as_ref().unwrap().as_array().unwrap().len();
    if cats < 2 {
        let _ = app.update(Message::Desk(W::ClassCatNew(Some("Location".into()))));
        let t = app.update(Message::Desk(W::ClassCatAdd)); settle(&mut app, t);
        assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    }
    let cats = app.be.call_now("tracking.list", json!({})).unwrap();
    assert_eq!(cats.as_array().unwrap().len(), 2);
    let second = cats.as_array().unwrap()[1].clone();
    let _ = app.update(Message::Desk(W::ClassCat(second["id"].as_i64().unwrap())));
    let _ = app.update(Message::Desk(W::ClassNew("Melbourne".into())));
    let t = app.update(Message::Desk(W::ClassAdd)); settle(&mut app, t);
    let after = app.be.call_now("tracking.list", json!({})).unwrap();
    let opts = after.as_array().unwrap().iter().find(|c| c["id"] == second["id"]).unwrap()["options"].to_string();
    assert!(opts.contains("Melbourne"), "{opts}");
    // A third category is refused by the engine.
    let _ = app.update(Message::Desk(W::ClassCatNew(Some("Region".into()))));
    let t = app.update(Message::Desk(W::ClassCatAdd)); settle(&mut app, t);
    assert!(app.toasts.iter().any(|t| t.2));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn xero_invoice_list_summary_and_repeating_tab() {
    use crate::theme::Layout;
    let (mut app, dir) = desktop_app("xero-invlist");
    let t = app.update(Message::SetLayout(Layout::Xero)); settle(&mut app, t);
    let t = app.update(Message::Nav(Route::Invoices)); settle(&mut app, t);
    // The summary boxes open their tab ("Overdue" text also appears in the status chips, so send the box's message).
    let t = app.update(Message::ListTab("overdue")); settle(&mut app, t);
    assert_eq!(app.list_tab, "overdue");
    let rows = app.data.as_ref().unwrap()["rows"].as_array().unwrap().clone();
    assert!(!rows.is_empty() && rows.iter().all(|r| r["overdue"] == 1), "only overdue invoices");
    for b in ["Import", "Export", "Send statements", "New credit note", "New repeating invoice"] { press(&mut app, b); let t = app.update(Message::Nav(Route::Invoices)); settle(&mut app, t); }
    press(&mut app, "Repeating");
    assert_eq!(app.route, Route::Memorized);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn xero_invoice_submit_for_approval_then_approve() {
    let (mut app, dir) = demo_app("xero-submit", Skin::Xero);
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None })); settle(&mut app, t);
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    {
        let e = app.doc.as_mut().unwrap();
        e.contact = Some(cust);
        e.lines[0].description = "Consulting".into();
        e.lines[0].price = "250".into();
    }
    // Xero's breadcrumb and buttons.
    for b in ["Sales overview", "Invoices", "Save draft", "Approve", "Approve & email"] { let _ = click(&app, b); }
    press(&mut app, "Submit for approval");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let id = app.doc.as_ref().and_then(|e| e.id).expect("saved");
    assert_eq!(app.be.call_now("docs.get", json!({"id": id})).unwrap()["status"], "SUBMITTED");
    // Awaiting approval: Save keeps it there; Approve authorises it.
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: Some(id) })); settle(&mut app, t);
    let _ = click(&app, "Save");
    press(&mut app, "Approve");
    assert_eq!(app.be.call_now("docs.get", json!({"id": id})).unwrap()["status"], "AUTHORISED");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn xero_reconcile_tabs_transfer_and_discuss() {
    use crate::banking::Msg as B;
    let (mut app, dir) = demo_app("xero-rec", Skin::Xero);
    let t = app.update(Message::Nav(Route::Banking(None))); settle(&mut app, t);
    let lines = app.data.as_ref().unwrap()["lines"].as_array().unwrap().clone();
    let line = lines.iter().find(|l| l["payee"].as_str().unwrap_or("").to_lowercase().contains("officeworks")).expect("unmatched line")["id"].as_i64().unwrap();
    // Discuss: a note stays on the line.
    let _ = app.update(Message::Bank(B::XTab(line, "discuss")));
    let _ = app.update(Message::Bank(B::Note("Check the receipt".into())));
    press(&mut app, "Save note");
    let l = app.data.as_ref().unwrap()["lines"].as_array().unwrap().iter().find(|l| l["id"] == line).cloned().unwrap();
    assert_eq!(l["notes"], "Check the receipt");
    // Transfer: reconciled as a transfer to the other bank account.
    let _ = app.update(Message::Bank(B::XTab(line, "transfer")));
    let saver = app.data.as_ref().unwrap()["banks"].as_array().unwrap().iter().find(|b| b["id"] != l["account_id"]).cloned().unwrap();
    let _ = app.update(Message::Bank(B::TransferTo(crate::app::Opt { id: saver["id"].as_i64().unwrap(), label: "Saver".into() })));
    press(&mut app, "Transfer & reconcile");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert!(app.data.as_ref().unwrap()["lines"].as_array().unwrap().iter().all(|l| l["id"] != line), "line reconciled");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn xero_contact_page_history_and_notes() {
    let (mut app, dir) = demo_app("xero-contact", Skin::Xero);
    let id = app.lk.contact_opts(true).into_iter().next().unwrap().id;
    let t = app.update(Message::Nav(Route::Contact(id))); settle(&mut app, t);
    for b in ["Contact details", "Money in", "Recent activity", "History & notes"] { let _g = sim_lock(); let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 1400.0), app.view()); assert!(ui.find(b).is_ok(), "missing {b}"); }
    let _ = app.update(Message::Extras(crate::extras::Msg::ContactNote("Prefers email reminders".into())));
    press(&mut app, "Add note");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let hist = app.data.as_ref().unwrap()["history"].as_array().unwrap().clone();
    assert!(hist.iter().any(|h| h["text"] == "Prefers email reminders" && h["kind"] == "NOTE"), "{hist:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn find_and_recode_moves_lines_to_another_account() {
    use crate::extras::Msg as X;
    let (mut app, dir) = demo_app("recode", Skin::Xero);
    let t = app.update(Message::Nav(Route::Recode)); settle(&mut app, t);
    let rent = app.lk.account_opts(|a| a["name"] == "Rent").into_iter().next().expect("Rent account");
    let _ = app.update(Message::Extras(X::RcAccount(rent.clone())));
    // ("Search" is also the top bar's search box.)
    let t = app.update(Message::Extras(X::RcFind)); settle(&mut app, t);
    let found = app.data.as_ref().unwrap().as_array().unwrap().clone();
    assert!(!found.is_empty() && found.iter().all(|l| l["account_name"] == "Rent"), "{found:?}");
    let first = found[0]["line_id"].as_i64().unwrap();
    let _ = app.update(Message::Extras(X::RcToggle(first)));
    let office = app.lk.account_opts(|a| a["type"] != "BANK" && a["name"].as_str().unwrap_or("").contains("Office")).into_iter().next().expect("office account");
    let _ = app.update(Message::Extras(X::RcToAccount(office.clone())));
    let on_office = |app: &App| app.be.call_now("recode.find", json!({"account_id": office.id})).unwrap().as_array().unwrap().len();
    let before = on_office(&app);
    press(&mut app, "Recode 1 line(s)");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    // The search reloads: one fewer Rent line; the moved line is on the new account.
    let after = app.data.as_ref().unwrap().as_array().unwrap().clone();
    assert_eq!(after.len(), found.len() - 1);
    // (Reposting rewrites the transaction's lines, so they get new ids.)
    assert_eq!(on_office(&app), before + 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn online_sales_action_column_receives_payment() {
    let (mut app, dir) = desktop_app("qbo-action");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Online)); settle(&mut app, t);
    let t = app.update(Message::Nav(Route::AllSales)); settle(&mut app, t);
    let msgs = click(&app, "Receive payment");
    // The header button comes first; a row's link carries its customer.
    assert!(!msgs.is_empty());
    let _g = sim_lock();
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 1400.0), app.view());
    assert!(ui.find("ACTION").is_ok() && ui.find("View").is_ok() && ui.find("Edit").is_ok());
    drop(ui); drop(_g);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn reports_page_star_search_and_custom_tab() {
    use crate::reports::Msg as R;
    let (mut app, dir) = desktop_app("rep-favs");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Xero)); settle(&mut app, t);
    let t = app.update(Message::Nav(Route::Reports)); settle(&mut app, t);
    assert!(!crate::reports::favourites(&app).contains(&"trial-balance".to_string()));
    let _ = app.update(Message::Report(R::Star("trial-balance".into())));
    assert!(crate::reports::favourites(&app).contains(&"trial-balance".to_string()));
    // Kept for next time.
    app.report.favs = None;
    let t = app.update(Message::Nav(Route::Reports)); settle(&mut app, t);
    assert!(crate::reports::favourites(&app).contains(&"trial-balance".to_string()));
    // Search narrows the list; Custom shows saved layouts.
    let _ = app.update(Message::ListQuery("aged".into()));
    {
        let _g = sim_lock();
        let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 1400.0), app.view());
        assert!(ui.find("Balance Sheet").is_err() && ui.find("Favourites").is_ok());
    }
    let t = app.update(Message::ListTab("custom")); settle(&mut app, t);
    let _ = click(&app, "New custom report");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn online_customer_page_tabs() {
    let (mut app, dir) = desktop_app("qbo-contact");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Online)); settle(&mut app, t);
    let id = app.lk.contact_opts(true).into_iter().next().unwrap().id;
    let t = app.update(Message::Nav(Route::Contact(id))); settle(&mut app, t);
    press(&mut app, "Customer details");
    assert_eq!(app.list_tab, "details");
    press(&mut app, "Notes");
    let _ = app.update(Message::Extras(crate::extras::Msg::ContactNote("Call before delivery".into())));
    press(&mut app, "Add note");
    assert!(app.data.as_ref().unwrap()["history"].as_array().unwrap().iter().any(|h| h["text"] == "Call before delivery"));
    press(&mut app, "Statements");
    assert_eq!(app.route, Route::Statements);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn xero_organisation_settings_tiles() {
    let (mut app, dir) = desktop_app("xero-tiles");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Xero)); settle(&mut app, t);
    let t = app.update(Message::Nav(Route::Settings)); settle(&mut app, t);
    press(&mut app, "Financial settings");
    assert_eq!(app.settings.cat, Some("Financial settings"));
    assert!(!click_fails(&app, "Organisation settings"));
    press(&mut app, "Organisation settings");
    assert_eq!(app.settings.cat, None);
    press(&mut app, "Tracking categories");
    assert_eq!(app.route, Route::ClassList);
    // Coming back opens on the tiles again.
    let _ = app.update(Message::Settings(crate::settings::Msg::Cat("Users")));
    let t = app.update(Message::Nav(Route::Settings)); settle(&mut app, t);
    assert_eq!(app.settings.cat, None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn xero_projects_tabs_and_pay_run_wording() {
    let (mut app, dir) = desktop_app("xero-projtabs");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Xero)); settle(&mut app, t);
    let t = app.update(Message::Nav(Route::Projects)); settle(&mut app, t);
    assert!(!click_fails(&app, "Pinecrest brand refresh"));
    press(&mut app, "Closed");
    assert_eq!(app.list_tab, "closed");
    assert!(click_fails(&app, "Pinecrest brand refresh"), "in-progress project hidden on Closed");
    press(&mut app, "In progress");
    assert!(!click_fails(&app, "Pinecrest brand refresh"));
    let t = app.update(Message::Nav(Route::Payroll)); settle(&mut app, t);
    assert!(!click_fails(&app, "Add pay run"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn every_feature_in_the_acumatica_layout() {
    let (mut app, dir) = desktop_app("acu-registry");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Acumatica)); settle(&mut app, t);
    assert_eq!(app.skin, Skin::QuickBooks);
    for (id, ws, _, label) in crate::acumatica::PLACES {
        app.route = Route::Dashboard; app.data = None; app.new_menu = false; app.gear_menu = false;
        app.menu = Some(ws);
        let msgs = {
            let _g = sim_lock();
            let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 900.0), app.view());
            ui.click(*label).unwrap_or_else(|e| panic!("Acumatica: {ws} ▸ {label} not clickable (feature {id}): {e:?}"));
            ui.into_messages().collect::<Vec<_>>()
        };
        assert!(!msgs.is_empty(), "Acumatica: {ws} ▸ {label} does nothing");
    }
    // Opening a screen from the panel closes it.
    app.menu = Some("payables");
    let t = app.update(Message::Nav(Route::Bills)); settle(&mut app, t);
    assert_eq!(app.menu, None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn acumatica_invoice_hold_release_and_record_navigation() {
    let (mut app, dir) = desktop_app("acu-doc");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Acumatica)); settle(&mut app, t);
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None })); settle(&mut app, t);
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    { let e = app.doc.as_mut().unwrap(); e.contact = Some(cust); e.lines[0].description = "Setup".into(); e.lines[0].price = "300".into(); }
    // Save keeps it On Hold; Release opens it.
    press(&mut app, "Save");
    let id = app.doc.as_ref().and_then(|e| e.id).expect("saved");
    assert_eq!(app.be.call_now("docs.get", json!({"id": id})).unwrap()["status"], "DRAFT");
    assert_eq!(crate::vocab::status(Skin::QuickBooks, "ACCREC", "DRAFT", false), "On Hold");
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: Some(id) })); settle(&mut app, t);
    press(&mut app, "Release");
    assert_eq!(app.be.call_now("docs.get", json!({"id": id})).unwrap()["status"], "AUTHORISED");
    // First / Next move through the invoices.
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: Some(id) })); settle(&mut app, t);
    let t = app.update(Message::Doc(crate::doc::Msg::Step("first"))); settle(&mut app, t);
    let first = match &app.route { Route::Doc { id: Some(x), .. } => *x, r => panic!("{r:?}") };
    assert_ne!(first, id);
    let t = app.update(Message::Doc(crate::doc::Msg::Step("next"))); settle(&mut app, t);
    assert!(matches!(&app.route, Route::Doc { id: Some(x), .. } if *x != first));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn every_feature_in_the_sage_layout() {
    let (mut app, dir) = desktop_app("sage-registry");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Sage)); settle(&mut app, t);
    assert_eq!(app.skin, Skin::Xero);
    for (id, menu, label) in crate::sage::PLACES {
        app.route = Route::Dashboard; app.data = None; app.new_menu = false; app.gear_menu = false;
        app.menu = Some(menu);
        let msgs = {
            let _g = sim_lock();
            let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 1000.0), app.view());
            ui.click(*label).unwrap_or_else(|e| panic!("Sage: {menu} ▸ {label} not clickable (feature {id}): {e:?}"));
            ui.into_messages().collect::<Vec<_>>()
        };
        assert!(!msgs.is_empty(), "Sage: {menu} ▸ {label} does nothing");
    }
    // Sage's words.
    assert_eq!(app.term("bills"), "Purchase Invoices");
    assert_eq!(crate::vocab::status(Skin::Xero, "ACCREC", "AUTHORISED", false), "Outstanding");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn acumatica_remove_hold_and_column_filters() {
    let (mut app, dir) = desktop_app("acu-hold");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Acumatica)); settle(&mut app, t);
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None })); settle(&mut app, t);
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    { let e = app.doc.as_mut().unwrap(); e.contact = Some(cust.clone()); e.lines[0].description = "Audit".into(); e.lines[0].price = "90".into(); }
    press(&mut app, "Remove Hold");
    let id = app.doc.as_ref().and_then(|e| e.id).expect("saved");
    assert_eq!(app.be.call_now("docs.get", json!({"id": id})).unwrap()["status"], "SUBMITTED");
    assert_eq!(crate::vocab::status(Skin::QuickBooks, "ACCREC", "SUBMITTED", false), "Balanced");
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: Some(id) })); settle(&mut app, t);
    assert!(click_fails(&app, "Remove Hold"), "already off hold");
    press(&mut app, "Release");
    assert_eq!(app.be.call_now("docs.get", json!({"id": id})).unwrap()["status"], "AUTHORISED");
    // Column filters narrow the list in place.
    let t = app.update(Message::Nav(Route::Invoices)); settle(&mut app, t);
    let number = app.be.call_now("docs.get", json!({"id": id})).unwrap()["number"].as_str().unwrap().to_string();
    let _ = app.update(Message::ListFilter("number", number.clone()));
    assert!(!click_fails(&app, &number));
    let others: Vec<String> = app.data.as_ref().unwrap()["rows"].as_array().unwrap().iter().map(|r| r["number"].as_str().unwrap_or("").to_string()).filter(|n| *n != number && !n.is_empty()).take(2).collect();
    for n in &others { assert!(click_fails(&app, n), "{n} filtered out"); }
    let _ = app.update(Message::ListFilter("number", String::new()));
    for n in &others { assert!(!click_fails(&app, n), "{n} back"); }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn credit_limit_save_anyway_and_credit_hold() {
    use crate::extras::Msg as X;
    let (mut app, dir) = demo_app("credit", Skin::Xero);
    let cust = app.lk.contact_opts(true).into_iter().next().unwrap();
    // Set a 100.00 limit on the contact page.
    let t = app.update(Message::Nav(Route::Contact(cust.id))); settle(&mut app, t);
    let _ = app.update(Message::Extras(X::CreditText("100".into())));
    press(&mut app, "Save");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert_eq!(app.be.call_now("contacts.get", json!({"id": cust.id})).unwrap()["credit_limit"], 10000);
    // An invoice over it is stopped with "Save anyway".
    let new_invoice = |app: &mut App| {
        let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None })); settle(app, t);
        let e = app.doc.as_mut().unwrap(); e.contact = Some(cust.clone()); e.lines[0].description = "Big job".into(); e.lines[0].price = "250".into();
    };
    new_invoice(&mut app);
    press(&mut app, "Approve");
    assert!(app.doc.as_ref().unwrap().error.as_deref().is_some_and(|e| e.starts_with("CREDIT_LIMIT:")));
    press(&mut app, "Save anyway");
    let id = app.doc.as_ref().and_then(|e| e.id).expect("saved over the limit");
    assert_eq!(app.be.call_now("docs.get", json!({"id": id})).unwrap()["status"], "AUTHORISED");
    // On credit hold nothing new can be approved.
    let t = app.update(Message::Nav(Route::Contact(cust.id))); settle(&mut app, t);
    let t = app.update(Message::Extras(X::CreditHold(cust.id, true))); settle(&mut app, t);
    new_invoice(&mut app);
    press(&mut app, "Approve");
    assert!(app.doc.as_ref().unwrap().error.as_deref().is_some_and(|e| e.contains("credit hold")));
    assert!(click_fails(&app, "Save anyway"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn invoice_profitability_from_item_costs() {
    use crate::doc::Msg as D;
    let (mut app, dir) = demo_app("profit", Skin::QuickBooks);
    let item = app.lk.items.iter().find(|x| x["purchase_unit_e2"].as_i64().unwrap_or(0) > 0 && x["is_sold"] == 1).cloned().expect("an item with a cost");
    let unit_cost = item["avg_cost"].as_i64().filter(|v| *v > 0 && item["is_tracked"] == 1).unwrap_or(item["purchase_unit_e2"].as_i64().unwrap() / 100);
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None })); settle(&mut app, t);
    let opt = crate::app::Opt { id: item["id"].as_i64().unwrap(), label: item["name"].as_str().unwrap_or("").to_string() };
    let t = app.update(Message::Doc(D::Item(0, opt))); settle(&mut app, t);
    let t = app.update(Message::Doc(D::Qty(0, "2".into()))); settle(&mut app, t);
    let price = app.doc.as_ref().unwrap().lines[0].price.clone();
    let _g = sim_lock();
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), iced::Size::new(1360.0, 1400.0), app.view());
    assert!(ui.find("PROFITABILITY").is_ok(), "price {price}");
    assert!(ui.find(crate::ui::money(unit_cost * 2).as_str()).is_ok(), "cost of 2 = {}", crate::ui::money(unit_cost * 2));
    drop(ui); drop(_g);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn progress_invoice_from_an_estimate_in_the_app() {
    use crate::doc::Msg as D;
    let (mut app, dir) = desktop_app("progress");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Online)); settle(&mut app, t);
    let cust = app.lk.contact_opts(true)[0].id;
    let q = app.be.call_now("docs.save", json!({"doc": {"type": "QUOTE", "contact_id": cust, "date": "2026-10-01", "line_amount_type": "NOTAX", "status": "SENT",
        "lines": [{"description": "Fit-out", "quantity": "1", "unit_price": "2000"}]}})).unwrap()["id"].as_i64().unwrap();
    let t = app.update(Message::Nav(Route::Doc { kind: "QUOTE".into(), id: Some(q) })); settle(&mut app, t);
    press(&mut app, "Progress invoice");
    assert_eq!(app.doc.as_ref().unwrap().progress.as_deref(), Some("100"), "suggests what's left");
    let _ = app.update(Message::Doc(D::ProgressPct("25".into())));
    press(&mut app, "Create invoice");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let inv = match &app.route { Route::Doc { kind, id: Some(id) } if kind == "ACCREC" => *id, r => panic!("{r:?}") };
    assert_eq!(app.be.call_now("docs.get", json!({"id": inv})).unwrap()["total"], 50000);
    // Back on the estimate: 25% invoiced so far.
    let t = app.update(Message::Nav(Route::Doc { kind: "QUOTE".into(), id: Some(q) })); settle(&mut app, t);
    assert!(!click_fails(&app, "Progress invoicing"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn payment_reminder_levels() {
    use crate::extras::Msg as X;
    let (mut app, dir) = demo_app("dunning", Skin::Xero);
    let t = app.update(Message::Nav(Route::PaymentReminders)); settle(&mut app, t);
    let before = app.data.as_ref().unwrap()["rules"].as_array().unwrap().len();
    let t = app.update(Message::Extras(X::DunOn(true))); settle(&mut app, t);
    assert_eq!(app.data.as_ref().unwrap()["on"], true);
    let _ = app.update(Message::Extras(X::DunDays("30".into())));
    press(&mut app, "Add level");
    let rules = app.data.as_ref().unwrap()["rules"].as_array().unwrap().clone();
    assert_eq!(rules.len(), before + 1);
    assert!(rules.iter().any(|r| r["days"] == 30));
    // The demo has overdue invoices with email addresses: running queues reminders.
    let queued = app.be.call_now("reminders.run", json!({})).unwrap()["queued"].as_i64().unwrap();
    assert!(queued > 0, "overdue invoices get reminders");
    // Each level goes once per invoice.
    assert_eq!(app.be.call_now("reminders.run", json!({})).unwrap()["queued"], 0);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn price_rule_quantity_break_on_an_invoice() {
    use crate::extras::Msg as X;
    use crate::doc::Msg as D;
    let (mut app, dir) = desktop_app("pricerules");
    let item = app.lk.items.iter().find(|x| x["is_sold"] == 1 && x["sale_unit_e2"].as_i64().unwrap_or(0) > 0).cloned().unwrap();
    let opt = crate::app::Opt { id: item["id"].as_i64().unwrap(), label: item["name"].as_str().unwrap_or("").to_string() };
    let t = app.update(Message::Nav(Route::PriceLevels)); settle(&mut app, t);
    let _ = app.update(Message::Extras(X::PrNew(true)));
    for (n, v) in [(0, "Bulk"), (1, "10"), (2, "-20")] { let _ = app.update(Message::Extras(X::PrField(n, v.into()))); }
    let _ = app.update(Message::Extras(X::PrItem(opt.clone())));
    press(&mut app, "Save rule");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert_eq!(app.lk.price_rules.len(), 1);
    // On an invoice: 1 at the normal price, 10 at 20% off.
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: None })); settle(&mut app, t);
    let t = app.update(Message::Doc(D::Item(0, opt))); settle(&mut app, t);
    let base = crate::doc::parse_e4(&app.doc.as_ref().unwrap().lines[0].price).unwrap();
    let t = app.update(Message::Doc(D::Qty(0, "10".into()))); settle(&mut app, t);
    let bulk = crate::doc::parse_e4(&app.doc.as_ref().unwrap().lines[0].price).unwrap();
    assert_eq!(bulk, ledger_core::pricelevels::apply_level(base, -2_000));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn batch_payment_bank_file() {
    let (mut app, dir) = demo_app("batchfile", Skin::Xero);
    // Two open bills, from suppliers with bank details, paid together.
    let bills: Vec<Value> = app.be.call_now("docs.list", json!({"type": "ACCPAY", "tab": "awaiting", "limit": 50})).unwrap()["rows"].as_array().unwrap().iter().filter(|b| b["status"] == "AUTHORISED").take(2).cloned().collect();
    assert_eq!(bills.len(), 2);
    {
        let g = app.be.st.org.lock().unwrap();
        let l = &g.as_ref().unwrap().ledger;
        for b in &bills { l.conn.execute("UPDATE contacts SET bank_account = '062000 12345678' WHERE id = ?1", [b["contact_id"].as_i64().unwrap()]).unwrap(); }
        l.conn.execute("UPDATE accounts SET bank_account_number = '062000 87654321' WHERE type = 'BANK'", []).unwrap();
    }
    let bank = crate::payments::bank_only(&app)[0].id;
    let items: Vec<Value> = bills.iter().map(|b| json!({"document_id": b["id"], "amount": crate::ui::plain(b["amount_due"].as_i64().unwrap())})).collect();
    app.be.call_now("batch.create", json!({"account_id": bank, "date": "2026-10-13", "reference": "Supplier run", "items": items})).unwrap();
    let t = app.update(Message::Nav(Route::BatchPayments)); settle(&mut app, t);
    assert!(!click_fails(&app, "Supplier run"));
    // The link names the country's format (ABA / BACS / NACHA).
    let id = app.data.as_ref().unwrap().as_array().unwrap().iter().find(|b| b["reference"] == "Supplier run").unwrap()["id"].as_i64().unwrap();
    let t = app.update(Message::Extras(crate::extras::Msg::BatchBankFile(id))); settle(&mut app, t);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let file = std::fs::read_dir(crate::pdf::output_dir()).unwrap().filter_map(|e| e.ok()).map(|e| e.path())
        .find(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&format!("batch-{id}.")))).expect("bank file written");
    let text = std::fs::read_to_string(file).unwrap().to_uppercase();
    for b in &bills { let name = b["contact_name"].as_str().unwrap().to_uppercase(); assert!(text.contains(&name[..name.len().min(16)]), "{name} in the file"); }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn w2_wage_and_tax_summary() {
    let (mut app, dir) = desktop_app("w2");
    assert_eq!(app.country(), "US");
    app.be.call_now("settings.update", json!({"payroll_jurisdiction": "US"})).unwrap();
    let settings = app.be.call_now("payroll.settings", json!({})).unwrap();
    let cal = settings["calendars"][0]["id"].as_i64().unwrap();
    let id = app.be.call_now("payroll.createPayRun", json!({"calendar_id": cal})).unwrap()["id"].as_i64().unwrap();
    let bank = crate::payments::bank_only(&app)[0].id;
    app.be.call_now("payroll.post", json!({"id": id, "bank_account_id": bank})).unwrap();
    let run = app.be.call_now("payroll.payRun", json!({"id": id})).unwrap();
    let year = &run["payment_date"].as_str().unwrap()[..4];
    let r = app.be.call_now("reports.run", json!({"name": "w2-summary", "params": {"date": format!("{year}-12-31")}})).unwrap();
    let rows = r["sections"][0]["rows"].as_array().unwrap();
    assert!(!rows.is_empty());
    // Wages add up to the posted gross; social security is 6.2% of it.
    let gross: i64 = run["payslips"].as_array().unwrap().iter().map(|s| s["gross"].as_i64().unwrap()).sum();
    let total = &r["sections"][0]["total"]["values"];
    assert!(total[0].as_i64().unwrap() >= gross);
    assert!((total[2].as_i64().unwrap() - gross * 62 / 1000).abs() <= rows.len() as i64 * 2);
    // It's in the US report list in every layout.
    let t = app.update(Message::SetLayout(crate::theme::Layout::Online)); settle(&mut app, t);
    let t = app.update(Message::Nav(Route::Reports)); settle(&mut app, t);
    let _ = app.update(Message::ListQuery("w-2".into()));
    assert!(!click_fails(&app, "W-2 Wage and Tax Summary"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn acumatica_record_tabs() {
    let (mut app, dir) = desktop_app("acu-tabs");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Acumatica)); settle(&mut app, t);
    let inv = app.be.call_now("docs.list", json!({"type": "ACCREC", "limit": 5})).unwrap()["rows"][0]["id"].as_i64().unwrap();
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCREC".into(), id: Some(inv) })); settle(&mut app, t);
    press(&mut app, "Financial");
    assert!(!click_fails(&app, "Post period") && !click_fails(&app, "Exchange rate"));
    press(&mut app, "Address");
    assert!(!click_fails(&app, "Bill-to address"));
    press(&mut app, "Details");
    assert_eq!(app.doc.as_ref().unwrap().acu_tab, "details");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sage_bank_feed_table() {
    let (mut app, dir) = demo_app("sage-bank", Skin::Xero);
    let t = app.update(Message::SetLayout(crate::theme::Layout::Sage)); settle(&mut app, t);
    let t = app.update(Message::Nav(Route::Banking(None))); settle(&mut app, t);
    for label in ["Bank Feed Transactions", "Matched", "Ignored", "LEDGER ACCOUNT / MATCH", "Other payment"] { assert!(!click_fails(&app, label), "{label}"); }
    // Rule suggestions are created with one click.
    let before = app.data.as_ref().unwrap()["lines"].as_array().unwrap().len();
    let msgs = click(&app, "Create");
    assert!(!msgs.is_empty());
    for m in msgs { let t = app.update(m); settle(&mut app, t); }
    let after = app.data.as_ref().unwrap()["lines"].as_array().unwrap().len();
    assert!(after <= before);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn inventory_locations_in_the_app() {
    use crate::extras::Msg as X;
    use crate::doc::Msg as D;
    let (mut app, dir) = desktop_app("locations");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Online)); settle(&mut app, t);
    let t = app.update(Message::Nav(Route::Locations)); settle(&mut app, t);
    let _ = app.update(Message::Extras(X::LocNew(true)));
    let _ = app.update(Message::Extras(X::LocField(0, "Back store".into())));
    press(&mut app, "Save location");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let loc = app.lk.location_opts().into_iter().find(|o| o.label == "Back store").expect("in lookups");
    // A bill puts stock into the new location.
    let item = app.lk.items.iter().find(|i| i["is_tracked"] == 1).cloned().expect("a tracked item");
    let opt = crate::app::Opt { id: item["id"].as_i64().unwrap(), label: item["name"].as_str().unwrap_or("").to_string() };
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCPAY".into(), id: None })); settle(&mut app, t);
    assert!(!click_fails(&app, "Location"));
    let vendor = app.lk.contact_opts(false)[0].clone();
    for m in [D::Contact(vendor), D::Location(loc.clone()), D::Item(0, opt.clone()), D::Qty(0, "12".into()), D::Price(0, "3".into())] { let t = app.update(Message::Doc(m)); settle(&mut app, t); }
    press(&mut app, "Save and close");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let item_id = item["id"].clone();
    let stock = |app: &App| app.be.call_now("locations.stock", json!({})).unwrap()["rows"].as_array().unwrap().iter()
        .filter(|r| r["id"] == item_id && r["location_id"].as_i64() == Some(loc.id)).map(|r| r["qty_e4"].as_i64().unwrap()).sum::<i64>();
    assert_eq!(stock(&app), 120_000);
    // Move 5 back to the main location.
    let t = app.update(Message::Nav(Route::Locations)); settle(&mut app, t);
    for m in [X::TrItem(opt), X::TrFrom(loc.clone()), X::TrTo(crate::app::Opt { id: 0, label: "Main location".into() }), X::TrQty("5".into())] { let _ = app.update(Message::Extras(m)); }
    press(&mut app, "Move");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert_eq!(stock(&app), 70_000);
    assert!(!click_fails(&app, "Back store"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn choose_fifo_costing_in_settings() {
    let (mut app, dir) = desktop_app("fifo");
    // Desktop: Preferences ▸ Items & Inventory.
    let t = app.update(Message::Nav(Route::Settings)); settle(&mut app, t);
    let _ = app.update(Message::Settings(crate::settings::Msg::Cat("Items & Inventory")));
    press(&mut app, "First in, first out (FIFO)");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert_eq!(app.setting("inventory_costing"), "FIFO");
    // QuickBooks Online: Account and settings ▸ Advanced.
    let t = app.update(Message::SetLayout(crate::theme::Layout::Online)); settle(&mut app, t);
    let t = app.update(Message::Nav(Route::Settings)); settle(&mut app, t);
    press(&mut app, "Advanced");
    press(&mut app, "Average cost");
    assert_eq!(app.setting("inventory_costing"), "AVERAGE");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn serial_numbers_in_the_app() {
    use crate::extras::Msg as X;
    use crate::doc::Msg as D;
    let (mut app, dir) = desktop_app("serials");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Xero)); settle(&mut app, t);
    let item = app.lk.items.iter().find(|i| i["is_tracked"] == 1).cloned().expect("a tracked item");
    let opt = crate::app::Opt { id: item["id"].as_i64().unwrap(), label: item["name"].as_str().unwrap_or("").to_string() };
    let t = app.update(Message::Nav(Route::Lots)); settle(&mut app, t);
    let _ = app.update(Message::Extras(X::LotItem(opt.clone())));
    press(&mut app, "By serial number");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert_eq!(app.lk.item(opt.id).unwrap()["lot_tracking"], "SERIAL");
    // A bill for 2 asks for 2 serial numbers.
    let t = app.update(Message::Nav(Route::Doc { kind: "ACCPAY".into(), id: None })); settle(&mut app, t);
    let vendor = app.lk.contact_opts(false)[0].clone();
    for m in [D::Contact(vendor), D::Item(0, opt.clone()), D::Qty(0, "2".into()), D::Price(0, "10".into())] { let t = app.update(Message::Doc(m)); settle(&mut app, t); }
    assert!(!click_fails(&app, "LOT AND SERIAL NUMBERS"));
    press(&mut app, "Approve");
    assert!(app.doc.as_ref().unwrap().error.as_deref().is_some_and(|e| e.contains("serial")), "{:?}", app.doc.as_ref().unwrap().error);
    let _ = app.update(Message::Doc(D::Lot(0, "A100, A101".into())));
    press(&mut app, "Approve");
    assert!(app.doc.as_ref().unwrap().error.is_none(), "{:?}", app.doc.as_ref().unwrap().error);
    let t = app.update(Message::Nav(Route::Lots)); settle(&mut app, t);
    assert!(!click_fails(&app, "A100") && !click_fails(&app, "A101"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn revenue_recognition_in_the_app() {
    use crate::extras::Msg as X;
    let (mut app, dir) = desktop_app("revrec");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Acumatica)); settle(&mut app, t);
    let t = app.update(Message::Nav(Route::RevenueRecognition)); settle(&mut app, t);
    assert!(!click_fails(&app, "Deferral Schedules"));
    let inv = app.data.as_ref().unwrap()["invoices"][0].clone();
    let opt = crate::app::Opt { id: inv["id"].as_i64().unwrap(), label: "x".into() };
    let start = app.d(inv["date"].as_str().unwrap());
    for m in [X::RrInvoice(opt), X::RrMonths("4".into()), X::RrStart(start)] { let _ = app.update(Message::Extras(m)); }
    press(&mut app, "Create schedule");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let s = app.data.as_ref().unwrap()["schedules"][0].clone();
    assert_eq!(s["months"], 4);
    // Recognise everything due by far in the future: all four months.
    let _ = app.update(Message::Extras(X::RrTo(app.d("2099-12-31"))));
    press(&mut app, "Recognise");
    let s = app.data.as_ref().unwrap()["schedules"][0].clone();
    assert_eq!(s["recognised"], s["total"]);
    let tb = app.be.call_now("reports.run", json!({"name": "trial-balance", "params": {"date": "2099-12-31"}})).unwrap();
    assert_eq!(tb["balanced"], true);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn custom_fields_on_contacts_and_items() {
    use crate::extras::Msg as X;
    let (mut app, dir) = desktop_app("customfields");
    let t = app.update(Message::Nav(Route::CustomFields)); settle(&mut app, t);
    for (entity, name, kind) in [("contact", "Region", "TEXT"), ("item", "Warranty months", "NUMBER")] {
        let _ = app.update(Message::Extras(X::CfNew(Some(entity))));
        let _ = app.update(Message::Extras(X::CfName(name.into())));
        let _ = app.update(Message::Extras(X::CfKind(kind)));
        press(&mut app, "Save field");
        assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    }
    assert_eq!(app.data.as_ref().unwrap().as_array().unwrap().len(), 2);
    // Fill in Region on a customer.
    let cust = app.lk.contact_opts(true)[0].id;
    let t = app.update(Message::Nav(Route::EditContact(cust))); settle(&mut app, t);
    let region = app.be.call_now("customFields.list", json!({"entity": "contact"})).unwrap()[0]["id"].as_i64().unwrap();
    let _ = app.update(Message::Extras(X::CfValue(region, "North".into())));
    press(&mut app, "Save custom fields");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert_eq!(app.be.call_now("contacts.get", json!({"id": cust})).unwrap()["custom_fields"][0]["value"], "North");
    // A number field refuses text.
    let item = app.lk.items[0]["id"].as_i64().unwrap();
    let t = app.update(Message::Nav(Route::EditItem(item))); settle(&mut app, t);
    let warranty = app.be.call_now("customFields.list", json!({"entity": "item"})).unwrap()[0]["id"].as_i64().unwrap();
    let _ = app.update(Message::Extras(X::CfValue(warranty, "lots".into())));
    press(&mut app, "Save custom fields");
    assert!(app.toasts.iter().any(|t| t.2 && t.1.contains("number")), "{:?}", app.toasts);
    let _ = app.update(Message::Extras(X::CfValue(warranty, "24".into())));
    press(&mut app, "Save custom fields");
    assert_eq!(app.be.call_now("items.get", json!({"id": item})).unwrap()["custom_fields"][0]["value"], "24");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn landed_cost_in_the_app() {
    use crate::extras::Msg as X;
    let (mut app, dir) = desktop_app("landed");
    // A bill that brings stock in.
    let item = app.lk.items.iter().find(|i| i["is_tracked"] == 1).cloned().expect("a tracked item");
    let vendor = app.lk.contact_opts(false)[0].id;
    let stock = app.be.call_now("docs.save", json!({"doc": {"type": "ACCPAY", "contact_id": vendor, "date": "2026-10-01", "line_amount_type": "NOTAX", "status": "AUTHORISED",
        "lines": [{"item_id": item["id"], "quantity": "10", "unit_price": "4"}]}})).unwrap()["id"].as_i64().unwrap();
    let t = app.update(Message::Nav(Route::LandedCosts)); settle(&mut app, t);
    let d = app.data.as_ref().unwrap().clone();
    assert!(d["stock_bills"].as_array().unwrap().iter().any(|v| v.as_i64() == Some(stock)));
    let cost_bill = d["bills"].as_array().unwrap().iter().find(|b| !d["stock_bills"].as_array().unwrap().contains(&b["id"])).unwrap()["id"].as_i64().unwrap();
    let before = app.be.call_now("items.get", json!({"id": item["id"]})).unwrap()["total_cost"].as_i64().unwrap();
    let _ = app.update(Message::Extras(X::LcBill(crate::app::Opt { id: cost_bill, label: "x".into() })));
    let _ = app.update(Message::Extras(X::LcReceipt(stock)));
    press(&mut app, "Add to stock cost");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let after = app.be.call_now("items.get", json!({"id": item["id"]})).unwrap()["total_cost"].as_i64().unwrap();
    assert!(after > before, "{before} -> {after}");
    assert_eq!(app.data.as_ref().unwrap()["applied"].as_array().unwrap().len(), 1);
    let tb = app.be.call_now("reports.run", json!({"name": "trial-balance", "params": {"date": "2099-12-31"}})).unwrap();
    assert_eq!(tb["balanced"], true);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn consolidated_reports_in_the_app() {
    use crate::extras::Msg as X;
    let (mut app, dir) = desktop_app("consol");
    let first = app.be.call_now("orgs.list", json!({})).unwrap()[0]["file"].as_str().unwrap().to_string();
    // A second company (creating it opens it), then back to the first.
    let second = app.be.call_now("orgs.create", json!({"org_name": "Sister Co", "country": "US", "demo": true, "owner_name": "Alex"})).unwrap()["file"].as_str().unwrap().to_string();
    app.be.call_now("orgs.open", json!({"file": first})).unwrap();
    let t = app.update(Message::Nav(Route::Consolidation)); settle(&mut app, t);
    let _ = app.update(Message::Extras(X::CoFile(first.clone())));
    let _ = app.update(Message::Extras(X::CoFile(second.clone())));
    let _ = app.update(Message::Extras(X::CoFrom(app.d("2025-01-01"))));
    let t = app.update(Message::Extras(X::CoRun)); settle(&mut app, t);
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    let r = app.extras.co_result.clone().expect("combined");
    assert_eq!(r["columns"].as_array().unwrap().len(), 4);
    assert!(!click_fails(&app, "CONSOLIDATED") && !click_fails(&app, "Net profit"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn item_receipts_in_the_app() {
    use crate::extras::Msg as X;
    let (mut app, dir) = desktop_app("itemreceipts");
    let t = app.update(Message::SetLayout(crate::theme::Layout::Online)); settle(&mut app, t);
    let t = app.update(Message::ReceiveItems); settle(&mut app, t);
    assert_eq!(app.route, Route::ItemReceipts);
    let item = app.lk.items.iter().find(|i| i["is_tracked"] == 1 && i["lot_tracking"] == "NONE").cloned().expect("a tracked item");
    let before = item["qty_on_hand_e4"].as_i64().unwrap();
    let vendor = app.lk.contact_opts(false)[0].clone();
    for m in [X::IrVendor(vendor), X::IrItem(0, crate::app::Opt { id: item["id"].as_i64().unwrap(), label: "x".into() }), X::IrQty(0, "6".into()), X::IrCost(0, "3".into())] { let _ = app.update(Message::Extras(m)); }
    press(&mut app, "Receive");
    assert!(app.toasts.iter().all(|t| !t.2), "{:?}", app.toasts);
    assert_eq!(app.be.call_now("items.get", json!({"id": item["id"]})).unwrap()["qty_on_hand_e4"].as_i64().unwrap(), before + 60_000);
    // Enter the bill: it opens as a draft against the receipt.
    let rid = app.data.as_ref().unwrap()[0]["id"].as_i64().unwrap();
    let t = app.update(Message::Extras(X::IrBill(rid))); settle(&mut app, t);
    let bill = match &app.route { Route::Doc { kind, id: Some(id) } if kind == "ACCPAY" => *id, r => panic!("{r:?}") };
    app.be.call_now("docs.setStatus", json!({"id": bill, "status": "AUTHORISED"})).unwrap();
    assert_eq!(app.be.call_now("items.get", json!({"id": item["id"]})).unwrap()["qty_on_hand_e4"].as_i64().unwrap(), before + 60_000, "no double stock");
    let tb = app.be.call_now("reports.run", json!({"name": "trial-balance", "params": {"date": "2099-12-31"}})).unwrap();
    assert_eq!(tb["balanced"], true);
    let t = app.update(Message::Nav(Route::ItemReceipts)); settle(&mut app, t);
    assert_eq!(app.data.as_ref().unwrap()[0]["state"], "Billed");
    let _ = std::fs::remove_dir_all(dir);
}

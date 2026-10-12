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
    let lk = Lookups { accounts: get("accounts.list", json!({})), tax_rates: get("taxRates.list", json!({})), items: get("items.list", json!({})), contacts: get("contacts.list", json!({"filter": "all"})) };
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
    let lk = Lookups { accounts: get("accounts.list", json!({})), tax_rates: get("taxRates.list", json!({})), items: get("items.list", json!({})), contacts: get("contacts.list", json!({"filter": "all"})) };
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

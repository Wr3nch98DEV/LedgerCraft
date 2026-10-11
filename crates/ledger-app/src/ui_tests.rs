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

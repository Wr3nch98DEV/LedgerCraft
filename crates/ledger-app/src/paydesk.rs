//! QuickBooks Desktop-style money forms: Write Checks (a check face with the amount in words),
//! Enter Credit Card Charges, Receive Payments, Pay Bills and Make Deposits. Same state, messages
//! and engine calls as the other layouts (see `payments.rs`).

use crate::app::{App, Message, Opt};
use crate::payments::{Mode, Msg, State};
use crate::theme::{self, t};
use crate::ui::{self, BOLD};
use iced::widget::{Space, button, checkbox, column, combo_box, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Element, Fill, Length, Padding};

fn label<'a>(s: &str) -> Element<'a, Message> { text(s.to_uppercase()).size(11).font(BOLD).color(t().info).into() }

fn input<'a>(val: &str, w: f32, f: fn(String) -> Msg) -> Element<'a, Message> {
    text_input("", val).on_input(move |s| Message::Pay(f(s))).padding([5, 7]).size(13).style(theme::input).width(Length::Fixed(w)).into()
}

/// "One thousand two hundred thirty-four and 56/100" — the written amount on a check.
pub fn words(cents: i64) -> String {
    const ONES: [&str; 20] = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen"];
    const TENS: [&str; 10] = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];
    fn under_1000(n: u64) -> String {
        let mut parts = vec![];
        if n >= 100 { parts.push(format!("{} hundred", ONES[(n / 100) as usize])); }
        let r = n % 100;
        if r >= 20 { parts.push(if r % 10 == 0 { TENS[(r / 10) as usize].to_string() } else { format!("{}-{}", TENS[(r / 10) as usize], ONES[(r % 10) as usize]) }); }
        else if r > 0 || n == 0 { parts.push(ONES[r as usize].to_string()); }
        parts.join(" ")
    }
    let c = cents.unsigned_abs();
    let mut dollars = c / 100;
    let mut groups = vec![];
    for scale in ["", " thousand", " million", " billion"] {
        let g = dollars % 1000;
        if g > 0 { groups.push(format!("{}{scale}", under_1000(g))); }
        dollars /= 1000;
        if dollars == 0 { break; }
    }
    let w = if groups.is_empty() { "zero".to_string() } else { groups.into_iter().rev().collect::<Vec<_>>().join(" ") };
    let mut w = format!("{w} and {:02}/100", c % 100);
    if let Some(f) = w.get(..1).map(|s| s.to_uppercase()) { w.replace_range(..1, &f); }
    w
}

fn buttons<'a>(s: &State, save: &'static str) -> Element<'a, Message> {
    let busy = s.busy;
    let mut r = row![ui::hspace()].spacing(8);
    if matches!(s.mode, Mode::Spend | Mode::Receive) { r = r.push(ui::btn("Save & New", if busy { None } else { Some(Message::Pay(Msg::SaveNew)) })); }
    r.push(ui::btn_primary(save, if busy { None } else { Some(Message::Pay(Msg::Save)) }))
        .push(ui::btn(if matches!(s.mode, Mode::PayBills) { "Cancel" } else { "Clear" }, Some(if matches!(s.mode, Mode::PayBills) { Message::Back } else { Message::Refresh })))
        .into()
}

fn grid_head<'a>(cols: &[(&str, u16, bool)]) -> Element<'a, Message> {
    let k = t();
    container(row(cols.iter().map(|(h, w, r)| {
        let tx = text(h.to_uppercase()).size(11).font(BOLD).color(k.muted);
        container(if *r { tx.align_x(iced::alignment::Horizontal::Right).width(Fill) } else { tx.width(Fill) }).width(Length::FillPortion(*w)).into()
    })).spacing(6).padding([6, 8])).style(theme::header_row).width(Fill).into()
}

fn grid_row<'a>(cells: Vec<Element<'a, Message>>, cols: &[(&str, u16, bool)]) -> Element<'a, Message> {
    let r = row(cells.into_iter().zip(cols.iter()).map(|(c, (_, w, _))| container(c).width(Length::FillPortion(*w)).into())).spacing(6).align_y(Alignment::Center).padding([2, 8]);
    column![r, ui::hr()].into()
}

/// Bank account balance from the chart of accounts.
fn balance(app: &App, acc: &Option<Opt>) -> i64 {
    acc.as_ref().and_then(|a| app.lk.accounts.iter().find(|x| x["id"].as_i64() == Some(a.id))).and_then(|a| a["ytd_balance"].as_i64()).unwrap_or(0)
}

pub fn view<'a>(app: &'a App, s: &'a State) -> Element<'a, Message> {
    let body: Element<Message> = match s.mode {
        Mode::Spend => spend(app, s),
        Mode::Receive => receive(app, s),
        Mode::PayBills => pay_bills(app, s),
        Mode::Deposit => deposit(app, s),
    };
    scrollable(container(body).padding(Padding { top: 12.0, left: 16.0, right: 16.0, bottom: 16.0 }).max_width(1150)).style(theme::scroll).height(Fill).into()
}

/// Write Checks / Enter Credit Card Charges.
fn spend<'a>(app: &'a App, s: &'a State) -> Element<'a, Message> {
    let k = t();
    let total: i64 = s.lines.iter().map(|l| ui::parse_money(&l.amount).unwrap_or(0)).sum();
    let top = row![
        column![label(if s.check { "Bank Account" } else { "Credit Card" }), pick_list(app.lk.bank_opts(), s.account.clone(), |a| Message::Pay(Msg::Account(a))).padding([5, 7]).text_size(13).style(theme::pick).width(280)].spacing(3),
        Space::new().width(20),
        column![label("Ending Balance"), container(text(ui::money(balance(app, &s.account))).size(14).font(BOLD)).padding([5, 0])].spacing(3),
    ].align_y(Alignment::End);
    let payee = combo_box(&s.contacts, "", s.contact.as_ref(), |c| Message::Pay(Msg::Contact(c))).on_input(|v| Message::Pay(Msg::ContactText(v))).padding([5, 7]).size(13).input_style(theme::input);
    let face: Element<Message> = if s.check {
        // The check itself.
        let paper = move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(if k.dark { iced::Color::from_rgb8(0x24, 0x2c, 0x2a) } else { iced::Color::from_rgb8(0xee, 0xf6, 0xee) })),
            border: iced::Border { color: k.line2, width: 1.0, radius: 4.0.into() }, ..Default::default()
        };
        container(column![
            row![ui::hspace(), label("No."), input(&s.check_no, 90.0, Msg::CheckNo)].spacing(8).align_y(Alignment::Center),
            row![ui::hspace(), label("Date"), input(&s.date, 120.0, Msg::Date)].spacing(8).align_y(Alignment::Center),
            row![column![label("Pay to the"), label("order of")].spacing(0), container(payee).width(440), ui::hspace(),
                text("$").size(16).font(BOLD), container(text(ui::money(total)).size(16).font(BOLD)).padding([4, 10]).width(150).style(theme::soft)].spacing(10).align_y(Alignment::Center),
            row![container(text(format!("{}{}", words(total), "*".repeat(60usize.saturating_sub(words(total).len())))).size(13)).width(Fill), label("Dollars")].spacing(8).align_y(Alignment::Center),
            container(ui::hr()).width(Fill),
            row![column![label("Address"), container(text(s.contact.as_ref().map(|c| c.label.clone()).unwrap_or_default()).size(13)).padding([6, 8]).width(300).height(60).style(theme::soft)].spacing(3), ui::hspace()],
            row![label("Memo"), input(&s.reference, 360.0, Msg::Reference)].spacing(10).align_y(Alignment::Center),
        ].spacing(10)).padding([14, 18]).width(Fill).style(paper).into()
    } else {
        container(column![
            row![column![label("Purchased From"), container(payee).width(360)].spacing(3), Space::new().width(20),
                column![label("Date"), input(&s.date, 120.0, Msg::Date)].spacing(3), column![label("Ref No."), input(&s.reference, 120.0, Msg::Reference)].spacing(3),
                ui::hspace(), column![label("Amount"), text(ui::money(total)).size(18).font(BOLD)].spacing(3)].spacing(10).align_y(Alignment::End),
        ].spacing(10)).padding([14, 18]).width(Fill).style(theme::card).into()
    };
    let accounts = app.lk.account_opts(|a| a["type"] != "BANK");
    let taxes = app.lk.tax_opts();
    let cols: Vec<(&str, u16, bool)> = vec![("Account", 24, false), ("Amount", 11, true), ("Memo", 30, false), ("Tax", 14, false), ("", 3, false)];
    let mut grid = column![grid_head(&cols)];
    for (n, l) in s.lines.iter().enumerate() {
        grid = grid.push(grid_row(vec![
            pick_list(accounts.clone(), l.account.clone(), move |a| Message::Pay(Msg::LineAccount(n, a))).padding([4, 5]).text_size(13).style(theme::pick).width(Fill).into(),
            text_input("", &l.amount).on_input(move |v| Message::Pay(Msg::LineAmount(n, v))).padding([4, 5]).size(13).align_x(iced::alignment::Horizontal::Right).style(theme::cell_input).into(),
            text_input("", &l.description).on_input(move |v| Message::Pay(Msg::LineDesc(n, v))).padding([4, 5]).size(13).style(theme::cell_input).into(),
            pick_list(taxes.clone(), l.tax.clone(), move |x| Message::Pay(Msg::LineTax(n, x))).padding([4, 5]).text_size(13).style(theme::pick).width(Fill).into(),
            button(crate::icons::icon("trash", 13.0, k.faint)).style(theme::ghost).padding(3).on_press(Message::Pay(Msg::RemoveLine(n))).into(),
        ], &cols));
    }
    column![
        top,
        face,
        row![text(format!("Expenses   {}", ui::money(total))).size(13).font(BOLD), text(format!("Items   {}", ui::money(0))).size(13).color(k.muted)].spacing(24),
        container(scrollable(grid).style(theme::scroll)).height(Length::Fixed(190.0)).style(theme::card).width(Fill).clip(true),
        row![ui::btn("Add lines", Some(Message::Pay(Msg::AddLine)))],
        buttons(s, "Save & Close"),
    ].spacing(12).into()
}

/// Receive Payments ("Customer Payment").
fn receive<'a>(app: &'a App, s: &'a State) -> Element<'a, Message> {
    let k = t();
    let applied: i64 = s.docs.iter().filter(|d| d.pick).map(|d| ui::parse_money(&d.amount).unwrap_or(0)).sum();
    let due: i64 = s.docs.iter().filter(|d| d.pick).map(|d| d.due).sum();
    let cust_bal: i64 = s.docs.iter().map(|d| d.due).sum();
    let head = column![
        text("Customer Payment").size(28).font(BOLD),
        row![
            column![label("Received From"), container(combo_box(&s.contacts, "", s.contact.as_ref(), |c| Message::Pay(Msg::Contact(c))).padding([5, 7]).size(13).input_style(theme::input)).width(320)].spacing(3),
            column![label("Customer Balance"), container(text(ui::money(cust_bal)).size(14)).padding([5, 0])].spacing(3),
        ].spacing(30).align_y(Alignment::End),
        row![
            column![label("Payment Amount"), container(text(ui::money(applied)).size(16).font(BOLD)).padding([4, 7]).width(160).style(theme::soft)].spacing(3),
            column![label("Date"), input(&s.date, 120.0, Msg::Date)].spacing(3),
            column![label("Reference #"), input(&s.reference, 140.0, Msg::Reference)].spacing(3),
            column![label("Deposit To"), pick_list(app.lk.bank_opts(), s.account.clone(), |a| Message::Pay(Msg::Account(a))).padding([5, 7]).text_size(13).style(theme::pick).width(240)].spacing(3),
        ].spacing(16).align_y(Alignment::End),
    ].spacing(12);
    let cols: Vec<(&str, u16, bool)> = vec![("✓", 3, false), ("Date", 12, false), ("Number", 12, false), ("Orig. Amt.", 13, true), ("Amt. Due", 13, true), ("Payment", 14, true)];
    let mut grid = column![grid_head(&cols)];
    if s.docs.is_empty() { grid = grid.push(container(text(if s.contact.is_none() { "Choose who the payment is from." } else { "No open invoices." }).size(13).color(k.muted)).padding(18)); }
    for (n, d) in s.docs.iter().enumerate() {
        grid = grid.push(grid_row(vec![
            checkbox(d.pick).on_toggle(move |v| Message::Pay(Msg::Pick(n, v))).into(), ui::cell(app.d(&d.date)), ui::cell(d.number.clone()), ui::cell_money(d.total), ui::cell_money(d.due),
            text_input("", &d.amount).on_input(move |a| Message::Pay(Msg::Amount(n, a))).padding([4, 5]).size(13).align_x(iced::alignment::Horizontal::Right).style(theme::cell_input).into(),
        ], &cols));
    }
    let summary = column![
        label("Amounts for Selected Invoices"),
        row![text("Amount Due").size(13).width(140), text(ui::money(due)).size(13)],
        row![text("Applied").size(13).width(140), text(ui::money(applied)).size(13)],
    ].spacing(4);
    column![
        head,
        container(scrollable(grid).style(theme::scroll)).height(Length::Fixed(240.0)).style(theme::card).width(Fill).clip(true),
        row![ui::hspace(), summary],
        buttons(s, "Save & Close"),
    ].spacing(12).into()
}

/// Pay Bills.
fn pay_bills<'a>(app: &'a App, s: &'a State) -> Element<'a, Message> {
    let k = t();
    let total: i64 = s.docs.iter().filter(|d| d.pick).map(|d| ui::parse_money(&d.amount).unwrap_or(0)).sum();
    let cols: Vec<(&str, u16, bool)> = vec![("✓", 3, false), ("Date Due", 12, false), ("Vendor", 24, false), ("Ref. No.", 12, false), ("Amt. Due", 13, true), ("Amt. To Pay", 14, true)];
    let mut grid = column![grid_head(&cols)];
    if s.docs.is_empty() { grid = grid.push(container(text("No unpaid bills.").size(13).color(k.muted)).padding(18)); }
    for (n, d) in s.docs.iter().enumerate() {
        grid = grid.push(grid_row(vec![
            checkbox(d.pick).on_toggle(move |v| Message::Pay(Msg::Pick(n, v))).into(), ui::cell(app.d(&d.due_date)), ui::cell(d.contact.clone()), ui::cell_muted(d.number.clone()), ui::cell_money(d.due),
            text_input("", &d.amount).on_input(move |a| Message::Pay(Msg::Amount(n, a))).padding([4, 5]).size(13).align_x(iced::alignment::Horizontal::Right).style(theme::cell_input).into(),
        ], &cols));
    }
    column![
        text("Select Bills to Be Paid").size(16).font(BOLD),
        container(scrollable(grid).style(theme::scroll)).height(Length::Fixed(300.0)).style(theme::card).width(Fill).clip(true),
        row![ui::hspace(), text("Totals").size(13).font(BOLD), text(ui::money(total)).size(14).font(BOLD)].spacing(16),
        container(column![
            text("Payment").size(14).font(BOLD),
            row![
                column![label("Date"), input(&s.date, 120.0, Msg::Date)].spacing(3),
                column![label("Method"), container(text("Check").size(13)).padding([5, 7]).width(120).style(theme::soft)].spacing(3),
                column![label("Account"), pick_list(app.lk.bank_opts(), s.account.clone(), |a| Message::Pay(Msg::Account(a))).padding([5, 7]).text_size(13).style(theme::pick).width(260)].spacing(3),
                column![label("Ending Balance"), container(text(ui::money(balance(app, &s.account))).size(13)).padding([5, 0])].spacing(3),
            ].spacing(16).align_y(Alignment::End),
        ].spacing(8)).padding(12).width(Fill).style(theme::card),
        buttons(s, "Pay Selected Bills"),
    ].spacing(12).into()
}

/// Make Deposits.
fn deposit<'a>(app: &'a App, s: &'a State) -> Element<'a, Message> {
    let k = t();
    let total: i64 = s.docs.iter().filter(|d| d.pick).map(|d| d.due).sum();
    let banks = crate::payments::bank_only(app);
    let cols: Vec<(&str, u16, bool)> = vec![("✓", 3, false), ("Received From", 24, false), ("Date", 12, false), ("Chk No.", 12, false), ("Pmt Meth.", 12, false), ("Amount", 13, true)];
    let mut grid = column![grid_head(&cols)];
    if s.docs.is_empty() { grid = grid.push(container(text("There are no payments waiting in Undeposited Funds.").size(13).color(k.muted)).padding(18)); }
    for (n, d) in s.docs.iter().enumerate() {
        grid = grid.push(grid_row(vec![checkbox(d.pick).on_toggle(move |v| Message::Pay(Msg::Pick(n, v))).into(), ui::cell(d.contact.clone()), ui::cell(app.d(&d.date)), ui::cell_muted(d.number.clone()), ui::cell_muted("Check"), ui::cell_money(d.due)], &cols));
    }
    column![
        row![
            column![label("Deposit To"), pick_list(banks, s.account.clone(), |a| Message::Pay(Msg::Account(a))).padding([5, 7]).text_size(13).style(theme::pick).width(280)].spacing(3),
            column![label("Date"), input(&s.date, 120.0, Msg::Date)].spacing(3),
            column![label("Memo"), input(&s.reference, 260.0, Msg::Reference)].spacing(3),
        ].spacing(16).align_y(Alignment::End),
        text("Click Payments to select customer payments that you have received. List any other amounts to deposit below.").size(13).color(k.muted),
        container(scrollable(grid).style(theme::scroll)).height(Length::Fixed(260.0)).style(theme::card).width(Fill).clip(true),
        row![ui::hspace(), text("Deposit Total").size(13).font(BOLD), text(ui::money(total)).size(15).font(BOLD)].spacing(16),
        buttons(s, "Save & Close"),
    ].spacing(12).into()
}

#[cfg(test)]
mod tests {
    #[test]
    fn check_amount_in_words() {
        assert_eq!(super::words(123456), "One thousand two hundred thirty-four and 56/100");
        assert_eq!(super::words(100), "One and 00/100");
        assert_eq!(super::words(5), "Zero and 05/100");
        assert_eq!(super::words(200000000), "Two million and 00/100");
        assert_eq!(super::words(9_001_015_00), "Nine million one thousand fifteen and 00/100");
    }
}

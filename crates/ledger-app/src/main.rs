//! LedgerCraft desktop app: native window (iced), QuickBooks-style and Xero-style layouts
//! over the same accounting engine and company files.
#![windows_subsystem = "windows"]

mod app;
mod backend;
mod banking;
mod dashboard;
mod doc;
mod forms;
mod icons;
mod lists;
mod payments;
mod reports;
mod settings;
mod setup;
mod shell;
mod theme;
mod ui;
#[cfg(test)]
mod ui_tests;
mod vocab;

use app::App;

fn main() -> iced::Result {
    // `--data-dir PATH` uses another data folder (for testing); default is %APPDATA%\LedgerCraft.
    let args: Vec<String> = std::env::args().collect();
    let data_dir = args.iter().position(|a| a == "--data-dir").and_then(|i| args.get(i + 1)).map(std::path::PathBuf::from).unwrap_or_else(backend::default_data_dir);
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let start = arg("--route").and_then(|r| app::parse_route(&r));
    let skin = arg("--skin").map(|s| theme::Skin::from_key(&s));
    iced::application(move || App::boot(data_dir.clone(), start.clone(), skin), App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .window_size(iced::Size::new(1360.0, 860.0))
        .antialiasing(true)
        .run()
}

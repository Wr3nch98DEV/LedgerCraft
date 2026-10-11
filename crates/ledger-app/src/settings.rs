use crate::app::{App, Message};
use iced::Element;
pub fn view(_app: &App) -> Element<'_, Message> { crate::ui::muted("Settings") }

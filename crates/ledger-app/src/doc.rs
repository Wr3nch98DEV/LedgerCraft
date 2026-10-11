use crate::app::{App, Message};
use iced::{Element, Task};
#[derive(Debug, Clone)] pub enum Msg { Save(bool) }
pub struct Editor;
pub fn open(_app: &mut App, _kind: String, _id: Option<i64>) -> Task<Message> { Task::none() }
pub fn update(_app: &mut App, _m: Msg) -> Task<Message> { Task::none() }
pub fn view(_app: &App) -> Element<'_, Message> { crate::ui::muted("Editor") }

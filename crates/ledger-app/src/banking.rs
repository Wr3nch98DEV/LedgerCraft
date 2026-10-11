use crate::app::{App, Message};
use iced::{Element, Task};
#[derive(Debug, Clone)] pub enum Msg {}
#[derive(Default)] pub struct State;
pub fn load(_app: &mut App, _a: Option<i64>) -> Task<Message> { Task::none() }
pub fn update(_app: &mut App, _m: Msg) -> Task<Message> { Task::none() }
pub fn view(_app: &App) -> Element<'_, Message> { crate::ui::muted("Banking") }

use crate::app::{App, Message};
use iced::{Element, Task};
#[derive(Debug, Clone)] pub enum Msg {}
#[derive(Default)] pub struct State;
pub fn load(_app: &mut App, _k: &str) -> Task<Message> { Task::none() }
pub fn update(_app: &mut App, _m: Msg) -> Task<Message> { Task::none() }
pub fn centre(_app: &App) -> Element<'_, Message> { crate::ui::muted("Reports") }
pub fn view(_app: &App) -> Element<'_, Message> { crate::ui::muted("Report") }

use crate::app::{App, Message};
use iced::{Element, Task};
#[derive(Debug, Clone)] pub enum Msg {}
pub struct State;
pub fn open_receive(_app: &mut App, _c: Option<i64>) -> Task<Message> { Task::none() }
pub fn open_pay_bills(_app: &mut App) -> Task<Message> { Task::none() }
pub fn open_spend(_app: &mut App, _a: Option<i64>) -> Task<Message> { Task::none() }
pub fn update(_app: &mut App, _m: Msg) -> Task<Message> { Task::none() }
pub fn view(_app: &App) -> Element<'_, Message> { crate::ui::muted("Payments") }

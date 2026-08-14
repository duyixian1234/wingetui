//! TUI 渲染：按 `AppState` 分发到各屏渲染函数。

pub mod install;
pub mod log;
pub mod main_menu;
pub mod search;
pub mod uninstall;
pub mod upgrade;

use ratatui::Frame;

use crate::app::App;
use crate::state::AppState;

/// 渲染当前状态对应的屏。
pub fn draw(frame: &mut Frame, app: &App) {
    match &app.state {
        AppState::MainMenu => main_menu::draw(frame),
        AppState::Search(s) => search::draw(frame, s),
        AppState::Upgrade(s) => upgrade::draw(frame, s),
        AppState::Install(s) => install::draw(frame, s),
        AppState::Uninstall(s) => uninstall::draw(frame, s),
        AppState::Log(s) => log::draw(frame, s),
    }
}

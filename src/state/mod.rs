//! 应用状态机：`AppState` 枚举 + 后台任务回传事件 `BackgroundEvent`。

pub mod install;
pub mod log;
pub mod search;
pub mod uninstall;
pub mod upgrade;

use winget::{Package, WingetError};

pub use install::InstallState;
pub use log::LogState;
pub use search::SearchState;
pub use uninstall::UninstallState;
pub use upgrade::UpgradeState;

/// 应用顶层状态。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AppState {
    #[default]
    MainMenu,
    Search(SearchState),
    Upgrade(UpgradeState),
    Install(InstallState),
    Uninstall(UninstallState),
    Log(LogState),
}

/// 后台任务回传事件。
#[derive(Debug)]
pub enum BackgroundEvent {
    SearchDone(Result<Vec<Package>, WingetError>),
    UpgradeListDone(Result<Vec<Package>, WingetError>),
    InstalledListDone(Result<Vec<Package>, WingetError>),
    ActionDone(Result<(), WingetError>),
    /// 变更命令实时 stdout/stderr 行。
    LogLine(String),
}

//! wingetui 二进制入口：初始化终端，运行 TUI 主循环，退出恢复终端。

use std::io;

use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use wingetui::app;
use wingetui::event::EventLoop;

#[tokio::main]
async fn main() -> io::Result<()> {
    let winget = winget::Winget::new();
    let (mut app, background_rx) = app::App::new(winget);
    let mut events = EventLoop::new(background_rx);

    let mut terminal: Terminal<CrosstermBackend<std::io::Stdout>> = ratatui::init();
    let result = app::run(&mut terminal, &mut app, &mut events).await;
    ratatui::restore();
    result
}

//! crossterm 事件循环：`tokio::select!` 合并键盘/终端事件与后台任务事件。

use crossterm::event::{self, KeyEvent};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::state::BackgroundEvent;

/// 应用事件：键盘 / 终端 Resize / 后台任务回传。
#[derive(Debug)]
pub enum Event {
    Key(KeyEvent),
    Resize(u16, u16),
    Background(BackgroundEvent),
}

/// 事件循环：crossterm 事件（Key / Resize）经专用线程读取，
/// 与 tokio 后台任务 channel 在 `next()` 中经 `tokio::select!` 合并。
pub struct EventLoop {
    rx: UnboundedReceiver<Event>,
    background_rx: UnboundedReceiver<BackgroundEvent>,
}

impl EventLoop {
    pub fn new(background_rx: UnboundedReceiver<BackgroundEvent>) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        // crossterm::event::read 是阻塞调用，放专用线程循环读取，
        // 经 tokio unbounded channel 送达主循环（不阻塞 tokio 工作线程）。
        std::thread::spawn(move || loop {
            match event::read() {
                Ok(event::Event::Key(key)) => {
                    if tx.send(Event::Key(key)).is_err() {
                        break;
                    }
                }
                Ok(event::Event::Resize(width, height)) => {
                    if tx.send(Event::Resize(width, height)).is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        });

        Self { rx, background_rx }
    }

    /// 等待下一个事件：crossterm 事件与后台事件公平合并。
    pub async fn next(&mut self) -> Event {
        tokio::select! {
            ev = self.rx.recv() => ev.expect("事件通道已关闭"),
            bg = self.background_rx.recv() => Event::Background(bg.expect("后台事件通道已关闭")),
        }
    }
}

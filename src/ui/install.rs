//! 安装屏渲染（占位：输入包 Id + 提示）。

use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::state::install::InstallState;

/// 安装屏：输入包 Id。
pub fn draw(frame: &mut Frame, s: &InstallState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(frame.area());

    let input = Paragraph::new(Line::from(format!("> {}", s.input))).block(
        Block::default()
            .title(" 安装包 (输入 Id, Esc 返回) ")
            .borders(Borders::ALL),
    );
    frame.render_widget(input, chunks[0]);

    let status = if let Some(err) = &s.error {
        Line::from(format!("错误: {err}")).style(Style::default().fg(Color::Red))
    } else if s.loading {
        Line::from("安装中...").style(Style::default().fg(Color::Yellow))
    } else if let Some(msg) = &s.message {
        Line::from(msg.clone()).style(Style::default().fg(Color::Green))
    } else {
        Line::from("输入包 Id 后按 Enter 安装").style(Style::default().fg(Color::DarkGray))
    };
    let status = Paragraph::new(status).alignment(Alignment::Center);
    frame.render_widget(status, chunks[1]);
}

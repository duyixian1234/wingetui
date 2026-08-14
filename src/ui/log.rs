//! 日志屏渲染（变更操作实时输出）。

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::state::log::LogState;

/// 日志屏：逐行显示变更命令输出；结束后显示结果提示。
pub fn draw(frame: &mut Frame, s: &LogState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(frame.area());

    let items: Vec<ListItem> = s
        .lines
        .iter()
        .map(|l| ListItem::new(Line::from(l.clone())))
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(" 操作日志 "));
    frame.render_widget(list, chunks[0]);

    let result_line = match &s.result {
        Some(r) => Line::from(r.clone()).style(Style::default().fg(Color::Green)),
        None => Line::from("执行中...").style(Style::default().fg(Color::Yellow)),
    };
    let hint = Paragraph::new(vec![
        result_line,
        Line::from("Esc 返回").style(Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(hint, chunks[1]);
}

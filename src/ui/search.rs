//! 搜索屏渲染（占位：显示查询词与结果列表）。

use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::state::search::SearchState;

/// 搜索屏：输入框 + 结果列表 + 加载/错误提示。
pub fn draw(frame: &mut Frame, s: &SearchState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let input = Paragraph::new(Line::from(format!("> {}", s.query))).block(
        Block::default()
            .title(" 搜索包 (输入后自动搜索, Esc 返回) ")
            .borders(Borders::ALL),
    );
    frame.render_widget(input, chunks[0]);

    let body = if s.loading {
        Paragraph::new("加载中...").alignment(Alignment::Center)
    } else if let Some(err) = &s.error {
        Paragraph::new(Line::from(format!("错误: {err}")).style(Style::default().fg(Color::Red)))
            .alignment(Alignment::Center)
    } else if s.results.is_empty() {
        Paragraph::new("无结果").alignment(Alignment::Center)
    } else {
        let items: Vec<ListItem> = s
            .results
            .iter()
            .map(|p| ListItem::new(Line::from(format!("{}  {}  {}", p.id, p.name, p.version))))
            .collect();
        let list = List::new(items).block(Block::default().borders(Borders::ALL).title(" 结果 "));
        frame.render_widget(list, chunks[1]);
        return;
    };
    frame.render_widget(body, chunks[1]);
}

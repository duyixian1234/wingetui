//! 卸载屏渲染（占位：已安装列表 + 操作提示）。

use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::state::uninstall::UninstallState;

/// 卸载屏：已安装列表 + 操作提示。
pub fn draw(frame: &mut Frame, s: &UninstallState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(frame.area());

    let body = if s.loading {
        Paragraph::new("加载已安装列表...").alignment(Alignment::Center)
    } else if s.items.is_empty() {
        Paragraph::new(match &s.message {
            Some(m) => Line::from(m.as_str()).style(Style::default().fg(Color::Red)),
            None => Line::from("没有已安装的包"),
        })
        .alignment(Alignment::Center)
    } else {
        let items: Vec<ListItem> = s
            .items
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let marker = if i == s.selected { ">" } else { " " };
                ListItem::new(Line::from(format!(
                    "{marker} {}  {}  {}",
                    p.id, p.name, p.version
                )))
            })
            .collect();
        let list = List::new(items).block(Block::default().borders(Borders::ALL).title(" 已安装 "));
        frame.render_widget(list, chunks[0]);
        return;
    };
    frame.render_widget(body, chunks[0]);

    let hint = Paragraph::new(
        Line::from(" ↑/↓ 选择 | Enter 卸载 | Esc 返回 ")
            .style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(hint, chunks[1]);
}

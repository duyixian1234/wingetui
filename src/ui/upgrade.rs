//! 升级屏渲染（占位：可升级列表 + 操作提示）。

use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::state::upgrade::UpgradeState;

/// 升级屏：可升级列表 + 操作提示（`u` 升级选中 / `a` 升级全部）。
pub fn draw(frame: &mut Frame, s: &UpgradeState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(frame.area());

    let body = if s.loading {
        Paragraph::new("加载可升级列表...").alignment(Alignment::Center)
    } else if s.items.is_empty() {
        Paragraph::new(match &s.message {
            Some(m) => Line::from(m.as_str()).style(Style::default().fg(Color::Red)),
            None => Line::from("没有可升级的包"),
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
                    "{marker} {}  {}  {} -> {}",
                    p.id,
                    p.name,
                    p.version,
                    p.available_version.as_deref().unwrap_or("?")
                )))
            })
            .collect();
        let list = List::new(items).block(Block::default().borders(Borders::ALL).title(" 可升级 "));
        frame.render_widget(list, chunks[0]);
        return;
    };
    frame.render_widget(body, chunks[0]);

    let hint = Paragraph::new(
        Line::from(" ↑/↓ 选择 | u 升级选中 | a 升级全部 | Esc 返回 ")
            .style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(hint, chunks[1]);
}

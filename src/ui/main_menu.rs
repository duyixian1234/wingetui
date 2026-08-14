//! 主菜单渲染。

use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// 主菜单 5 入口：搜索包 / 查看可升级 / 安装包 / 卸载包 / 退出。
pub fn draw(frame: &mut Frame) {
    let area = frame.area();
    let block = Block::default().title(" wingetui ").borders(Borders::ALL);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Length(6),
            Constraint::Percentage(65),
        ])
        .split(block.inner(area));

    let menu = Paragraph::new(vec![
        Line::from("管理 winget 的 TUI 工具").style(Style::default().fg(Color::Cyan)),
        Line::from(""),
        Line::from("  s) 搜索包").style(Style::default().fg(Color::Green)),
        Line::from("  u) 查看可升级 / 升级").style(Style::default().fg(Color::Green)),
        Line::from("  i) 安装包").style(Style::default().fg(Color::Green)),
        Line::from("  x) 卸载包").style(Style::default().fg(Color::Green)),
        Line::from("  q) 退出").style(Style::default().fg(Color::Red)),
    ])
    .alignment(Alignment::Left)
    .block(block);

    frame.render_widget(menu, inner[1]);
}

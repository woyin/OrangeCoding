//! 侧边栏组件
//!
//! 提供类似 Opencode 风格的侧边栏，支持三个面板：
//! - 文件树：显示项目文件结构
//! - 智能体状态：显示当前智能体的运行状态
//! - 会话列表：显示历史对话会话
//!
//! 使用 Ctrl+B 切换侧边栏显示/隐藏，在普通模式下使用 Tab 切换面板。

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
    Frame,
};

use crate::app::{InteractionMode, SidebarPanel, SidebarState, ThinkingDepth};

// ---------------------------------------------------------------------------
// 侧边栏视图
// ---------------------------------------------------------------------------

/// 侧边栏组件 - 渲染左侧信息面板
///
/// 包含标签页头部和内容区域，根据当前活动面板渲染不同内容。
/// 宽度固定为终端宽度的约 25%（最小 20 列，最大 40 列）。
pub struct SidebarView;

impl SidebarView {
    /// 计算侧边栏的推荐宽度
    ///
    /// 根据终端总宽度计算合适的侧边栏宽度，
    /// 保证在不同终端大小下都有良好的显示效果。
    pub fn recommended_width(total_width: u16) -> u16 {
        let width = total_width / 4;
        width.clamp(20, 40)
    }

    /// 渲染侧边栏到指定区域
    ///
    /// # 参数
    /// - `frame`: ratatui 渲染帧
    /// - `area`: 渲染区域
    /// - `sidebar`: 侧边栏状态
    /// - `interaction_mode`: 当前交互模式（用于智能体状态面板）
    /// - `thinking_depth`: 当前思考深度（用于智能体状态面板）
    pub fn render(
        frame: &mut Frame<'_>,
        area: Rect,
        sidebar: &SidebarState,
        interaction_mode: &InteractionMode,
        thinking_depth: &ThinkingDepth,
    ) {
        // 将侧边栏区域分为标签页头部和内容区域
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // 标签页头部
                Constraint::Min(0),    // 内容区域
            ])
            .split(area);

        // 渲染标签页头部
        Self::render_tabs(frame, chunks[0], sidebar);

        // 根据当前面板渲染内容
        match sidebar.active_panel {
            SidebarPanel::FileTree => {
                Self::render_file_tree(frame, chunks[1], sidebar);
            }
            SidebarPanel::AgentStatus => {
                Self::render_agent_status(frame, chunks[1], interaction_mode, thinking_depth);
            }
            SidebarPanel::SessionList => {
                Self::render_session_list(frame, chunks[1], sidebar);
            }
        }
    }

    /// 渲染标签页头部
    fn render_tabs(frame: &mut Frame<'_>, area: Rect, sidebar: &SidebarState) {
        let titles = vec!["📁 文件", "🤖 智能体", "💬 会话"];
        let selected = match sidebar.active_panel {
            SidebarPanel::FileTree => 0,
            SidebarPanel::AgentStatus => 1,
            SidebarPanel::SessionList => 2,
        };

        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" 面板 ")
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .select(selected)
            .style(Style::default().fg(Color::DarkGray))
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(tabs, area);
    }

    /// 渲染文件树面板
    fn render_file_tree(frame: &mut Frame<'_>, area: Rect, sidebar: &SidebarState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" 📁 项目文件 ")
            .border_style(Style::default().fg(Color::DarkGray));

        if sidebar.file_entries.is_empty() {
            // 文件树为空时显示提示
            let hint = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  加载中...",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  项目文件将在此处显示",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .block(block);
            frame.render_widget(hint, area);
            return;
        }

        // 构建文件树列表项
        let items: Vec<ListItem> = sidebar
            .file_entries
            .iter()
            .enumerate()
            .map(|(idx, entry)| {
                // 根据缩进层级添加前缀空格
                let indent = "  ".repeat(entry.depth);
                let icon = if entry.is_dir {
                    if entry.is_expanded {
                        "📂 "
                    } else {
                        "📁 "
                    }
                } else {
                    "📄 "
                };

                let style = if idx == sidebar.file_tree_index {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else if entry.is_dir {
                    Style::default().fg(Color::Blue)
                } else {
                    Style::default().fg(Color::White)
                };

                ListItem::new(Line::from(Span::styled(
                    format!("{indent}{icon}{}", entry.name),
                    style,
                )))
            })
            .collect();

        let list = List::new(items).block(block);
        frame.render_widget(list, area);
    }

    /// 渲染智能体状态面板
    fn render_agent_status(
        frame: &mut Frame<'_>,
        area: Rect,
        interaction_mode: &InteractionMode,
        thinking_depth: &ThinkingDepth,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" 🤖 智能体状态 ")
            .border_style(Style::default().fg(Color::DarkGray));

        // 构建状态信息行
        let mode_color = match interaction_mode {
            InteractionMode::Normal => Color::Green,
            InteractionMode::Plan => Color::Yellow,
            InteractionMode::Autopilot => Color::Cyan,
            InteractionMode::UltraWork => Color::Magenta,
        };

        let depth_color = match thinking_depth {
            ThinkingDepth::Off => Color::DarkGray,
            ThinkingDepth::Light => Color::Green,
            ThinkingDepth::Medium => Color::Yellow,
            ThinkingDepth::Deep => Color::Cyan,
            ThinkingDepth::Maximum => Color::Red,
        };

        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  交互模式: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    interaction_mode.label(),
                    Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(Span::styled(
                format!("  {}", interaction_mode.description()),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("  思考深度: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{} {}", thinking_depth.icon(), thinking_depth.label()),
                    Style::default()
                        .fg(depth_color)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  ─────────────────",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  快捷键:",
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "  Shift+Tab  切换模式",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "  Ctrl+L     思考深度",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "  Ctrl+B     侧边栏",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }

    /// 渲染会话列表面板
    fn render_session_list(frame: &mut Frame<'_>, area: Rect, sidebar: &SidebarState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" 💬 历史会话 ")
            .border_style(Style::default().fg(Color::DarkGray));

        if sidebar.session_entries.is_empty() {
            let hint = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  暂无历史会话",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  开始新对话后",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(Span::styled(
                    "  会话将在此列出",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .block(block);
            frame.render_widget(hint, area);
            return;
        }

        // 构建会话列表项
        let items: Vec<ListItem> = sidebar
            .session_entries
            .iter()
            .enumerate()
            .map(|(idx, entry)| {
                let style = if entry.is_active {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else if idx == sidebar.session_list_index {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let prefix = if entry.is_active { "▶ " } else { "  " };
                let time_str = entry.updated_at.format("%m/%d %H:%M").to_string();

                ListItem::new(vec![
                    Line::from(Span::styled(format!("{prefix}{}", entry.title), style)),
                    Line::from(Span::styled(
                        format!("    {time_str}"),
                        Style::default().fg(Color::DarkGray),
                    )),
                ])
            })
            .collect();

        let list = List::new(items).block(block);
        frame.render_widget(list, area);
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 测试侧边栏推荐宽度() {
        // 小终端
        assert_eq!(SidebarView::recommended_width(60), 20);
        // 中等终端
        assert_eq!(SidebarView::recommended_width(120), 30);
        // 大终端
        assert_eq!(SidebarView::recommended_width(200), 40);
        // 极小终端
        assert_eq!(SidebarView::recommended_width(40), 20);
    }
}

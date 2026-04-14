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
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Tabs},
    Frame,
};

use chrono::{DateTime, Local, Utc};

use crate::app::{InteractionMode, McpConnectionState, SidebarPanel, SidebarState, ThinkingDepth};

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
            SidebarPanel::ContextOverview => {
                Self::render_context_overview(
                    frame,
                    chunks[1],
                    sidebar,
                    interaction_mode,
                    thinking_depth,
                );
            }
            SidebarPanel::McpStatus => {
                Self::render_mcp_status(frame, chunks[1], sidebar);
            }
            SidebarPanel::Changes => {
                Self::render_changes(frame, chunks[1], sidebar);
            }
        }
    }

    /// 渲染标签页头部
    fn render_tabs(frame: &mut Frame<'_>, area: Rect, sidebar: &SidebarState) {
        let titles = vec!["📊 上下文", "🔌 MCP", "📝 变更"];
        let selected = match sidebar.active_panel {
            SidebarPanel::ContextOverview => 0,
            SidebarPanel::McpStatus => 1,
            SidebarPanel::Changes => 2,
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

    fn render_context_overview(
        frame: &mut Frame<'_>,
        area: Rect,
        sidebar: &SidebarState,
        interaction_mode: &InteractionMode,
        thinking_depth: &ThinkingDepth,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" 📊 执行上下文 ")
            .border_style(Style::default().fg(Color::DarkGray));

        let percent = if sidebar.context_usage.max_tokens == 0 {
            0
        } else {
            ((sidebar.context_usage.used_tokens.saturating_mul(100))
                / sidebar.context_usage.max_tokens)
                .min(100) as u16
        };

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(inner);

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

        let summary = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("  模式: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    interaction_mode.label(),
                    Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("  思考: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{} {}", thinking_depth.icon(), thinking_depth.label()),
                    Style::default()
                        .fg(depth_color)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("  会话: ", Style::default().fg(Color::Gray)),
                Span::raw(format!("{} 个", sidebar.context_usage.session_count)),
            ]),
        ]);
        frame.render_widget(summary, chunks[0]);

        let gauge = Gauge::default()
            .label(format!(
                "{} / {} tokens",
                sidebar.context_usage.used_tokens, sidebar.context_usage.max_tokens
            ))
            .percent(percent)
            .gauge_style(Style::default().fg(Color::Cyan));
        frame.render_widget(gauge, chunks[1]);

        let hints = Paragraph::new(vec![
            Line::from(Span::styled(
                "  侧边栏用于展示任务态信息",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "  不再展示文件树/智能体/会话列表",
                Style::default().fg(Color::DarkGray),
            )),
        ]);
        frame.render_widget(hints, chunks[2]);
    }

    fn render_mcp_status(frame: &mut Frame<'_>, area: Rect, sidebar: &SidebarState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" 🔌 MCP 连接 ")
            .border_style(Style::default().fg(Color::DarkGray));

        if sidebar.mcp_servers.is_empty() {
            let hint = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  暂无 MCP 连接",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .block(block);
            frame.render_widget(hint, area);
            return;
        }

        let items: Vec<ListItem> = sidebar
            .mcp_servers
            .iter()
            .enumerate()
            .map(|(idx, server)| {
                let (icon, color) = match server.state {
                    McpConnectionState::Connected => ("●", Color::Green),
                    McpConnectionState::Disconnected => ("○", Color::Red),
                    McpConnectionState::Degraded => ("◐", Color::Yellow),
                };
                let name_style = if idx == sidebar.mcp_index {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(format!(" {icon} "), Style::default().fg(color)),
                        Span::styled(server.name.clone(), name_style),
                    ]),
                    Line::from(Span::styled(
                        format!("   {}", server.detail),
                        Style::default().fg(Color::DarkGray),
                    )),
                ])
            })
            .collect();

        let list = List::new(items).block(block);
        frame.render_widget(list, area);
    }

    fn render_changes(frame: &mut Frame<'_>, area: Rect, sidebar: &SidebarState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" 📝 修改文件 ")
            .border_style(Style::default().fg(Color::DarkGray));

        if sidebar.modified_files.is_empty() {
            let hint = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  暂无已记录修改",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .block(block);
            frame.render_widget(hint, area);
            return;
        }

        let items: Vec<ListItem> = sidebar
            .modified_files
            .iter()
            .enumerate()
            .map(|(idx, entry)| {
                let style = if idx == sidebar.modified_file_index {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                ListItem::new(vec![
                    Line::from(Span::styled(format!(" {}", entry.path), style)),
                    Line::from(Span::styled(
                        format!(
                            "   {} · {}",
                            entry.change_kind,
                            Self::format_local_timestamp(&entry.updated_at)
                        ),
                        Style::default().fg(Color::DarkGray),
                    )),
                ])
            })
            .collect();

        let list = List::new(items).block(block);
        frame.render_widget(list, area);
    }

    fn format_local_timestamp(timestamp: &DateTime<Utc>) -> String {
        timestamp
            .with_timezone(&Local)
            .format("%m/%d %H:%M")
            .to_string()
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{ContextUsage, McpServerStatus, ModifiedFileEntry, SidebarState};
    use chrono::TimeZone;

    #[test]
    fn 测试侧边栏推荐宽度() {
        assert_eq!(SidebarView::recommended_width(60), 20);
        assert_eq!(SidebarView::recommended_width(120), 30);
        assert_eq!(SidebarView::recommended_width(200), 40);
        assert_eq!(SidebarView::recommended_width(40), 20);
    }

    #[test]
    fn 测试侧边栏标签标题() {
        assert_eq!(SidebarPanel::ContextOverview.title(), "📊 上下文");
        assert_eq!(SidebarPanel::McpStatus.title(), "🔌 MCP");
        assert_eq!(SidebarPanel::Changes.title(), "📝 变更");
    }

    #[test]
    fn 测试侧边栏默认状态包含任务态信息() {
        let sidebar = SidebarState::new();
        assert_eq!(sidebar.active_panel, SidebarPanel::ContextOverview);
        assert_eq!(sidebar.context_usage.max_tokens, 128_000);
        assert_eq!(sidebar.context_usage.session_count, 1);
        assert!(!sidebar.mcp_servers.is_empty());
    }

    #[test]
    fn 测试上下文占用百分比计算输入可用() {
        let sidebar = SidebarState {
            visible: true,
            active_panel: SidebarPanel::ContextOverview,
            mcp_index: 0,
            modified_file_index: 0,
            mcp_servers: vec![McpServerStatus {
                name: "filesystem".to_string(),
                state: McpConnectionState::Connected,
                detail: "已连接".to_string(),
            }],
            modified_files: vec![ModifiedFileEntry {
                path: "src/app.rs".to_string(),
                change_kind: "modified".to_string(),
                updated_at: Utc.with_ymd_and_hms(2026, 4, 14, 12, 34, 56).unwrap(),
            }],
            context_usage: ContextUsage {
                used_tokens: 64_000,
                max_tokens: 128_000,
                session_count: 2,
            },
        };

        assert_eq!(
            sidebar.context_usage.used_tokens * 100 / sidebar.context_usage.max_tokens,
            50
        );
    }

    #[test]
    fn 测试侧边栏本地时间格式固定() {
        let utc = Utc.with_ymd_and_hms(2026, 4, 14, 12, 34, 56).unwrap();
        let formatted = SidebarView::format_local_timestamp(&utc);

        assert_eq!(formatted.len(), 11);
        assert_eq!(formatted.chars().filter(|ch| *ch == '/').count(), 1);
        assert_eq!(formatted.chars().filter(|ch| *ch == ':').count(), 1);
    }
}

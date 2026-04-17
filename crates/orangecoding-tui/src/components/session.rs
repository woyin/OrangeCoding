//! 会话视图组件
//!
//! 本模块实现了消息列表的渲染逻辑，负责将对话历史以美观的方式
//! 展示在终端界面中。不同角色的消息使用不同的颜色区分，
//! 支持 Markdown 渲染和滚动浏览。

use orangecoding_core::message::Role;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use chrono::{DateTime, Local, Utc};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, DisplayMessage};
use crate::markdown::MarkdownRenderer;

// ---------------------------------------------------------------------------
// 会话视图
// ---------------------------------------------------------------------------

/// 会话视图组件 - 渲染消息列表
///
/// 负责将 `App` 中的消息列表渲染为可滚动的终端界面。
/// 每条消息包含角色标识、时间戳和经过 Markdown 渲染的内容。
///
/// # 颜色方案
///
/// - 用户消息：蓝色
/// - 助手消息：绿色
/// - 系统消息：黄色
/// - 工具消息：品红色
pub struct SessionView;

impl SessionView {
    /// 渲染消息列表到指定区域
    ///
    /// 将所有消息转换为带样式的文本行，构建为 `Paragraph` 组件，
    /// 并根据 `scroll_offset` 进行滚动显示。
    ///
    /// # 参数
    /// - `frame`: ratatui 渲染帧
    /// - `area`: 渲染区域（矩形）
    /// - `app`: 应用状态引用
    pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App) {
        // 构建消息区域的边框和标题
        let message_count = app.messages.len();
        let title = format!(" 对话 ({message_count} 条消息) ");

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_alignment(ratatui::layout::Alignment::Left)
            .border_style(Style::default().fg(Color::Gray));

        // 如果没有消息，显示欢迎提示
        if app.messages.is_empty() {
            let welcome_lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "   ██████╗ ██████╗  █████╗ ███╗   ██╗ ██████╗ ███████╗",
                    Style::default().fg(Color::Rgb(255, 140, 0)),
                )),
                Line::from(Span::styled(
                    "  ██╔═══██╗██╔══██╗██╔══██╗████╗  ██║██╔════╝ ██╔════╝",
                    Style::default().fg(Color::Rgb(255, 140, 0)),
                )),
                Line::from(Span::styled(
                    "  ██║   ██║██████╔╝███████║██╔██╗ ██║██║  ███╗█████╗  ",
                    Style::default().fg(Color::Rgb(255, 140, 0)),
                )),
                Line::from(Span::styled(
                    "  ██║   ██║██╔══██╗██╔══██║██║╚██╗██║██║   ██║██╔══╝  ",
                    Style::default().fg(Color::Rgb(255, 165, 0)),
                )),
                Line::from(Span::styled(
                    "  ╚██████╔╝██║  ██║██║  ██║██║ ╚████║╚██████╔╝███████╗",
                    Style::default().fg(Color::Rgb(255, 165, 0)),
                )),
                Line::from(Span::styled(
                    "   ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═══╝ ╚═════╝ ╚══════╝",
                    Style::default().fg(Color::Rgb(255, 165, 0)),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  🍊 OrangeCoding — 顶级 AI 编码代理",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  /help 查看命令  ·  /doctor 健康检查  ·  /cost 查看用量",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(Span::styled(
                    "  Shift+Tab 切换规划模式  ·  Ctrl+C 退出",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
            ];

            let paragraph = Paragraph::new(Text::from(welcome_lines))
                .block(block)
                .wrap(Wrap { trim: false });

            frame.render_widget(paragraph, area);
            return;
        }

        // 将所有消息转换为样式化的文本行
        let mut all_lines: Vec<Line<'static>> = Vec::new();
        let md_renderer = MarkdownRenderer::new();

        for (idx, message) in app.messages.iter().enumerate() {
            // 添加消息间分隔空行（第一条消息除外）
            if idx > 0 {
                all_lines.push(Line::from(""));
            }

            // 渲染消息头部（角色 + 时间戳）
            let header = Self::render_message_header(message);
            all_lines.push(header);

            // 渲染消息内容（支持 Markdown）
            let content_lines = md_renderer.render(&message.content);
            for line in content_lines {
                // 为内容行添加缩进
                let mut indented_spans = vec![Span::raw("  ".to_string())];
                indented_spans.extend(line.spans);
                all_lines.push(Line::from(indented_spans));
            }

            // 如果消息正在流式输出，显示闪烁的光标
            if message.is_streaming {
                all_lines.push(Line::from(Span::styled(
                    "  ▌",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::SLOW_BLINK),
                )));
            }

            // 显示 token 使用量（如果有）
            if let Some(ref usage) = message.token_usage {
                all_lines.push(Line::from(Span::styled(
                    format!(
                        "  [Token: 提示词={}, 补全={}, 总计={}]",
                        usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
                    ),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }

        // 计算滚动偏移量
        // scroll_offset 为 0 表示显示最底部的内容
        let visible_height = area.height.saturating_sub(2); // 减去边框高度
        let content_width = area.width.saturating_sub(2) as usize; // 减去边框宽度

        // 计算换行后的实际可视行数（考虑 Wrap 对长行的折叠）
        let total_lines = if content_width > 0 {
            Self::count_wrapped_lines(&all_lines, content_width) as u16
        } else {
            all_lines.len() as u16
        };

        // 将 scroll_offset（从底部计算）转换为从顶部计算的偏移
        let scroll_from_top = if total_lines > visible_height {
            let max_scroll = total_lines - visible_height;
            max_scroll.saturating_sub(app.scroll_offset)
        } else {
            0
        };

        let paragraph = Paragraph::new(Text::from(all_lines))
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((scroll_from_top, 0));

        frame.render_widget(paragraph, area);
    }

    /// 渲染消息头部行
    ///
    /// 包含角色图标、角色名称和时间戳，使用角色对应的颜色。
    ///
    /// # 参数
    /// - `message`: 要渲染的消息
    ///
    /// # 返回
    /// - 带样式的头部行
    fn render_message_header(message: &DisplayMessage) -> Line<'static> {
        let role_color = Self::role_color(&message.role);
        let timestamp = Self::format_local_timestamp(&message.timestamp);

        Line::from(vec![
            // 角色标签
            Span::styled(
                format!(" {} ", message.role_label()),
                Style::default().fg(role_color).add_modifier(Modifier::BOLD),
            ),
            // 时间戳
            Span::styled(
                format!("[{timestamp}]"),
                Style::default().fg(Color::DarkGray),
            ),
        ])
    }

    /// 获取角色对应的显示颜色
    ///
    /// 每种角色使用独特的颜色以便在对话中快速区分：
    /// - 用户：蓝色
    /// - 助手：绿色
    /// - 系统：黄色
    /// - 工具：品红色
    fn role_color(role: &Role) -> Color {
        match role {
            Role::User => Color::Blue,
            Role::Assistant => Color::Green,
            Role::System => Color::Yellow,
            Role::Tool => Color::Magenta,
        }
    }

    fn format_local_timestamp(timestamp: &DateTime<Utc>) -> String {
        timestamp
            .with_timezone(&Local)
            .format("%H:%M:%S")
            .to_string()
    }

    /// 计算文本行在给定宽度下换行后的实际可视行数
    fn count_wrapped_lines(lines: &[Line<'_>], width: usize) -> usize {
        if width == 0 {
            return lines.len();
        }
        lines
            .iter()
            .map(|line| {
                let line_width: usize = line.spans.iter().map(|s| s.content.width()).sum();
                if line_width == 0 {
                    1 // 空行仍占1行
                } else {
                    (line_width + width - 1) / width // 向上取整
                }
            })
            .sum()
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn 测试角色颜色映射() {
        assert_eq!(SessionView::role_color(&Role::User), Color::Blue);
        assert_eq!(SessionView::role_color(&Role::Assistant), Color::Green);
        assert_eq!(SessionView::role_color(&Role::System), Color::Yellow);
        assert_eq!(SessionView::role_color(&Role::Tool), Color::Magenta);
    }

    #[test]
    fn 测试消息头部渲染() {
        let msg = DisplayMessage::new(Role::User, "测试消息");
        let header = SessionView::render_message_header(&msg);

        assert_eq!(header.spans.len(), 2);

        let label_text: String = header.spans[0].content.to_string();
        assert!(label_text.contains("用户"));
    }

    #[test]
    fn 测试本地时间格式化输出固定格式() {
        let utc = Utc.with_ymd_and_hms(2026, 4, 14, 12, 34, 56).unwrap();
        let formatted = SessionView::format_local_timestamp(&utc);

        assert_eq!(formatted.len(), 8);
        assert_eq!(formatted.chars().filter(|ch| *ch == ':').count(), 2);
    }

    #[test]
    fn 测试换行行数计算_短行不换行() {
        let lines = vec![Line::from("short"), Line::from("also short")];
        // 每行都短于 80 列，不需要换行
        assert_eq!(SessionView::count_wrapped_lines(&lines, 80), 2);
    }

    #[test]
    fn 测试换行行数计算_长行换行() {
        let long_text = "a".repeat(160);
        let lines = vec![Line::from(long_text)];
        // 160 字符在 80 列宽度下应换成 2 行
        assert_eq!(SessionView::count_wrapped_lines(&lines, 80), 2);
    }

    #[test]
    fn 测试换行行数计算_空行() {
        let lines = vec![Line::from("")];
        assert_eq!(SessionView::count_wrapped_lines(&lines, 80), 1);
    }

    #[test]
    fn 测试换行行数计算_零宽度回退() {
        let lines = vec![Line::from("test"), Line::from("line")];
        assert_eq!(SessionView::count_wrapped_lines(&lines, 0), 2);
    }

    #[test]
    fn 测试换行行数计算_中文字符() {
        // 每个中文字符占2列宽度
        let text = "你好世界测试"; // 6 chars × 2 = 12 columns
        let lines = vec![Line::from(text)];
        // 在 10 列宽度下应换成 2 行 (12 / 10 向上取整 = 2)
        assert_eq!(SessionView::count_wrapped_lines(&lines, 10), 2);
    }
}

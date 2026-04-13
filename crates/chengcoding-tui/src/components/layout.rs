//! TUI 主布局组件
//!
//! 提供完整的终端界面布局，将侧边栏、消息区域、输入框和状态栏
//! 组合在一起形成统一的用户界面。
//!
//! # 布局结构
//!
//! ```text
//! ┌──────────┬──────────────────────────────────┐
//! │          │                                  │
//! │  侧边栏  │        消息 / 对话区域             │
//! │          │                                  │
//! │  (Ctrl+B │                                  │
//! │  切换)   │                                  │
//! │          ├──────────────────────────────────┤
//! │          │        输入框                      │
//! │          ├──────────────────────────────────┤
//! │          │   状态栏 [模式|思考深度|模型|token]  │
//! └──────────┴──────────────────────────────────┘
//! ```

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;
use crate::components::{InputView, SessionView, SidebarView};

// ---------------------------------------------------------------------------
// 主布局
// ---------------------------------------------------------------------------

/// TUI 主布局组件 - 协调所有子组件的渲染
///
/// 根据应用状态（侧边栏是否可见等）动态调整布局比例。
pub struct MainLayout;

impl MainLayout {
    /// 渲染完整的 TUI 界面
    ///
    /// # 参数
    /// - `frame`: ratatui 渲染帧
    /// - `app`: 应用状态引用
    pub fn render(frame: &mut Frame<'_>, app: &App) {
        let size = frame.size();

        // 根据侧边栏是否可见决定水平布局
        if app.sidebar.visible && size.width > 60 {
            // 带侧边栏的布局
            let sidebar_width = SidebarView::recommended_width(size.width);
            let horizontal = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(sidebar_width), Constraint::Min(40)])
                .split(size);

            // 渲染侧边栏
            SidebarView::render(
                frame,
                horizontal[0],
                &app.sidebar,
                &app.interaction_mode,
                &app.thinking_depth,
            );

            // 渲染主区域（消息 + 输入 + 状态栏）
            Self::render_main_area(frame, horizontal[1], app);
        } else {
            // 无侧边栏的全宽布局
            Self::render_main_area(frame, size, app);
        }

        // 如果帮助面板可见，在中心渲染帮助弹窗
        if app.show_help {
            Self::render_help_overlay(frame, size);
        }
    }

    /// 渲染主区域（消息 + 输入 + 状态栏）
    fn render_main_area(frame: &mut Frame<'_>, area: Rect, app: &App) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),    // 消息区域
                Constraint::Length(3), // 输入区域
                Constraint::Length(1), // 状态栏
            ])
            .split(area);

        // 渲染消息区域
        SessionView::render(frame, chunks[0], app);

        // 渲染输入区域
        InputView::render(frame, chunks[1], app);

        // 渲染状态栏
        Self::render_status_bar(frame, chunks[2], app);
    }

    /// 渲染底部状态栏
    ///
    /// 显示格式: [交互模式] | [思考深度] | [模型] | [Token] | [状态]
    fn render_status_bar(frame: &mut Frame<'_>, area: Rect, app: &App) {
        // 交互模式颜色
        let mode_color = match app.interaction_mode {
            crate::app::InteractionMode::Normal => Color::Green,
            crate::app::InteractionMode::Plan => Color::Yellow,
            crate::app::InteractionMode::Autopilot => Color::Cyan,
            crate::app::InteractionMode::UltraWork => Color::Magenta,
        };

        // 思考深度颜色
        let depth_color = match app.thinking_depth {
            crate::app::ThinkingDepth::Off => Color::DarkGray,
            crate::app::ThinkingDepth::Light => Color::Green,
            crate::app::ThinkingDepth::Medium => Color::Yellow,
            crate::app::ThinkingDepth::Deep => Color::Cyan,
            crate::app::ThinkingDepth::Maximum => Color::Red,
        };

        let spans = vec![
            Span::styled(" ", Style::default()),
            // 交互模式
            Span::styled(
                format!(" {} ", app.interaction_mode.label()),
                Style::default()
                    .fg(Color::Black)
                    .bg(mode_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            // 思考深度
            Span::styled(
                format!(
                    "{} {} ",
                    app.thinking_depth.icon(),
                    app.thinking_depth.label()
                ),
                Style::default().fg(depth_color),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            // 模型名称
            Span::styled(
                format!("🤖 {} ", app.status.model_name),
                Style::default().fg(Color::White),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            // Token 使用量
            Span::styled(
                format!("📊 {} tokens ", app.status.token_count),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            // 状态文本
            Span::styled(
                format!("{} ", app.status.status_text),
                Style::default().fg(Color::Gray),
            ),
            // TUI 导航模式
            Span::styled(
                format!(" [{}] ", app.mode),
                Style::default().fg(Color::DarkGray),
            ),
        ];

        let status_line =
            Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Rgb(30, 30, 30)));

        frame.render_widget(status_line, area);
    }

    /// 渲染帮助弹窗（覆盖层）
    fn render_help_overlay(frame: &mut Frame<'_>, area: Rect) {
        // 计算弹窗区域（居中，60x20）
        let popup_width = 60.min(area.width.saturating_sub(4));
        let popup_height = 22.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(popup_width)) / 2;
        let y = (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(x, y, popup_width, popup_height);

        let help_lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  ChengCoding 快捷键帮助",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  ─── 全局快捷键 ───",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from("  Ctrl+C       退出应用"),
            Line::from("  Shift+Tab    切换交互模式"),
            Line::from("  Ctrl+L       切换思考深度"),
            Line::from("  Ctrl+B       切换侧边栏"),
            Line::from(""),
            Line::from(Span::styled(
                "  ─── 普通模式 ───",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from("  i / Enter    进入输入模式"),
            Line::from("  q            退出"),
            Line::from("  ?            帮助"),
            Line::from("  j/k          上下滚动"),
            Line::from("  / 或 :       命令模式"),
            Line::from("  Tab          切换面板"),
            Line::from(""),
            Line::from(Span::styled(
                "  ─── 斜杠命令 ───",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from("  /model <名称>  切换模型"),
            Line::from("  /mode <模式>   切换交互模式"),
            Line::from("  /clear         清除对话"),
            Line::from(""),
            Line::from(Span::styled(
                "  按 Esc 或 ? 关闭帮助",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let help = Paragraph::new(help_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" ❓ 帮助 ")
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .style(Style::default().bg(Color::Rgb(20, 20, 20)));

        // 先清除弹窗区域
        frame.render_widget(
            Block::default().style(Style::default().bg(Color::Rgb(20, 20, 20))),
            popup_area,
        );
        frame.render_widget(help, popup_area);
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn 测试主布局渲染不崩溃() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new("gpt-4");

        terminal
            .draw(|frame| {
                MainLayout::render(frame, &app);
            })
            .unwrap();
    }

    #[test]
    fn 测试无侧边栏布局渲染不崩溃() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new("gpt-4");
        app.sidebar.visible = false;

        terminal
            .draw(|frame| {
                MainLayout::render(frame, &app);
            })
            .unwrap();
    }

    #[test]
    fn 测试窄终端布局渲染不崩溃() {
        // 窄终端下侧边栏应自动隐藏
        let backend = TestBackend::new(50, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new("gpt-4");

        terminal
            .draw(|frame| {
                MainLayout::render(frame, &app);
            })
            .unwrap();
    }

    #[test]
    fn 测试帮助弹窗渲染不崩溃() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new("gpt-4");
        app.show_help = true;

        terminal
            .draw(|frame| {
                MainLayout::render(frame, &app);
            })
            .unwrap();
    }
}

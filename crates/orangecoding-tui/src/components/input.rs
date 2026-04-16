//! 输入框组件
//!
//! 本模块实现了用户输入区域的渲染逻辑，包括文本编辑区、
//! 光标显示、模式指示器和多行输入支持。

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, AppMode, CommandMenuKind};

// ---------------------------------------------------------------------------
// 输入视图
// ---------------------------------------------------------------------------

/// 输入视图组件 - 渲染用户输入区域
///
/// 负责绘制输入框、显示当前输入内容、光标位置以及模式指示器。
/// 支持 Unicode 字符的正确宽度计算（包括中文等双宽字符）。
pub struct InputView;

impl InputView {
    /// 渲染输入区域到指定位置
    ///
    /// 根据当前应用模式显示不同的输入提示和样式：
    /// - 输入模式：显示消息输入提示，允许编辑
    /// - 命令模式：显示命令前缀 `:`
    /// - 其他模式：显示提示信息
    ///
    /// # 参数
    /// - `frame`: ratatui 渲染帧
    /// - `area`: 渲染区域
    /// - `app`: 应用状态引用
    pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App) {
        // 根据当前模式选择标题和样式
        let (title, border_color, content_line) = match app.mode {
            AppMode::Input => {
                let title = " 输入 (Enter 发送, Esc 取消) ";
                let border_color = Color::Cyan;
                let content = Self::build_input_content(app);
                (title, border_color, content)
            }
            AppMode::Command => {
                let title = " 命令 (Enter 执行, Esc 取消) ";
                let border_color = Color::Yellow;
                let content = Self::build_command_content(app);
                (title, border_color, content)
            }
            AppMode::Normal => {
                let title = " 按 i 进入输入模式, ? 查看帮助 ";
                let border_color = Color::DarkGray;
                let content = Line::from(Span::styled(
                    "-- 普通模式 --",
                    Style::default().fg(Color::DarkGray),
                ));
                (title, border_color, content)
            }
            AppMode::Help => {
                let title = " 帮助模式 (Esc 退出) ";
                let border_color = Color::Green;
                let content = Line::from(Span::styled(
                    "-- 帮助模式 --",
                    Style::default().fg(Color::DarkGray),
                ));
                (title, border_color, content)
            }
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(border_color));

        let paragraph = Paragraph::new(content_line).block(block);
        frame.render_widget(paragraph, area);

        if let Some(menu) = &app.command_menu {
            if area.height >= 3 {
                let preview = menu
                    .items
                    .iter()
                    .take(3)
                    .enumerate()
                    .map(|(idx, item)| {
                        let prefix = if idx == menu.selected_index {
                            "›"
                        } else {
                            " "
                        };
                        let color = if idx == menu.selected_index {
                            Color::Cyan
                        } else {
                            Color::DarkGray
                        };
                        Line::from(vec![
                            Span::styled(format!("{prefix} "), Style::default().fg(color)),
                            Span::styled(item.value.clone(), Style::default().fg(Color::White)),
                            Span::styled(
                                format!(" — {}", item.description),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ])
                    })
                    .collect::<Vec<_>>();

                let menu_title = match menu.kind {
                    CommandMenuKind::Slash => " 命令建议 ",
                    CommandMenuKind::Model => " 模型选择 ",
                    CommandMenuKind::Mode => " 交互模式 ",
                    CommandMenuKind::Think => " 思考深度 ",
                };

                let menu_area = Rect::new(area.x, area.y.saturating_sub(4), area.width.min(80), 4);
                let menu_widget = Paragraph::new(preview).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(menu_title)
                        .border_style(Style::default().fg(Color::Yellow)),
                );
                frame.render_widget(menu_widget, menu_area);
            }
        }

        if matches!(app.mode, AppMode::Input | AppMode::Command) {
            let cursor_x = Self::calculate_cursor_x(app, area);
            let cursor_y = area.y + 1;
            frame.set_cursor(cursor_x, cursor_y);
        }
    }

    /// 构建输入模式下的内容行
    ///
    /// 包含模式指示器和输入缓冲区的内容。
    fn build_input_content(app: &App) -> Line<'static> {
        let mut spans = vec![
            // 输入提示符
            Span::styled(
                "❯ ".to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ];

        if app.input.buffer.is_empty() {
            // 空缓冲区时显示占位提示
            spans.push(Span::styled(
                "输入消息...".to_string(),
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            // 显示输入内容
            spans.push(Span::raw(app.input.buffer.clone()));
        }

        Line::from(spans)
    }

    /// 构建命令模式下的内容行
    ///
    /// 以 `:` 前缀显示命令输入。
    fn build_command_content(app: &App) -> Line<'static> {
        Line::from(vec![
            // 命令前缀
            Span::styled(
                ":".to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            // 命令内容
            Span::raw(app.input.buffer.clone()),
        ])
    }

    /// 计算光标在终端中的 X 坐标
    ///
    /// 需要考虑 Unicode 字符的显示宽度（如中文字符占两个终端列宽），
    /// 以及输入提示符的宽度。
    ///
    /// # 参数
    /// - `app`: 应用状态
    /// - `area`: 输入区域
    ///
    /// # 返回
    /// - 光标的终端 X 坐标
    fn calculate_cursor_x(app: &App, area: Rect) -> u16 {
        // 提示符的宽度
        let prefix_width: u16 = match app.mode {
            AppMode::Input => 2,   // "❯ " 的显示宽度
            AppMode::Command => 1, // ":" 的显示宽度
            _ => 0,
        };

        // 计算光标位置之前的文本显示宽度
        let text_before_cursor: String = app
            .input
            .buffer
            .chars()
            .take(app.input.cursor_position)
            .collect();
        let text_width = UnicodeWidthStr::width(text_before_cursor.as_str()) as u16;

        // 光标 X = 区域起始 X + 左边框(1) + 前缀宽度 + 文本宽度
        area.x + 1 + prefix_width + text_width
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{CommandMenuItem, CommandMenuKind, CommandMenuState};
    use ratatui::{backend::TestBackend, layout::Rect, Terminal};

    #[test]
    fn 测试光标位置计算_ascii() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Input;
        app.input.buffer = "hello".to_string();
        app.input.cursor_position = 5;

        let area = Rect::new(0, 0, 80, 3);
        let cursor_x = InputView::calculate_cursor_x(&app, area);

        // X = 0(区域起始) + 1(边框) + 2(提示符) + 5(文本宽度) = 8
        assert_eq!(cursor_x, 8);
    }

    #[test]
    fn 测试光标位置计算_中文() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Input;
        app.input.buffer = "你好".to_string();
        app.input.cursor_position = 2;

        let area = Rect::new(0, 0, 80, 3);
        let cursor_x = InputView::calculate_cursor_x(&app, area);

        // X = 0(区域起始) + 1(边框) + 2(提示符) + 4(中文宽度，每字2列) = 7
        assert_eq!(cursor_x, 7);
    }

    #[test]
    fn 测试光标位置计算_混合文本() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Input;
        app.input.buffer = "hi你好".to_string();
        app.input.cursor_position = 4; // 光标在末尾

        let area = Rect::new(0, 0, 80, 3);
        let cursor_x = InputView::calculate_cursor_x(&app, area);

        // X = 0 + 1 + 2 + (2 + 4) = 9
        // "hi" 宽度=2, "你好" 宽度=4
        assert_eq!(cursor_x, 9);
    }

    #[test]
    fn 测试命令模式光标位置() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Command;
        app.input.buffer = "quit".to_string();
        app.input.cursor_position = 4;

        let area = Rect::new(0, 0, 80, 3);
        let cursor_x = InputView::calculate_cursor_x(&app, area);

        // X = 0 + 1(边框) + 1(命令前缀 ":") + 4(文本宽度) = 6
        assert_eq!(cursor_x, 6);
    }

    #[test]
    fn 测试空缓冲区光标位置() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Input;
        app.input.buffer = String::new();
        app.input.cursor_position = 0;

        let area = Rect::new(0, 0, 80, 3);
        let cursor_x = InputView::calculate_cursor_x(&app, area);

        assert_eq!(cursor_x, 3);
    }

    #[test]
    fn 测试命令菜单渲染不崩溃() {
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Command;
        app.input.buffer = "/model".to_string();
        app.command_menu = Some(CommandMenuState {
            kind: CommandMenuKind::Model,
            query: String::new(),
            items: vec![CommandMenuItem {
                value: "glm-5.1".to_string(),
                description: "GLM 5.1".to_string(),
            }],
            selected_index: 0,
        });

        terminal
            .draw(|frame| {
                InputView::render(frame, Rect::new(0, 5, 100, 3), &app);
            })
            .unwrap();
    }
}

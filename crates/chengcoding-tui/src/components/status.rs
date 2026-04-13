//! 状态栏组件
//!
//! 本模块实现了底部状态栏的渲染逻辑，显示 AI 模型名称、
//! token 使用量、连接状态和当前操作模式等运行时信息。

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{App, AppMode};

// ---------------------------------------------------------------------------
// 状态栏
// ---------------------------------------------------------------------------

/// 状态栏组件 - 渲染底部状态信息栏
///
/// 状态栏采用单行布局，从左到右依次显示：
/// 1. 当前模式指示器
/// 2. 模型名称
/// 3. 状态提示文本
/// 4. Token 使用量
/// 5. 连接状态指示
pub struct StatusBar;

impl StatusBar {
    /// 渲染状态栏到指定区域
    ///
    /// 将各项状态信息组合为一行带样式的文本，使用不同的颜色
    /// 和分隔符清晰地区分各个信息段。
    ///
    /// # 参数
    /// - `frame`: ratatui 渲染帧
    /// - `area`: 渲染区域（通常为单行高度）
    /// - `app`: 应用状态引用
    pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App) {
        let mut spans: Vec<Span<'static>> = Vec::new();

        // 1. 模式指示器 - 使用不同颜色和背景标识当前模式
        let (mode_text, mode_color) = Self::mode_indicator(&app.mode);
        spans.push(Span::styled(
            format!(" {mode_text} "),
            Style::default()
                .fg(Color::Black)
                .bg(mode_color)
                .add_modifier(Modifier::BOLD),
        ));

        // 分隔符
        spans.push(Span::styled(
            " │ ".to_string(),
            Style::default().fg(Color::DarkGray),
        ));

        // 2. 模型名称
        spans.push(Span::styled(
            format!("🤖 {}", app.status.model_name),
            Style::default().fg(Color::Cyan),
        ));

        // 分隔符
        spans.push(Span::styled(
            " │ ".to_string(),
            Style::default().fg(Color::DarkGray),
        ));

        // 3. 状态提示文本
        let status_color = if app.has_streaming_message() {
            Color::Yellow // 生成中用黄色
        } else {
            Color::White // 就绪用白色
        };
        spans.push(Span::styled(
            app.status.status_text.clone(),
            Style::default().fg(status_color),
        ));

        // 添加弹性空格填充（使右侧内容靠右对齐）
        // 计算已使用的宽度，用空格填充剩余空间
        let used_width: usize = spans.iter().map(|s| s.content.len()).sum();
        let right_content = Self::build_right_content(app);
        let right_width: usize = right_content.iter().map(|s| s.content.len()).sum();
        let available_width = area.width as usize;

        if used_width + right_width < available_width {
            let padding = available_width - used_width - right_width;
            spans.push(Span::raw(" ".repeat(padding)));
        } else {
            spans.push(Span::raw(" ".to_string()));
        }

        // 添加右侧内容
        spans.extend(right_content);

        let line = Line::from(spans);
        let paragraph =
            Paragraph::new(line).style(Style::default().bg(Color::DarkGray).fg(Color::White));

        frame.render_widget(paragraph, area);
    }

    /// 获取模式指示器的文本和颜色
    ///
    /// 不同模式使用不同的颜色以便快速识别：
    /// - 普通模式：蓝色背景
    /// - 输入模式：绿色背景
    /// - 命令模式：黄色背景
    /// - 帮助模式：品红色背景
    fn mode_indicator(mode: &AppMode) -> (&'static str, Color) {
        match mode {
            AppMode::Normal => ("普通", Color::Blue),
            AppMode::Input => ("输入", Color::Green),
            AppMode::Command => ("命令", Color::Yellow),
            AppMode::Help => ("帮助", Color::Magenta),
        }
    }

    /// 构建状态栏右侧的内容
    ///
    /// 包含 token 使用量和连接状态指示器。
    ///
    /// # 参数
    /// - `app`: 应用状态
    ///
    /// # 返回
    /// - 右侧区域的 Span 列表
    fn build_right_content(app: &App) -> Vec<Span<'static>> {
        let mut spans = Vec::new();

        // Token 使用量
        if app.status.token_count > 0 {
            spans.push(Span::styled(
                format!("📊 {} tokens", app.status.token_count),
                Style::default().fg(Color::White),
            ));

            // 分隔符
            spans.push(Span::styled(
                " │ ".to_string(),
                Style::default().fg(Color::DarkGray),
            ));
        }

        // 连接状态指示器
        let (status_icon, status_color) = if app.status.is_connected {
            ("●", Color::Green) // 已连接：绿色圆点
        } else {
            ("○", Color::Red) // 未连接：红色空心圆
        };

        let status_label = if app.status.is_connected {
            "已连接"
        } else {
            "未连接"
        };

        spans.push(Span::styled(
            format!("{status_icon} {status_label} "),
            Style::default().fg(status_color),
        ));

        spans
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 测试模式指示器颜色() {
        // 验证每种模式都有对应的指示器文本和颜色
        let (text, color) = StatusBar::mode_indicator(&AppMode::Normal);
        assert_eq!(text, "普通");
        assert_eq!(color, Color::Blue);

        let (text, color) = StatusBar::mode_indicator(&AppMode::Input);
        assert_eq!(text, "输入");
        assert_eq!(color, Color::Green);

        let (text, color) = StatusBar::mode_indicator(&AppMode::Command);
        assert_eq!(text, "命令");
        assert_eq!(color, Color::Yellow);

        let (text, color) = StatusBar::mode_indicator(&AppMode::Help);
        assert_eq!(text, "帮助");
        assert_eq!(color, Color::Magenta);
    }

    #[test]
    fn 测试右侧内容_无token() {
        let app = App::new("gpt-4");
        let spans = StatusBar::build_right_content(&app);

        // 没有 token 时不应包含 token 信息
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(!text.contains("tokens"));
        // 应该包含连接状态
        assert!(text.contains("未连接"));
    }

    #[test]
    fn 测试右侧内容_有token() {
        let mut app = App::new("gpt-4");
        app.status.token_count = 1500;

        let spans = StatusBar::build_right_content(&app);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();

        // 应该包含 token 数量
        assert!(text.contains("1500"));
        assert!(text.contains("tokens"));
    }

    #[test]
    fn 测试右侧内容_已连接() {
        let mut app = App::new("gpt-4");
        app.status.is_connected = true;

        let spans = StatusBar::build_right_content(&app);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();

        assert!(text.contains("已连接"));
        assert!(text.contains("●"));
    }

    #[test]
    fn 测试右侧内容_未连接() {
        let app = App::new("gpt-4");
        let spans = StatusBar::build_right_content(&app);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();

        assert!(text.contains("未连接"));
        assert!(text.contains("○"));
    }
}

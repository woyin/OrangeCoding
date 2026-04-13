//! Markdown 渲染模块
//!
//! 本模块提供基础的 Markdown 文本到终端样式文本的转换功能。
//! 支持常见的 Markdown 语法元素，将其转换为 ratatui 的 `Line` 和 `Span`
//! 类型，以便在终端界面中美观地显示富文本内容。
//!
//! # 支持的语法
//!
//! - 标题：`# H1`、`## H2`、`### H3` 等
//! - 粗体：`**粗体文本**`
//! - 代码块：` ```代码``` `
//! - 行内代码：`` `代码` ``
//! - 列表项：`- 列表项`、`* 列表项`
//! - 链接：`[文本](URL)`

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

// ---------------------------------------------------------------------------
// 代码块状态
// ---------------------------------------------------------------------------

/// 代码块解析状态 - 追踪是否处于代码块内部
#[derive(Debug, PartialEq, Eq)]
enum CodeBlockState {
    /// 不在代码块中
    Outside,
    /// 在代码块中，记录语言标识（可为空）
    Inside(String),
}

// ---------------------------------------------------------------------------
// Markdown 渲染器
// ---------------------------------------------------------------------------

/// Markdown 渲染器 - 将 Markdown 文本转换为 ratatui 样式文本
///
/// 提供将 Markdown 源文本解析并转换为带样式的终端文本行的功能。
/// 渲染器是无状态的，每次调用 `render` 方法都独立处理输入文本。
///
/// # 使用示例
///
/// ```rust
/// use chengcoding_tui::markdown::MarkdownRenderer;
///
/// let renderer = MarkdownRenderer::new();
/// let lines = renderer.render("# 标题\n\n这是**粗体**文本");
/// assert!(!lines.is_empty());
/// ```
pub struct MarkdownRenderer {
    /// 代码块的边框样式字符
    code_border_char: char,
    /// 代码块的边框宽度（字符数）
    code_border_width: usize,
}

impl MarkdownRenderer {
    /// 创建一个新的 Markdown 渲染器
    pub fn new() -> Self {
        Self {
            code_border_char: '─',
            code_border_width: 40,
        }
    }

    /// 将 Markdown 文本渲染为 ratatui 的 Line 列表
    ///
    /// 逐行解析输入文本，识别各种 Markdown 语法元素，
    /// 并转换为带有适当样式的终端文本行。
    ///
    /// # 参数
    /// - `text`: Markdown 源文本
    ///
    /// # 返回
    /// - 带样式的 `Line` 列表，可直接用于 ratatui 的 `Paragraph` 等组件
    pub fn render(&self, text: &str) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut code_block_state = CodeBlockState::Outside;

        for raw_line in text.lines() {
            // 检查代码块的开始或结束标记
            if raw_line.trim_start().starts_with("```") {
                match code_block_state {
                    CodeBlockState::Outside => {
                        // 进入代码块，提取语言标识
                        let lang = raw_line
                            .trim_start()
                            .trim_start_matches('`')
                            .trim()
                            .to_string();
                        code_block_state = CodeBlockState::Inside(lang.clone());

                        // 渲染代码块起始边框
                        let border = self
                            .code_border_char
                            .to_string()
                            .repeat(self.code_border_width);
                        let mut spans =
                            vec![Span::styled(border, Style::default().fg(Color::DarkGray))];

                        // 如果有语言标识，显示在边框后
                        if !lang.is_empty() {
                            spans.push(Span::styled(
                                format!(" {lang} "),
                                Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::ITALIC),
                            ));
                        }
                        lines.push(Line::from(spans));
                    }
                    CodeBlockState::Inside(_) => {
                        // 退出代码块，渲染结束边框
                        let border = self
                            .code_border_char
                            .to_string()
                            .repeat(self.code_border_width);
                        lines.push(Line::from(Span::styled(
                            border,
                            Style::default().fg(Color::DarkGray),
                        )));
                        code_block_state = CodeBlockState::Outside;
                    }
                }
                continue;
            }

            // 在代码块内部，所有行都使用代码样式
            if let CodeBlockState::Inside(_) = &code_block_state {
                lines.push(Line::from(Span::styled(
                    format!("  {raw_line}"),
                    Style::default().fg(Color::Green),
                )));
                continue;
            }

            // 解析标题
            if let Some(header_line) = self.parse_header(raw_line) {
                lines.push(header_line);
                continue;
            }

            // 解析无序列表项
            if let Some(list_line) = self.parse_list_item(raw_line) {
                lines.push(list_line);
                continue;
            }

            // 解析水平分割线
            let trimmed = raw_line.trim();
            if (trimmed.starts_with("---")
                || trimmed.starts_with("***")
                || trimmed.starts_with("___"))
                && trimmed
                    .chars()
                    .all(|c| c == '-' || c == '*' || c == '_' || c == ' ')
                && trimmed.chars().filter(|c| !c.is_whitespace()).count() >= 3
            {
                let divider = "─".repeat(self.code_border_width);
                lines.push(Line::from(Span::styled(
                    divider,
                    Style::default().fg(Color::DarkGray),
                )));
                continue;
            }

            // 空行保留为空行
            if raw_line.trim().is_empty() {
                lines.push(Line::from(""));
                continue;
            }

            // 普通文本行 - 解析行内格式
            let spans = Self::parse_inline(raw_line);
            lines.push(Line::from(spans));
        }

        // 如果文本以未关闭的代码块结束，添加结束边框
        if let CodeBlockState::Inside(_) = code_block_state {
            let border = self
                .code_border_char
                .to_string()
                .repeat(self.code_border_width);
            lines.push(Line::from(Span::styled(
                border,
                Style::default().fg(Color::DarkGray),
            )));
        }

        // 确保至少返回一行
        if lines.is_empty() {
            lines.push(Line::from(""));
        }

        lines
    }

    /// 解析标题行
    ///
    /// 识别 Markdown 标题语法（# 到 ######），根据级别设置不同的颜色和样式。
    ///
    /// # 返回
    /// - `Some(Line)`: 成功解析为标题行
    /// - `None`: 不是标题语法
    fn parse_header(&self, line: &str) -> Option<Line<'static>> {
        let trimmed = line.trim_start();

        // 计算 # 的数量（1-6 级标题）
        let level = trimmed.chars().take_while(|&c| c == '#').count();
        if level == 0 || level > 6 {
            return None;
        }

        // # 后面必须跟空格
        let rest = &trimmed[level..];
        if !rest.starts_with(' ') {
            return None;
        }

        let content = rest.trim().to_string();

        // 根据标题级别选择样式
        let (color, bold) = match level {
            1 => (Color::Magenta, true), // H1: 品红色加粗
            2 => (Color::Cyan, true),    // H2: 青色加粗
            3 => (Color::Blue, true),    // H3: 蓝色加粗
            4 => (Color::Yellow, false), // H4: 黄色
            5 => (Color::Green, false),  // H5: 绿色
            _ => (Color::White, false),  // H6: 白色
        };

        let mut style = Style::default().fg(color);
        if bold {
            style = style.add_modifier(Modifier::BOLD);
        }

        // 构建标题前缀（使用 █ 标记）
        let prefix = "█ ".to_string();
        Some(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(content, style),
        ]))
    }

    /// 解析无序列表项
    ///
    /// 识别以 `- ` 或 `* ` 开头的列表项，转换为带有圆点前缀的样式行。
    ///
    /// # 返回
    /// - `Some(Line)`: 成功解析为列表项
    /// - `None`: 不是列表项语法
    fn parse_list_item(&self, line: &str) -> Option<Line<'static>> {
        let trimmed = line.trim_start();
        // 计算缩进级别
        let indent = line.len() - trimmed.len();

        if !(trimmed.starts_with("- ") || trimmed.starts_with("* ")) {
            return None;
        }

        let content = &trimmed[2..];
        let indent_str = " ".repeat(indent);

        // 解析列表项内容中的行内格式
        let mut spans = vec![
            Span::raw(indent_str),
            Span::styled("  • ", Style::default().fg(Color::Cyan)),
        ];
        spans.extend(Self::parse_inline(content));

        Some(Line::from(spans))
    }

    /// 解析行内格式元素
    ///
    /// 扫描文本中的行内 Markdown 语法，包括：
    /// - `**粗体**`：加粗样式
    /// - `` `代码` ``：行内代码高亮
    /// - `[文本](URL)`：链接（带下划线和蓝色）
    ///
    /// 返回带样式的 Span 列表。
    fn parse_inline(text: &str) -> Vec<Span<'static>> {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut current = String::new();
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            // 解析粗体：**文本**
            if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
                // 将之前积累的普通文本作为 Span 推入
                if !current.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut current)));
                }

                i += 2; // 跳过开始的 **
                let mut bold_text = String::new();

                // 查找结束的 **
                while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '*') {
                    bold_text.push(chars[i]);
                    i += 1;
                }

                // 跳过结束的 **（如果找到的话）
                if i + 1 < len {
                    i += 2;
                }

                spans.push(Span::styled(
                    bold_text,
                    Style::default().add_modifier(Modifier::BOLD),
                ));
                continue;
            }

            // 解析行内代码：`代码`
            if chars[i] == '`' {
                if !current.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut current)));
                }

                i += 1; // 跳过开始的 `
                let mut code_text = String::new();

                // 查找结束的 `
                while i < len && chars[i] != '`' {
                    code_text.push(chars[i]);
                    i += 1;
                }

                // 跳过结束的 `（如果找到的话）
                if i < len {
                    i += 1;
                }

                spans.push(Span::styled(
                    code_text,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
                continue;
            }

            // 解析链接：[文本](URL)
            if chars[i] == '[' {
                // 尝试解析完整的链接语法
                if let Some((link_span, consumed)) = Self::try_parse_link(&chars, i) {
                    if !current.is_empty() {
                        spans.push(Span::raw(std::mem::take(&mut current)));
                    }
                    spans.push(link_span);
                    i += consumed;
                    continue;
                }
            }

            // 普通字符，添加到当前文本缓冲区
            current.push(chars[i]);
            i += 1;
        }

        // 将剩余的普通文本推入
        if !current.is_empty() {
            spans.push(Span::raw(current));
        }

        // 确保至少有一个空 Span（避免空行问题）
        if spans.is_empty() {
            spans.push(Span::raw(String::new()));
        }

        spans
    }

    /// 尝试解析链接语法
    ///
    /// 从给定位置开始尝试解析 `[文本](URL)` 格式的链接。
    ///
    /// # 参数
    /// - `chars`: 整行的字符数组
    /// - `start`: 起始位置（`[` 的位置）
    ///
    /// # 返回
    /// - `Some((Span, consumed))`: 成功解析，返回样式化的 Span 和消耗的字符数
    /// - `None`: 不是有效的链接语法
    fn try_parse_link(chars: &[char], start: usize) -> Option<(Span<'static>, usize)> {
        let len = chars.len();
        let mut i = start + 1; // 跳过 [

        // 提取链接文本
        let mut link_text = String::new();
        while i < len && chars[i] != ']' {
            link_text.push(chars[i]);
            i += 1;
        }

        // 必须找到 ]
        if i >= len {
            return None;
        }
        i += 1; // 跳过 ]

        // 必须紧跟 (
        if i >= len || chars[i] != '(' {
            return None;
        }
        i += 1; // 跳过 (

        // 提取 URL
        let mut _url = String::new();
        while i < len && chars[i] != ')' {
            _url.push(chars[i]);
            i += 1;
        }

        // 必须找到 )
        if i >= len {
            return None;
        }
        i += 1; // 跳过 )

        // 链接样式：蓝色、下划线
        let span = Span::styled(
            link_text,
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
        );

        let consumed = i - start;
        Some((span, consumed))
    }
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助函数：提取 Line 中所有 Span 的纯文本内容
    fn line_text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn 测试空文本渲染() {
        let renderer = MarkdownRenderer::new();
        let lines = renderer.render("");
        // 空文本应该返回至少一行
        assert!(!lines.is_empty());
    }

    #[test]
    fn 测试纯文本渲染() {
        let renderer = MarkdownRenderer::new();
        let lines = renderer.render("这是一段普通文本");
        assert_eq!(lines.len(), 1);
        assert_eq!(line_text(&lines[0]), "这是一段普通文本");
    }

    #[test]
    fn 测试标题渲染() {
        let renderer = MarkdownRenderer::new();

        // H1 标题
        let lines = renderer.render("# 一级标题");
        assert_eq!(lines.len(), 1);
        let text = line_text(&lines[0]);
        assert!(text.contains("一级标题"));

        // H2 标题
        let lines = renderer.render("## 二级标题");
        assert_eq!(lines.len(), 1);
        let text = line_text(&lines[0]);
        assert!(text.contains("二级标题"));
    }

    #[test]
    fn 测试非标题行不误判() {
        let renderer = MarkdownRenderer::new();

        // #后面没有空格不应被识别为标题
        let lines = renderer.render("#不是标题");
        let text = line_text(&lines[0]);
        assert_eq!(text, "#不是标题");
    }

    #[test]
    fn 测试粗体渲染() {
        let renderer = MarkdownRenderer::new();
        let lines = renderer.render("这是**粗体**文本");
        assert_eq!(lines.len(), 1);

        // 应该有三个 Span：普通、粗体、普通
        assert_eq!(lines[0].spans.len(), 3);
        assert_eq!(lines[0].spans[0].content.as_ref(), "这是");
        assert_eq!(lines[0].spans[1].content.as_ref(), "粗体");
        assert_eq!(lines[0].spans[2].content.as_ref(), "文本");

        // 验证粗体样式
        assert!(lines[0].spans[1]
            .style
            .add_modifier
            .contains(Modifier::BOLD));
    }

    #[test]
    fn 测试行内代码渲染() {
        let renderer = MarkdownRenderer::new();
        let lines = renderer.render("使用 `println!` 宏");
        assert_eq!(lines.len(), 1);

        // 应该有三个 Span：普通、代码、普通
        assert_eq!(lines[0].spans.len(), 3);
        assert_eq!(lines[0].spans[1].content.as_ref(), "println!");

        // 验证代码样式（黄色）
        assert_eq!(lines[0].spans[1].style.fg, Some(Color::Yellow));
    }

    #[test]
    fn 测试代码块渲染() {
        let renderer = MarkdownRenderer::new();
        let input = "```rust\nfn main() {}\n```";
        let lines = renderer.render(input);

        // 应该有三行：边框+语言、代码、边框
        assert_eq!(lines.len(), 3);

        // 代码行应该有缩进和绿色
        let code_text = line_text(&lines[1]);
        assert!(code_text.contains("fn main() {}"));
        assert_eq!(lines[1].spans[0].style.fg, Some(Color::Green));
    }

    #[test]
    fn 测试列表项渲染() {
        let renderer = MarkdownRenderer::new();
        let lines = renderer.render("- 第一项\n- 第二项");
        assert_eq!(lines.len(), 2);

        // 每个列表项应该包含圆点符号
        let text1 = line_text(&lines[0]);
        assert!(text1.contains("•"));
        assert!(text1.contains("第一项"));
    }

    #[test]
    fn 测试链接渲染() {
        let renderer = MarkdownRenderer::new();
        let lines = renderer.render("访问 [GitHub](https://github.com) 网站");
        assert_eq!(lines.len(), 1);

        // 查找链接 Span
        let link_span = lines[0]
            .spans
            .iter()
            .find(|s| s.content.as_ref() == "GitHub");
        assert!(link_span.is_some());

        // 验证链接样式（蓝色、下划线）
        let link = link_span.unwrap();
        assert_eq!(link.style.fg, Some(Color::Blue));
        assert!(link.style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn 测试混合格式渲染() {
        let renderer = MarkdownRenderer::new();
        let input = "# 标题\n\n这是**粗体**和`代码`\n\n- 列表项";
        let lines = renderer.render(input);

        // 标题 + 空行 + 混合行 + 空行 + 列表项
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn 测试未关闭的代码块() {
        let renderer = MarkdownRenderer::new();
        let input = "```\n未关闭的代码块";
        let lines = renderer.render(input);

        // 应该自动添加结束边框
        assert!(lines.len() >= 3); // 开始边框 + 代码 + 自动结束边框
    }

    #[test]
    fn 测试水平分割线() {
        let renderer = MarkdownRenderer::new();
        let lines = renderer.render("---");
        assert_eq!(lines.len(), 1);
        let text = line_text(&lines[0]);
        assert!(text.contains('─'));
    }

    #[test]
    fn 测试多行文本() {
        let renderer = MarkdownRenderer::new();
        let input = "第一行\n第二行\n第三行";
        let lines = renderer.render(input);
        assert_eq!(lines.len(), 3);
        assert_eq!(line_text(&lines[0]), "第一行");
        assert_eq!(line_text(&lines[1]), "第二行");
        assert_eq!(line_text(&lines[2]), "第三行");
    }
}

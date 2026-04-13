//! 主题系统模块
//!
//! 提供终端 UI 的主题支持，包括暗色和亮色主题、多种颜色模式
//! （TrueColor / 256色 / 16色）以及颜色降级功能。

use ratatui::style::{Color, Modifier, Style};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// 主题变体
// ---------------------------------------------------------------------------

/// 主题变体 — 暗色或亮色
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeVariant {
    /// 暗色主题
    Dark,
    /// 亮色主题
    Light,
}

// ---------------------------------------------------------------------------
// 颜色模式
// ---------------------------------------------------------------------------

/// 颜色模式 — 终端支持的色彩深度
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColorMode {
    /// 24 位真彩色
    TrueColor,
    /// 256 色模式
    Color256,
    /// 16 色基础模式
    Color16,
}

// ---------------------------------------------------------------------------
// 主题颜色定义
// ---------------------------------------------------------------------------

/// 主题颜色定义 — 包含所有 UI 元素的颜色映射
#[derive(Clone, Debug)]
pub struct ThemeColors {
    // 基础颜色
    /// 主色调
    pub primary: Color,
    /// 辅助色
    pub secondary: Color,
    /// 强调色
    pub accent: Color,
    /// 背景色
    pub background: Color,
    /// 前景色（默认文本颜色）
    pub foreground: Color,
    /// 表面色（卡片、面板背景）
    pub surface: Color,
    /// 边框色
    pub border: Color,

    // 消息颜色
    /// 用户消息颜色
    pub user_message: Color,
    /// 助手消息颜色
    pub assistant_message: Color,
    /// 系统消息颜色
    pub system_message: Color,
    /// 错误消息颜色
    pub error_message: Color,
    /// 警告消息颜色
    pub warning_message: Color,
    /// 信息提示颜色
    pub info_message: Color,

    // 代码高亮
    /// 代码块背景色
    pub code_background: Color,
    /// 关键字颜色
    pub code_keyword: Color,
    /// 字符串颜色
    pub code_string: Color,
    /// 注释颜色
    pub code_comment: Color,
    /// 函数名颜色
    pub code_function: Color,
    /// 数字颜色
    pub code_number: Color,
    /// 运算符颜色
    pub code_operator: Color,
    /// 类型名颜色
    pub code_type: Color,

    // 状态颜色
    /// 成功状态
    pub status_success: Color,
    /// 错误状态
    pub status_error: Color,
    /// 警告状态
    pub status_warning: Color,
    /// 信息状态
    pub status_info: Color,
    /// 等待/处理中状态
    pub status_pending: Color,

    // UI 元素
    /// 输入框边框
    pub input_border: Color,
    /// 输入光标
    pub input_cursor: Color,
    /// 选中文本背景
    pub selection: Color,
    /// 滚动条颜色
    pub scrollbar: Color,
    /// 分割线颜色
    pub divider: Color,
    /// 徽章背景色
    pub badge_bg: Color,
    /// 徽章前景色
    pub badge_fg: Color,
    /// 链接颜色
    pub link: Color,
    /// 暗淡文本颜色
    pub muted: Color,
}

// ---------------------------------------------------------------------------
// 主题
// ---------------------------------------------------------------------------

/// 主题系统 — 支持多种颜色模式和暗/亮主题切换
pub struct Theme {
    /// 主题名称
    pub name: String,
    /// 主题变体（暗色/亮色）
    pub variant: ThemeVariant,
    /// 颜色定义
    pub colors: ThemeColors,
    /// 样式缓存 — 按名称索引的预计算样式
    styles: HashMap<String, Style>,
}

impl Theme {
    /// 创建默认暗色主题
    ///
    /// 采用深蓝/紫色基调，配合明亮的强调色，适合长时间在暗色终端中使用。
    pub fn dark() -> Self {
        let colors = ThemeColors {
            // 基础颜色 — 深色背景、明亮前景
            primary: Color::Rgb(100, 149, 237),    // 矢车菊蓝
            secondary: Color::Rgb(138, 112, 198),  // 紫色
            accent: Color::Rgb(255, 179, 71),      // 琥珀色
            background: Color::Rgb(22, 22, 30),    // 深蓝黑
            foreground: Color::Rgb(220, 220, 230), // 浅灰白
            surface: Color::Rgb(35, 35, 48),       // 深灰蓝
            border: Color::Rgb(68, 68, 90),        // 暗灰蓝

            // 消息颜色
            user_message: Color::Rgb(100, 149, 237),
            assistant_message: Color::Rgb(80, 200, 120),
            system_message: Color::Rgb(255, 215, 0),
            error_message: Color::Rgb(255, 85, 85),
            warning_message: Color::Rgb(255, 179, 71),
            info_message: Color::Rgb(100, 149, 237),

            // 代码高亮 — 参考常见暗色主题配色
            code_background: Color::Rgb(30, 30, 40),
            code_keyword: Color::Rgb(198, 120, 221),
            code_string: Color::Rgb(152, 195, 121),
            code_comment: Color::Rgb(92, 99, 112),
            code_function: Color::Rgb(97, 175, 239),
            code_number: Color::Rgb(209, 154, 102),
            code_operator: Color::Rgb(86, 182, 194),
            code_type: Color::Rgb(229, 192, 123),

            // 状态颜色
            status_success: Color::Rgb(80, 200, 120),
            status_error: Color::Rgb(255, 85, 85),
            status_warning: Color::Rgb(255, 179, 71),
            status_info: Color::Rgb(100, 149, 237),
            status_pending: Color::Rgb(180, 180, 180),

            // UI 元素
            input_border: Color::Rgb(100, 149, 237),
            input_cursor: Color::Rgb(255, 255, 255),
            selection: Color::Rgb(55, 55, 80),
            scrollbar: Color::Rgb(80, 80, 100),
            divider: Color::Rgb(50, 50, 65),
            badge_bg: Color::Rgb(100, 149, 237),
            badge_fg: Color::Rgb(255, 255, 255),
            link: Color::Rgb(100, 149, 237),
            muted: Color::Rgb(110, 110, 130),
        };

        let mut theme = Self {
            name: "dark".to_string(),
            variant: ThemeVariant::Dark,
            colors: colors.clone(),
            styles: HashMap::new(),
        };
        theme.build_styles();
        theme
    }

    /// 创建默认亮色主题
    ///
    /// 采用干净的白色背景配合深色文本，适合明亮环境中使用。
    pub fn light() -> Self {
        let colors = ThemeColors {
            // 基础颜色 — 明亮背景、深色前景
            primary: Color::Rgb(30, 102, 200),
            secondary: Color::Rgb(110, 70, 170),
            accent: Color::Rgb(200, 120, 0),
            background: Color::Rgb(250, 250, 252), // 近白色
            foreground: Color::Rgb(30, 30, 40),    // 深灰黑
            surface: Color::Rgb(240, 240, 245),
            border: Color::Rgb(200, 200, 210),

            // 消息颜色
            user_message: Color::Rgb(30, 102, 200),
            assistant_message: Color::Rgb(20, 150, 60),
            system_message: Color::Rgb(180, 140, 0),
            error_message: Color::Rgb(200, 40, 40),
            warning_message: Color::Rgb(200, 120, 0),
            info_message: Color::Rgb(30, 102, 200),

            // 代码高亮 — 亮色主题配色
            code_background: Color::Rgb(245, 245, 248),
            code_keyword: Color::Rgb(150, 60, 180),
            code_string: Color::Rgb(60, 130, 60),
            code_comment: Color::Rgb(140, 140, 160),
            code_function: Color::Rgb(30, 102, 200),
            code_number: Color::Rgb(180, 100, 20),
            code_operator: Color::Rgb(40, 140, 150),
            code_type: Color::Rgb(170, 120, 20),

            // 状态颜色
            status_success: Color::Rgb(20, 150, 60),
            status_error: Color::Rgb(200, 40, 40),
            status_warning: Color::Rgb(200, 120, 0),
            status_info: Color::Rgb(30, 102, 200),
            status_pending: Color::Rgb(140, 140, 150),

            // UI 元素
            input_border: Color::Rgb(30, 102, 200),
            input_cursor: Color::Rgb(30, 30, 40),
            selection: Color::Rgb(200, 220, 255),
            scrollbar: Color::Rgb(180, 180, 195),
            divider: Color::Rgb(210, 210, 220),
            badge_bg: Color::Rgb(30, 102, 200),
            badge_fg: Color::Rgb(255, 255, 255),
            link: Color::Rgb(30, 102, 200),
            muted: Color::Rgb(140, 140, 160),
        };

        let mut theme = Self {
            name: "light".to_string(),
            variant: ThemeVariant::Light,
            colors: colors.clone(),
            styles: HashMap::new(),
        };
        theme.build_styles();
        theme
    }

    /// 自动检测终端颜色模式
    ///
    /// 通过环境变量 `COLORTERM` 判断终端是否支持真彩色，
    /// 或通过 `TERM` 判断 256 色支持。
    pub fn detect_color_mode() -> ColorMode {
        // 检查 COLORTERM 环境变量（truecolor / 24bit）
        if let Ok(val) = std::env::var("COLORTERM") {
            let val_lower = val.to_lowercase();
            if val_lower == "truecolor" || val_lower == "24bit" {
                return ColorMode::TrueColor;
            }
        }

        // 检查 TERM 环境变量是否包含 256color
        if let Ok(val) = std::env::var("TERM") {
            if val.contains("256color") {
                return ColorMode::Color256;
            }
        }

        // 默认为 16 色
        ColorMode::Color16
    }

    /// 自动检测暗/亮模式
    ///
    /// 尝试通过 `COLORFGBG` 环境变量判断终端背景色调。
    /// 如果无法检测，默认返回暗色模式。
    pub fn detect_variant() -> ThemeVariant {
        // COLORFGBG 格式: "foreground;background"，背景值较大表示亮色
        if let Ok(val) = std::env::var("COLORFGBG") {
            if let Some(bg_str) = val.split(';').last() {
                if let Ok(bg) = bg_str.trim().parse::<u8>() {
                    // 背景索引 >= 8 通常表示亮色背景
                    if bg >= 8 {
                        return ThemeVariant::Light;
                    }
                }
            }
        }
        // 默认暗色模式
        ThemeVariant::Dark
    }

    /// 获取指定名称的样式
    ///
    /// 支持的名称包括：
    /// - `"primary"`, `"secondary"`, `"accent"` — 基础样式
    /// - `"user_message"`, `"assistant_message"` 等 — 消息样式
    /// - `"code_keyword"`, `"code_string"` 等 — 代码高亮样式
    ///
    /// 如果名称不存在，返回默认样式。
    pub fn style_for(&self, name: &str) -> Style {
        self.styles
            .get(name)
            .copied()
            .unwrap_or_else(Style::default)
    }

    /// 降级颜色到 256 色
    ///
    /// 将 RGB 颜色映射到最近的 256 色索引。
    /// 如果输入已是非 RGB 颜色，原样返回。
    pub fn downgrade_to_256(color: Color) -> Color {
        match color {
            Color::Rgb(r, g, b) => {
                // 使用 6x6x6 色彩立方体（索引 16-231）
                let ri = Self::rgb_to_ansi_index(r);
                let gi = Self::rgb_to_ansi_index(g);
                let bi = Self::rgb_to_ansi_index(b);
                let index = 16 + 36 * ri + 6 * gi + bi;
                Color::Indexed(index)
            }
            other => other,
        }
    }

    /// 降级颜色到 16 色
    ///
    /// 将 RGB 颜色映射到最近的 16 色基础色。
    /// 如果输入已是基础色，原样返回。
    pub fn downgrade_to_16(color: Color) -> Color {
        match color {
            Color::Rgb(r, g, b) => Self::rgb_to_base16(r, g, b),
            Color::Indexed(idx) => {
                // 256 色索引降级到 16 色
                if idx < 16 {
                    Color::Indexed(idx)
                } else {
                    // 将索引转回 RGB 再映射
                    let (r, g, b) = Self::indexed_to_rgb(idx);
                    Self::rgb_to_base16(r, g, b)
                }
            }
            other => other,
        }
    }

    // -----------------------------------------------------------------------
    // 私有辅助方法
    // -----------------------------------------------------------------------

    /// 构建所有命名样式的缓存
    fn build_styles(&mut self) {
        let c = &self.colors;
        let mut m = HashMap::new();

        // 基础样式
        m.insert("primary".into(), Style::default().fg(c.primary));
        m.insert("secondary".into(), Style::default().fg(c.secondary));
        m.insert("accent".into(), Style::default().fg(c.accent));
        m.insert("background".into(), Style::default().bg(c.background));
        m.insert("foreground".into(), Style::default().fg(c.foreground));
        m.insert("surface".into(), Style::default().bg(c.surface));
        m.insert("border".into(), Style::default().fg(c.border));

        // 消息样式
        m.insert("user_message".into(), Style::default().fg(c.user_message));
        m.insert(
            "assistant_message".into(),
            Style::default().fg(c.assistant_message),
        );
        m.insert(
            "system_message".into(),
            Style::default().fg(c.system_message),
        );
        m.insert("error_message".into(), Style::default().fg(c.error_message));
        m.insert(
            "warning_message".into(),
            Style::default().fg(c.warning_message),
        );
        m.insert("info_message".into(), Style::default().fg(c.info_message));

        // 代码高亮样式
        m.insert(
            "code_background".into(),
            Style::default().bg(c.code_background),
        );
        m.insert("code_keyword".into(), Style::default().fg(c.code_keyword));
        m.insert("code_string".into(), Style::default().fg(c.code_string));
        m.insert(
            "code_comment".into(),
            Style::default()
                .fg(c.code_comment)
                .add_modifier(Modifier::ITALIC),
        );
        m.insert("code_function".into(), Style::default().fg(c.code_function));
        m.insert("code_number".into(), Style::default().fg(c.code_number));
        m.insert("code_operator".into(), Style::default().fg(c.code_operator));
        m.insert("code_type".into(), Style::default().fg(c.code_type));

        // 状态样式
        m.insert(
            "status_success".into(),
            Style::default().fg(c.status_success),
        );
        m.insert("status_error".into(), Style::default().fg(c.status_error));
        m.insert(
            "status_warning".into(),
            Style::default().fg(c.status_warning),
        );
        m.insert("status_info".into(), Style::default().fg(c.status_info));
        m.insert(
            "status_pending".into(),
            Style::default().fg(c.status_pending),
        );

        // UI 元素样式
        m.insert("input_border".into(), Style::default().fg(c.input_border));
        m.insert("input_cursor".into(), Style::default().fg(c.input_cursor));
        m.insert("selection".into(), Style::default().bg(c.selection));
        m.insert("scrollbar".into(), Style::default().fg(c.scrollbar));
        m.insert("divider".into(), Style::default().fg(c.divider));
        m.insert(
            "badge".into(),
            Style::default().fg(c.badge_fg).bg(c.badge_bg),
        );
        m.insert(
            "link".into(),
            Style::default()
                .fg(c.link)
                .add_modifier(Modifier::UNDERLINED),
        );
        m.insert("muted".into(), Style::default().fg(c.muted));

        self.styles = m;
    }

    /// 将 0-255 的 RGB 分量映射到 6x6x6 色彩立方体索引（0-5）
    fn rgb_to_ansi_index(v: u8) -> u8 {
        // 色彩立方体的阶梯值: 0, 95, 135, 175, 215, 255
        match v {
            0..=47 => 0,
            48..=114 => 1,
            115..=154 => 2,
            155..=194 => 3,
            195..=234 => 4,
            235..=255 => 5,
        }
    }

    /// 将 RGB 映射到 16 色基础色
    fn rgb_to_base16(r: u8, g: u8, b: u8) -> Color {
        let brightness = (r as u16 + g as u16 + b as u16) / 3;
        let is_bright = brightness > 128;

        // 判断主色调
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);

        // 接近灰色
        if max - min < 30 {
            return if brightness > 200 {
                Color::White
            } else if brightness > 100 {
                Color::Gray
            } else if brightness > 50 {
                Color::DarkGray
            } else {
                Color::Black
            };
        }

        // 有明显色相
        if r >= g && r >= b {
            // 红色为主
            if g > b + 30 {
                if is_bright {
                    Color::LightYellow
                } else {
                    Color::Yellow
                }
            } else {
                if is_bright {
                    Color::LightRed
                } else {
                    Color::Red
                }
            }
        } else if g >= r && g >= b {
            // 绿色为主
            if b > r + 30 {
                if is_bright {
                    Color::LightCyan
                } else {
                    Color::Cyan
                }
            } else {
                if is_bright {
                    Color::LightGreen
                } else {
                    Color::Green
                }
            }
        } else {
            // 蓝色为主
            if r > g + 30 {
                if is_bright {
                    Color::LightMagenta
                } else {
                    Color::Magenta
                }
            } else {
                if is_bright {
                    Color::LightBlue
                } else {
                    Color::Blue
                }
            }
        }
    }

    /// 将 256 色索引转为近似 RGB 值
    fn indexed_to_rgb(idx: u8) -> (u8, u8, u8) {
        if idx < 16 {
            // 基础 16 色的近似 RGB
            match idx {
                0 => (0, 0, 0),
                1 => (128, 0, 0),
                2 => (0, 128, 0),
                3 => (128, 128, 0),
                4 => (0, 0, 128),
                5 => (128, 0, 128),
                6 => (0, 128, 128),
                7 => (192, 192, 192),
                8 => (128, 128, 128),
                9 => (255, 0, 0),
                10 => (0, 255, 0),
                11 => (255, 255, 0),
                12 => (0, 0, 255),
                13 => (255, 0, 255),
                14 => (0, 255, 255),
                15 => (255, 255, 255),
                _ => unreachable!(),
            }
        } else if idx < 232 {
            // 6x6x6 色彩立方体
            let idx = idx - 16;
            let levels = [0u8, 95, 135, 175, 215, 255];
            let r = levels[(idx / 36) as usize];
            let g = levels[((idx % 36) / 6) as usize];
            let b = levels[(idx % 6) as usize];
            (r, g, b)
        } else {
            // 灰阶 (232-255) -> 24 级灰度
            let gray = 8 + 10 * (idx - 232);
            (gray, gray, gray)
        }
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_theme_creation() {
        // 验证暗色主题能正常创建并设置正确的元数据
        let theme = Theme::dark();
        assert_eq!(theme.name, "dark");
        assert_eq!(theme.variant, ThemeVariant::Dark);
    }

    #[test]
    fn test_light_theme_creation() {
        // 验证亮色主题能正常创建并设置正确的元数据
        let theme = Theme::light();
        assert_eq!(theme.name, "light");
        assert_eq!(theme.variant, ThemeVariant::Light);
    }

    #[test]
    fn test_color_mode_detection() {
        // 验证颜色模式检测返回有效枚举值
        let mode = Theme::detect_color_mode();
        // 只验证返回值是有效的枚举变体（不依赖具体环境）
        assert!(matches!(
            mode,
            ColorMode::TrueColor | ColorMode::Color256 | ColorMode::Color16
        ));
    }

    #[test]
    fn test_style_for_known_name() {
        // 已知样式名称应返回非默认样式
        let theme = Theme::dark();

        let primary_style = theme.style_for("primary");
        assert_ne!(primary_style, Style::default());

        let user_msg_style = theme.style_for("user_message");
        assert_ne!(user_msg_style, Style::default());

        let code_kw_style = theme.style_for("code_keyword");
        assert_ne!(code_kw_style, Style::default());
    }

    #[test]
    fn test_style_for_unknown_name() {
        // 未知名称应返回默认样式
        let theme = Theme::dark();
        let style = theme.style_for("不存在的名称");
        assert_eq!(style, Style::default());
    }

    #[test]
    fn test_downgrade_to_256() {
        // RGB 颜色应被降级为 Indexed 颜色
        let color = Color::Rgb(100, 149, 237);
        let downgraded = Theme::downgrade_to_256(color);
        match downgraded {
            Color::Indexed(_) => {} // 预期结果
            other => panic!("应为 Indexed 颜色，实际为 {:?}", other),
        }

        // 非 RGB 颜色应原样返回
        let base = Color::Red;
        assert_eq!(Theme::downgrade_to_256(base), Color::Red);
    }

    #[test]
    fn test_downgrade_to_16() {
        // RGB 颜色应被降级为 16 色基础色
        let color = Color::Rgb(255, 0, 0);
        let downgraded = Theme::downgrade_to_16(color);
        // 纯红色应映射到 Red 或 LightRed
        assert!(
            matches!(downgraded, Color::Red | Color::LightRed),
            "纯红色应降级为 Red 或 LightRed，实际为 {:?}",
            downgraded
        );

        // 非 RGB 颜色应原样返回
        assert_eq!(Theme::downgrade_to_16(Color::Green), Color::Green);
    }

    #[test]
    fn test_theme_colors_distinct() {
        // 关键颜色应当彼此不同，确保视觉区分度
        let theme = Theme::dark();
        let c = &theme.colors;

        // 背景与前景必须不同
        assert_ne!(format!("{:?}", c.background), format!("{:?}", c.foreground));

        // 不同消息类型应有不同颜色
        assert_ne!(
            format!("{:?}", c.user_message),
            format!("{:?}", c.assistant_message)
        );
        assert_ne!(
            format!("{:?}", c.user_message),
            format!("{:?}", c.system_message)
        );
        assert_ne!(
            format!("{:?}", c.assistant_message),
            format!("{:?}", c.error_message)
        );

        // 状态颜色应彼此不同
        assert_ne!(
            format!("{:?}", c.status_success),
            format!("{:?}", c.status_error)
        );
        assert_ne!(
            format!("{:?}", c.status_error),
            format!("{:?}", c.status_warning)
        );
    }

    #[test]
    fn test_dark_has_dark_background() {
        // 暗色主题的背景色应该较暗（RGB 分量平均值 < 80）
        let theme = Theme::dark();
        match theme.colors.background {
            Color::Rgb(r, g, b) => {
                let avg = (r as u16 + g as u16 + b as u16) / 3;
                assert!(avg < 80, "暗色主题背景色平均亮度应 < 80，实际为 {avg}");
            }
            _ => panic!("暗色主题背景色应为 RGB 颜色"),
        }
    }

    #[test]
    fn test_light_has_light_background() {
        // 亮色主题的背景色应该较亮（RGB 分量平均值 > 200）
        let theme = Theme::light();
        match theme.colors.background {
            Color::Rgb(r, g, b) => {
                let avg = (r as u16 + g as u16 + b as u16) / 3;
                assert!(avg > 200, "亮色主题背景色平均亮度应 > 200，实际为 {avg}");
            }
            _ => panic!("亮色主题背景色应为 RGB 颜色"),
        }
    }
}

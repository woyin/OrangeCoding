//! ChengCoding 终端用户界面库
//!
//! `chengcoding-tui` 提供基于 ratatui 的终端用户界面，用于与 ChengCoding AI 编程助手进行交互。
//!
//! # 模块结构
//!
//! - [`app`] - 应用状态管理（消息、输入、模式切换）
//! - [`markdown`] - Markdown 文本渲染（转换为终端样式）
//! - [`components`] - UI 组件（会话视图、输入框、状态栏）
//!
//! # 使用示例
//!
//! ```rust,no_run
//! use chengcoding_tui::app::{App, AppMode};
//!
//! // 创建应用实例
//! let app = App::new("gpt-4");
//! assert!(app.is_running);
//! ```

/// 应用状态模块 - 管理 TUI 应用的核心状态和键盘事件处理
pub mod app;

/// Markdown 渲染模块 - 将 Markdown 文本转换为带样式的终端文本
pub mod markdown;

/// 主题系统模块 - 颜色主题、颜色模式检测和颜色降级
pub mod theme;

/// UI 组件模块 - 会话视图、输入框、状态栏等界面组件
pub mod components;

// ---------------------------------------------------------------------------
// 便捷的重导出 - 让常用类型可以直接从 crate 根引用
// ---------------------------------------------------------------------------

/// 重导出应用核心类型
pub use app::{
    App, AppAction, AppMode, ContextUsage, DisplayMessage, InputState, InteractionMode,
    McpConnectionState, McpServerStatus, ModifiedFileEntry, SidebarPanel, SidebarState, StatusInfo,
    ThinkingDepth,
};

/// 重导出 Markdown 渲染器
pub use markdown::MarkdownRenderer;

/// 重导出主题系统类型
pub use theme::{ColorMode, Theme, ThemeColors, ThemeVariant};

//! UI 组件模块
//!
//! 本模块包含 TUI 应用的所有可视化组件，每个组件负责
//! 渲染界面的一个特定区域。组件从 `App` 状态中读取数据，
//! 通过 ratatui 的 `Frame` 进行绘制。
//!
//! # 组件列表
//!
//! - [`session`] - 会话/消息列表组件，显示对话历史
//! - [`input`] - 输入框组件，显示和编辑用户输入
//! - [`status`] - 状态栏组件，显示运行时状态信息

/// 会话视图组件 - 渲染消息列表和对话历史
pub mod session;

/// 输入框组件 - 渲染用户输入区域
pub mod input;

/// 状态栏组件 - 渲染底部状态信息栏
pub mod status;

// 便捷的重导出
pub use input::InputView;
pub use session::SessionView;
pub use status::StatusBar;

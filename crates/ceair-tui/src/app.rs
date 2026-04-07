//! 应用状态管理模块
//!
//! 本模块定义了 TUI 应用的核心状态结构，包括消息列表、输入状态、
//! 应用模式以及键盘事件处理逻辑。

use ceair_core::message::Role;
use ceair_core::TokenUsage;
use chrono::{DateTime, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

// ---------------------------------------------------------------------------
// 应用模式
// ---------------------------------------------------------------------------

/// 应用模式枚举 - 表示应用当前所处的交互模式
///
/// 不同模式下键盘事件的处理逻辑不同：
/// - `Normal`：浏览消息，支持滚动和快捷键
/// - `Input`：输入消息文本，支持编辑和历史浏览
/// - `Command`：输入命令（类似 vim 的命令行模式）
/// - `Help`：显示帮助信息
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppMode {
    /// 普通模式 - 浏览消息，使用快捷键操作
    Normal,
    /// 输入模式 - 输入和编辑消息内容
    Input,
    /// 命令模式 - 输入命令（如 :q 退出、:clear 清屏）
    Command,
    /// 帮助模式 - 显示快捷键帮助信息
    Help,
}

impl std::fmt::Display for AppMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            AppMode::Normal => "普通",
            AppMode::Input => "输入",
            AppMode::Command => "命令",
            AppMode::Help => "帮助",
        };
        write!(f, "{label}")
    }
}

// ---------------------------------------------------------------------------
// 应用操作
// ---------------------------------------------------------------------------

/// 应用操作枚举 - 键盘事件处理后返回的操作指令
///
/// `handle_key_event` 方法根据当前模式和按键返回相应的操作，
/// 由上层调用者执行具体的业务逻辑。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppAction {
    /// 无操作 - 按键已被内部处理，无需额外操作
    None,
    /// 发送消息 - 包含用户输入的消息文本
    SendMessage(String),
    /// 退出应用
    Quit,
    /// 清除所有消息
    Clear,
}

// ---------------------------------------------------------------------------
// 显示消息
// ---------------------------------------------------------------------------

/// 显示消息结构 - 用于在 TUI 中展示的消息
///
/// 与 ceair-core 的 `Message` 不同，`DisplayMessage` 专注于显示层面的需求，
/// 包含流式输出状态和 token 使用量等 UI 相关信息。
#[derive(Clone, Debug)]
pub struct DisplayMessage {
    /// 消息发送者的角色（用户、助手、系统、工具）
    pub role: Role,
    /// 消息的文本内容
    pub content: String,
    /// 消息的时间戳
    pub timestamp: DateTime<Utc>,
    /// 该消息对应的 token 使用量（仅助手消息可能有值）
    pub token_usage: Option<TokenUsage>,
    /// 是否正在流式输出中（内容可能尚未完整）
    pub is_streaming: bool,
}

impl DisplayMessage {
    /// 创建一个新的显示消息
    ///
    /// # 参数
    /// - `role`: 消息角色
    /// - `content`: 消息内容
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            timestamp: Utc::now(),
            token_usage: None,
            is_streaming: false,
        }
    }

    /// 创建一个流式输出中的显示消息
    ///
    /// 流式消息的内容会随着数据到达而逐步更新。
    pub fn streaming(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            timestamp: Utc::now(),
            token_usage: None,
            is_streaming: true,
        }
    }

    /// 获取角色的显示标签
    ///
    /// 返回角色对应的中文标签，用于消息列表中的显示。
    pub fn role_label(&self) -> &'static str {
        match self.role {
            Role::User => "👤 用户",
            Role::Assistant => "🤖 助手",
            Role::System => "⚙️ 系统",
            Role::Tool => "🔧 工具",
        }
    }
}

// ---------------------------------------------------------------------------
// 输入状态
// ---------------------------------------------------------------------------

/// 输入状态结构 - 管理用户文本输入的完整状态
///
/// 包含输入缓冲区、光标位置、命令历史等信息，
/// 支持基本的文本编辑和历史浏览功能。
#[derive(Clone, Debug)]
pub struct InputState {
    /// 当前输入缓冲区的文本内容
    pub buffer: String,
    /// 光标在缓冲区中的字符位置（非字节位置）
    pub cursor_position: usize,
    /// 历史输入记录（从旧到新排列）
    pub history: Vec<String>,
    /// 当前浏览的历史记录索引（None 表示未在浏览历史）
    pub history_index: Option<usize>,
}

impl InputState {
    /// 创建一个空的输入状态
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor_position: 0,
            history: Vec::new(),
            history_index: None,
        }
    }

    /// 获取光标位置对应的字节索引
    ///
    /// 由于 Rust 字符串是 UTF-8 编码，光标的字符位置需要
    /// 转换为字节索引才能进行字符串操作。
    fn cursor_byte_index(&self) -> usize {
        self.buffer
            .char_indices()
            .nth(self.cursor_position)
            .map(|(i, _)| i)
            .unwrap_or(self.buffer.len())
    }

    /// 获取缓冲区的字符总数
    pub fn char_count(&self) -> usize {
        self.buffer.chars().count()
    }

    /// 在光标位置插入一个字符
    ///
    /// 插入后光标自动右移一位。
    pub fn insert_char(&mut self, ch: char) {
        let idx = self.cursor_byte_index();
        self.buffer.insert(idx, ch);
        self.cursor_position += 1;
    }

    /// 删除光标前一个字符（退格键行为）
    ///
    /// 如果光标在最开头则无操作。
    pub fn delete_char_before_cursor(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            let idx = self.cursor_byte_index();
            self.buffer.remove(idx);
        }
    }

    /// 将光标向左移动一个字符
    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    /// 将光标向右移动一个字符
    pub fn move_cursor_right(&mut self) {
        if self.cursor_position < self.char_count() {
            self.cursor_position += 1;
        }
    }

    /// 将光标移动到行首
    pub fn move_cursor_home(&mut self) {
        self.cursor_position = 0;
    }

    /// 将光标移动到行尾
    pub fn move_cursor_end(&mut self) {
        self.cursor_position = self.char_count();
    }

    /// 清空输入缓冲区并重置光标位置
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor_position = 0;
        self.history_index = None;
    }

    /// 将当前缓冲区内容添加到历史记录
    ///
    /// 仅当缓冲区不为空且与最近一条历史记录不同时才添加。
    pub fn push_history(&mut self) {
        let text = self.buffer.trim().to_string();
        if !text.is_empty() {
            // 避免连续重复的历史记录
            if self.history.last().map_or(true, |last| last != &text) {
                self.history.push(text);
            }
        }
        self.history_index = None;
    }

    /// 浏览上一条历史记录
    ///
    /// 将历史记录的内容填充到缓冲区中，光标移到末尾。
    pub fn history_previous(&mut self) {
        if self.history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            // 还没开始浏览历史，从最后一条开始
            None => self.history.len() - 1,
            // 已在浏览中，向前（更旧的方向）移动
            Some(idx) => {
                if idx > 0 {
                    idx - 1
                } else {
                    return; // 已经是最早的一条
                }
            }
        };

        self.history_index = Some(new_index);
        self.buffer = self.history[new_index].clone();
        self.cursor_position = self.char_count();
    }

    /// 浏览下一条历史记录
    ///
    /// 如果已经浏览到最新一条，则恢复为空缓冲区。
    pub fn history_next(&mut self) {
        match self.history_index {
            None => {} // 未在浏览历史，无操作
            Some(idx) => {
                if idx + 1 < self.history.len() {
                    // 向后（更新的方向）移动
                    self.history_index = Some(idx + 1);
                    self.buffer = self.history[idx + 1].clone();
                } else {
                    // 已经是最新一条，恢复空缓冲区
                    self.history_index = None;
                    self.buffer.clear();
                }
                self.cursor_position = self.char_count();
            }
        }
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 状态信息
// ---------------------------------------------------------------------------

/// 状态信息结构 - 显示在状态栏中的运行时信息
///
/// 包含模型名称、token 计数、连接状态等需要实时展示的信息。
#[derive(Clone, Debug)]
pub struct StatusInfo {
    /// 当前使用的 AI 模型名称
    pub model_name: String,
    /// 当前会话累计的 token 使用量
    pub token_count: u64,
    /// 状态提示文本（如"等待输入"、"正在生成..."）
    pub status_text: String,
    /// 是否已连接到 AI 服务
    pub is_connected: bool,
}

impl StatusInfo {
    /// 创建一个新的状态信息
    pub fn new(model_name: impl Into<String>) -> Self {
        Self {
            model_name: model_name.into(),
            token_count: 0,
            status_text: "就绪".to_string(),
            is_connected: false,
        }
    }
}

// ---------------------------------------------------------------------------
// 应用主状态
// ---------------------------------------------------------------------------

/// 应用主状态结构 - 管理整个 TUI 应用的核心状态
///
/// `App` 是 TUI 应用的中心数据结构，包含所有需要在界面上展示和交互的状态。
/// 它不直接进行渲染，而是由各个 UI 组件读取其状态来绘制界面。
///
/// # 使用示例
///
/// ```rust
/// use ceair_tui::app::App;
/// use ceair_core::message::Role;
///
/// let mut app = App::new("gpt-4");
/// app.add_message(Role::User, "你好！");
/// app.add_message(Role::Assistant, "你好！有什么可以帮助你的？");
/// ```
pub struct App {
    /// 消息列表 - 按时间顺序存储所有显示消息
    pub messages: Vec<DisplayMessage>,
    /// 输入状态 - 管理用户输入缓冲区和历史记录
    pub input: InputState,
    /// 状态信息 - 显示在状态栏中的运行时信息
    pub status: StatusInfo,
    /// 滚动偏移量 - 消息列表的垂直滚动位置（行数）
    pub scroll_offset: u16,
    /// 是否显示帮助面板
    pub show_help: bool,
    /// 应用是否正在运行（设为 false 将退出主循环）
    pub is_running: bool,
    /// 当前应用模式
    pub mode: AppMode,
}

impl App {
    /// 创建一个新的应用实例
    ///
    /// 初始状态为输入模式，便于用户立即开始输入。
    ///
    /// # 参数
    /// - `model_name`: 当前使用的 AI 模型名称
    pub fn new(model_name: impl Into<String>) -> Self {
        Self {
            messages: Vec::new(),
            input: InputState::new(),
            status: StatusInfo::new(model_name),
            scroll_offset: 0,
            show_help: false,
            is_running: true,
            mode: AppMode::Input,
        }
    }

    /// 添加一条消息到消息列表
    ///
    /// 新消息添加后自动滚动到底部，确保用户能看到最新消息。
    ///
    /// # 参数
    /// - `role`: 消息角色
    /// - `content`: 消息内容
    pub fn add_message(&mut self, role: Role, content: impl Into<String>) {
        let message = DisplayMessage::new(role, content);
        self.messages.push(message);
        // 自动滚动到底部
        self.scroll_to_bottom();
    }

    /// 更新最后一条流式消息的内容
    ///
    /// 在流式输出场景下，AI 的回复会逐步到达。此方法用于追加内容到
    /// 最后一条正在流式输出的消息中。如果最后一条消息不是流式消息，
    /// 则创建一条新的流式消息。
    ///
    /// # 参数
    /// - `content`: 要追加的内容片段
    /// - `done`: 流式输出是否已完成
    pub fn update_streaming_message(&mut self, content: &str, done: bool) {
        if let Some(last) = self.messages.last_mut() {
            if last.is_streaming {
                // 追加内容到现有的流式消息
                last.content.push_str(content);
                if done {
                    last.is_streaming = false;
                }
                self.scroll_to_bottom();
                return;
            }
        }

        // 没有正在流式输出的消息，创建一条新的
        let mut msg = DisplayMessage::streaming(Role::Assistant, content);
        if done {
            msg.is_streaming = false;
        }
        self.messages.push(msg);
        self.scroll_to_bottom();
    }

    /// 处理键盘事件
    ///
    /// 根据当前应用模式分发键盘事件到对应的处理逻辑。
    /// 返回 `AppAction` 指示上层需要执行的操作。
    ///
    /// # 参数
    /// - `event`: crossterm 键盘事件
    ///
    /// # 返回
    /// - `AppAction`: 需要执行的应用操作
    pub fn handle_key_event(&mut self, event: KeyEvent) -> AppAction {
        // 仅处理按下事件，忽略释放和重复事件
        if event.kind != KeyEventKind::Press {
            return AppAction::None;
        }

        // Ctrl+C 在任何模式下都退出
        if event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('c') {
            self.is_running = false;
            return AppAction::Quit;
        }

        // 根据当前模式分发事件处理
        match self.mode {
            AppMode::Normal => self.handle_normal_mode(event),
            AppMode::Input => self.handle_input_mode(event),
            AppMode::Command => self.handle_command_mode(event),
            AppMode::Help => self.handle_help_mode(event),
        }
    }

    /// 普通模式下的键盘事件处理
    ///
    /// 支持的快捷键：
    /// - `i` / `Enter`: 进入输入模式
    /// - `q`: 退出应用
    /// - `?`: 切换帮助面板
    /// - `Up` / `k`: 向上滚动
    /// - `Down` / `j`: 向下滚动
    /// - `Ctrl+L`: 清除消息
    /// - `/`: 进入命令模式
    fn handle_normal_mode(&mut self, event: KeyEvent) -> AppAction {
        // Ctrl+L 清除消息
        if event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('l') {
            self.messages.clear();
            self.scroll_offset = 0;
            return AppAction::Clear;
        }

        match event.code {
            // 进入输入模式
            KeyCode::Char('i') | KeyCode::Enter => {
                self.mode = AppMode::Input;
                AppAction::None
            }
            // 退出应用
            KeyCode::Char('q') => {
                self.is_running = false;
                AppAction::Quit
            }
            // 切换帮助面板
            KeyCode::Char('?') => {
                self.toggle_help();
                AppAction::None
            }
            // 向上滚动
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_up(1);
                AppAction::None
            }
            // 向下滚动
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_down(1);
                AppAction::None
            }
            // 快速向上滚动（半页）
            KeyCode::PageUp => {
                self.scroll_up(10);
                AppAction::None
            }
            // 快速向下滚动（半页）
            KeyCode::PageDown => {
                self.scroll_down(10);
                AppAction::None
            }
            // 进入命令模式
            KeyCode::Char('/') | KeyCode::Char(':') => {
                self.mode = AppMode::Command;
                self.input.clear();
                AppAction::None
            }
            _ => AppAction::None,
        }
    }

    /// 输入模式下的键盘事件处理
    ///
    /// 支持完整的文本编辑操作：
    /// - 字符输入、退格删除、光标移动
    /// - `Enter`: 发送消息
    /// - `Escape`: 返回普通模式
    /// - `Up`/`Down`: 浏览历史记录
    /// - `Ctrl+L`: 清除消息
    fn handle_input_mode(&mut self, event: KeyEvent) -> AppAction {
        // Ctrl+L 清除消息
        if event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('l') {
            self.messages.clear();
            self.scroll_offset = 0;
            return AppAction::Clear;
        }

        match event.code {
            // 发送消息
            KeyCode::Enter => {
                let text = self.input.buffer.trim().to_string();
                if text.is_empty() {
                    return AppAction::None;
                }
                // 保存到历史记录
                self.input.push_history();
                // 清空输入缓冲区
                self.input.clear();
                AppAction::SendMessage(text)
            }
            // 返回普通模式
            KeyCode::Esc => {
                self.mode = AppMode::Normal;
                AppAction::None
            }
            // 退格删除
            KeyCode::Backspace => {
                self.input.delete_char_before_cursor();
                AppAction::None
            }
            // 光标左移
            KeyCode::Left => {
                self.input.move_cursor_left();
                AppAction::None
            }
            // 光标右移
            KeyCode::Right => {
                self.input.move_cursor_right();
                AppAction::None
            }
            // 浏览历史（上一条）
            KeyCode::Up => {
                self.input.history_previous();
                AppAction::None
            }
            // 浏览历史（下一条）
            KeyCode::Down => {
                self.input.history_next();
                AppAction::None
            }
            // 光标移到行首
            KeyCode::Home => {
                self.input.move_cursor_home();
                AppAction::None
            }
            // 光标移到行尾
            KeyCode::End => {
                self.input.move_cursor_end();
                AppAction::None
            }
            // Tab 键插入空格（防止焦点切换）
            KeyCode::Tab => {
                self.input.insert_char(' ');
                self.input.insert_char(' ');
                AppAction::None
            }
            // 普通字符输入
            KeyCode::Char(ch) => {
                self.input.insert_char(ch);
                AppAction::None
            }
            _ => AppAction::None,
        }
    }

    /// 命令模式下的键盘事件处理
    ///
    /// 支持简单的命令输入：
    /// - `Enter`: 执行命令
    /// - `Escape`: 取消并返回普通模式
    /// - 字符输入：追加到命令缓冲区
    fn handle_command_mode(&mut self, event: KeyEvent) -> AppAction {
        match event.code {
            // 执行命令
            KeyCode::Enter => {
                let cmd = self.input.buffer.trim().to_lowercase();
                self.input.clear();
                self.mode = AppMode::Normal;

                match cmd.as_str() {
                    "q" | "quit" | "exit" => {
                        self.is_running = false;
                        AppAction::Quit
                    }
                    "clear" | "cls" => {
                        self.messages.clear();
                        self.scroll_offset = 0;
                        AppAction::Clear
                    }
                    "help" | "h" => {
                        self.show_help = true;
                        self.mode = AppMode::Help;
                        AppAction::None
                    }
                    _ => AppAction::None,
                }
            }
            // 取消命令
            KeyCode::Esc => {
                self.input.clear();
                self.mode = AppMode::Normal;
                AppAction::None
            }
            // 退格删除
            KeyCode::Backspace => {
                self.input.delete_char_before_cursor();
                // 如果命令缓冲区清空，自动返回普通模式
                if self.input.buffer.is_empty() {
                    self.mode = AppMode::Normal;
                }
                AppAction::None
            }
            // 字符输入
            KeyCode::Char(ch) => {
                self.input.insert_char(ch);
                AppAction::None
            }
            _ => AppAction::None,
        }
    }

    /// 帮助模式下的键盘事件处理
    ///
    /// 按任意键退出帮助模式。
    fn handle_help_mode(&mut self, event: KeyEvent) -> AppAction {
        match event.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                self.show_help = false;
                self.mode = AppMode::Normal;
                AppAction::None
            }
            _ => {
                // 帮助模式下忽略其他按键
                AppAction::None
            }
        }
    }

    /// 向上滚动消息列表
    ///
    /// # 参数
    /// - `lines`: 滚动的行数
    pub fn scroll_up(&mut self, lines: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
    }

    /// 向下滚动消息列表
    ///
    /// 滚动偏移量不会低于 0。
    ///
    /// # 参数
    /// - `lines`: 滚动的行数
    pub fn scroll_down(&mut self, lines: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// 滚动到消息列表底部
    fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// 切换帮助面板的显示状态
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
        if self.show_help {
            self.mode = AppMode::Help;
        } else {
            self.mode = AppMode::Normal;
        }
    }

    /// 获取消息总数
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// 检查是否有正在流式输出的消息
    pub fn has_streaming_message(&self) -> bool {
        self.messages.last().map_or(false, |m| m.is_streaming)
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助函数：创建一个按键按下事件
    fn key_press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// 辅助函数：创建一个带修饰键的按键事件
    fn key_press_with_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn 测试应用初始状态() {
        let app = App::new("gpt-4");
        assert!(app.is_running);
        assert_eq!(app.mode, AppMode::Input);
        assert!(app.messages.is_empty());
        assert_eq!(app.scroll_offset, 0);
        assert!(!app.show_help);
        assert_eq!(app.status.model_name, "gpt-4");
    }

    #[test]
    fn 测试添加消息() {
        let mut app = App::new("gpt-4");
        app.add_message(Role::User, "你好");
        app.add_message(Role::Assistant, "你好！有什么可以帮助你的？");

        assert_eq!(app.message_count(), 2);
        assert_eq!(app.messages[0].role, Role::User);
        assert_eq!(app.messages[0].content, "你好");
        assert_eq!(app.messages[1].role, Role::Assistant);
    }

    #[test]
    fn 测试流式消息更新() {
        let mut app = App::new("gpt-4");

        // 开始流式输出
        app.update_streaming_message("你", false);
        assert_eq!(app.message_count(), 1);
        assert!(app.messages[0].is_streaming);
        assert_eq!(app.messages[0].content, "你");

        // 继续流式输出
        app.update_streaming_message("好", false);
        assert_eq!(app.message_count(), 1);
        assert_eq!(app.messages[0].content, "你好");

        // 完成流式输出
        app.update_streaming_message("！", true);
        assert_eq!(app.message_count(), 1);
        assert!(!app.messages[0].is_streaming);
        assert_eq!(app.messages[0].content, "你好！");
    }

    #[test]
    fn 测试输入模式下发送消息() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Input;

        // 输入文字
        app.handle_key_event(key_press(KeyCode::Char('h')));
        app.handle_key_event(key_press(KeyCode::Char('i')));
        assert_eq!(app.input.buffer, "hi");

        // 发送消息
        let action = app.handle_key_event(key_press(KeyCode::Enter));
        assert_eq!(action, AppAction::SendMessage("hi".to_string()));
        assert!(app.input.buffer.is_empty());
    }

    #[test]
    fn 测试空消息不发送() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Input;

        let action = app.handle_key_event(key_press(KeyCode::Enter));
        assert_eq!(action, AppAction::None);
    }

    #[test]
    fn 测试escape键切换到普通模式() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Input;

        app.handle_key_event(key_press(KeyCode::Esc));
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn 测试ctrl_c退出() {
        let mut app = App::new("gpt-4");

        let action =
            app.handle_key_event(key_press_with_mod(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(action, AppAction::Quit);
        assert!(!app.is_running);
    }

    #[test]
    fn 测试ctrl_l清除消息() {
        let mut app = App::new("gpt-4");
        app.add_message(Role::User, "test");
        assert_eq!(app.message_count(), 1);

        // 在输入模式下按 Ctrl+L
        app.mode = AppMode::Input;
        let action =
            app.handle_key_event(key_press_with_mod(KeyCode::Char('l'), KeyModifiers::CONTROL));
        assert_eq!(action, AppAction::Clear);
        assert!(app.messages.is_empty());
    }

    #[test]
    fn 测试普通模式下的滚动() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Normal;

        // 向上滚动
        app.handle_key_event(key_press(KeyCode::Up));
        assert_eq!(app.scroll_offset, 1);

        // 向下滚动
        app.handle_key_event(key_press(KeyCode::Down));
        assert_eq!(app.scroll_offset, 0);

        // 不能滚动到负数
        app.handle_key_event(key_press(KeyCode::Down));
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn 测试输入状态的光标操作() {
        let mut input = InputState::new();

        // 插入字符
        input.insert_char('你');
        input.insert_char('好');
        assert_eq!(input.buffer, "你好");
        assert_eq!(input.cursor_position, 2);

        // 光标左移
        input.move_cursor_left();
        assert_eq!(input.cursor_position, 1);

        // 在光标位置插入字符
        input.insert_char('们');
        assert_eq!(input.buffer, "你们好");
        assert_eq!(input.cursor_position, 2);

        // 退格删除
        input.delete_char_before_cursor();
        assert_eq!(input.buffer, "你好");
        assert_eq!(input.cursor_position, 1);
    }

    #[test]
    fn 测试输入历史记录() {
        let mut input = InputState::new();

        // 添加历史记录
        input.buffer = "第一条".to_string();
        input.push_history();
        input.clear();

        input.buffer = "第二条".to_string();
        input.push_history();
        input.clear();

        // 浏览历史（向上）
        input.history_previous();
        assert_eq!(input.buffer, "第二条");

        input.history_previous();
        assert_eq!(input.buffer, "第一条");

        // 浏览历史（向下）
        input.history_next();
        assert_eq!(input.buffer, "第二条");

        // 继续向下回到空缓冲区
        input.history_next();
        assert!(input.buffer.is_empty());
    }

    #[test]
    fn 测试重复历史不添加() {
        let mut input = InputState::new();

        input.buffer = "重复".to_string();
        input.push_history();
        input.clear();

        input.buffer = "重复".to_string();
        input.push_history();
        input.clear();

        // 只应该有一条历史记录
        assert_eq!(input.history.len(), 1);
    }

    #[test]
    fn 测试帮助模式切换() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Normal;

        // 按 ? 打开帮助
        app.handle_key_event(key_press(KeyCode::Char('?')));
        assert!(app.show_help);
        assert_eq!(app.mode, AppMode::Help);

        // 按 Esc 关闭帮助
        app.handle_key_event(key_press(KeyCode::Esc));
        assert!(!app.show_help);
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn 测试命令模式() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Normal;

        // 进入命令模式
        app.handle_key_event(key_press(KeyCode::Char(':')));
        assert_eq!(app.mode, AppMode::Command);

        // 输入 "q" 并执行
        app.handle_key_event(key_press(KeyCode::Char('q')));
        let action = app.handle_key_event(key_press(KeyCode::Enter));
        assert_eq!(action, AppAction::Quit);
    }

    #[test]
    fn 测试显示消息的角色标签() {
        let msg = DisplayMessage::new(Role::User, "test");
        assert_eq!(msg.role_label(), "👤 用户");

        let msg = DisplayMessage::new(Role::Assistant, "test");
        assert_eq!(msg.role_label(), "🤖 助手");

        let msg = DisplayMessage::new(Role::System, "test");
        assert_eq!(msg.role_label(), "⚙️ 系统");

        let msg = DisplayMessage::new(Role::Tool, "test");
        assert_eq!(msg.role_label(), "🔧 工具");
    }

    #[test]
    fn 测试应用模式显示() {
        assert_eq!(format!("{}", AppMode::Normal), "普通");
        assert_eq!(format!("{}", AppMode::Input), "输入");
        assert_eq!(format!("{}", AppMode::Command), "命令");
        assert_eq!(format!("{}", AppMode::Help), "帮助");
    }

    #[test]
    fn 测试has_streaming_message() {
        let mut app = App::new("gpt-4");

        // 空消息列表
        assert!(!app.has_streaming_message());

        // 添加普通消息
        app.add_message(Role::User, "hello");
        assert!(!app.has_streaming_message());

        // 开始流式输出
        app.update_streaming_message("hi", false);
        assert!(app.has_streaming_message());

        // 完成流式输出
        app.update_streaming_message("!", true);
        assert!(!app.has_streaming_message());
    }

    #[test]
    fn 测试光标边界操作() {
        let mut input = InputState::new();

        // 在空缓冲区时左移不应 panic
        input.move_cursor_left();
        assert_eq!(input.cursor_position, 0);

        // 在空缓冲区时右移不应超出
        input.move_cursor_right();
        assert_eq!(input.cursor_position, 0);

        // 在空缓冲区时退格不应 panic
        input.delete_char_before_cursor();
        assert_eq!(input.cursor_position, 0);

        // Home 和 End
        input.insert_char('a');
        input.insert_char('b');
        input.move_cursor_home();
        assert_eq!(input.cursor_position, 0);
        input.move_cursor_end();
        assert_eq!(input.cursor_position, 2);
    }
}

//! 应用状态管理模块
//!
//! 本模块定义了 TUI 应用的核心状态结构，包括消息列表、输入状态、
//! 应用模式、交互模式（Plan / Autopilot / UltraWork）、思考深度控制
//! 以及键盘事件处理逻辑。

use chengcoding_core::message::Role;
use chengcoding_core::TokenUsage;
use chrono::{DateTime, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};

// ---------------------------------------------------------------------------
// 应用模式（TUI 导航状态）
// ---------------------------------------------------------------------------

/// 应用模式枚举 - 表示 TUI 当前所处的导航/编辑模式
///
/// 与 `InteractionMode`（交互模式）不同，`AppMode` 控制的是
/// TUI 界面的导航状态，决定键盘事件如何被解释。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppMode {
    /// 普通模式 - 浏览消息，支持滚动和快捷键
    Normal,
    /// 输入模式 - 输入消息文本，支持编辑和历史浏览
    Input,
    /// 命令模式 - 输入斜杠命令（如 /model、/help）
    Command,
    /// 帮助模式 - 显示帮助信息
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
// 交互模式（智能体工作策略）
// ---------------------------------------------------------------------------

/// 交互模式枚举 - 控制 AI 智能体的工作方式和自主程度
///
/// 通过 Shift+Tab 在各模式间循环切换。每种模式对应不同的
/// AI 行为策略，影响工具调用权限、自动化程度和用户交互频率。
///
/// # 设计原则
///
/// 参考 Harness Engineering 设计理念：
/// - 模型是不稳定组件，不同模式下给予不同程度的自主权
/// - 工具是受管理的执行接口，模式决定了自动审批策略
/// - 错误路径即主路径，高自主模式需要更强的错误恢复能力
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InteractionMode {
    /// 普通模式 - 每次操作需用户确认，最安全
    Normal,
    /// 计划模式 - AI 先制定计划再执行，用户审批计划
    Plan,
    /// 自动驾驶模式 - AI 自动执行任务，仅在关键节点暂停
    Autopilot,
    /// 极限工作模式 - 最高自主权，AI 全程自动化执行
    UltraWork,
}

impl InteractionMode {
    /// 获取所有交互模式的列表（按切换顺序排列）
    pub fn all() -> &'static [InteractionMode] {
        &[
            InteractionMode::Normal,
            InteractionMode::Plan,
            InteractionMode::Autopilot,
            InteractionMode::UltraWork,
        ]
    }

    /// 切换到下一个交互模式（循环切换）
    pub fn next(&self) -> InteractionMode {
        match self {
            InteractionMode::Normal => InteractionMode::Plan,
            InteractionMode::Plan => InteractionMode::Autopilot,
            InteractionMode::Autopilot => InteractionMode::UltraWork,
            InteractionMode::UltraWork => InteractionMode::Normal,
        }
    }

    /// 获取模式的显示标签
    pub fn label(&self) -> &'static str {
        match self {
            InteractionMode::Normal => "Normal",
            InteractionMode::Plan => "Plan",
            InteractionMode::Autopilot => "Autopilot",
            InteractionMode::UltraWork => "UltraWork",
        }
    }

    /// 获取模式的中文说明
    pub fn description(&self) -> &'static str {
        match self {
            InteractionMode::Normal => "普通模式 - 每步确认",
            InteractionMode::Plan => "计划模式 - 先规划后执行",
            InteractionMode::Autopilot => "自动驾驶 - 自动执行任务",
            InteractionMode::UltraWork => "极限模式 - 全程自动化",
        }
    }

    /// 从字符串解析交互模式
    pub fn from_str_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "normal" => Some(InteractionMode::Normal),
            "plan" => Some(InteractionMode::Plan),
            "autopilot" | "auto" => Some(InteractionMode::Autopilot),
            "ultrawork" | "ultra" => Some(InteractionMode::UltraWork),
            _ => None,
        }
    }
}

impl Default for InteractionMode {
    fn default() -> Self {
        InteractionMode::Normal
    }
}

impl std::fmt::Display for InteractionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

// ---------------------------------------------------------------------------
// 思考深度
// ---------------------------------------------------------------------------

/// 思考深度枚举 - 控制 AI 模型的推理深度
///
/// 通过 Ctrl+L 在各级别间循环切换。不同 AI 提供商对思考深度的
/// 处理方式不同：
/// - Claude: 映射为 extended thinking budget
/// - GPT: 映射为 reasoning effort 参数
/// - DeepSeek: 映射为思维链控制参数
///
/// # 设计说明
///
/// 本枚举与 `chengcoding_ai::model_roles::ThinkingLevel` 功能类似，
/// 但面向 TUI 层的用户交互，提供更友好的显示和切换逻辑。
/// 在实际发送请求时会转换为对应的 `ThinkingLevel`。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThinkingDepth {
    /// 关闭思考 - 不使用推理模式
    Off,
    /// 浅层思考 - 简单推理
    Light,
    /// 中等思考 - 标准推理深度
    Medium,
    /// 深度思考 - 深入推理和验证
    Deep,
    /// 极限思考 - 最大推理预算
    Maximum,
}

impl ThinkingDepth {
    /// 切换到下一个思考深度（循环切换）
    pub fn next(&self) -> ThinkingDepth {
        match self {
            ThinkingDepth::Off => ThinkingDepth::Light,
            ThinkingDepth::Light => ThinkingDepth::Medium,
            ThinkingDepth::Medium => ThinkingDepth::Deep,
            ThinkingDepth::Deep => ThinkingDepth::Maximum,
            ThinkingDepth::Maximum => ThinkingDepth::Off,
        }
    }

    /// 获取深度的显示标签
    pub fn label(&self) -> &'static str {
        match self {
            ThinkingDepth::Off => "关闭",
            ThinkingDepth::Light => "浅层",
            ThinkingDepth::Medium => "中等",
            ThinkingDepth::Deep => "深度",
            ThinkingDepth::Maximum => "极限",
        }
    }

    /// 获取深度对应的图标（用于状态栏简洁显示）
    pub fn icon(&self) -> &'static str {
        match self {
            ThinkingDepth::Off => "💤",
            ThinkingDepth::Light => "💡",
            ThinkingDepth::Medium => "🧠",
            ThinkingDepth::Deep => "🔬",
            ThinkingDepth::Maximum => "⚡",
        }
    }

    /// 从字符串解析思考深度
    pub fn from_str_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "off" | "none" => Some(ThinkingDepth::Off),
            "light" | "low" | "minimal" => Some(ThinkingDepth::Light),
            "medium" | "med" | "normal" => Some(ThinkingDepth::Medium),
            "deep" | "high" => Some(ThinkingDepth::Deep),
            "maximum" | "max" | "xhigh" => Some(ThinkingDepth::Maximum),
            _ => None,
        }
    }
}

impl Default for ThinkingDepth {
    fn default() -> Self {
        ThinkingDepth::Medium
    }
}

impl std::fmt::Display for ThinkingDepth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
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
    /// 执行斜杠命令 - 包含命令名和参数
    SlashCommand { name: String, args: String },
    /// 切换交互模式 - 包含切换后的新模式
    SwitchInteractionMode(InteractionMode),
    /// 切换思考深度 - 包含切换后的新深度
    SwitchThinkingDepth(ThinkingDepth),
    /// 切换侧边栏显示
    ToggleSidebar,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandMenuKind {
    Slash,
    Model,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandMenuItem {
    pub value: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandMenuState {
    pub kind: CommandMenuKind,
    pub query: String,
    pub items: Vec<CommandMenuItem>,
    pub selected_index: usize,
}

impl CommandMenuState {
    pub fn selected_item(&self) -> Option<&CommandMenuItem> {
        self.items.get(self.selected_index)
    }

    pub fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.items.len();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.items.len() - 1
            } else {
                self.selected_index - 1
            };
        }
    }
}

// ---------------------------------------------------------------------------
// 显示消息
// ---------------------------------------------------------------------------

/// 显示消息结构 - 用于在 TUI 中展示的消息
///
/// 与 chengcoding-core 的 `Message` 不同，`DisplayMessage` 专注于显示层面的需求，
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
// 侧边栏状态
// ---------------------------------------------------------------------------

/// 侧边栏面板类型 - 控制侧边栏显示的内容面板
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SidebarPanel {
    ContextOverview,
    McpStatus,
    Changes,
}

impl SidebarPanel {
    pub fn next(&self) -> SidebarPanel {
        match self {
            SidebarPanel::ContextOverview => SidebarPanel::McpStatus,
            SidebarPanel::McpStatus => SidebarPanel::Changes,
            SidebarPanel::Changes => SidebarPanel::ContextOverview,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            SidebarPanel::ContextOverview => "📊 上下文",
            SidebarPanel::McpStatus => "🔌 MCP",
            SidebarPanel::Changes => "📝 变更",
        }
    }
}

impl Default for SidebarPanel {
    fn default() -> Self {
        SidebarPanel::ContextOverview
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McpConnectionState {
    Connected,
    Disconnected,
    Degraded,
}

#[derive(Clone, Debug)]
pub struct McpServerStatus {
    pub name: String,
    pub state: McpConnectionState,
    pub detail: String,
}

#[derive(Clone, Debug)]
pub struct ModifiedFileEntry {
    pub path: String,
    pub change_kind: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default)]
pub struct ContextUsage {
    pub used_tokens: u64,
    pub max_tokens: u64,
    pub session_count: usize,
}

#[derive(Clone, Debug)]
pub struct SidebarState {
    pub visible: bool,
    pub active_panel: SidebarPanel,
    pub mcp_index: usize,
    pub modified_file_index: usize,
    pub mcp_servers: Vec<McpServerStatus>,
    pub modified_files: Vec<ModifiedFileEntry>,
    pub context_usage: ContextUsage,
}

impl SidebarState {
    pub fn new() -> Self {
        Self {
            visible: true,
            active_panel: SidebarPanel::ContextOverview,
            mcp_index: 0,
            modified_file_index: 0,
            mcp_servers: vec![
                McpServerStatus {
                    name: "filesystem".to_string(),
                    state: McpConnectionState::Connected,
                    detail: "已连接".to_string(),
                },
                McpServerStatus {
                    name: "github".to_string(),
                    state: McpConnectionState::Disconnected,
                    detail: "未配置".to_string(),
                },
            ],
            modified_files: vec![ModifiedFileEntry {
                path: "crates/chengcoding-tui/src/app.rs".to_string(),
                change_kind: "modified".to_string(),
                updated_at: Utc::now(),
            }],
            context_usage: ContextUsage {
                used_tokens: 0,
                max_tokens: 128_000,
                session_count: 1,
            },
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn next_panel(&mut self) {
        self.active_panel = self.active_panel.next();
    }
}

impl Default for SidebarState {
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
/// # 状态层次
///
/// - `mode` (AppMode): TUI 导航状态（普通/输入/命令/帮助）
/// - `interaction_mode` (InteractionMode): 智能体工作策略（Normal/Plan/Autopilot/UltraWork）
/// - `thinking_depth` (ThinkingDepth): AI 推理深度（关闭/浅层/中等/深度/极限）
///
/// # 使用示例
///
/// ```rust
/// use chengcoding_tui::app::App;
/// use chengcoding_core::message::Role;
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
    /// 当前 TUI 导航模式
    pub mode: AppMode,
    /// 当前交互模式（智能体工作策略）
    pub interaction_mode: InteractionMode,
    /// 当前思考深度
    pub thinking_depth: ThinkingDepth,
    /// 侧边栏状态
    pub sidebar: SidebarState,
    pub command_menu: Option<CommandMenuState>,
    pub available_models: Vec<CommandMenuItem>,
}

impl App {
    /// 创建一个新的应用实例
    ///
    /// 初始状态为输入模式，便于用户立即开始输入。
    /// 默认交互模式为 Normal，思考深度为 Medium，侧边栏可见。
    ///
    /// # 参数
    /// - `model_name`: 当前使用的 AI 模型名称
    pub fn new(model_name: impl Into<String>) -> Self {
        let model_name = model_name.into();
        Self {
            messages: Vec::new(),
            input: InputState::new(),
            status: StatusInfo::new(model_name.clone()),
            scroll_offset: 0,
            show_help: false,
            is_running: true,
            mode: AppMode::Input,
            interaction_mode: InteractionMode::default(),
            thinking_depth: ThinkingDepth::default(),
            sidebar: SidebarState::new(),
            command_menu: None,
            available_models: vec![
                CommandMenuItem {
                    value: model_name.clone(),
                    description: "当前配置模型".to_string(),
                },
                CommandMenuItem {
                    value: "deepseek-chat".to_string(),
                    description: "DeepSeek Chat".to_string(),
                },
                CommandMenuItem {
                    value: "qwen-plus".to_string(),
                    description: "通义千问 Plus".to_string(),
                },
                CommandMenuItem {
                    value: "ernie-bot".to_string(),
                    description: "文心一言".to_string(),
                },
            ],
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
    /// 全局快捷键（在任意模式下生效）：
    /// - Ctrl+C: 退出应用
    /// - Shift+Tab: 切换交互模式（Normal → Plan → Autopilot → UltraWork）
    /// - Ctrl+L: 切换思考深度（关闭 → 浅层 → 中等 → 深度 → 极限）
    /// - Ctrl+B: 切换侧边栏显示/隐藏
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

        // Shift+Tab: 切换交互模式（全局快捷键）
        if event.modifiers.contains(KeyModifiers::SHIFT) && event.code == KeyCode::BackTab {
            self.interaction_mode = self.interaction_mode.next();
            return AppAction::SwitchInteractionMode(self.interaction_mode.clone());
        }

        // Ctrl+L: 切换思考深度（全局快捷键）
        if event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('l') {
            self.thinking_depth = self.thinking_depth.next();
            return AppAction::SwitchThinkingDepth(self.thinking_depth.clone());
        }

        // Ctrl+B: 切换侧边栏（全局快捷键）
        if event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('b') {
            self.sidebar.toggle();
            return AppAction::ToggleSidebar;
        }

        // 根据当前模式分发事件处理
        match self.mode {
            AppMode::Normal => self.handle_normal_mode(event),
            AppMode::Input => self.handle_input_mode(event),
            AppMode::Command => self.handle_command_mode(event),
            AppMode::Help => self.handle_help_mode(event),
        }
    }

    pub fn handle_mouse_event(&mut self, event: MouseEvent) -> AppAction {
        match event.kind {
            MouseEventKind::ScrollUp => {
                self.scroll_up(3);
                AppAction::None
            }
            MouseEventKind::ScrollDown => {
                self.scroll_down(3);
                AppAction::None
            }
            _ => AppAction::None,
        }
    }

    pub fn set_available_models(&mut self, models: Vec<CommandMenuItem>) {
        self.available_models = models;
    }

    fn filtered_model_items(&self, query: &str) -> Vec<CommandMenuItem> {
        let normalized = query.trim().to_lowercase();
        let mut items: Vec<CommandMenuItem> = self
            .available_models
            .iter()
            .filter(|item| {
                normalized.is_empty()
                    || item.value.to_lowercase().contains(&normalized)
                    || item.description.to_lowercase().contains(&normalized)
            })
            .cloned()
            .collect();

        if items.is_empty() && !normalized.is_empty() {
            items.push(CommandMenuItem {
                value: normalized,
                description: "使用自定义模型值".to_string(),
            });
        }

        items
    }

    fn slash_menu_items() -> Vec<CommandMenuItem> {
        vec![
            CommandMenuItem {
                value: "model".to_string(),
                description: "打开模型选择菜单".to_string(),
            },
            CommandMenuItem {
                value: "help".to_string(),
                description: "显示命令帮助".to_string(),
            },
            CommandMenuItem {
                value: "clear".to_string(),
                description: "清空当前对话".to_string(),
            },
        ]
    }

    fn open_slash_menu(&mut self) {
        self.command_menu = Some(CommandMenuState {
            kind: CommandMenuKind::Slash,
            query: String::new(),
            items: Self::slash_menu_items(),
            selected_index: 0,
        });
    }

    fn open_model_menu(&mut self, query: &str) {
        self.command_menu = Some(CommandMenuState {
            kind: CommandMenuKind::Model,
            query: query.to_string(),
            items: self.filtered_model_items(query),
            selected_index: 0,
        });
    }

    fn refresh_command_menu(&mut self) {
        let Some(kind) = self.command_menu.as_ref().map(|menu| menu.kind.clone()) else {
            return;
        };

        let (query, items) = match kind {
            CommandMenuKind::Slash => {
                let query = self
                    .input
                    .buffer
                    .trim()
                    .trim_start_matches('/')
                    .to_lowercase();
                let items = Self::slash_menu_items()
                    .into_iter()
                    .filter(|item| query.is_empty() || item.value.contains(&query))
                    .collect::<Vec<_>>();
                (query, items)
            }
            CommandMenuKind::Model => {
                let query = self
                    .input
                    .buffer
                    .trim()
                    .strip_prefix("/model")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let items = self.filtered_model_items(&query);
                (query, items)
            }
        };

        if let Some(menu) = self.command_menu.as_mut() {
            menu.query = query;
            menu.items = items;
            menu.selected_index = 0;
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
    /// - `/`: 进入命令模式
    /// - `Tab`: 切换侧边栏面板
    fn handle_normal_mode(&mut self, event: KeyEvent) -> AppAction {
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
                self.open_slash_menu();
                AppAction::None
            }
            // Tab 切换侧边栏面板
            KeyCode::Tab => {
                self.sidebar.next_panel();
                AppAction::None
            }
            _ => AppAction::None,
        }
    }

    /// 输入模式下的键盘事件处理
    ///
    /// 支持完整的文本编辑操作：
    /// - 字符输入、退格删除、光标移动
    /// - `Enter`: 发送消息（如果以 / 开头则作为斜杠命令处理）
    /// - `Escape`: 返回普通模式
    /// - `Up`/`Down`: 浏览历史记录
    fn handle_input_mode(&mut self, event: KeyEvent) -> AppAction {
        match event.code {
            KeyCode::Enter => {
                let text = self.input.buffer.trim().to_string();
                if text.is_empty() {
                    return AppAction::None;
                }
                self.input.push_history();
                self.input.clear();

                if let Some(cmd_text) = text.strip_prefix('/') {
                    let parts: Vec<&str> = cmd_text.splitn(2, char::is_whitespace).collect();
                    let name = parts[0].to_string();
                    let args = parts.get(1).unwrap_or(&"").trim().to_string();
                    AppAction::SlashCommand { name, args }
                } else {
                    AppAction::SendMessage(text)
                }
            }
            KeyCode::Esc => {
                self.mode = AppMode::Normal;
                self.command_menu = None;
                AppAction::None
            }
            KeyCode::Backspace => {
                self.input.delete_char_before_cursor();
                AppAction::None
            }
            KeyCode::Left => {
                self.input.move_cursor_left();
                AppAction::None
            }
            KeyCode::Right => {
                self.input.move_cursor_right();
                AppAction::None
            }
            KeyCode::Up => {
                self.input.history_previous();
                AppAction::None
            }
            KeyCode::Down => {
                self.input.history_next();
                AppAction::None
            }
            KeyCode::Home => {
                self.input.move_cursor_home();
                AppAction::None
            }
            KeyCode::End => {
                self.input.move_cursor_end();
                AppAction::None
            }
            KeyCode::Tab => {
                self.input.insert_char(' ');
                self.input.insert_char(' ');
                AppAction::None
            }
            KeyCode::Char(ch) => {
                self.input.insert_char(ch);
                AppAction::None
            }
            _ => AppAction::None,
        }
    }

    /// 命令模式下的键盘事件处理
    ///
    /// 支持斜杠命令和简单命令：
    /// - `Enter`: 执行命令
    /// - `Escape`: 取消并返回普通模式
    /// - 字符输入：追加到命令缓冲区
    fn handle_command_mode(&mut self, event: KeyEvent) -> AppAction {
        match event.code {
            KeyCode::Enter => {
                if let Some((kind, value)) = self.command_menu.as_ref().and_then(|menu| {
                    menu.selected_item()
                        .map(|item| (menu.kind.clone(), item.value.clone()))
                }) {
                    match kind {
                        CommandMenuKind::Slash => {
                            if value == "model" {
                                self.input.buffer = "/model ".to_string();
                                self.input.cursor_position = self.input.char_count();
                                self.open_model_menu("");
                                return AppAction::None;
                            }

                            self.input.clear();
                            self.command_menu = None;
                            self.mode = AppMode::Input;
                            return AppAction::SlashCommand {
                                name: value,
                                args: String::new(),
                            };
                        }
                        CommandMenuKind::Model => {
                            self.input.clear();
                            self.command_menu = None;
                            self.mode = AppMode::Input;
                            return AppAction::SlashCommand {
                                name: "model".to_string(),
                                args: value,
                            };
                        }
                    }
                }

                let cmd = self.input.buffer.trim().trim_start_matches('/').to_string();
                self.input.clear();
                self.command_menu = None;
                self.mode = AppMode::Input;

                if cmd.is_empty() {
                    return AppAction::None;
                }

                let parts: Vec<&str> = cmd.splitn(2, char::is_whitespace).collect();
                let cmd_name = parts[0].to_lowercase();
                let cmd_args = parts.get(1).unwrap_or(&"").trim().to_string();

                match cmd_name.as_str() {
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
                    _ => AppAction::SlashCommand {
                        name: cmd_name,
                        args: cmd_args,
                    },
                }
            }
            KeyCode::Esc => {
                self.input.clear();
                self.command_menu = None;
                self.mode = AppMode::Input;
                AppAction::None
            }
            KeyCode::Backspace => {
                self.input.delete_char_before_cursor();
                if self.input.buffer.is_empty() {
                    self.command_menu = None;
                    self.mode = AppMode::Input;
                } else {
                    self.refresh_command_menu();
                }
                AppAction::None
            }
            KeyCode::Up => {
                if let Some(menu) = self.command_menu.as_mut() {
                    menu.select_previous();
                }
                AppAction::None
            }
            KeyCode::Down => {
                if let Some(menu) = self.command_menu.as_mut() {
                    menu.select_next();
                }
                AppAction::None
            }
            KeyCode::Char(ch) => {
                if self.input.buffer.is_empty() && ch == '/' {
                    self.input.insert_char(ch);
                    self.open_slash_menu();
                    return AppAction::None;
                }

                self.input.insert_char(ch);
                if self.input.buffer.starts_with("/model") {
                    let query = self
                        .input
                        .buffer
                        .trim()
                        .strip_prefix("/model")
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    self.open_model_menu(&query);
                } else {
                    self.refresh_command_menu();
                }
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
    use crossterm::event::MouseEvent;

    /// 辅助函数：创建一个按键按下事件
    fn key_press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn mouse_scroll(kind: MouseEventKind) -> MouseEvent {
        MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }
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
        assert_eq!(app.interaction_mode, InteractionMode::Normal);
        assert_eq!(app.thinking_depth, ThinkingDepth::Medium);
        assert!(app.sidebar.visible);
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

        let action = app.handle_key_event(key_press_with_mod(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        ));
        assert_eq!(action, AppAction::Quit);
        assert!(!app.is_running);
    }

    #[test]
    fn 测试ctrl_l切换思考深度() {
        let mut app = App::new("gpt-4");
        assert_eq!(app.thinking_depth, ThinkingDepth::Medium);

        // 按 Ctrl+L 切换到 Deep
        let action = app.handle_key_event(key_press_with_mod(
            KeyCode::Char('l'),
            KeyModifiers::CONTROL,
        ));
        assert_eq!(action, AppAction::SwitchThinkingDepth(ThinkingDepth::Deep));
        assert_eq!(app.thinking_depth, ThinkingDepth::Deep);

        // 再按切换到 Maximum
        let action = app.handle_key_event(key_press_with_mod(
            KeyCode::Char('l'),
            KeyModifiers::CONTROL,
        ));
        assert_eq!(
            action,
            AppAction::SwitchThinkingDepth(ThinkingDepth::Maximum)
        );
        assert_eq!(app.thinking_depth, ThinkingDepth::Maximum);

        // 再按循环回 Off
        let action = app.handle_key_event(key_press_with_mod(
            KeyCode::Char('l'),
            KeyModifiers::CONTROL,
        ));
        assert_eq!(action, AppAction::SwitchThinkingDepth(ThinkingDepth::Off));
        assert_eq!(app.thinking_depth, ThinkingDepth::Off);
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
    fn 测试鼠标滚轮向上滚动消息() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Normal;

        app.handle_mouse_event(mouse_scroll(MouseEventKind::ScrollUp));

        assert_eq!(app.scroll_offset, 3);
    }

    #[test]
    fn 测试鼠标滚轮向下滚动消息() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Normal;
        app.scroll_offset = 5;

        app.handle_mouse_event(mouse_scroll(MouseEventKind::ScrollDown));

        assert_eq!(app.scroll_offset, 2);
    }

    #[test]
    fn 测试鼠标滚轮向下不会滚成负数() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Normal;
        app.scroll_offset = 1;

        app.handle_mouse_event(mouse_scroll(MouseEventKind::ScrollDown));

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

        app.handle_key_event(key_press(KeyCode::Char(':')));
        assert_eq!(app.mode, AppMode::Command);

        app.command_menu = None;
        app.handle_key_event(key_press(KeyCode::Char('q')));
        let action = app.handle_key_event(key_press(KeyCode::Enter));
        assert_eq!(action, AppAction::Quit);
    }

    #[test]
    fn 测试斜杠进入命令菜单() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Normal;

        app.handle_key_event(key_press(KeyCode::Char('/')));

        assert_eq!(app.mode, AppMode::Command);
        assert!(app.command_menu.is_some());
        assert_eq!(
            app.command_menu.as_ref().unwrap().kind,
            CommandMenuKind::Slash
        );
    }

    #[test]
    fn 测试选择model进入模型菜单() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Normal;
        app.handle_key_event(key_press(KeyCode::Char('/')));

        let action = app.handle_key_event(key_press(KeyCode::Enter));

        assert_eq!(action, AppAction::None);
        assert!(app.command_menu.is_some());
        assert_eq!(
            app.command_menu.as_ref().unwrap().kind,
            CommandMenuKind::Model
        );
        assert_eq!(app.input.buffer, "/model ");
    }

    #[test]
    fn 测试模型菜单回车产生斜杠命令() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Normal;
        app.set_available_models(vec![
            CommandMenuItem {
                value: "glm-5.1".to_string(),
                description: "GLM 5.1".to_string(),
            },
            CommandMenuItem {
                value: "deepseek-chat".to_string(),
                description: "DeepSeek Chat".to_string(),
            },
        ]);

        app.handle_key_event(key_press(KeyCode::Char('/')));
        app.handle_key_event(key_press(KeyCode::Enter));
        let action = app.handle_key_event(key_press(KeyCode::Enter));

        assert_eq!(
            action,
            AppAction::SlashCommand {
                name: "model".to_string(),
                args: "glm-5.1".to_string(),
            }
        );
    }

    #[test]
    fn 测试斜杠菜单可直接执行help() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Normal;

        app.handle_key_event(key_press(KeyCode::Char('/')));
        app.handle_key_event(key_press(KeyCode::Char('h')));
        let action = app.handle_key_event(key_press(KeyCode::Enter));

        assert_eq!(
            action,
            AppAction::SlashCommand {
                name: "help".to_string(),
                args: String::new(),
            }
        );
    }

    #[test]
    fn 测试斜杠菜单可直接执行clear() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Normal;

        app.handle_key_event(key_press(KeyCode::Char('/')));
        app.handle_key_event(key_press(KeyCode::Char('c')));
        let action = app.handle_key_event(key_press(KeyCode::Enter));

        assert_eq!(
            action,
            AppAction::SlashCommand {
                name: "clear".to_string(),
                args: String::new(),
            }
        );
    }

    // -----------------------------------------------------------------------
    // 新功能测试：交互模式切换
    // -----------------------------------------------------------------------

    #[test]
    fn 测试shift_tab切换交互模式() {
        let mut app = App::new("gpt-4");
        assert_eq!(app.interaction_mode, InteractionMode::Normal);

        // Shift+Tab 切换到 Plan
        let action =
            app.handle_key_event(key_press_with_mod(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(
            action,
            AppAction::SwitchInteractionMode(InteractionMode::Plan)
        );
        assert_eq!(app.interaction_mode, InteractionMode::Plan);

        // 再次切换到 Autopilot
        app.handle_key_event(key_press_with_mod(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.interaction_mode, InteractionMode::Autopilot);

        // 再次切换到 UltraWork
        app.handle_key_event(key_press_with_mod(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.interaction_mode, InteractionMode::UltraWork);

        // 循环回 Normal
        app.handle_key_event(key_press_with_mod(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.interaction_mode, InteractionMode::Normal);
    }

    #[test]
    fn 测试交互模式在所有app模式下生效() {
        // Shift+Tab 应该在任何 AppMode 下都能切换交互模式
        for start_mode in [AppMode::Normal, AppMode::Input, AppMode::Command] {
            let mut app = App::new("gpt-4");
            app.mode = start_mode;
            app.handle_key_event(key_press_with_mod(KeyCode::BackTab, KeyModifiers::SHIFT));
            assert_eq!(app.interaction_mode, InteractionMode::Plan);
        }
    }

    // -----------------------------------------------------------------------
    // 新功能测试：思考深度
    // -----------------------------------------------------------------------

    #[test]
    fn 测试思考深度完整循环() {
        let mut depth = ThinkingDepth::Off;
        let expected = [
            ThinkingDepth::Light,
            ThinkingDepth::Medium,
            ThinkingDepth::Deep,
            ThinkingDepth::Maximum,
            ThinkingDepth::Off,
        ];
        for expected_next in &expected {
            depth = depth.next();
            assert_eq!(&depth, expected_next);
        }
    }

    #[test]
    fn 测试思考深度字符串解析() {
        assert_eq!(
            ThinkingDepth::from_str_name("off"),
            Some(ThinkingDepth::Off)
        );
        assert_eq!(
            ThinkingDepth::from_str_name("light"),
            Some(ThinkingDepth::Light)
        );
        assert_eq!(
            ThinkingDepth::from_str_name("medium"),
            Some(ThinkingDepth::Medium)
        );
        assert_eq!(
            ThinkingDepth::from_str_name("deep"),
            Some(ThinkingDepth::Deep)
        );
        assert_eq!(
            ThinkingDepth::from_str_name("maximum"),
            Some(ThinkingDepth::Maximum)
        );
        assert_eq!(
            ThinkingDepth::from_str_name("max"),
            Some(ThinkingDepth::Maximum)
        );
        assert_eq!(ThinkingDepth::from_str_name("invalid"), None);
    }

    // -----------------------------------------------------------------------
    // 新功能测试：斜杠命令
    // -----------------------------------------------------------------------

    #[test]
    fn 测试输入模式下的斜杠命令() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Input;

        // 输入 "/model gpt-5"
        for ch in "/model gpt-5".chars() {
            app.handle_key_event(key_press(KeyCode::Char(ch)));
        }

        let action = app.handle_key_event(key_press(KeyCode::Enter));
        assert_eq!(
            action,
            AppAction::SlashCommand {
                name: "model".to_string(),
                args: "gpt-5".to_string(),
            }
        );
    }

    #[test]
    fn 测试斜杠命令无参数() {
        let mut app = App::new("gpt-4");
        app.mode = AppMode::Input;

        for ch in "/help".chars() {
            app.handle_key_event(key_press(KeyCode::Char(ch)));
        }

        let action = app.handle_key_event(key_press(KeyCode::Enter));
        assert_eq!(
            action,
            AppAction::SlashCommand {
                name: "help".to_string(),
                args: String::new(),
            }
        );
    }

    // -----------------------------------------------------------------------
    // 新功能测试：侧边栏
    // -----------------------------------------------------------------------

    #[test]
    fn 测试ctrl_b切换侧边栏() {
        let mut app = App::new("gpt-4");
        assert!(app.sidebar.visible);

        let action = app.handle_key_event(key_press_with_mod(
            KeyCode::Char('b'),
            KeyModifiers::CONTROL,
        ));
        assert_eq!(action, AppAction::ToggleSidebar);
        assert!(!app.sidebar.visible);

        app.handle_key_event(key_press_with_mod(
            KeyCode::Char('b'),
            KeyModifiers::CONTROL,
        ));
        assert!(app.sidebar.visible);
    }

    #[test]
    fn 测试侧边栏面板切换() {
        let mut sidebar = SidebarState::new();
        assert_eq!(sidebar.active_panel, SidebarPanel::ContextOverview);

        sidebar.next_panel();
        assert_eq!(sidebar.active_panel, SidebarPanel::McpStatus);

        sidebar.next_panel();
        assert_eq!(sidebar.active_panel, SidebarPanel::Changes);

        sidebar.next_panel();
        assert_eq!(sidebar.active_panel, SidebarPanel::ContextOverview);
    }

    // -----------------------------------------------------------------------
    // 新功能测试：交互模式属性
    // -----------------------------------------------------------------------

    #[test]
    fn 测试交互模式属性() {
        assert_eq!(InteractionMode::Normal.label(), "Normal");
        assert_eq!(InteractionMode::Plan.label(), "Plan");
        assert_eq!(InteractionMode::Autopilot.label(), "Autopilot");
        assert_eq!(InteractionMode::UltraWork.label(), "UltraWork");
        assert_eq!(InteractionMode::all().len(), 4);
    }

    #[test]
    fn 测试交互模式字符串解析() {
        assert_eq!(
            InteractionMode::from_str_name("normal"),
            Some(InteractionMode::Normal)
        );
        assert_eq!(
            InteractionMode::from_str_name("plan"),
            Some(InteractionMode::Plan)
        );
        assert_eq!(
            InteractionMode::from_str_name("autopilot"),
            Some(InteractionMode::Autopilot)
        );
        assert_eq!(
            InteractionMode::from_str_name("auto"),
            Some(InteractionMode::Autopilot)
        );
        assert_eq!(
            InteractionMode::from_str_name("ultrawork"),
            Some(InteractionMode::UltraWork)
        );
        assert_eq!(
            InteractionMode::from_str_name("ultra"),
            Some(InteractionMode::UltraWork)
        );
        assert_eq!(InteractionMode::from_str_name("invalid"), None);
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

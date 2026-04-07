//! 消息类型模块
//!
//! 本模块定义了 AI 对话中使用的各种消息类型，
//! 包括角色定义、消息结构、工具调用和对话管理。
//! 这些类型与主流 AI 模型 API（如 OpenAI、Anthropic）的消息格式兼容。

use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 角色枚举
// ---------------------------------------------------------------------------

/// 消息角色 - 标识消息的发送者身份
///
/// 遵循主流 AI API 的角色定义：
/// - `System`：系统提示词，设定 AI 的行为规范
/// - `User`：用户输入的消息
/// - `Assistant`：AI 助手生成的回复
/// - `Tool`：工具执行结果的返回消息
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// 系统角色 - 用于设定 AI 的行为规范和上下文
    System,
    /// 用户角色 - 代表人类用户的输入
    User,
    /// 助手角色 - 代表 AI 模型的输出
    Assistant,
    /// 工具角色 - 代表工具执行结果的返回
    Tool,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Role::System => "系统",
            Role::User => "用户",
            Role::Assistant => "助手",
            Role::Tool => "工具",
        };
        write!(f, "{label}")
    }
}

// ---------------------------------------------------------------------------
// 消息结构
// ---------------------------------------------------------------------------

/// 对话消息 - AI 对话中的单条消息
///
/// 包含角色、内容、可选的工具调用信息等。
/// 兼容 OpenAI 和 Anthropic 的消息格式。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    /// 消息角色
    pub role: Role,
    /// 消息文本内容（对于工具调用消息可能为空）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// 消息发送者名称（可选，用于多代理场景）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// AI 助手请求的工具调用列表（仅 Assistant 角色使用）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// 工具调用 ID（仅 Tool 角色使用，关联到对应的工具调用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// 消息创建时间
    pub created_at: DateTime<Utc>,
}

impl Message {
    /// 创建一个系统消息
    ///
    /// 系统消息用于设定 AI 的行为规范，通常放在对话开头。
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: Some(content.into()),
            name: None,
            tool_calls: Vec::new(),
            tool_call_id: None,
            created_at: Utc::now(),
        }
    }

    /// 创建一个用户消息
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Some(content.into()),
            name: None,
            tool_calls: Vec::new(),
            tool_call_id: None,
            created_at: Utc::now(),
        }
    }

    /// 创建一个助手消息（纯文本回复）
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: Some(content.into()),
            name: None,
            tool_calls: Vec::new(),
            tool_call_id: None,
            created_at: Utc::now(),
        }
    }

    /// 创建一个包含工具调用的助手消息
    ///
    /// AI 模型通过此消息请求执行一个或多个工具。
    pub fn assistant_with_tool_calls(
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        Self {
            role: Role::Assistant,
            content,
            name: None,
            tool_calls,
            tool_call_id: None,
            created_at: Utc::now(),
        }
    }

    /// 创建一个工具结果消息
    ///
    /// 将工具执行结果作为消息返回给 AI 模型。
    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: Some(content.into()),
            name: None,
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
            created_at: Utc::now(),
        }
    }

    /// 设置消息的发送者名称
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// 判断消息是否包含工具调用
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }

    /// 获取消息内容的长度（字符数），内容为空时返回 0
    pub fn content_len(&self) -> usize {
        self.content.as_ref().map_or(0, |c| c.len())
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let content = self.content.as_deref().unwrap_or("<无内容>");
        write!(f, "[{}] {}", self.role, content)
    }
}

// ---------------------------------------------------------------------------
// 工具调用结构
// ---------------------------------------------------------------------------

/// 工具调用 - 描述 AI 模型请求执行的一次工具调用
///
/// 包含唯一标识符、函数名和参数，
/// 对应 OpenAI API 中的 tool_call 结构。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    /// 工具调用的唯一标识符（由 AI 模型生成）
    pub id: String,
    /// 要调用的函数名称
    pub function_name: String,
    /// 函数参数（JSON 格式）
    pub arguments: serde_json::Value,
}

impl ToolCall {
    /// 创建一个新的工具调用
    pub fn new(
        id: impl Into<String>,
        function_name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self {
            id: id.into(),
            function_name: function_name.into(),
            arguments,
        }
    }
}

impl fmt::Display for ToolCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ToolCall[{}]({})",
            self.id, self.function_name
        )
    }
}

// ---------------------------------------------------------------------------
// 工具结果结构
// ---------------------------------------------------------------------------

/// 工具执行结果 - 描述工具执行后的返回值
///
/// 包含关联的工具调用 ID、结果内容和是否为错误。
/// AI 模型通过 tool_call_id 将结果与对应的调用关联起来。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolResult {
    /// 关联的工具调用 ID
    pub tool_call_id: String,
    /// 工具执行结果的文本内容
    pub content: String,
    /// 标记该结果是否为错误
    ///
    /// 某些 AI API 需要区分工具的正常结果和错误结果。
    pub is_error: bool,
}

impl ToolResult {
    /// 创建一个成功的工具结果
    pub fn success(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
            is_error: false,
        }
    }

    /// 创建一个失败的工具结果
    pub fn error(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
            is_error: true,
        }
    }

    /// 将工具结果转换为对应的 Tool 角色消息
    pub fn into_message(self) -> Message {
        Message::tool(self.tool_call_id, self.content)
    }
}

impl fmt::Display for ToolResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.is_error { "错误" } else { "成功" };
        write!(
            f,
            "ToolResult[{}]({status}): {}",
            self.tool_call_id,
            // 截断过长的内容
            if self.content.len() > 100 {
                format!("{}...", &self.content[..100])
            } else {
                self.content.clone()
            }
        )
    }
}

// ---------------------------------------------------------------------------
// 对话管理
// ---------------------------------------------------------------------------

/// 对话 - 管理一组有序的消息列表
///
/// 提供对话的创建、消息添加、查询等功能。
/// 支持系统提示词管理和简易的 token 估算。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Conversation {
    /// 消息列表，按时间顺序排列
    messages: Vec<Message>,
}

impl Conversation {
    /// 创建一个空的对话
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    /// 创建一个带系统提示词的对话
    pub fn with_system_prompt(system_prompt: impl Into<String>) -> Self {
        let mut conv = Self::new();
        conv.add_message(Message::system(system_prompt));
        conv
    }

    /// 向对话中添加一条消息
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// 获取所有消息的不可变引用
    pub fn get_messages(&self) -> &[Message] {
        &self.messages
    }

    /// 获取系统提示词
    ///
    /// 返回第一条系统角色消息的内容。
    /// 如果没有系统消息，返回 None。
    pub fn get_system_prompt(&self) -> Option<&str> {
        self.messages
            .iter()
            .find(|m| m.role == Role::System)
            .and_then(|m| m.content.as_deref())
    }

    /// 清空所有消息
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// 获取对话中的消息数量
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// 判断对话是否为空
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// 估算对话中所有消息的总 token 数
    ///
    /// 使用简易估算方法：约每 4 个字符对应 1 个 token（英文文本）。
    /// 对于中文文本，约每 2 个字符对应 1 个 token。
    /// 此方法仅提供粗略估算，实际 token 数取决于模型的分词器。
    ///
    /// 每条消息额外加上一定的固定开销（角色标签等），参考 OpenAI 的计算方式。
    pub fn token_estimate(&self) -> usize {
        /// 每条消息的固定 token 开销（角色标签、分隔符等）
        const MESSAGE_OVERHEAD: usize = 4;
        /// 英文文本中每个 token 对应的平均字符数
        const CHARS_PER_TOKEN: usize = 4;

        self.messages
            .iter()
            .map(|msg| {
                let content_tokens = msg.content_len() / CHARS_PER_TOKEN;

                // 工具调用参数也要计入 token
                let tool_call_tokens: usize = msg
                    .tool_calls
                    .iter()
                    .map(|tc| {
                        let name_tokens = tc.function_name.len() / CHARS_PER_TOKEN;
                        let args_tokens =
                            tc.arguments.to_string().len() / CHARS_PER_TOKEN;
                        name_tokens + args_tokens + MESSAGE_OVERHEAD
                    })
                    .sum();

                content_tokens + tool_call_tokens + MESSAGE_OVERHEAD
            })
            .sum()
    }

    /// 获取最后一条消息
    pub fn last_message(&self) -> Option<&Message> {
        self.messages.last()
    }

    /// 获取最后一条助手消息
    pub fn last_assistant_message(&self) -> Option<&Message> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == Role::Assistant)
    }

    /// 获取所有工具调用消息（未处理的）
    ///
    /// 返回最后一条助手消息中的所有工具调用。
    pub fn pending_tool_calls(&self) -> Vec<&ToolCall> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == Role::Assistant && !m.tool_calls.is_empty())
            .map(|m| m.tool_calls.iter().collect())
            .unwrap_or_default()
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Conversation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "对话 (共 {} 条消息):", self.messages.len())?;
        for (i, msg) in self.messages.iter().enumerate() {
            writeln!(f, "  {}: {msg}", i + 1)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 测试角色的序列化() {
        assert_eq!(serde_json::to_string(&Role::System).unwrap(), "\"system\"");
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            "\"assistant\""
        );
        assert_eq!(serde_json::to_string(&Role::Tool).unwrap(), "\"tool\"");
    }

    #[test]
    fn 测试角色的反序列化() {
        let role: Role = serde_json::from_str("\"system\"").unwrap();
        assert_eq!(role, Role::System);
    }

    #[test]
    fn 测试角色的显示() {
        assert_eq!(format!("{}", Role::System), "系统");
        assert_eq!(format!("{}", Role::User), "用户");
        assert_eq!(format!("{}", Role::Assistant), "助手");
        assert_eq!(format!("{}", Role::Tool), "工具");
    }

    #[test]
    fn 测试系统消息的创建() {
        let msg = Message::system("你是一个代码助手");
        assert_eq!(msg.role, Role::System);
        assert_eq!(msg.content.as_deref(), Some("你是一个代码助手"));
        assert!(!msg.has_tool_calls());
    }

    #[test]
    fn 测试用户消息的创建() {
        let msg = Message::user("请帮我写一个函数");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content.as_deref(), Some("请帮我写一个函数"));
    }

    #[test]
    fn 测试助手消息的创建() {
        let msg = Message::assistant("好的，我来帮你编写");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content.as_deref(), Some("好的，我来帮你编写"));
    }

    #[test]
    fn 测试带名称的消息() {
        let msg = Message::user("你好").with_name("张三");
        assert_eq!(msg.name.as_deref(), Some("张三"));
    }

    #[test]
    fn 测试带工具调用的助手消息() {
        let tool_call = ToolCall::new(
            "call_1",
            "file_read",
            serde_json::json!({"path": "src/main.rs"}),
        );
        let msg =
            Message::assistant_with_tool_calls(None, vec![tool_call]);

        assert_eq!(msg.role, Role::Assistant);
        assert!(msg.content.is_none());
        assert!(msg.has_tool_calls());
        assert_eq!(msg.tool_calls.len(), 1);
        assert_eq!(msg.tool_calls[0].function_name, "file_read");
    }

    #[test]
    fn 测试工具结果消息() {
        let msg = Message::tool("call_1", "文件内容：fn main() {}");
        assert_eq!(msg.role, Role::Tool);
        assert_eq!(msg.tool_call_id.as_deref(), Some("call_1"));
    }

    #[test]
    fn 测试工具调用的创建和显示() {
        let tc = ToolCall::new(
            "call_001",
            "bash_exec",
            serde_json::json!({"command": "ls -la"}),
        );
        assert_eq!(tc.id, "call_001");
        assert_eq!(tc.function_name, "bash_exec");
        let display = format!("{tc}");
        assert!(display.contains("call_001"));
        assert!(display.contains("bash_exec"));
    }

    #[test]
    fn 测试工具结果的成功和失败() {
        let success = ToolResult::success("call_1", "操作完成");
        assert!(!success.is_error);

        let error = ToolResult::error("call_2", "权限不足");
        assert!(error.is_error);
    }

    #[test]
    fn 测试工具结果转换为消息() {
        let result = ToolResult::success("call_1", "文件内容");
        let msg = result.into_message();
        assert_eq!(msg.role, Role::Tool);
        assert_eq!(msg.tool_call_id.as_deref(), Some("call_1"));
        assert_eq!(msg.content.as_deref(), Some("文件内容"));
    }

    #[test]
    fn 测试空对话的创建() {
        let conv = Conversation::new();
        assert!(conv.is_empty());
        assert_eq!(conv.len(), 0);
        assert!(conv.get_system_prompt().is_none());
    }

    #[test]
    fn 测试带系统提示词的对话创建() {
        let conv = Conversation::with_system_prompt("你是代码助手");
        assert_eq!(conv.len(), 1);
        assert_eq!(conv.get_system_prompt(), Some("你是代码助手"));
    }

    #[test]
    fn 测试对话的消息管理() {
        let mut conv = Conversation::new();
        conv.add_message(Message::system("系统提示"));
        conv.add_message(Message::user("用户输入"));
        conv.add_message(Message::assistant("助手回复"));

        assert_eq!(conv.len(), 3);
        assert!(!conv.is_empty());
        assert_eq!(conv.get_system_prompt(), Some("系统提示"));

        // 验证消息顺序
        let msgs = conv.get_messages();
        assert_eq!(msgs[0].role, Role::System);
        assert_eq!(msgs[1].role, Role::User);
        assert_eq!(msgs[2].role, Role::Assistant);
    }

    #[test]
    fn 测试对话的清空() {
        let mut conv = Conversation::with_system_prompt("系统提示");
        conv.add_message(Message::user("你好"));
        assert_eq!(conv.len(), 2);

        conv.clear();
        assert!(conv.is_empty());
    }

    #[test]
    fn 测试token估算() {
        let mut conv = Conversation::new();
        // 添加一些消息
        conv.add_message(Message::system("你是一个AI编程助手"));
        conv.add_message(Message::user("请帮我写一个排序函数"));

        let estimate = conv.token_estimate();
        // 估算值应该大于0
        assert!(estimate > 0);
    }

    #[test]
    fn 测试获取最后一条消息() {
        let mut conv = Conversation::new();
        assert!(conv.last_message().is_none());

        conv.add_message(Message::user("第一条"));
        conv.add_message(Message::assistant("第二条"));

        let last = conv.last_message().unwrap();
        assert_eq!(last.role, Role::Assistant);
    }

    #[test]
    fn 测试获取最后一条助手消息() {
        let mut conv = Conversation::new();
        conv.add_message(Message::user("问题1"));
        conv.add_message(Message::assistant("回答1"));
        conv.add_message(Message::user("问题2"));

        let last_assistant = conv.last_assistant_message().unwrap();
        assert_eq!(last_assistant.content.as_deref(), Some("回答1"));
    }

    #[test]
    fn 测试获取待处理的工具调用() {
        let mut conv = Conversation::new();
        conv.add_message(Message::user("查看文件"));

        let tc = ToolCall::new(
            "call_1",
            "file_read",
            serde_json::json!({"path": "main.rs"}),
        );
        conv.add_message(Message::assistant_with_tool_calls(None, vec![tc]));

        let pending = conv.pending_tool_calls();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].function_name, "file_read");
    }

    #[test]
    fn 测试没有待处理工具调用时返回空() {
        let mut conv = Conversation::new();
        conv.add_message(Message::user("你好"));
        conv.add_message(Message::assistant("你好！"));

        let pending = conv.pending_tool_calls();
        assert!(pending.is_empty());
    }

    #[test]
    fn 测试消息的JSON序列化和反序列化() {
        let msg = Message::user("测试消息");
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role, Role::User);
        assert_eq!(deserialized.content.as_deref(), Some("测试消息"));
    }

    #[test]
    fn 测试对话的JSON序列化和反序列化() {
        let mut conv = Conversation::with_system_prompt("系统提示");
        conv.add_message(Message::user("你好"));
        conv.add_message(Message::assistant("你好！"));

        let json = serde_json::to_string(&conv).unwrap();
        let deserialized: Conversation = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 3);
        assert_eq!(deserialized.get_system_prompt(), Some("系统提示"));
    }

    #[test]
    fn 测试对话的显示格式() {
        let mut conv = Conversation::new();
        conv.add_message(Message::user("你好"));
        let display = format!("{conv}");
        assert!(display.contains("1 条消息"));
    }

    #[test]
    fn 测试消息内容长度() {
        let msg = Message::user("hello");
        assert_eq!(msg.content_len(), 5);

        let empty_msg = Message::assistant_with_tool_calls(None, vec![]);
        assert_eq!(empty_msg.content_len(), 0);
    }

    #[test]
    fn 测试工具结果的显示格式() {
        let result = ToolResult::success("call_1", "短内容");
        let display = format!("{result}");
        assert!(display.contains("成功"));

        let error = ToolResult::error("call_2", "错误内容");
        let display2 = format!("{error}");
        assert!(display2.contains("错误"));
    }
}

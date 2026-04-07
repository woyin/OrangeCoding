//! # 代理上下文管理
//!
//! 本模块提供 `AgentContext`，用于管理代理执行过程中的所有状态信息，
//! 包括会话标识、对话历史、工作目录和环境变量等。

use std::collections::HashMap;
use std::path::PathBuf;

use ceair_core::message::{Conversation, Message, ToolResult};
use ceair_core::SessionId;

// ---------------------------------------------------------------------------
// 代理上下文
// ---------------------------------------------------------------------------

/// 代理上下文 - 维护代理执行期间的全部状态
///
/// 上下文包含：
/// - 会话标识符：关联当前对话会话
/// - 对话记录：完整的消息历史（含系统提示、用户消息、助手回复、工具结果）
/// - 工作目录：工具执行时的默认工作路径
/// - 环境变量：传递给子进程的额外环境变量
/// - 元数据：可自由扩展的键值对存储
#[derive(Debug, Clone)]
pub struct AgentContext {
    /// 当前会话的唯一标识
    session_id: SessionId,
    /// 对话记录，存储所有消息历史
    conversation: Conversation,
    /// 工具执行时的工作目录
    working_directory: PathBuf,
    /// 环境变量映射表
    environment: HashMap<String, String>,
    /// 可扩展的元数据存储
    metadata: HashMap<String, String>,
}

impl AgentContext {
    /// 创建新的代理上下文
    ///
    /// # 参数
    /// - `session_id`: 会话标识符
    /// - `working_directory`: 初始工作目录路径
    ///
    /// # 返回值
    /// 初始化后的空上下文实例
    pub fn new(session_id: SessionId, working_directory: PathBuf) -> Self {
        Self {
            session_id,
            conversation: Conversation::new(),
            working_directory,
            environment: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    /// 获取会话标识符的引用
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// 设置系统提示词
    ///
    /// 会先清空现有对话，然后添加一条系统消息作为开头。
    /// 通常在代理初始化阶段调用。
    ///
    /// # 参数
    /// - `prompt`: 系统提示词内容
    pub fn set_system_prompt(&mut self, prompt: impl Into<String>) {
        // 保留已有的非系统消息
        let existing_messages: Vec<Message> = self
            .conversation
            .get_messages()
            .iter()
            .filter(|m| m.role != ceair_core::message::Role::System)
            .cloned()
            .collect();

        // 用新的系统提示词重建对话
        self.conversation = Conversation::with_system_prompt(prompt);
        for msg in existing_messages {
            self.conversation.add_message(msg);
        }
    }

    /// 添加一条用户消息到对话中
    ///
    /// # 参数
    /// - `content`: 用户消息的文本内容
    pub fn add_user_message(&mut self, content: impl Into<String>) {
        self.conversation.add_message(Message::user(content));
    }

    /// 添加一条助手消息到对话中
    ///
    /// # 参数
    /// - `content`: 助手消息的文本内容
    pub fn add_assistant_message(&mut self, content: impl Into<String>) {
        self.conversation.add_message(Message::assistant(content));
    }

    /// 添加工具执行结果到对话中
    ///
    /// 将 `ToolResult` 转换为 Tool 角色的消息并追加到对话历史。
    ///
    /// # 参数
    /// - `result`: 工具执行结果
    pub fn add_tool_result(&mut self, result: ToolResult) {
        self.conversation.add_message(result.into_message());
    }

    /// 获取对话记录的不可变引用
    pub fn get_conversation(&self) -> &Conversation {
        &self.conversation
    }

    /// 获取对话记录的可变引用
    pub fn get_conversation_mut(&mut self) -> &mut Conversation {
        &mut self.conversation
    }

    /// 设置工作目录
    ///
    /// # 参数
    /// - `path`: 新的工作目录路径
    pub fn set_working_dir(&mut self, path: PathBuf) {
        self.working_directory = path;
    }

    /// 获取当前工作目录的引用
    pub fn get_working_dir(&self) -> &PathBuf {
        &self.working_directory
    }

    /// 设置环境变量
    ///
    /// # 参数
    /// - `key`: 环境变量名
    /// - `value`: 环境变量值
    pub fn set_env(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.environment.insert(key.into(), value.into());
    }

    /// 获取指定环境变量的值
    pub fn get_env(&self, key: &str) -> Option<&str> {
        self.environment.get(key).map(|s| s.as_str())
    }

    /// 获取所有环境变量的引用
    pub fn environment(&self) -> &HashMap<String, String> {
        &self.environment
    }

    /// 设置元数据项
    ///
    /// # 参数
    /// - `key`: 元数据键名
    /// - `value`: 元数据值
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// 获取指定元数据项的值
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }

    /// 获取所有元数据的引用
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }

    /// 清空对话记录、环境变量和元数据
    ///
    /// 保留 session_id 和 working_directory 不变。
    pub fn clear(&mut self) {
        self.conversation.clear();
        self.environment.clear();
        self.metadata.clear();
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use ceair_core::message::Role;

    /// 测试创建新的上下文实例
    #[test]
    fn test_context_new() {
        let session_id = SessionId::new();
        let work_dir = PathBuf::from("/test/project");
        let ctx = AgentContext::new(session_id.clone(), work_dir.clone());

        assert_eq!(ctx.session_id(), &session_id);
        assert_eq!(ctx.get_working_dir(), &work_dir);
        assert!(ctx.get_conversation().is_empty());
        assert!(ctx.environment().is_empty());
        assert!(ctx.metadata().is_empty());
    }

    /// 测试设置和获取系统提示词
    #[test]
    fn test_set_system_prompt() {
        let mut ctx = AgentContext::new(SessionId::new(), PathBuf::from("."));

        ctx.set_system_prompt("你是一个AI编程助手");

        let conv = ctx.get_conversation();
        assert_eq!(conv.len(), 1);
        assert_eq!(conv.get_system_prompt(), Some("你是一个AI编程助手"));
    }

    /// 测试设置系统提示词时保留已有的非系统消息
    #[test]
    fn test_set_system_prompt_preserves_messages() {
        let mut ctx = AgentContext::new(SessionId::new(), PathBuf::from("."));

        ctx.set_system_prompt("初始提示词");
        ctx.add_user_message("用户消息");
        ctx.add_assistant_message("助手回复");

        // 更新系统提示词
        ctx.set_system_prompt("更新后的提示词");

        let conv = ctx.get_conversation();
        // 应包含：新系统提示 + 用户消息 + 助手回复
        assert_eq!(conv.len(), 3);
        assert_eq!(conv.get_system_prompt(), Some("更新后的提示词"));
    }

    /// 测试添加各类消息
    #[test]
    fn test_add_messages() {
        let mut ctx = AgentContext::new(SessionId::new(), PathBuf::from("."));

        ctx.add_user_message("你好");
        ctx.add_assistant_message("你好！有什么可以帮助你的？");

        let messages = ctx.get_conversation().get_messages();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[1].role, Role::Assistant);
    }

    /// 测试添加工具结果消息
    #[test]
    fn test_add_tool_result() {
        let mut ctx = AgentContext::new(SessionId::new(), PathBuf::from("."));

        let result = ToolResult::success("call_123", "文件内容...");
        ctx.add_tool_result(result);

        let messages = ctx.get_conversation().get_messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, Role::Tool);
        assert_eq!(messages[0].tool_call_id.as_deref(), Some("call_123"));
    }

    /// 测试工作目录的设置和获取
    #[test]
    fn test_working_directory() {
        let mut ctx = AgentContext::new(SessionId::new(), PathBuf::from("/initial"));

        assert_eq!(ctx.get_working_dir(), &PathBuf::from("/initial"));

        ctx.set_working_dir(PathBuf::from("/updated"));
        assert_eq!(ctx.get_working_dir(), &PathBuf::from("/updated"));
    }

    /// 测试环境变量的设置和获取
    #[test]
    fn test_environment_variables() {
        let mut ctx = AgentContext::new(SessionId::new(), PathBuf::from("."));

        ctx.set_env("RUST_LOG", "debug");
        ctx.set_env("HOME", "/home/user");

        assert_eq!(ctx.get_env("RUST_LOG"), Some("debug"));
        assert_eq!(ctx.get_env("HOME"), Some("/home/user"));
        assert_eq!(ctx.get_env("NONEXISTENT"), None);
        assert_eq!(ctx.environment().len(), 2);
    }

    /// 测试元数据的设置和获取
    #[test]
    fn test_metadata() {
        let mut ctx = AgentContext::new(SessionId::new(), PathBuf::from("."));

        ctx.set_metadata("model", "deepseek-chat");
        ctx.set_metadata("version", "0.1.0");

        assert_eq!(ctx.get_metadata("model"), Some("deepseek-chat"));
        assert_eq!(ctx.get_metadata("version"), Some("0.1.0"));
        assert_eq!(ctx.get_metadata("missing"), None);
        assert_eq!(ctx.metadata().len(), 2);
    }

    /// 测试清空上下文
    #[test]
    fn test_clear() {
        let mut ctx = AgentContext::new(SessionId::new(), PathBuf::from("/work"));

        ctx.set_system_prompt("系统提示");
        ctx.add_user_message("消息");
        ctx.set_env("KEY", "VALUE");
        ctx.set_metadata("k", "v");

        ctx.clear();

        // 对话、环境变量和元数据应被清空
        assert!(ctx.get_conversation().is_empty());
        assert!(ctx.environment().is_empty());
        assert!(ctx.metadata().is_empty());

        // 工作目录应保持不变
        assert_eq!(ctx.get_working_dir(), &PathBuf::from("/work"));
    }
}

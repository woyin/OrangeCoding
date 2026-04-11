//! # Fork Agent 模式
//!
//! 派生子 Agent 继承父对话历史，独立执行子任务。
//!
//! # 设计思想
//! 参考 reference 中 SubAgent 和 fork 的设计：
//! - 子 Agent 克隆父 context（共享 prompt cache 基础）
//! - 工具过滤通过 can_use_tool 回调，而非删除工具
//! - 独立执行循环，不影响父 transcript
//! - 防递归：限制 fork 深度（最大 3 层）

/// 最大 fork 深度
pub const MAX_FORK_DEPTH: u32 = 3;

// ---------------------------------------------------------------------------
// 工具过滤策略
// ---------------------------------------------------------------------------

/// 工具过滤模式
///
/// 控制子 Agent 可以使用哪些工具
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolFilter {
    /// 允许所有工具
    AllowAll,
    /// 只允许列表中的工具
    AllowList(Vec<String>),
    /// 禁止列表中的工具，其他都允许
    DenyList(Vec<String>),
}

impl ToolFilter {
    /// 判断指定工具是否可用
    pub fn can_use(&self, tool_name: &str) -> bool {
        match self {
            Self::AllowAll => true,
            Self::AllowList(allowed) => allowed.iter().any(|t| t == tool_name),
            Self::DenyList(denied) => !denied.iter().any(|t| t == tool_name),
        }
    }
}

impl Default for ToolFilter {
    fn default() -> Self {
        Self::AllowAll
    }
}

// ---------------------------------------------------------------------------
// Fork 配置
// ---------------------------------------------------------------------------

/// Fork 子 Agent 配置
#[derive(Clone, Debug)]
pub struct ForkConfig {
    /// 子 Agent 最大执行轮次
    pub max_turns: usize,
    /// 工具过滤策略
    pub tool_filter: ToolFilter,
    /// 是否跳过父 transcript 复制（仅继承 system prompt）
    pub skip_transcript: bool,
    /// 当前 fork 深度
    pub fork_depth: u32,
    /// fork 子 Agent 的专用指令
    pub instruction: String,
}

impl Default for ForkConfig {
    fn default() -> Self {
        Self {
            max_turns: 10,
            tool_filter: ToolFilter::AllowAll,
            skip_transcript: false,
            fork_depth: 0,
            instruction: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Fork 结果
// ---------------------------------------------------------------------------

/// Fork 子 Agent 执行结果
#[derive(Clone, Debug)]
pub struct ForkResult {
    /// 最终回复内容
    pub final_response: String,
    /// 子 Agent 执行的轮次数
    pub turns_used: usize,
    /// token 使用量
    pub token_usage: TokenUsage,
    /// 是否因达到 max_turns 而终止
    pub truncated: bool,
}

/// Token 使用量
#[derive(Clone, Debug, Default)]
pub struct TokenUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
}

impl TokenUsage {
    pub fn total(&self) -> usize {
        self.input_tokens + self.output_tokens
    }
}

// ---------------------------------------------------------------------------
// Fork 上下文
// ---------------------------------------------------------------------------

/// Fork 父上下文（用于派生子 Agent）
///
/// 轻量结构，只包含创建子 Agent 需要的信息
#[derive(Clone, Debug)]
pub struct ForkParentContext {
    /// 系统提示词
    pub system_prompt: String,
    /// 对话历史（消息列表的简化表示）
    pub conversation_messages: Vec<String>,
    /// 当前 fork 深度
    pub current_depth: u32,
}

/// Fork 请求验证结果
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForkValidation {
    /// 验证通过
    Valid,
    /// 超过最大 fork 深度
    TooDeep(u32),
    /// 缺少指令
    MissingInstruction,
}

/// 验证 fork 请求
///
/// 检查：
/// 1. fork 深度是否在限制内
/// 2. 是否提供了指令
pub fn validate_fork(
    parent: &ForkParentContext,
    config: &ForkConfig,
) -> ForkValidation {
    // 检查深度（实际深度 = 父深度 + 1）
    let new_depth = parent.current_depth + 1;
    if new_depth > MAX_FORK_DEPTH {
        return ForkValidation::TooDeep(new_depth);
    }

    // 检查指令
    if config.instruction.trim().is_empty() {
        return ForkValidation::MissingInstruction;
    }

    ForkValidation::Valid
}

/// 构建子 Agent 的初始消息列表
///
/// 根据配置决定是否继承父对话历史
pub fn build_fork_messages(
    parent: &ForkParentContext,
    config: &ForkConfig,
) -> Vec<String> {
    let mut messages = Vec::new();

    // 始终包含 system prompt
    messages.push(parent.system_prompt.clone());

    // 根据配置决定是否继承对话历史
    if !config.skip_transcript {
        messages.extend(parent.conversation_messages.iter().cloned());
    }

    // 追加 fork 专用指令
    messages.push(config.instruction.clone());

    messages
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- ToolFilter 测试 ---

    #[test]
    fn test_allow_all() {
        let filter = ToolFilter::AllowAll;
        assert!(filter.can_use("read_file"));
        assert!(filter.can_use("bash"));
        assert!(filter.can_use("anything"));
    }

    #[test]
    fn test_allow_list() {
        let filter = ToolFilter::AllowList(vec!["read_file".into(), "grep".into()]);
        assert!(filter.can_use("read_file"));
        assert!(filter.can_use("grep"));
        assert!(!filter.can_use("bash"));
        assert!(!filter.can_use("edit_file"));
    }

    #[test]
    fn test_deny_list() {
        let filter = ToolFilter::DenyList(vec!["bash".into(), "edit_file".into()]);
        assert!(filter.can_use("read_file"));
        assert!(filter.can_use("grep"));
        assert!(!filter.can_use("bash"));
        assert!(!filter.can_use("edit_file"));
    }

    #[test]
    fn test_allow_list_empty() {
        let filter = ToolFilter::AllowList(vec![]);
        assert!(!filter.can_use("anything"));
    }

    #[test]
    fn test_deny_list_empty() {
        let filter = ToolFilter::DenyList(vec![]);
        assert!(filter.can_use("anything"));
    }

    // --- ForkConfig 测试 ---

    #[test]
    fn test_default_config() {
        let config = ForkConfig::default();
        assert_eq!(config.max_turns, 10);
        assert_eq!(config.tool_filter, ToolFilter::AllowAll);
        assert!(!config.skip_transcript);
        assert_eq!(config.fork_depth, 0);
    }

    // --- Fork 验证测试 ---

    fn make_parent(depth: u32) -> ForkParentContext {
        ForkParentContext {
            system_prompt: "You are a helper.".into(),
            conversation_messages: vec!["hello".into(), "world".into()],
            current_depth: depth,
        }
    }

    #[test]
    fn test_validate_valid_fork() {
        let parent = make_parent(0);
        let config = ForkConfig {
            instruction: "检查文件".into(),
            ..Default::default()
        };
        assert_eq!(validate_fork(&parent, &config), ForkValidation::Valid);
    }

    #[test]
    fn test_validate_too_deep() {
        let parent = make_parent(MAX_FORK_DEPTH);
        let config = ForkConfig {
            instruction: "检查文件".into(),
            ..Default::default()
        };
        assert_eq!(
            validate_fork(&parent, &config),
            ForkValidation::TooDeep(MAX_FORK_DEPTH + 1)
        );
    }

    #[test]
    fn test_validate_depth_2_ok() {
        let parent = make_parent(2);
        let config = ForkConfig {
            instruction: "检查".into(),
            ..Default::default()
        };
        // depth 2 + 1 = 3 = MAX_FORK_DEPTH → valid
        assert_eq!(validate_fork(&parent, &config), ForkValidation::Valid);
    }

    #[test]
    fn test_validate_missing_instruction() {
        let parent = make_parent(0);
        let config = ForkConfig::default(); // instruction is empty
        assert_eq!(
            validate_fork(&parent, &config),
            ForkValidation::MissingInstruction
        );
    }

    #[test]
    fn test_validate_whitespace_instruction() {
        let parent = make_parent(0);
        let config = ForkConfig {
            instruction: "   ".into(),
            ..Default::default()
        };
        assert_eq!(
            validate_fork(&parent, &config),
            ForkValidation::MissingInstruction
        );
    }

    // --- build_fork_messages 测试 ---

    #[test]
    fn test_build_messages_with_history() {
        let parent = make_parent(0);
        let config = ForkConfig {
            instruction: "分析代码".into(),
            skip_transcript: false,
            ..Default::default()
        };
        let msgs = build_fork_messages(&parent, &config);
        // system_prompt + 2 history + instruction = 4
        assert_eq!(msgs.len(), 4);
        assert_eq!(msgs[0], "You are a helper.");
        assert_eq!(msgs[1], "hello");
        assert_eq!(msgs[2], "world");
        assert_eq!(msgs[3], "分析代码");
    }

    #[test]
    fn test_build_messages_skip_transcript() {
        let parent = make_parent(0);
        let config = ForkConfig {
            instruction: "分析代码".into(),
            skip_transcript: true,
            ..Default::default()
        };
        let msgs = build_fork_messages(&parent, &config);
        // system_prompt + instruction = 2
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0], "You are a helper.");
        assert_eq!(msgs[1], "分析代码");
    }

    // --- ForkResult 和 TokenUsage 测试 ---

    #[test]
    fn test_token_usage_total() {
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
        };
        assert_eq!(usage.total(), 1500);
    }

    #[test]
    fn test_fork_result() {
        let result = ForkResult {
            final_response: "完成".into(),
            turns_used: 3,
            token_usage: TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
            },
            truncated: false,
        };
        assert_eq!(result.final_response, "完成");
        assert_eq!(result.turns_used, 3);
        assert!(!result.truncated);
    }

    #[test]
    fn test_fork_result_truncated() {
        let result = ForkResult {
            final_response: "部分完成".into(),
            turns_used: 10,
            token_usage: TokenUsage::default(),
            truncated: true,
        };
        assert!(result.truncated);
        assert_eq!(result.turns_used, 10);
    }
}

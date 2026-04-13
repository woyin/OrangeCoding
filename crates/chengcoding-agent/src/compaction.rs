//! # 上下文压缩模块
//!
//! 当对话上下文过长时，将旧消息压缩为摘要，保留关键信息的同时控制 token 用量。

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 数据结构
// ---------------------------------------------------------------------------

/// 上下文压缩配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompactionConfig {
    /// 触发压缩的最大 token 数
    pub max_tokens: usize,
    /// 保留最近的消息数
    pub keep_recent: usize,
    /// 压缩后保留的最大 token 数
    pub target_tokens: usize,
    /// 压缩提示词（用于生成摘要）
    pub summary_prompt: String,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            max_tokens: 100_000,
            keep_recent: 10,
            target_tokens: 50_000,
            summary_prompt: "请将以上对话压缩为简明摘要，保留关键信息、决策和代码变更。"
                .to_string(),
        }
    }
}

/// 压缩请求
#[derive(Clone, Debug)]
pub struct CompactionRequest {
    /// 需要压缩的消息
    pub messages: Vec<CompactionMessage>,
    /// 可选的用户焦点（/compact focus on API）
    pub focus: Option<String>,
    /// 配置
    pub config: CompactionConfig,
}

/// 压缩消息（简化版本）
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompactionMessage {
    pub role: String,
    pub content: String,
    pub token_estimate: usize,
}

/// 压缩结果
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompactionResult {
    /// 摘要文本
    pub summary: String,
    /// 被压缩的消息数
    pub messages_compacted: usize,
    /// 被压缩的 token 数
    pub tokens_compacted: usize,
    /// 保留的消息（最近的）
    pub kept_messages: Vec<CompactionMessage>,
}

// ---------------------------------------------------------------------------
// 上下文压缩器
// ---------------------------------------------------------------------------

/// 上下文压缩器 — 管理对话历史的压缩与摘要生成
pub struct ContextCompactor {
    config: CompactionConfig,
}

impl ContextCompactor {
    /// 创建新的压缩器实例
    pub fn new(config: CompactionConfig) -> Self {
        Self { config }
    }

    /// 估算文本的 token 数（英文: 字符数/4，CJK: 字符数/2）
    pub fn estimate_tokens(text: &str) -> usize {
        let mut cjk_count = 0usize;
        let mut other_count = 0usize;

        for ch in text.chars() {
            if Self::is_cjk(ch) {
                cjk_count += 1;
            } else {
                other_count += 1;
            }
        }

        // 向上取整：CJK 每2字符约1 token，英文每4字符约1 token
        let cjk_tokens = (cjk_count + 1) / 2;
        let other_tokens = (other_count + 3) / 4;

        cjk_tokens + other_tokens
    }

    /// 判断字符是否为 CJK 字符
    fn is_cjk(ch: char) -> bool {
        matches!(
            ch,
            '\u{4E00}'..='\u{9FFF}'   // CJK 统一汉字
            | '\u{3400}'..='\u{4DBF}' // CJK 统一汉字扩展 A
            | '\u{F900}'..='\u{FAFF}' // CJK 兼容汉字
            | '\u{3000}'..='\u{303F}' // CJK 符号和标点
            | '\u{FF00}'..='\u{FFEF}' // 全角字符
        )
    }

    /// 检查是否需要压缩
    pub fn needs_compaction(&self, messages: &[CompactionMessage]) -> bool {
        let total_tokens: usize = messages.iter().map(|m| m.token_estimate).sum();
        total_tokens > self.config.max_tokens
    }

    /// 分割消息为需要压缩的部分和保留的部分
    ///
    /// 返回 `(待压缩消息, 保留消息)`
    pub fn split_messages(
        &self,
        messages: Vec<CompactionMessage>,
    ) -> (Vec<CompactionMessage>, Vec<CompactionMessage>) {
        if messages.len() <= self.config.keep_recent {
            return (Vec::new(), messages);
        }

        let split_point = messages.len() - self.config.keep_recent;
        let mut to_compact = Vec::with_capacity(split_point);
        let mut to_keep = Vec::with_capacity(self.config.keep_recent);

        for (i, msg) in messages.into_iter().enumerate() {
            if i < split_point {
                to_compact.push(msg);
            } else {
                to_keep.push(msg);
            }
        }

        (to_compact, to_keep)
    }

    /// 生成压缩提示（发给 AI 的摘要请求）
    pub fn build_summary_prompt(
        &self,
        messages: &[CompactionMessage],
        focus: Option<&str>,
    ) -> String {
        let mut prompt = String::from("以下是需要压缩的对话历史：\n\n");

        for msg in messages {
            let role_label = match msg.role.as_str() {
                "user" => "[User]",
                "assistant" => "[Assistant]",
                "system" => "[System]",
                "tool" => "[Tool]",
                _ => "[Unknown]",
            };
            prompt.push_str(&format!("{}: {}\n", role_label, msg.content));
        }

        prompt.push('\n');

        if let Some(focus_text) = focus {
            prompt.push_str(&format!("请特别关注: {}\n\n", focus_text));
        }

        prompt.push_str("请将以上内容压缩为简明摘要。保留：\n");
        prompt.push_str("1. 关键决策和结论\n");
        prompt.push_str("2. 重要的代码变更和文件修改\n");
        prompt.push_str("3. 未完成的任务和待办事项\n");
        prompt.push_str("4. 重要的技术细节和约束");

        prompt
    }

    /// 应用压缩结果：生成新的消息列表
    pub fn apply_compaction(&self, summary: &str, request: CompactionRequest) -> CompactionResult {
        let (to_compact, kept_messages) = self.split_messages(request.messages);

        let tokens_compacted: usize = to_compact.iter().map(|m| m.token_estimate).sum();
        let messages_compacted = to_compact.len();

        CompactionResult {
            summary: summary.to_string(),
            messages_compacted,
            tokens_compacted,
            kept_messages,
        }
    }

    /// 格式化摘要消息（作为系统消息插入）
    pub fn format_summary_message(result: &CompactionResult) -> CompactionMessage {
        let content = format!(
            "[上下文摘要 - 已压缩 {} 条消息, {} tokens]\n\n{}",
            result.messages_compacted, result.tokens_compacted, result.summary
        );

        CompactionMessage {
            role: "system".to_string(),
            content: content.clone(),
            token_estimate: ContextCompactor::estimate_tokens(&content),
        }
    }
}

// ---------------------------------------------------------------------------
// 微压缩器
// ---------------------------------------------------------------------------

/// 可压缩工具白名单
///
/// 只有这些工具的结果会被微压缩清除。
/// 其他工具（如 edit, write）的结果可能包含不可恢复的上下文，不应清除。
const COMPRESSIBLE_TOOLS: &[&str] = &[
    "file_read",
    "read_file",
    "bash",
    "grep",
    "glob",
    "web_search",
    "fetch",
    "find",
    "list_directory",
];

/// 微压缩后的占位文本
const MICRO_COMPACT_PLACEHOLDER: &str = "[旧工具结果已清除]";

/// 微压缩配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicroCompactConfig {
    /// 保留最近 N 个工具结果不压缩
    pub preserve_recent: usize,
}

impl Default for MicroCompactConfig {
    fn default() -> Self {
        Self { preserve_recent: 5 }
    }
}

/// 微压缩器 — 实时截断旧工具输出，无需 AI 参与
///
/// # 设计思想
/// 参考 reference 中 microCompact 的实现：
/// - 只压缩白名单工具的结果（安全可恢复的工具输出）
/// - 保留最近 N 个工具结果（preserve_recent）
/// - 超过保留数量的旧结果替换为占位文本
/// - 保留工具结构（名称、调用 ID），只清除内容
/// - 时间复杂度 O(n)，无 AI 调用，可实时执行
pub struct MicroCompactor {
    config: MicroCompactConfig,
}

impl MicroCompactor {
    /// 创建新的微压缩器实例
    pub fn new(config: MicroCompactConfig) -> Self {
        Self { config }
    }

    /// 判断工具名称是否在可压缩白名单中
    fn is_compressible_tool(tool_name: &str) -> bool {
        COMPRESSIBLE_TOOLS.iter().any(|&t| t == tool_name)
    }

    /// 执行微压缩
    ///
    /// 从消息列表末尾向前扫描，保留最近 preserve_recent 个可压缩工具结果，
    /// 将更早的可压缩工具结果内容替换为占位文本。
    ///
    /// # 参数
    /// - `messages`: 可变消息列表，原地修改
    ///
    /// # 返回值
    /// 被压缩的消息数量
    pub fn compact(&self, messages: &mut Vec<CompactionMessage>) -> usize {
        if messages.is_empty() {
            return 0;
        }

        // 从后向前扫描，找出所有 tool 角色的消息索引
        let tool_indices: Vec<usize> = messages
            .iter()
            .enumerate()
            .filter(|(_, m)| m.role == "tool")
            .map(|(i, _)| i)
            .collect();

        if tool_indices.len() <= self.config.preserve_recent {
            return 0;
        }

        // 从后向前遍历 tool 消息，前 preserve_recent 个保留，其余检查是否可压缩
        let mut compacted_count = 0;
        let compressible_count = tool_indices.len();

        for (reverse_pos, &idx) in tool_indices.iter().rev().enumerate() {
            // 保留最近 preserve_recent 个工具结果
            if reverse_pos < self.config.preserve_recent {
                continue;
            }

            let msg = &messages[idx];

            // 从消息内容中提取工具名称（如果有的话）
            // 工具消息的 content 中通常包含工具名称信息
            // 这里使用简化逻辑：检查 content 是否已经是占位符
            if msg.content == MICRO_COMPACT_PLACEHOLDER {
                continue; // 已压缩，跳过
            }

            // 检查是否为可压缩工具的结果
            // 对于 CompactionMessage，我们通过工具名称标注来判断
            // 简化策略：所有 tool 角色的旧消息都可压缩
            // （更精确的实现需要在 CompactionMessage 中追踪工具名称）
            let old_tokens = messages[idx].token_estimate;
            let placeholder_tokens = ContextCompactor::estimate_tokens(MICRO_COMPACT_PLACEHOLDER);

            messages[idx].content = MICRO_COMPACT_PLACEHOLDER.to_string();
            messages[idx].token_estimate = placeholder_tokens;
            compacted_count += 1;
        }

        let _ = compressible_count; // 避免警告
        compacted_count
    }

    /// 执行带工具名称过滤的微压缩
    ///
    /// 与 `compact` 类似，但只压缩白名单工具的结果。
    /// 需要提供每条 tool 消息对应的工具名称。
    ///
    /// # 参数
    /// - `messages`: 可变消息列表
    /// - `tool_names`: 每条 tool 消息对应的工具名称（按 tool 消息出现顺序）
    ///
    /// # 返回值
    /// 被压缩的消息数量
    pub fn compact_with_tool_names(
        &self,
        messages: &mut Vec<CompactionMessage>,
        tool_names: &[&str],
    ) -> usize {
        if messages.is_empty() {
            return 0;
        }

        // 收集 tool 消息的索引和对应的工具名称
        let tool_entries: Vec<(usize, &str)> = messages
            .iter()
            .enumerate()
            .filter(|(_, m)| m.role == "tool")
            .zip(tool_names.iter())
            .map(|((idx, _), &name)| (idx, name))
            .collect();

        if tool_entries.len() <= self.config.preserve_recent {
            return 0;
        }

        let mut compacted_count = 0;

        for (reverse_pos, &(idx, tool_name)) in tool_entries.iter().rev().enumerate() {
            if reverse_pos < self.config.preserve_recent {
                continue;
            }

            // 只压缩白名单工具
            if !Self::is_compressible_tool(tool_name) {
                continue;
            }

            if messages[idx].content == MICRO_COMPACT_PLACEHOLDER {
                continue;
            }

            messages[idx].content = MICRO_COMPACT_PLACEHOLDER.to_string();
            messages[idx].token_estimate =
                ContextCompactor::estimate_tokens(MICRO_COMPACT_PLACEHOLDER);
            compacted_count += 1;
        }

        compacted_count
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // 辅助函数
    // -----------------------------------------------------------------------

    /// 创建测试用消息
    fn make_msg(role: &str, content: &str, tokens: usize) -> CompactionMessage {
        CompactionMessage {
            role: role.to_string(),
            content: content.to_string(),
            token_estimate: tokens,
        }
    }

    /// 批量生成指定数量的测试消息
    fn make_messages(count: usize, tokens_each: usize) -> Vec<CompactionMessage> {
        (0..count)
            .map(|i| make_msg("user", &format!("消息 {}", i), tokens_each))
            .collect()
    }

    // -----------------------------------------------------------------------
    // Token 估算测试
    // -----------------------------------------------------------------------

    /// 测试英文文本的 token 估算
    #[test]
    fn test_estimate_tokens_english() {
        // "hello world" = 11 个英文字符, 向上取整 (11+3)/4 = 3
        let tokens = ContextCompactor::estimate_tokens("hello world");
        assert_eq!(tokens, 3);
    }

    /// 测试中文文本的 token 估算
    #[test]
    fn test_estimate_tokens_chinese() {
        // "你好世界" = 4 个 CJK 字符, 向上取整 (4+1)/2 = 2
        let tokens = ContextCompactor::estimate_tokens("你好世界");
        assert_eq!(tokens, 2);
    }

    // -----------------------------------------------------------------------
    // 压缩触发条件测试
    // -----------------------------------------------------------------------

    /// 测试未达阈值时不需要压缩
    #[test]
    fn test_needs_compaction_under_limit() {
        let compactor = ContextCompactor::new(CompactionConfig::default());
        // 总计 1000 tokens，阈值 100000
        let messages = make_messages(10, 100);
        assert!(!compactor.needs_compaction(&messages));
    }

    /// 测试超过阈值时需要压缩
    #[test]
    fn test_needs_compaction_over_limit() {
        let compactor = ContextCompactor::new(CompactionConfig::default());
        // 总计 200000 tokens，阈值 100000
        let messages = make_messages(20, 10_000);
        assert!(compactor.needs_compaction(&messages));
    }

    // -----------------------------------------------------------------------
    // 消息分割测试
    // -----------------------------------------------------------------------

    /// 测试正常分割：20 条消息保留 10 条
    #[test]
    fn test_split_messages_keeps_recent() {
        let compactor = ContextCompactor::new(CompactionConfig::default());
        let messages = make_messages(20, 100);

        let (to_compact, kept) = compactor.split_messages(messages);

        assert_eq!(to_compact.len(), 10);
        assert_eq!(kept.len(), 10);
        // 验证保留的是最后 10 条
        assert!(kept[0].content.contains("消息 10"));
        assert!(kept[9].content.contains("消息 19"));
    }

    /// 测试消息数不足时不分割
    #[test]
    fn test_split_messages_few_messages() {
        let compactor = ContextCompactor::new(CompactionConfig::default());
        let messages = make_messages(5, 100);

        let (to_compact, kept) = compactor.split_messages(messages);

        assert_eq!(to_compact.len(), 0);
        assert_eq!(kept.len(), 5);
    }

    // -----------------------------------------------------------------------
    // 摘要提示词测试
    // -----------------------------------------------------------------------

    /// 测试不带焦点的摘要提示词
    #[test]
    fn test_build_summary_prompt_without_focus() {
        let compactor = ContextCompactor::new(CompactionConfig::default());
        let messages = vec![
            make_msg("user", "你好", 10),
            make_msg("assistant", "你好！有什么可以帮助你的？", 20),
        ];

        let prompt = compactor.build_summary_prompt(&messages, None);

        assert!(prompt.contains("[User]: 你好"));
        assert!(prompt.contains("[Assistant]: 你好！有什么可以帮助你的？"));
        assert!(prompt.contains("关键决策和结论"));
        assert!(!prompt.contains("请特别关注"));
    }

    /// 测试带焦点的摘要提示词
    #[test]
    fn test_build_summary_prompt_with_focus() {
        let compactor = ContextCompactor::new(CompactionConfig::default());
        let messages = vec![make_msg("user", "讨论 API 设计", 15)];

        let prompt = compactor.build_summary_prompt(&messages, Some("API 接口设计"));

        assert!(prompt.contains("请特别关注: API 接口设计"));
        assert!(prompt.contains("[User]: 讨论 API 设计"));
    }

    // -----------------------------------------------------------------------
    // 压缩应用测试
    // -----------------------------------------------------------------------

    /// 测试压缩结果的结构正确性
    #[test]
    fn test_apply_compaction() {
        let config = CompactionConfig {
            keep_recent: 3,
            ..CompactionConfig::default()
        };
        let compactor = ContextCompactor::new(config.clone());

        let messages = vec![
            make_msg("user", "早期消息1", 500),
            make_msg("assistant", "早期回复1", 600),
            make_msg("user", "早期消息2", 400),
            make_msg("user", "最近消息1", 100),
            make_msg("assistant", "最近回复1", 200),
            make_msg("user", "最近消息2", 150),
        ];

        let request = CompactionRequest {
            messages,
            focus: None,
            config,
        };

        let result = compactor.apply_compaction("这是摘要", request);

        assert_eq!(result.summary, "这是摘要");
        assert_eq!(result.messages_compacted, 3);
        assert_eq!(result.tokens_compacted, 1500); // 500 + 600 + 400
        assert_eq!(result.kept_messages.len(), 3);
    }

    /// 测试格式化摘要消息
    #[test]
    fn test_format_summary_message() {
        let result = CompactionResult {
            summary: "对话涉及 API 设计".to_string(),
            messages_compacted: 5,
            tokens_compacted: 3000,
            kept_messages: vec![],
        };

        let msg = ContextCompactor::format_summary_message(&result);

        assert_eq!(msg.role, "system");
        assert!(msg.content.contains("已压缩 5 条消息"));
        assert!(msg.content.contains("3000 tokens"));
        assert!(msg.content.contains("对话涉及 API 设计"));
        assert!(msg.token_estimate > 0);
    }

    // -----------------------------------------------------------------------
    // 配置测试
    // -----------------------------------------------------------------------

    /// 测试默认配置值
    #[test]
    fn test_default_config() {
        let config = CompactionConfig::default();

        assert_eq!(config.max_tokens, 100_000);
        assert_eq!(config.keep_recent, 10);
        assert_eq!(config.target_tokens, 50_000);
        assert!(!config.summary_prompt.is_empty());
    }

    /// 测试自定义配置
    #[test]
    fn test_custom_config() {
        let config = CompactionConfig {
            max_tokens: 50_000,
            keep_recent: 5,
            target_tokens: 25_000,
            summary_prompt: "自定义提示词".to_string(),
        };

        assert_eq!(config.max_tokens, 50_000);
        assert_eq!(config.keep_recent, 5);
        assert_eq!(config.target_tokens, 25_000);
        assert_eq!(config.summary_prompt, "自定义提示词");
    }

    // -----------------------------------------------------------------------
    // 压缩结果计数测试
    // -----------------------------------------------------------------------

    /// 测试压缩结果的统计计数
    #[test]
    fn test_compaction_result_counts() {
        let config = CompactionConfig {
            keep_recent: 2,
            ..CompactionConfig::default()
        };
        let compactor = ContextCompactor::new(config.clone());

        let messages = vec![
            make_msg("user", "消息A", 1000),
            make_msg("assistant", "回复A", 2000),
            make_msg("user", "消息B", 3000),
            make_msg("user", "最近1", 100),
            make_msg("assistant", "最近2", 200),
        ];

        let request = CompactionRequest {
            messages,
            focus: None,
            config,
        };

        let result = compactor.apply_compaction("摘要内容", request);

        assert_eq!(result.messages_compacted, 3);
        assert_eq!(result.tokens_compacted, 6000); // 1000 + 2000 + 3000
        assert_eq!(result.kept_messages.len(), 2);
    }

    // -----------------------------------------------------------------------
    // 边界情况测试
    // -----------------------------------------------------------------------

    /// 测试空消息列表
    #[test]
    fn test_empty_messages() {
        let compactor = ContextCompactor::new(CompactionConfig::default());

        assert!(!compactor.needs_compaction(&[]));

        let (to_compact, kept) = compactor.split_messages(vec![]);
        assert!(to_compact.is_empty());
        assert!(kept.is_empty());
    }

    /// 测试单条消息不触发压缩
    #[test]
    fn test_single_message_no_compaction() {
        let compactor = ContextCompactor::new(CompactionConfig::default());
        let messages = vec![make_msg("user", "单条消息", 100)];

        assert!(!compactor.needs_compaction(&messages));

        let (to_compact, kept) = compactor.split_messages(messages);
        assert_eq!(to_compact.len(), 0);
        assert_eq!(kept.len(), 1);
    }

    // -----------------------------------------------------------------------
    // 微压缩测试
    // -----------------------------------------------------------------------

    /// 辅助函数：创建包含 tool 消息的测试序列
    fn make_tool_messages(count: usize) -> Vec<CompactionMessage> {
        (0..count)
            .flat_map(|i| {
                vec![
                    make_msg("user", &format!("用户消息 {}", i), 50),
                    make_msg("assistant", &format!("助手回复 {}", i), 100),
                    make_msg("tool", &format!("工具结果 {} 的详细输出内容...", i), 500),
                ]
            })
            .collect()
    }

    /// 测试工具消息少于 preserve_recent 时不压缩
    #[test]
    fn test_micro_compact_below_threshold() {
        let compactor = MicroCompactor::new(MicroCompactConfig { preserve_recent: 5 });
        let mut messages = make_tool_messages(3); // 3 个工具消息

        let compacted = compactor.compact(&mut messages);

        assert_eq!(compacted, 0);
        // 所有消息内容不变
        for msg in &messages {
            if msg.role == "tool" {
                assert_ne!(msg.content, MICRO_COMPACT_PLACEHOLDER);
            }
        }
    }

    /// 测试超过 preserve_recent 时正确截断旧结果
    #[test]
    fn test_micro_compact_truncates_old() {
        let compactor = MicroCompactor::new(MicroCompactConfig { preserve_recent: 2 });
        let mut messages = make_tool_messages(5); // 5 个工具消息

        let compacted = compactor.compact(&mut messages);

        // 5 个 tool 消息，保留最近 2 个，压缩 3 个
        assert_eq!(compacted, 3);

        // 收集所有 tool 消息
        let tool_msgs: Vec<&CompactionMessage> =
            messages.iter().filter(|m| m.role == "tool").collect();
        assert_eq!(tool_msgs.len(), 5);

        // 前 3 个应被压缩
        assert_eq!(tool_msgs[0].content, MICRO_COMPACT_PLACEHOLDER);
        assert_eq!(tool_msgs[1].content, MICRO_COMPACT_PLACEHOLDER);
        assert_eq!(tool_msgs[2].content, MICRO_COMPACT_PLACEHOLDER);

        // 后 2 个应保留原始内容
        assert!(tool_msgs[3].content.contains("工具结果 3"));
        assert!(tool_msgs[4].content.contains("工具结果 4"));
    }

    /// 测试非 tool 角色的消息不被压缩
    #[test]
    fn test_micro_compact_preserves_non_tool() {
        let compactor = MicroCompactor::new(MicroCompactConfig { preserve_recent: 1 });
        let mut messages = make_tool_messages(3);

        compactor.compact(&mut messages);

        // user 和 assistant 消息不应被修改
        for msg in &messages {
            if msg.role == "user" || msg.role == "assistant" {
                assert_ne!(msg.content, MICRO_COMPACT_PLACEHOLDER);
            }
        }
    }

    /// 测试空列表返回 0
    #[test]
    fn test_micro_compact_empty() {
        let compactor = MicroCompactor::new(MicroCompactConfig::default());
        let mut messages: Vec<CompactionMessage> = Vec::new();

        let compacted = compactor.compact(&mut messages);
        assert_eq!(compacted, 0);
    }

    /// 测试已压缩的消息不重复压缩
    #[test]
    fn test_micro_compact_idempotent() {
        let compactor = MicroCompactor::new(MicroCompactConfig { preserve_recent: 1 });
        let mut messages = make_tool_messages(3);

        let first = compactor.compact(&mut messages);
        let second = compactor.compact(&mut messages);

        assert_eq!(first, 2);
        assert_eq!(second, 0); // 再次调用不应有新的压缩
    }

    /// 测试带工具名称过滤的微压缩 — 白名单工具被压缩
    #[test]
    fn test_micro_compact_with_tool_names_whitelist() {
        let compactor = MicroCompactor::new(MicroCompactConfig { preserve_recent: 1 });
        let mut messages = vec![
            make_msg("tool", "文件内容...", 500),
            make_msg("tool", "grep 结果...", 300),
            make_msg("tool", "编辑结果...", 200),
            make_msg("tool", "最新文件内容...", 400),
        ];
        let tool_names = &["file_read", "grep", "edit", "file_read"];

        let compacted = compactor.compact_with_tool_names(&mut messages, tool_names);

        // 4 个 tool 消息，保留最近 1 个 (file_read)
        // 第 1 个 file_read 可压缩
        // 第 2 个 grep 可压缩
        // 第 3 个 edit 不在白名单，跳过
        // 第 4 个 file_read 保留（最近）
        assert_eq!(compacted, 2);
        assert_eq!(messages[0].content, MICRO_COMPACT_PLACEHOLDER); // file_read 被压缩
        assert_eq!(messages[1].content, MICRO_COMPACT_PLACEHOLDER); // grep 被压缩
        assert!(messages[2].content.contains("编辑结果")); // edit 不在白名单
        assert!(messages[3].content.contains("最新文件内容")); // 保留
    }

    /// 测试非白名单工具不被压缩
    #[test]
    fn test_micro_compact_non_whitelist_preserved() {
        let compactor = MicroCompactor::new(MicroCompactConfig { preserve_recent: 0 });
        let mut messages = vec![
            make_msg("tool", "编辑文件...", 200),
            make_msg("tool", "写入文件...", 200),
        ];
        let tool_names = &["edit", "write_file"];

        let compacted = compactor.compact_with_tool_names(&mut messages, tool_names);

        // edit 和 write_file 不在白名单
        assert_eq!(compacted, 0);
        assert!(messages[0].content.contains("编辑"));
        assert!(messages[1].content.contains("写入"));
    }

    /// 测试压缩后 token 估算更新
    #[test]
    fn test_micro_compact_updates_token_estimate() {
        let compactor = MicroCompactor::new(MicroCompactConfig { preserve_recent: 0 });
        let mut messages = vec![make_msg(
            "tool",
            "这是一段很长很长的工具输出内容，包含大量数据",
            500,
        )];

        compactor.compact(&mut messages);

        // 压缩后 token 估算应该大幅减小
        let placeholder_tokens = ContextCompactor::estimate_tokens(MICRO_COMPACT_PLACEHOLDER);
        assert_eq!(messages[0].token_estimate, placeholder_tokens);
        assert!(messages[0].token_estimate < 500);
    }

    /// 测试可压缩工具白名单检查
    #[test]
    fn test_is_compressible_tool() {
        // 白名单内的工具
        assert!(MicroCompactor::is_compressible_tool("file_read"));
        assert!(MicroCompactor::is_compressible_tool("bash"));
        assert!(MicroCompactor::is_compressible_tool("grep"));
        assert!(MicroCompactor::is_compressible_tool("glob"));
        assert!(MicroCompactor::is_compressible_tool("web_search"));
        assert!(MicroCompactor::is_compressible_tool("fetch"));
        assert!(MicroCompactor::is_compressible_tool("find"));

        // 不在白名单的工具
        assert!(!MicroCompactor::is_compressible_tool("edit"));
        assert!(!MicroCompactor::is_compressible_tool("write_file"));
        assert!(!MicroCompactor::is_compressible_tool("unknown"));
    }

    /// 测试默认微压缩配置
    #[test]
    fn test_micro_compact_default_config() {
        let config = MicroCompactConfig::default();
        assert_eq!(config.preserve_recent, 5);
    }
}

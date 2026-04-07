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
    pub fn apply_compaction(
        &self,
        summary: &str,
        request: CompactionRequest,
    ) -> CompactionResult {
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
}

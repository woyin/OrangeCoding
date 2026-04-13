//! # Prompt Suggestion 引擎
//!
//! 基于上下文预测用户下一步操作的建议系统。
//!
//! # 设计思想
//! 参考 reference 中 PromptSuggestion 的设计：
//! - 建议词数控制在 2-12 个词
//! - 16 个拒绝过滤器排除低质量建议
//! - 抑制条件避免在不合适时机推送建议
//! - 每个建议带有唯一 ID 和置信度

use std::time::Instant;

// ---------------------------------------------------------------------------
// 建议结构
// ---------------------------------------------------------------------------

/// Prompt 建议
#[derive(Clone, Debug)]
pub struct PromptSuggestion {
    /// 建议文本
    pub text: String,
    /// 唯一 ID
    pub prompt_id: String,
    /// 置信度 (0.0 - 1.0)
    pub confidence: f64,
}

// ---------------------------------------------------------------------------
// 拒绝原因
// ---------------------------------------------------------------------------

/// 建议被拒绝的原因
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RejectReason {
    /// 太短（<2 词）
    TooShort,
    /// 太长（>12 词）
    TooLong,
    /// 包含格式化标记（markdown）
    ContainsFormatting,
    /// 包含评价性语言
    ContainsEvaluation,
    /// 包含 AI 腔调
    ContainsAiTone,
    /// 包含错误消息
    ContainsErrorMessage,
    /// 多个句子
    MultipleSentences,
    /// 以标点开头
    StartsWithPunctuation,
    /// 全部大写
    AllUppercase,
    /// 包含 URL
    ContainsUrl,
    /// 纯数字
    PureNumbers,
    /// 包含代码块
    ContainsCodeBlock,
    /// 太短的内容（<5 字符）
    TooFewChars,
    /// 以 "I" 开头的第一人称
    FirstPerson,
    /// 包含问号（不应是问题）
    ContainsQuestion,
    /// 空白内容
    Empty,
}

// ---------------------------------------------------------------------------
// 过滤器实现
// ---------------------------------------------------------------------------

/// 对建议文本应用 16 个拒绝过滤器
///
/// 返回 None 表示通过，返回 Some(reason) 表示被拒绝
pub fn check_suggestion(text: &str) -> Option<RejectReason> {
    let trimmed = text.trim();

    // 过滤器 1: 空白
    if trimmed.is_empty() {
        return Some(RejectReason::Empty);
    }

    // 过滤器 2: 太短（字符）
    if trimmed.len() < 5 {
        return Some(RejectReason::TooFewChars);
    }

    // 过滤器 3: 词数太少
    // 中文文本没有空格分隔词，用字符数估算
    let word_count = if trimmed.chars().any(|c| c > '\u{4E00}' && c < '\u{9FFF}') {
        // CJK 文本：大致每 2 个字符算 1 个词
        let cjk_count = trimmed
            .chars()
            .filter(|c| *c > '\u{4E00}' && *c < '\u{9FFF}')
            .count();
        cjk_count / 2 + trimmed.split_whitespace().count()
    } else {
        trimmed.split_whitespace().count()
    };
    if word_count < 2 {
        return Some(RejectReason::TooShort);
    }

    // 过滤器 4: 词数太多
    if word_count > 12 {
        return Some(RejectReason::TooLong);
    }

    // 过滤器 5: 包含格式化标记
    if trimmed.contains("```")
        || trimmed.contains("**")
        || trimmed.contains("##")
        || trimmed.contains("- [")
    {
        return Some(RejectReason::ContainsFormatting);
    }

    // 过滤器 6: 包含评价性语言
    let eval_words = [
        "great",
        "awesome",
        "perfect",
        "excellent",
        "terrible",
        "bad",
        "good job",
        "well done",
        "很好",
        "太棒了",
        "不错",
    ];
    let lower = trimmed.to_lowercase();
    for word in &eval_words {
        if lower.contains(word) {
            return Some(RejectReason::ContainsEvaluation);
        }
    }

    // 过滤器 7: AI 腔调
    let ai_phrases = [
        "as an ai",
        "i'd be happy to",
        "certainly",
        "absolutely",
        "of course",
        "i can help",
        "作为 AI",
    ];
    for phrase in &ai_phrases {
        if lower.contains(phrase) {
            return Some(RejectReason::ContainsAiTone);
        }
    }

    // 过滤器 8: 包含错误消息
    if lower.contains("error:")
        || lower.contains("failed:")
        || lower.contains("exception")
        || lower.contains("panic")
    {
        return Some(RejectReason::ContainsErrorMessage);
    }

    // 过滤器 9: 多个句子（包含句号后有更多内容）
    let sentence_enders = [". ", "。", "! ", "！"];
    for ender in &sentence_enders {
        if let Some(pos) = trimmed.find(ender) {
            // 句号后面还有实质内容
            let after = &trimmed[pos + ender.len()..];
            if !after.trim().is_empty() {
                return Some(RejectReason::MultipleSentences);
            }
        }
    }

    // 过滤器 10: 以标点开头
    if let Some(c) = trimmed.chars().next() {
        if c.is_ascii_punctuation() {
            return Some(RejectReason::StartsWithPunctuation);
        }
    }

    // 过滤器 11: 全部大写（仅 ASCII 字母）
    let alpha_chars: Vec<char> = trimmed
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect();
    if !alpha_chars.is_empty() && alpha_chars.iter().all(|c| c.is_uppercase()) {
        return Some(RejectReason::AllUppercase);
    }

    // 过滤器 12: 包含 URL
    if lower.contains("http://") || lower.contains("https://") || lower.contains("www.") {
        return Some(RejectReason::ContainsUrl);
    }

    // 过滤器 13: 纯数字
    if trimmed
        .chars()
        .all(|c| c.is_ascii_digit() || c.is_whitespace())
    {
        return Some(RejectReason::PureNumbers);
    }

    // 过滤器 14: 包含代码块
    if trimmed.contains('`') {
        return Some(RejectReason::ContainsCodeBlock);
    }

    // 过滤器 15: 第一人称 "I" 开头
    if trimmed.starts_with("I ") || trimmed.starts_with("I'") {
        return Some(RejectReason::FirstPerson);
    }

    // 过滤器 16: 包含问号
    if trimmed.contains('?') || trimmed.contains('？') {
        return Some(RejectReason::ContainsQuestion);
    }

    None
}

// ---------------------------------------------------------------------------
// 抑制条件
// ---------------------------------------------------------------------------

/// 建议抑制条件
#[derive(Clone, Debug)]
pub struct SuppressionState {
    /// 对话轮数
    pub conversation_turns: usize,
    /// 是否处于错误状态
    pub in_error_state: bool,
    /// 上次建议的时间
    pub last_suggestion_time: Option<Instant>,
    /// 最小建议间隔（秒）
    pub min_interval_secs: u64,
}

impl Default for SuppressionState {
    fn default() -> Self {
        Self {
            conversation_turns: 0,
            in_error_state: false,
            last_suggestion_time: None,
            min_interval_secs: 5,
        }
    }
}

impl SuppressionState {
    /// 检查是否应该抑制建议
    ///
    /// 返回 true 表示应该抑制（不输出建议）
    pub fn should_suppress(&self) -> bool {
        // 早期对话不建议
        if self.conversation_turns < 3 {
            return true;
        }

        // 错误状态不建议
        if self.in_error_state {
            return true;
        }

        // 限速：短时间内不重复建议
        if let Some(last) = self.last_suggestion_time {
            if last.elapsed().as_secs() < self.min_interval_secs {
                return true;
            }
        }

        false
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_suggestion_passes() {
        assert!(check_suggestion("Add unit tests for parser").is_none());
    }

    #[test]
    fn test_valid_chinese_suggestion() {
        assert!(check_suggestion("添加解析器的单元测试").is_none());
    }

    #[test]
    fn test_reject_empty() {
        assert_eq!(check_suggestion(""), Some(RejectReason::Empty));
        assert_eq!(check_suggestion("   "), Some(RejectReason::Empty));
    }

    #[test]
    fn test_reject_too_few_chars() {
        assert_eq!(check_suggestion("hi"), Some(RejectReason::TooFewChars));
    }

    #[test]
    fn test_reject_too_short() {
        assert_eq!(check_suggestion("hello"), Some(RejectReason::TooShort));
    }

    #[test]
    fn test_reject_too_long() {
        let long = "one two three four five six seven eight nine ten eleven twelve thirteen";
        assert_eq!(check_suggestion(long), Some(RejectReason::TooLong));
    }

    #[test]
    fn test_reject_formatting() {
        assert_eq!(
            check_suggestion("add **bold** text"),
            Some(RejectReason::ContainsFormatting)
        );
        assert_eq!(
            check_suggestion("use ```code``` block"),
            Some(RejectReason::ContainsFormatting)
        );
    }

    #[test]
    fn test_reject_evaluation() {
        assert_eq!(
            check_suggestion("great job implementing this"),
            Some(RejectReason::ContainsEvaluation)
        );
    }

    #[test]
    fn test_reject_ai_tone() {
        assert_eq!(
            check_suggestion("certainly I can do"),
            Some(RejectReason::ContainsAiTone)
        );
    }

    #[test]
    fn test_reject_error_message() {
        assert_eq!(
            check_suggestion("handle error: not found"),
            Some(RejectReason::ContainsErrorMessage)
        );
    }

    #[test]
    fn test_reject_multiple_sentences() {
        assert_eq!(
            check_suggestion("Do this. Then that"),
            Some(RejectReason::MultipleSentences)
        );
    }

    #[test]
    fn test_reject_punctuation_start() {
        assert_eq!(
            check_suggestion("...continue working"),
            Some(RejectReason::StartsWithPunctuation)
        );
    }

    #[test]
    fn test_reject_all_uppercase() {
        assert_eq!(
            check_suggestion("ADD MORE TESTS"),
            Some(RejectReason::AllUppercase)
        );
    }

    #[test]
    fn test_reject_url() {
        assert_eq!(
            check_suggestion("check https://example.com for docs"),
            Some(RejectReason::ContainsUrl)
        );
    }

    #[test]
    fn test_reject_pure_numbers() {
        assert_eq!(check_suggestion("123 456"), Some(RejectReason::PureNumbers));
    }

    #[test]
    fn test_reject_code_block() {
        assert_eq!(
            check_suggestion("run `cargo test` now"),
            Some(RejectReason::ContainsCodeBlock)
        );
    }

    #[test]
    fn test_reject_first_person() {
        assert_eq!(
            check_suggestion("I would add tests"),
            Some(RejectReason::FirstPerson)
        );
    }

    #[test]
    fn test_reject_question() {
        assert_eq!(
            check_suggestion("what should we do?"),
            Some(RejectReason::ContainsQuestion)
        );
    }

    #[test]
    fn test_suppression_early_conversation() {
        let state = SuppressionState {
            conversation_turns: 1,
            ..Default::default()
        };
        assert!(state.should_suppress());
    }

    #[test]
    fn test_suppression_error_state() {
        let state = SuppressionState {
            conversation_turns: 10,
            in_error_state: true,
            ..Default::default()
        };
        assert!(state.should_suppress());
    }

    #[test]
    fn test_suppression_rate_limit() {
        let state = SuppressionState {
            conversation_turns: 10,
            in_error_state: false,
            last_suggestion_time: Some(Instant::now()),
            min_interval_secs: 60,
        };
        assert!(state.should_suppress());
    }

    #[test]
    fn test_no_suppression_normal() {
        let state = SuppressionState {
            conversation_turns: 10,
            in_error_state: false,
            last_suggestion_time: None,
            min_interval_secs: 5,
        };
        assert!(!state.should_suppress());
    }

    #[test]
    fn test_prompt_suggestion_struct() {
        let s = PromptSuggestion {
            text: "Run tests".to_string(),
            prompt_id: "ps-001".to_string(),
            confidence: 0.85,
        };
        assert_eq!(s.confidence, 0.85);
        assert_eq!(s.prompt_id, "ps-001");
    }
}

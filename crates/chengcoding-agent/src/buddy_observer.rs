//! # 异步观察者模式 (Buddy Observer)
//!
//! Buddy 的异步反应生成：后台观察对话，产生简短评论。
//!
//! # 设计思想
//! 参考 reference 中的 Buddy 反应机制：
//! - 作为 Fork 子 Agent 在后台运行，不阻塞主 Agent
//! - 只观察最近 N 条消息，生成简短评论
//! - 反应有限的显示时间（display_ticks），自然消失
//! - 通过 channel 异步返回结果

use super::buddy::BuddyIdentity;

// ---------------------------------------------------------------------------
// Buddy 反应
// ---------------------------------------------------------------------------

/// Buddy 的反应消息
///
/// display_ticks 表示反应在 UI 上显示的剩余刷新次数，
/// 每次主循环迭代减 1，减到 0 后不再显示
#[derive(Clone, Debug)]
pub struct BuddyReaction {
    /// 反应文本
    pub text: String,
    /// 产生反应的 buddy 身份
    pub buddy: BuddyIdentity,
    /// 时间戳
    pub timestamp: std::time::Instant,
    /// 显示剩余次数（默认 20）
    pub display_ticks: u32,
}

/// 默认显示次数
const DEFAULT_DISPLAY_TICKS: u32 = 20;

impl BuddyReaction {
    /// 创建新的反应
    pub fn new(text: String, buddy: BuddyIdentity) -> Self {
        Self {
            text,
            buddy,
            timestamp: std::time::Instant::now(),
            display_ticks: DEFAULT_DISPLAY_TICKS,
        }
    }

    /// 消耗一个显示 tick
    ///
    /// 返回 true 表示仍然可见，false 表示应该移除
    pub fn tick(&mut self) -> bool {
        if self.display_ticks > 0 {
            self.display_ticks -= 1;
        }
        self.display_ticks > 0
    }

    /// 是否仍然可见
    pub fn is_visible(&self) -> bool {
        self.display_ticks > 0
    }

    /// 格式化显示文本
    pub fn display(&self) -> String {
        format!("{}: {}", self.buddy.display(), self.text)
    }
}

// ---------------------------------------------------------------------------
// 观察者配置
// ---------------------------------------------------------------------------

/// 观察者配置
#[derive(Clone, Debug)]
pub struct ObserverConfig {
    /// 最近 N 条消息作为观察输入
    pub recent_messages: usize,
    /// 自定义系统提示模板（{buddy_name} 会被替换）
    pub system_prompt_template: String,
}

impl Default for ObserverConfig {
    fn default() -> Self {
        Self {
            recent_messages: 5,
            system_prompt_template: "你是 {buddy_name}，简短评论你观察到的内容，一句话即可。"
                .to_string(),
        }
    }
}

impl ObserverConfig {
    /// 构建系统提示
    pub fn build_system_prompt(&self, buddy_name: &str) -> String {
        self.system_prompt_template
            .replace("{buddy_name}", buddy_name)
    }
}

// ---------------------------------------------------------------------------
// 观察者输入
// ---------------------------------------------------------------------------

/// 观察者输入：最近的对话消息
#[derive(Clone, Debug)]
pub struct ObserverInput {
    pub messages: Vec<String>,
}

impl ObserverInput {
    /// 检查输入是否为空
    ///
    /// 空消息不应触发观察
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty() || self.messages.iter().all(|m| m.trim().is_empty())
    }
}

// ---------------------------------------------------------------------------
// 提取反应文本
// ---------------------------------------------------------------------------

/// 从 Agent 返回的文本中提取第一个有意义的文本块
///
/// 跳过空行、格式标记、代码块内部内容
pub fn extract_reaction_text(raw_output: &str) -> Option<String> {
    let trimmed = raw_output.trim();
    if trimmed.is_empty() {
        return None;
    }

    // 跳过代码块内部内容
    let mut in_code_block = false;
    for line in trimmed.lines() {
        let line = line.trim();
        if line.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        // 跳过空行和分隔线
        if line.is_empty() || line.starts_with("---") {
            continue;
        }
        return Some(line.to_string());
    }

    None
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_buddy() -> BuddyIdentity {
        BuddyIdentity {
            name: "Echo".to_string(),
            emoji: "🦊".to_string(),
        }
    }

    #[test]
    fn test_reaction_creation() {
        let r = BuddyReaction::new("看起来不错".to_string(), test_buddy());
        assert_eq!(r.text, "看起来不错");
        assert_eq!(r.display_ticks, DEFAULT_DISPLAY_TICKS);
        assert!(r.is_visible());
    }

    #[test]
    fn test_default_display_ticks() {
        let r = BuddyReaction::new("test".to_string(), test_buddy());
        assert_eq!(r.display_ticks, 20);
    }

    #[test]
    fn test_tick_decrements() {
        let mut r = BuddyReaction::new("test".to_string(), test_buddy());
        r.display_ticks = 3;
        assert!(r.tick()); // 2 left
        assert!(r.tick()); // 1 left
        assert!(!r.tick()); // 0 left
        assert!(!r.is_visible());
    }

    #[test]
    fn test_tick_at_zero_stays_zero() {
        let mut r = BuddyReaction::new("test".to_string(), test_buddy());
        r.display_ticks = 0;
        assert!(!r.tick());
        assert_eq!(r.display_ticks, 0);
    }

    #[test]
    fn test_display_format() {
        let r = BuddyReaction::new("好的设计".to_string(), test_buddy());
        let display = r.display();
        assert!(display.contains("🦊 Echo"));
        assert!(display.contains("好的设计"));
    }

    #[test]
    fn test_observer_config_default() {
        let config = ObserverConfig::default();
        assert_eq!(config.recent_messages, 5);
    }

    #[test]
    fn test_build_system_prompt() {
        let config = ObserverConfig::default();
        let prompt = config.build_system_prompt("Echo");
        assert!(prompt.contains("Echo"));
        assert!(!prompt.contains("{buddy_name}"));
    }

    #[test]
    fn test_observer_input_empty() {
        let input = ObserverInput { messages: vec![] };
        assert!(input.is_empty());
    }

    #[test]
    fn test_observer_input_whitespace_only() {
        let input = ObserverInput {
            messages: vec!["  ".to_string(), "\n".to_string()],
        };
        assert!(input.is_empty());
    }

    #[test]
    fn test_observer_input_not_empty() {
        let input = ObserverInput {
            messages: vec!["hello".to_string()],
        };
        assert!(!input.is_empty());
    }

    #[test]
    fn test_extract_reaction_text_simple() {
        assert_eq!(
            extract_reaction_text("看起来不错"),
            Some("看起来不错".to_string())
        );
    }

    #[test]
    fn test_extract_reaction_text_skip_empty_lines() {
        assert_eq!(
            extract_reaction_text("\n\n有意思的实现\n\n"),
            Some("有意思的实现".to_string())
        );
    }

    #[test]
    fn test_extract_reaction_text_skip_code_block() {
        assert_eq!(
            extract_reaction_text("```\ncode\n```\n实际反应"),
            Some("实际反应".to_string())
        );
    }

    #[test]
    fn test_extract_reaction_text_empty() {
        assert_eq!(extract_reaction_text(""), None);
        assert_eq!(extract_reaction_text("   "), None);
    }

    #[test]
    fn test_extract_reaction_text_only_formatting() {
        assert_eq!(extract_reaction_text("```\n---\n```"), None);
    }
}

//! # TTSR（时间旅行流式规则）模块
//!
//! 在模型输出流中基于正则触发器注入规则内容。
//! 这些规则在 token 计数之前修改流，因此"零成本"。

use regex::Regex;
use serde::{Deserialize, Serialize};

/// TTSR 规则定义
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TtsrRule {
    /// 规则名称
    pub name: String,
    /// 触发正则表达式
    pub trigger: String,
    /// 注入的内容（插入到输出流中）
    pub injection: String,
    /// 是否只触发一次
    pub once: bool,
    /// 优先级（数值越大优先级越高）
    pub priority: i32,
}

/// 编译后的规则（内部使用）
struct CompiledRule {
    rule: TtsrRule,
    regex: Regex,
    /// 标记该规则是否已触发过
    fired: bool,
}

/// TTSR 处理结果
#[derive(Clone, Debug)]
pub struct TtsrResult {
    /// 原始文本
    pub original: String,
    /// 处理后的文本（含注入）
    pub processed: String,
    /// 触发的规则名称列表
    pub triggered_rules: Vec<String>,
}

/// TTSR 引擎，管理规则并处理输出流
pub struct TtsrEngine {
    rules: Vec<CompiledRule>,
}

impl TtsrEngine {
    /// 创建空的 TTSR 引擎
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// 添加规则，正则无效时返回错误
    pub fn add_rule(&mut self, rule: TtsrRule) -> Result<(), String> {
        let regex = Regex::new(&rule.trigger)
            .map_err(|e| format!("无效的正则表达式 '{}': {}", rule.trigger, e))?;
        self.rules.push(CompiledRule {
            rule,
            regex,
            fired: false,
        });
        // 按优先级降序排列，保证高优先级规则先处理
        self.rules
            .sort_by(|a, b| b.rule.priority.cmp(&a.rule.priority));
        Ok(())
    }

    /// 处理输出流片段，匹配规则并注入内容
    pub fn process_chunk(&mut self, chunk: &str) -> TtsrResult {
        self.process_text(chunk)
    }

    /// 处理完整文本，匹配规则并注入内容
    pub fn process_text(&mut self, text: &str) -> TtsrResult {
        let original = text.to_string();
        let mut processed = text.to_string();
        let mut triggered_rules = Vec::new();

        for compiled in self.rules.iter_mut() {
            // 如果是一次性规则且已触发，跳过
            if compiled.rule.once && compiled.fired {
                continue;
            }

            if compiled.regex.is_match(&processed) {
                // 在匹配位置之后注入内容
                let injected = compiled
                    .regex
                    .replace_all(&processed, |caps: &regex::Captures| {
                        format!("{}{}", &caps[0], compiled.rule.injection)
                    })
                    .to_string();
                processed = injected;
                compiled.fired = true;
                triggered_rules.push(compiled.rule.name.clone());
            }
        }

        TtsrResult {
            original,
            processed,
            triggered_rules,
        }
    }

    /// 重置所有规则的触发状态
    pub fn reset(&mut self) {
        for compiled in self.rules.iter_mut() {
            compiled.fired = false;
        }
    }

    /// 获取规则数量
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// 按名称移除规则，返回是否成功移除
    pub fn remove_rule(&mut self, name: &str) -> bool {
        let before = self.rules.len();
        self.rules.retain(|c| c.rule.name != name);
        self.rules.len() < before
    }
}

impl Default for TtsrEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助方法：创建测试规则
    fn make_rule(
        name: &str,
        trigger: &str,
        injection: &str,
        once: bool,
        priority: i32,
    ) -> TtsrRule {
        TtsrRule {
            name: name.to_string(),
            trigger: trigger.to_string(),
            injection: injection.to_string(),
            once,
            priority,
        }
    }

    #[test]
    fn test_add_rule() {
        let mut engine = TtsrEngine::new();
        let rule = make_rule("r1", r"hello", "[注入]", false, 0);
        assert!(engine.add_rule(rule).is_ok());
        assert_eq!(engine.rule_count(), 1);
    }

    #[test]
    fn test_process_chunk_no_match() {
        // 没有匹配时，文本不应被修改
        let mut engine = TtsrEngine::new();
        engine
            .add_rule(make_rule("r1", r"xyz", "[注入]", false, 0))
            .unwrap();
        let result = engine.process_chunk("hello world");
        assert_eq!(result.original, "hello world");
        assert_eq!(result.processed, "hello world");
        assert!(result.triggered_rules.is_empty());
    }

    #[test]
    fn test_process_chunk_match() {
        // 匹配时，注入内容追加在匹配文本之后
        let mut engine = TtsrEngine::new();
        engine
            .add_rule(make_rule("r1", r"hello", "[注入]", false, 0))
            .unwrap();
        let result = engine.process_chunk("hello world");
        assert_eq!(result.processed, "hello[注入] world");
        assert_eq!(result.triggered_rules, vec!["r1"]);
    }

    #[test]
    fn test_once_rule_fires_once() {
        // 一次性规则只在第一次匹配时触发
        let mut engine = TtsrEngine::new();
        engine
            .add_rule(make_rule("once", r"hello", "[注入]", true, 0))
            .unwrap();

        let r1 = engine.process_chunk("hello");
        assert_eq!(r1.triggered_rules, vec!["once"]);
        assert_eq!(r1.processed, "hello[注入]");

        // 第二次不再触发
        let r2 = engine.process_chunk("hello");
        assert!(r2.triggered_rules.is_empty());
        assert_eq!(r2.processed, "hello");
    }

    #[test]
    fn test_repeating_rule() {
        // 可重复触发的规则每次都生效
        let mut engine = TtsrEngine::new();
        engine
            .add_rule(make_rule("repeat", r"hi", "[!]", false, 0))
            .unwrap();

        let r1 = engine.process_chunk("hi");
        assert_eq!(r1.triggered_rules, vec!["repeat"]);

        let r2 = engine.process_chunk("hi");
        assert_eq!(r2.triggered_rules, vec!["repeat"]);

        let r3 = engine.process_chunk("hi");
        assert_eq!(r3.triggered_rules, vec!["repeat"]);
    }

    #[test]
    fn test_priority_ordering() {
        // 高优先级规则先于低优先级规则处理
        let mut engine = TtsrEngine::new();
        engine
            .add_rule(make_rule("low", r"text", "[低]", false, 1))
            .unwrap();
        engine
            .add_rule(make_rule("high", r"text", "[高]", false, 10))
            .unwrap();

        let result = engine.process_chunk("text");
        // 高优先级先触发，"text" → "text[高]"，之后低优先级对 "text[高]" 再触发
        assert_eq!(result.triggered_rules, vec!["high", "low"]);
        assert!(result.processed.contains("[高]"));
        assert!(result.processed.contains("[低]"));
    }

    #[test]
    fn test_multiple_rules_same_trigger() {
        // 多条规则匹配相同触发器都会生效
        let mut engine = TtsrEngine::new();
        engine
            .add_rule(make_rule("a", r"foo", "[A]", false, 0))
            .unwrap();
        engine
            .add_rule(make_rule("b", r"foo", "[B]", false, 0))
            .unwrap();

        let result = engine.process_chunk("foo");
        assert_eq!(result.triggered_rules.len(), 2);
        assert!(result.triggered_rules.contains(&"a".to_string()));
        assert!(result.triggered_rules.contains(&"b".to_string()));
    }

    #[test]
    fn test_reset_clears_fired_state() {
        // 重置后一次性规则可以重新触发
        let mut engine = TtsrEngine::new();
        engine
            .add_rule(make_rule("once", r"hello", "[注入]", true, 0))
            .unwrap();

        let r1 = engine.process_chunk("hello");
        assert_eq!(r1.triggered_rules.len(), 1);

        // 第二次不触发
        let r2 = engine.process_chunk("hello");
        assert!(r2.triggered_rules.is_empty());

        // 重置后可以再次触发
        engine.reset();
        let r3 = engine.process_chunk("hello");
        assert_eq!(r3.triggered_rules, vec!["once"]);
    }

    #[test]
    fn test_invalid_regex_error() {
        // 无效正则应返回错误
        let mut engine = TtsrEngine::new();
        let rule = make_rule("bad", r"[invalid", "[注入]", false, 0);
        let result = engine.add_rule(rule);
        assert!(result.is_err());
        assert_eq!(engine.rule_count(), 0);
    }

    #[test]
    fn test_remove_rule() {
        let mut engine = TtsrEngine::new();
        engine
            .add_rule(make_rule("r1", r"a", "[A]", false, 0))
            .unwrap();
        engine
            .add_rule(make_rule("r2", r"b", "[B]", false, 0))
            .unwrap();
        assert_eq!(engine.rule_count(), 2);

        assert!(engine.remove_rule("r1"));
        assert_eq!(engine.rule_count(), 1);

        // 移除不存在的规则返回 false
        assert!(!engine.remove_rule("r1"));
    }

    #[test]
    fn test_rule_count() {
        let mut engine = TtsrEngine::new();
        assert_eq!(engine.rule_count(), 0);
        engine
            .add_rule(make_rule("a", r"x", "[X]", false, 0))
            .unwrap();
        assert_eq!(engine.rule_count(), 1);
        engine
            .add_rule(make_rule("b", r"y", "[Y]", false, 0))
            .unwrap();
        assert_eq!(engine.rule_count(), 2);
    }

    #[test]
    fn test_empty_engine() {
        // 空引擎处理文本应原样返回
        let mut engine = TtsrEngine::new();
        let result = engine.process_text("anything");
        assert_eq!(result.original, "anything");
        assert_eq!(result.processed, "anything");
        assert!(result.triggered_rules.is_empty());
    }
}

//! # 密钥混淆模块
//!
//! 提供密钥混淆功能，支持两种模式：
//! - 占位符模式：用 `<<$env:NAME>>` 替换密钥值
//! - 单向替换模式：用 `***` 替换（不可逆）
//!
//! 系统默认禁用，需要显式启用。

use serde::{Deserialize, Serialize};
use std::sync::RwLock;

/// 密钥混淆模式
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObfuscationMode {
    /// 禁用（默认）
    Disabled,
    /// 占位符模式：用 <<$env:NAME>> 替换
    Placeholder,
    /// 单向替换：用 *** 替换
    Redact,
}

/// 密钥定义
#[derive(Clone, Debug)]
pub struct SecretEntry {
    /// 密钥名称（环境变量名等）
    pub name: String,
    /// 密钥值
    pub value: String,
    /// 来源
    pub source: SecretSource,
}

/// 密钥来源
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecretSource {
    /// 环境变量
    Environment,
    /// 配置文件
    Config,
    /// 用户手动注册
    Manual,
}

/// 默认的环境变量名匹配关键词
const DEFAULT_ENV_PATTERNS: &[&str] = &["KEY", "SECRET", "TOKEN", "PASSWORD", "PASS"];

/// 密钥混淆器
pub struct SecretObfuscator {
    /// 混淆模式
    mode: ObfuscationMode,
    /// 已注册的密钥
    secrets: RwLock<Vec<SecretEntry>>,
    /// 自动检测的环境变量模式
    env_patterns: Vec<String>,
}

impl SecretObfuscator {
    /// 创建新的混淆器（默认禁用）
    pub fn new() -> Self {
        Self::with_mode(ObfuscationMode::Disabled)
    }

    /// 以指定模式创建
    pub fn with_mode(mode: ObfuscationMode) -> Self {
        Self {
            mode,
            secrets: RwLock::new(Vec::new()),
            env_patterns: DEFAULT_ENV_PATTERNS.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// 设置混淆模式
    pub fn set_mode(&mut self, mode: ObfuscationMode) {
        self.mode = mode;
    }

    /// 注册密钥
    pub fn register_secret(&self, name: &str, value: &str, source: SecretSource) {
        let mut secrets = self.secrets.write().unwrap();
        secrets.push(SecretEntry {
            name: name.to_string(),
            value: value.to_string(),
            source,
        });
    }

    /// 从环境变量自动检测密钥
    /// 匹配模式: *_KEY, *_SECRET, *_TOKEN, *_PASSWORD, *_API_KEY 等
    pub fn detect_env_secrets(&self) {
        for (key, value) in std::env::vars() {
            let upper = key.to_uppercase();
            let matches = self.env_patterns.iter().any(|p| upper.contains(p.as_str()));
            if matches {
                // 避免重复注册
                let already = self.secrets.read().unwrap().iter().any(|s| s.name == key);
                if !already {
                    self.register_secret(&key, &value, SecretSource::Environment);
                }
            }
        }
    }

    /// 混淆文本
    pub fn obfuscate(&self, text: &str) -> String {
        if self.mode == ObfuscationMode::Disabled {
            return text.to_string();
        }

        let secrets = self.secrets.read().unwrap();
        let mut result = text.to_string();

        for entry in secrets.iter() {
            // 只混淆长度 >= 8 的密钥值
            if entry.value.len() < 8 {
                continue;
            }

            let replacement = match &self.mode {
                ObfuscationMode::Placeholder => format!("<<$env:{}>>", entry.name),
                ObfuscationMode::Redact => "***".to_string(),
                ObfuscationMode::Disabled => unreachable!(),
            };

            result = result.replace(&entry.value, &replacement);
        }

        result
    }

    /// 反向混淆（仅 Placeholder 模式有效）
    pub fn deobfuscate(&self, text: &str) -> String {
        if self.mode != ObfuscationMode::Placeholder {
            return text.to_string();
        }

        let secrets = self.secrets.read().unwrap();
        let mut result = text.to_string();

        for entry in secrets.iter() {
            let placeholder = format!("<<$env:{}>>", entry.name);
            result = result.replace(&placeholder, &entry.value);
        }

        result
    }

    /// 检查文本中是否包含已知密钥
    pub fn contains_secret(&self, text: &str) -> bool {
        let secrets = self.secrets.read().unwrap();
        secrets
            .iter()
            .any(|entry| entry.value.len() >= 8 && text.contains(&entry.value))
    }

    /// 获取已注册密钥数量
    pub fn secret_count(&self) -> usize {
        self.secrets.read().unwrap().len()
    }

    /// 清除所有密钥
    pub fn clear(&self) {
        self.secrets.write().unwrap().clear();
    }

    /// 获取环境变量匹配模式列表
    pub fn env_patterns(&self) -> &[String] {
        &self.env_patterns
    }
}

impl Default for SecretObfuscator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试：默认模式为 Disabled
    #[test]
    fn test_new_is_disabled() {
        let obfuscator = SecretObfuscator::new();
        assert_eq!(obfuscator.mode, ObfuscationMode::Disabled);
    }

    /// 测试：禁用模式下文本原样返回
    #[test]
    fn test_disabled_mode_no_obfuscation() {
        let obfuscator = SecretObfuscator::new();
        obfuscator.register_secret("OPENAI_API_KEY", "sk-abc123xyz", SecretSource::Manual);
        let text = "my key is sk-abc123xyz";
        assert_eq!(obfuscator.obfuscate(text), text);
    }

    /// 测试：占位符模式替换密钥值
    #[test]
    fn test_placeholder_mode() {
        let obfuscator = SecretObfuscator::with_mode(ObfuscationMode::Placeholder);
        obfuscator.register_secret("OPENAI_API_KEY", "sk-abc123xyz", SecretSource::Manual);
        let result = obfuscator.obfuscate("my key is sk-abc123xyz");
        assert_eq!(result, "my key is <<$env:OPENAI_API_KEY>>");
    }

    /// 测试：单向替换模式用 *** 替换密钥值
    #[test]
    fn test_redact_mode() {
        let obfuscator = SecretObfuscator::with_mode(ObfuscationMode::Redact);
        obfuscator.register_secret("OPENAI_API_KEY", "sk-abc123xyz", SecretSource::Manual);
        let result = obfuscator.obfuscate("my key is sk-abc123xyz");
        assert_eq!(result, "my key is ***");
    }

    /// 测试：注册密钥
    #[test]
    fn test_register_secret() {
        let obfuscator = SecretObfuscator::new();
        obfuscator.register_secret("MY_KEY", "secret12", SecretSource::Manual);
        assert_eq!(obfuscator.secret_count(), 1);
    }

    /// 测试：注册多个密钥
    #[test]
    fn test_register_multiple_secrets() {
        let obfuscator = SecretObfuscator::new();
        obfuscator.register_secret("KEY_A", "value_aaa", SecretSource::Manual);
        obfuscator.register_secret("KEY_B", "value_bbb", SecretSource::Config);
        obfuscator.register_secret("KEY_C", "value_ccc", SecretSource::Environment);
        assert_eq!(obfuscator.secret_count(), 3);
    }

    /// 测试：从环境变量自动检测密钥
    #[test]
    fn test_detect_env_secrets() {
        // 设置测试用的环境变量
        let test_key = "ChengCoding_TEST_SECRET_KEY";
        let test_value = "detect-me-12345678";
        std::env::set_var(test_key, test_value);

        let obfuscator = SecretObfuscator::with_mode(ObfuscationMode::Placeholder);
        obfuscator.detect_env_secrets();

        // 验证该环境变量被检测到
        assert!(
            obfuscator.secret_count() > 0,
            "应至少检测到一个环境变量密钥"
        );
        let result = obfuscator.obfuscate(&format!("val={}", test_value));
        assert!(
            result.contains(&format!("<<$env:{}>>", test_key)),
            "检测到的环境变量密钥应能被混淆"
        );

        // 清理测试环境变量
        std::env::remove_var(test_key);
    }

    /// 测试：替换文本中所有出现的密钥值
    #[test]
    fn test_obfuscate_multiple_occurrences() {
        let obfuscator = SecretObfuscator::with_mode(ObfuscationMode::Redact);
        obfuscator.register_secret("MY_TOKEN", "abcdefgh", SecretSource::Manual);
        let result = obfuscator.obfuscate("first=abcdefgh second=abcdefgh third=abcdefgh");
        assert_eq!(result, "first=*** second=*** third=***");
    }

    /// 测试：文本中无匹配时原样返回
    #[test]
    fn test_obfuscate_no_match() {
        let obfuscator = SecretObfuscator::with_mode(ObfuscationMode::Redact);
        obfuscator.register_secret("MY_TOKEN", "abcdefgh", SecretSource::Manual);
        let text = "no secrets here at all";
        assert_eq!(obfuscator.obfuscate(text), text);
    }

    /// 测试：占位符模式可反向混淆
    #[test]
    fn test_deobfuscate_placeholder() {
        let obfuscator = SecretObfuscator::with_mode(ObfuscationMode::Placeholder);
        obfuscator.register_secret("OPENAI_API_KEY", "sk-abc123xyz", SecretSource::Manual);

        let obfuscated = obfuscator.obfuscate("my key is sk-abc123xyz");
        assert_eq!(obfuscated, "my key is <<$env:OPENAI_API_KEY>>");

        let restored = obfuscator.deobfuscate(&obfuscated);
        assert_eq!(restored, "my key is sk-abc123xyz");
    }

    /// 测试：单向替换模式无法反向混淆
    #[test]
    fn test_deobfuscate_redact_fails() {
        let obfuscator = SecretObfuscator::with_mode(ObfuscationMode::Redact);
        obfuscator.register_secret("OPENAI_API_KEY", "sk-abc123xyz", SecretSource::Manual);

        let obfuscated = obfuscator.obfuscate("my key is sk-abc123xyz");
        assert_eq!(obfuscated, "my key is ***");

        // 单向替换不可逆，反向混淆应返回原文（不变）
        let result = obfuscator.deobfuscate(&obfuscated);
        assert_eq!(result, "my key is ***");
    }

    /// 测试：检测文本中是否包含已知密钥
    #[test]
    fn test_contains_secret() {
        let obfuscator = SecretObfuscator::new();
        obfuscator.register_secret("MY_SECRET", "supersecret123", SecretSource::Manual);
        assert!(obfuscator.contains_secret("the value is supersecret123 here"));
    }

    /// 测试：文本中不包含已知密钥时返回 false
    #[test]
    fn test_contains_secret_no_match() {
        let obfuscator = SecretObfuscator::new();
        obfuscator.register_secret("MY_SECRET", "supersecret123", SecretSource::Manual);
        assert!(!obfuscator.contains_secret("nothing sensitive here"));
    }

    /// 测试：短密钥（< 8 字符）不被混淆
    #[test]
    fn test_short_secret_ignored() {
        let obfuscator = SecretObfuscator::with_mode(ObfuscationMode::Redact);
        obfuscator.register_secret("SHORT", "abc", SecretSource::Manual);
        let text = "value is abc okay";
        assert_eq!(obfuscator.obfuscate(text), text, "短密钥不应被混淆");
    }

    /// 测试：清除所有密钥
    #[test]
    fn test_clear_secrets() {
        let obfuscator = SecretObfuscator::new();
        obfuscator.register_secret("A", "value_aaa", SecretSource::Manual);
        obfuscator.register_secret("B", "value_bbb", SecretSource::Manual);
        assert_eq!(obfuscator.secret_count(), 2);

        obfuscator.clear();
        assert_eq!(obfuscator.secret_count(), 0);
    }

    /// 测试：获取密钥数量
    #[test]
    fn test_secret_count() {
        let obfuscator = SecretObfuscator::new();
        assert_eq!(obfuscator.secret_count(), 0);

        obfuscator.register_secret("X", "longvalue1", SecretSource::Manual);
        assert_eq!(obfuscator.secret_count(), 1);

        obfuscator.register_secret("Y", "longvalue2", SecretSource::Config);
        assert_eq!(obfuscator.secret_count(), 2);
    }

    /// 测试：验证默认环境变量匹配模式列表
    #[test]
    fn test_env_pattern_matching() {
        let obfuscator = SecretObfuscator::new();
        let patterns = obfuscator.env_patterns();

        // 验证包含所有必需的模式关键词
        let required = ["KEY", "SECRET", "TOKEN", "PASSWORD", "PASS"];
        for keyword in &required {
            assert!(
                patterns.iter().any(|p| p == *keyword),
                "环境变量匹配模式应包含 '{}'",
                keyword
            );
        }
    }
}

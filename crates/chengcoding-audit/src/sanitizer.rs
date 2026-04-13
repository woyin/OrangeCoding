//! # 敏感信息脱敏器
//!
//! 使用正则表达式匹配并替换敏感信息，内置常见的敏感数据模式：
//! - API 密钥
//! - 密码
//! - 令牌（Token）
//! - 电子邮件地址
//! - IP 地址
//! - 信用卡号

use regex::Regex;

/// 脱敏模式定义
///
/// 每个模式包含名称和对应的正则表达式。
#[derive(Debug, Clone)]
struct SanitizePattern {
    /// 模式名称（用于标识和管理）
    name: String,

    /// 匹配敏感信息的正则表达式
    regex: Regex,
}

/// 敏感信息脱敏器
///
/// 使用一组可配置的正则表达式模式来检测和替换文本中的敏感信息。
/// 初始化时会自动加载内置的常见敏感数据模式。
#[derive(Debug, Clone)]
pub struct Sanitizer {
    /// 脱敏模式列表
    patterns: Vec<SanitizePattern>,

    /// 替换文本，默认为 "[REDACTED]"
    replacement: String,
}

impl Sanitizer {
    /// 创建新的脱敏器，自动加载所有内置模式
    pub fn new() -> Self {
        let mut sanitizer = Self {
            patterns: Vec::new(),
            replacement: "[REDACTED]".to_string(),
        };

        // 加载所有内置的敏感数据匹配模式
        sanitizer.load_builtin_patterns();
        sanitizer
    }

    /// 对文本执行脱敏处理
    ///
    /// 依次应用所有已注册的模式，将匹配到的敏感信息替换为 "[REDACTED]"。
    ///
    /// # 参数
    /// - `text`: 需要脱敏的原始文本
    ///
    /// # 返回
    /// 脱敏处理后的文本
    pub fn sanitize_text(&self, text: &str) -> String {
        let mut result = text.to_string();

        // 按顺序应用每个脱敏模式
        for pattern in &self.patterns {
            result = pattern
                .regex
                .replace_all(&result, self.replacement.as_str())
                .to_string();
        }

        result
    }

    /// 添加自定义脱敏模式
    ///
    /// # 参数
    /// - `name`: 模式名称
    /// - `pattern`: 正则表达式字符串
    ///
    /// # 返回
    /// 如果正则表达式编译成功返回 `Ok(())`，否则返回错误
    pub fn add_pattern(&mut self, name: &str, pattern: &str) -> Result<(), regex::Error> {
        let regex = Regex::new(pattern)?;
        self.patterns.push(SanitizePattern {
            name: name.to_string(),
            regex,
        });
        Ok(())
    }

    /// 移除指定名称的脱敏模式
    ///
    /// # 参数
    /// - `name`: 要移除的模式名称
    ///
    /// # 返回
    /// 如果成功移除返回 `true`，未找到返回 `false`
    pub fn remove_pattern(&mut self, name: &str) -> bool {
        let original_len = self.patterns.len();
        self.patterns.retain(|p| p.name != name);
        self.patterns.len() < original_len
    }

    /// 加载内置的敏感数据匹配模式
    ///
    /// 包括 API 密钥、密码、令牌、邮箱、IP 地址和信用卡号等模式。
    fn load_builtin_patterns(&mut self) {
        // 内置模式定义：(名称, 正则表达式)
        let builtin_patterns: Vec<(&str, &str)> = vec![
            // API 密钥模式（常见的 sk-、ak-、key- 前缀格式）
            (
                "api_key",
                r"(?i)(?:sk|ak|key|api[_-]?key)[_-]?[a-zA-Z0-9]{16,}",
            ),
            // 密码模式（匹配 password=xxx 或 password: xxx 格式）
            ("password", r#"(?i)(?:password|passwd|pwd)\s*[=:]\s*\S+"#),
            // Bearer 令牌模式
            ("bearer_token", r"(?i)Bearer\s+[a-zA-Z0-9\-._~+/]+=*"),
            // 通用令牌/密钥模式（匹配 token=xxx 或 secret=xxx 格式）
            (
                "token_secret",
                r#"(?i)(?:token|secret|credential)\s*[=:]\s*\S+"#,
            ),
            // 电子邮件地址
            ("email", r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}"),
            // IPv4 地址
            (
                "ipv4",
                r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b",
            ),
            // 信用卡号（匹配常见的信用卡号格式：16位数字，可能有空格或横线分隔）
            ("credit_card", r"\b(?:\d{4}[- ]?){3}\d{4}\b"),
        ];

        // 逐个编译并注册内置模式
        for (name, pattern) in builtin_patterns {
            match Regex::new(pattern) {
                Ok(regex) => {
                    self.patterns.push(SanitizePattern {
                        name: name.to_string(),
                        regex,
                    });
                }
                Err(e) => {
                    // 内置模式编译失败时记录警告（不应该发生）
                    tracing::warn!(
                        pattern_name = name,
                        error = %e,
                        "警告：内置脱敏模式编译失败"
                    );
                }
            }
        }
    }

    /// 获取当前已注册的模式数量
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
}

impl Default for Sanitizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试：API 密钥脱敏
    #[test]
    fn test_sanitize_api_key() {
        let sanitizer = Sanitizer::new();

        // 常见的 API 密钥格式
        let text = "使用密钥 sk-abc123456789xyzABCDEF 访问服务";
        let result = sanitizer.sanitize_text(text);
        assert!(
            !result.contains("sk-abc123456789xyzABCDEF"),
            "API 密钥应被脱敏"
        );
        assert!(result.contains("[REDACTED]"));
    }

    /// 测试：密码脱敏
    #[test]
    fn test_sanitize_password() {
        let sanitizer = Sanitizer::new();

        // 等号分隔的密码格式
        let text = "配置 password=my_super_secret_123 已保存";
        let result = sanitizer.sanitize_text(text);
        assert!(!result.contains("my_super_secret_123"), "密码应被脱敏");
        assert!(result.contains("[REDACTED]"));

        // 冒号分隔的密码格式
        let text2 = "密码 password: hunter2_abc 很不安全";
        let result2 = sanitizer.sanitize_text(text2);
        assert!(!result2.contains("hunter2_abc"), "密码应被脱敏");
    }

    /// 测试：Bearer 令牌脱敏
    #[test]
    fn test_sanitize_bearer_token() {
        let sanitizer = Sanitizer::new();

        let text = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.test";
        let result = sanitizer.sanitize_text(text);
        assert!(
            !result.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"),
            "Bearer 令牌应被脱敏"
        );
        assert!(result.contains("[REDACTED]"));
    }

    /// 测试：电子邮件地址脱敏
    #[test]
    fn test_sanitize_email() {
        let sanitizer = Sanitizer::new();

        let text = "联系邮箱：user@example.com 或 admin@test.org";
        let result = sanitizer.sanitize_text(text);
        assert!(!result.contains("user@example.com"), "邮箱地址应被脱敏");
        assert!(!result.contains("admin@test.org"), "邮箱地址应被脱敏");
    }

    /// 测试：IP 地址脱敏
    #[test]
    fn test_sanitize_ip_address() {
        let sanitizer = Sanitizer::new();

        let text = "服务器地址：192.168.1.100 和 10.0.0.1";
        let result = sanitizer.sanitize_text(text);
        assert!(!result.contains("192.168.1.100"), "IP 地址应被脱敏");
        assert!(!result.contains("10.0.0.1"), "IP 地址应被脱敏");
    }

    /// 测试：信用卡号脱敏
    #[test]
    fn test_sanitize_credit_card() {
        let sanitizer = Sanitizer::new();

        // 无分隔符的信用卡号
        let text = "卡号 4111111111111111 已验证";
        let result = sanitizer.sanitize_text(text);
        assert!(!result.contains("4111111111111111"), "信用卡号应被脱敏");

        // 带横线分隔的信用卡号
        let text2 = "卡号 4111-1111-1111-1111 已验证";
        let result2 = sanitizer.sanitize_text(text2);
        assert!(!result2.contains("4111-1111-1111-1111"), "信用卡号应被脱敏");

        // 带空格分隔的信用卡号
        let text3 = "卡号 4111 1111 1111 1111 已验证";
        let result3 = sanitizer.sanitize_text(text3);
        assert!(!result3.contains("4111 1111 1111 1111"), "信用卡号应被脱敏");
    }

    /// 测试：无敏感信息的文本不受影响
    #[test]
    fn test_sanitize_clean_text() {
        let sanitizer = Sanitizer::new();

        let text = "这是一段普通的日志信息，不包含任何敏感数据。";
        let result = sanitizer.sanitize_text(text);
        assert_eq!(result, text, "无敏感信息的文本不应被修改");
    }

    /// 测试：添加自定义脱敏模式
    #[test]
    fn test_add_custom_pattern() {
        let mut sanitizer = Sanitizer::new();
        let initial_count = sanitizer.pattern_count();

        // 添加自定义的手机号匹配模式
        sanitizer.add_pattern("phone_cn", r"1[3-9]\d{9}").unwrap();
        assert_eq!(sanitizer.pattern_count(), initial_count + 1);

        // 验证自定义模式生效
        let text = "联系电话：13812345678";
        let result = sanitizer.sanitize_text(text);
        assert!(!result.contains("13812345678"), "手机号应被脱敏");
        assert!(result.contains("[REDACTED]"));
    }

    /// 测试：移除脱敏模式
    #[test]
    fn test_remove_pattern() {
        let mut sanitizer = Sanitizer::new();
        let initial_count = sanitizer.pattern_count();

        // 移除 email 模式
        assert!(
            sanitizer.remove_pattern("email"),
            "移除已存在的模式应返回 true"
        );
        assert_eq!(sanitizer.pattern_count(), initial_count - 1);

        // 移除不存在的模式应返回 false
        assert!(
            !sanitizer.remove_pattern("nonexistent"),
            "移除不存在的模式应返回 false"
        );

        // 验证 email 模式已被移除，邮箱不再被脱敏
        let text = "邮箱：test@example.com";
        let result = sanitizer.sanitize_text(text);
        assert!(
            result.contains("test@example.com"),
            "移除 email 模式后邮箱不应被脱敏"
        );
    }

    /// 测试：无效的正则表达式应返回错误
    #[test]
    fn test_add_invalid_pattern() {
        let mut sanitizer = Sanitizer::new();

        // 无效的正则表达式
        let result = sanitizer.add_pattern("invalid", "[invalid");
        assert!(result.is_err(), "无效的正则表达式应返回错误");
    }

    /// 测试：多种敏感信息混合脱敏
    #[test]
    fn test_sanitize_mixed_sensitive_data() {
        let sanitizer = Sanitizer::new();

        let text = "用户 user@test.com 使用密钥 sk-testkey1234567890ab 从 192.168.0.1 发起请求";
        let result = sanitizer.sanitize_text(text);

        // 所有敏感信息都应被脱敏
        assert!(!result.contains("user@test.com"), "邮箱应被脱敏");
        assert!(
            !result.contains("sk-testkey1234567890ab"),
            "API 密钥应被脱敏"
        );
        assert!(!result.contains("192.168.0.1"), "IP 地址应被脱敏");
    }
}

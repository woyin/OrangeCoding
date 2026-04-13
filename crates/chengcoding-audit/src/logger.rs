//! # 审计日志记录器
//!
//! 提供异步审计日志记录功能，支持：
//! - JSON Lines 格式写入文件
//! - 异步批量写入
//! - 可配置的日志轮转

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing;
use uuid::Uuid;

use crate::chain::HashChain;
use crate::sanitizer::Sanitizer;
use crate::AuditResult;

/// 审计日志条目
///
/// 记录每一次操作的完整信息，包括操作者、操作目标、详细内容等。
/// 每个条目都包含哈希值，用于构成哈希链以防止篡改。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// 条目唯一标识符
    pub id: Uuid,

    /// 条目创建时间戳
    pub timestamp: DateTime<Utc>,

    /// 操作类型（例如: "tool_call", "ai_request", "file_operation"）
    pub action: String,

    /// 操作执行者（例如: 用户名或系统组件名）
    pub actor: String,

    /// 操作目标（可选，例如: 文件路径、API端点）
    pub target: Option<String>,

    /// 操作详细信息（JSON 格式的额外数据）
    pub details: serde_json::Value,

    /// 当前条目的 SHA-256 哈希值
    pub hash: String,

    /// 前一个条目的哈希值，用于构成哈希链
    pub previous_hash: String,
}

/// 审计日志记录器配置
#[derive(Debug, Clone)]
pub struct AuditLoggerConfig {
    /// 日志文件路径
    pub log_path: std::path::PathBuf,

    /// 触发日志轮转的最大文件大小（字节）
    pub max_file_size: u64,

    /// 批量写入的缓冲区大小（条目数）
    pub batch_size: usize,

    /// 是否启用敏感信息脱敏
    pub sanitize: bool,
}

impl Default for AuditLoggerConfig {
    /// 返回默认配置：日志文件为 "audit.jsonl"，最大 10MB，批量 100 条
    fn default() -> Self {
        Self {
            log_path: std::path::PathBuf::from("audit.jsonl"),
            max_file_size: 10 * 1024 * 1024, // 10MB
            batch_size: 100,
            sanitize: true,
        }
    }
}

/// 审计日志记录器
///
/// 提供异步的审计日志记录功能，内部维护哈希链和写入缓冲区。
/// 使用 Mutex 保证并发安全。
pub struct AuditLogger {
    /// 记录器配置
    config: AuditLoggerConfig,

    /// 哈希链，用于防篡改验证
    chain: Mutex<HashChain>,

    /// 待写入的日志条目缓冲区
    buffer: Mutex<Vec<AuditEntry>>,

    /// 敏感信息脱敏器
    sanitizer: Sanitizer,
}

impl AuditLogger {
    /// 创建新的审计日志记录器
    ///
    /// # 参数
    /// - `config`: 记录器配置
    ///
    /// # 返回
    /// 初始化后的 AuditLogger 实例
    pub fn new(config: AuditLoggerConfig) -> Self {
        Self {
            config,
            chain: Mutex::new(HashChain::new()),
            buffer: Mutex::new(Vec::new()),
            sanitizer: Sanitizer::new(),
        }
    }

    /// 记录一个通用操作
    ///
    /// # 参数
    /// - `action`: 操作类型
    /// - `actor`: 操作者
    /// - `target`: 操作目标（可选）
    /// - `details`: 操作详细信息
    pub async fn log_action(
        &self,
        action: &str,
        actor: &str,
        target: Option<&str>,
        details: serde_json::Value,
    ) -> AuditResult<AuditEntry> {
        // 如果启用了脱敏，对详细信息进行脱敏处理
        let sanitized_details = if self.config.sanitize {
            self.sanitize_value(&details)
        } else {
            details
        };

        // 通过哈希链生成带有哈希值的审计条目
        let mut chain = self.chain.lock().await;
        let entry = chain.append(
            action.to_string(),
            actor.to_string(),
            target.map(|s| s.to_string()),
            sanitized_details,
        );

        tracing::info!(
            action = %entry.action,
            actor = %entry.actor,
            "审计日志: 记录操作"
        );

        // 将条目加入写入缓冲区
        let mut buffer = self.buffer.lock().await;
        buffer.push(entry.clone());

        // 当缓冲区达到批量大小时自动刷新
        if buffer.len() >= self.config.batch_size {
            drop(buffer); // 释放锁后再刷新
            self.flush().await?;
        }

        Ok(entry)
    }

    /// 记录工具调用
    ///
    /// # 参数
    /// - `actor`: 调用者
    /// - `tool_name`: 工具名称
    /// - `arguments`: 工具参数
    /// - `result_summary`: 调用结果摘要
    pub async fn log_tool_call(
        &self,
        actor: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
        result_summary: &str,
    ) -> AuditResult<AuditEntry> {
        let details = serde_json::json!({
            "tool_name": tool_name,
            "arguments": arguments,
            "result_summary": result_summary,
        });

        self.log_action("tool_call", actor, Some(tool_name), details)
            .await
    }

    /// 记录 AI 请求
    ///
    /// # 参数
    /// - `actor`: 请求者
    /// - `model`: AI 模型名称
    /// - `prompt_tokens`: 提示词 token 数量
    /// - `completion_tokens`: 补全 token 数量
    pub async fn log_ai_request(
        &self,
        actor: &str,
        model: &str,
        prompt_tokens: u64,
        completion_tokens: u64,
    ) -> AuditResult<AuditEntry> {
        let details = serde_json::json!({
            "model": model,
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": prompt_tokens + completion_tokens,
        });

        self.log_action("ai_request", actor, Some(model), details)
            .await
    }

    /// 记录文件操作
    ///
    /// # 参数
    /// - `actor`: 操作者
    /// - `operation`: 操作类型（例如: "read", "write", "delete"）
    /// - `file_path`: 文件路径
    /// - `bytes`: 操作涉及的字节数（可选）
    pub async fn log_file_operation(
        &self,
        actor: &str,
        operation: &str,
        file_path: &str,
        bytes: Option<u64>,
    ) -> AuditResult<AuditEntry> {
        let details = serde_json::json!({
            "operation": operation,
            "file_path": file_path,
            "bytes": bytes,
        });

        self.log_action("file_operation", actor, Some(file_path), details)
            .await
    }

    /// 将缓冲区中的日志条目刷新到文件
    ///
    /// 以 JSON Lines 格式追加写入日志文件。
    /// 如果文件大小超过配置的阈值，会自动触发日志轮转。
    pub async fn flush(&self) -> AuditResult<()> {
        let mut buffer = self.buffer.lock().await;
        if buffer.is_empty() {
            return Ok(());
        }

        // 取出所有待写入的条目
        let entries: Vec<AuditEntry> = buffer.drain(..).collect();
        drop(buffer); // 尽早释放锁

        // 检查是否需要轮转日志文件
        if self.should_rotate().await? {
            self.rotate().await?;
        }

        // 以追加模式打开文件并写入 JSON Lines
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.log_path)
            .await?;

        // 批量序列化并写入
        let mut output = String::new();
        for entry in &entries {
            let line = serde_json::to_string(entry)?;
            output.push_str(&line);
            output.push('\n');
        }

        file.write_all(output.as_bytes()).await?;
        file.flush().await?;

        tracing::debug!(
            count = entries.len(),
            "审计日志: 已刷新 {} 条记录到文件",
            entries.len()
        );

        Ok(())
    }

    /// 执行日志文件轮转
    ///
    /// 将当前日志文件重命名为带时间戳的备份文件，
    /// 然后创建新的空日志文件继续写入。
    pub async fn rotate(&self) -> AuditResult<()> {
        let log_path = &self.config.log_path;

        // 如果当前日志文件不存在，无需轮转
        if !log_path.exists() {
            return Ok(());
        }

        // 生成带时间戳的备份文件名
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let file_stem = log_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("audit");
        let extension = log_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("jsonl");

        let backup_name = format!("{}_{}.{}", file_stem, timestamp, extension);
        let backup_path = log_path.with_file_name(backup_name);

        // 重命名当前文件为备份
        tokio::fs::rename(log_path, &backup_path).await?;

        tracing::info!(
            backup = %backup_path.display(),
            "审计日志: 日志轮转完成"
        );

        Ok(())
    }

    /// 检查是否应该执行日志轮转
    ///
    /// 当日志文件大小超过配置的最大值时返回 true。
    async fn should_rotate(&self) -> AuditResult<bool> {
        let log_path = &self.config.log_path;

        // 文件不存在时不需要轮转
        if !log_path.exists() {
            return Ok(false);
        }

        let metadata = tokio::fs::metadata(log_path).await?;
        Ok(metadata.len() >= self.config.max_file_size)
    }

    /// 对 JSON 值进行递归脱敏处理
    ///
    /// 遍历 JSON 结构，对所有字符串值应用脱敏规则。
    fn sanitize_value(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            // 对字符串值直接进行脱敏
            serde_json::Value::String(s) => {
                serde_json::Value::String(self.sanitizer.sanitize_text(s))
            }
            // 递归处理数组中的每个元素
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(|v| self.sanitize_value(v)).collect())
            }
            // 递归处理对象中的每个值
            serde_json::Value::Object(map) => {
                let sanitized: serde_json::Map<String, serde_json::Value> = map
                    .iter()
                    .map(|(k, v)| (k.clone(), self.sanitize_value(v)))
                    .collect();
                serde_json::Value::Object(sanitized)
            }
            // 非字符串的基础类型无需脱敏
            other => other.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试：创建审计日志条目
    #[test]
    fn test_audit_entry_serialization() {
        // 构造一个审计日志条目
        let entry = AuditEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            action: "test_action".to_string(),
            actor: "test_user".to_string(),
            target: Some("test_target".to_string()),
            details: serde_json::json!({"key": "value"}),
            hash: "abc123".to_string(),
            previous_hash: "000000".to_string(),
        };

        // 验证序列化和反序列化的正确性
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: AuditEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(entry.id, deserialized.id);
        assert_eq!(entry.action, deserialized.action);
        assert_eq!(entry.actor, deserialized.actor);
        assert_eq!(entry.target, deserialized.target);
    }

    /// 测试：默认配置值
    #[test]
    fn test_default_config() {
        let config = AuditLoggerConfig::default();

        // 验证默认配置的各项值
        assert_eq!(config.log_path, std::path::PathBuf::from("audit.jsonl"));
        assert_eq!(config.max_file_size, 10 * 1024 * 1024);
        assert_eq!(config.batch_size, 100);
        assert!(config.sanitize);
    }

    /// 测试：异步记录操作并刷新到文件
    #[tokio::test]
    async fn test_log_action_and_flush() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("test_audit.jsonl");

        // 创建配置并初始化记录器
        let config = AuditLoggerConfig {
            log_path: log_path.clone(),
            max_file_size: 1024 * 1024,
            batch_size: 10,
            sanitize: false,
        };
        let logger = AuditLogger::new(config);

        // 记录一个操作
        let entry = logger
            .log_action(
                "test",
                "user1",
                Some("target1"),
                serde_json::json!({"info": "测试数据"}),
            )
            .await
            .unwrap();

        // 验证返回的条目信息
        assert_eq!(entry.action, "test");
        assert_eq!(entry.actor, "user1");
        assert_eq!(entry.target, Some("target1".to_string()));

        // 刷新到文件并验证文件内容
        logger.flush().await.unwrap();
        let content = tokio::fs::read_to_string(&log_path).await.unwrap();
        assert!(content.contains("test"));
        assert!(content.contains("user1"));
    }

    /// 测试：记录工具调用
    #[tokio::test]
    async fn test_log_tool_call() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("test_tool.jsonl");

        let config = AuditLoggerConfig {
            log_path: log_path.clone(),
            max_file_size: 1024 * 1024,
            batch_size: 10,
            sanitize: false,
        };
        let logger = AuditLogger::new(config);

        // 记录工具调用
        let entry = logger
            .log_tool_call(
                "agent",
                "file_reader",
                &serde_json::json!({"path": "/etc/hosts"}),
                "读取成功",
            )
            .await
            .unwrap();

        // 验证条目类型和目标
        assert_eq!(entry.action, "tool_call");
        assert_eq!(entry.target, Some("file_reader".to_string()));
    }

    /// 测试：记录 AI 请求
    #[tokio::test]
    async fn test_log_ai_request() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("test_ai.jsonl");

        let config = AuditLoggerConfig {
            log_path,
            max_file_size: 1024 * 1024,
            batch_size: 10,
            sanitize: false,
        };
        let logger = AuditLogger::new(config);

        // 记录 AI 请求
        let entry = logger
            .log_ai_request("user", "gpt-4", 100, 200)
            .await
            .unwrap();

        assert_eq!(entry.action, "ai_request");
        assert_eq!(entry.target, Some("gpt-4".to_string()));
    }

    /// 测试：记录文件操作
    #[tokio::test]
    async fn test_log_file_operation() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("test_file_op.jsonl");

        let config = AuditLoggerConfig {
            log_path,
            max_file_size: 1024 * 1024,
            batch_size: 10,
            sanitize: false,
        };
        let logger = AuditLogger::new(config);

        // 记录文件写入操作
        let entry = logger
            .log_file_operation("editor", "write", "/home/user/file.txt", Some(1024))
            .await
            .unwrap();

        assert_eq!(entry.action, "file_operation");
        assert_eq!(entry.target, Some("/home/user/file.txt".to_string()));
    }

    /// 测试：启用脱敏的日志记录
    #[tokio::test]
    async fn test_log_with_sanitization() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("test_sanitize.jsonl");

        // 启用脱敏功能
        let config = AuditLoggerConfig {
            log_path: log_path.clone(),
            max_file_size: 1024 * 1024,
            batch_size: 10,
            sanitize: true,
        };
        let logger = AuditLogger::new(config);

        // 记录包含敏感信息的操作
        let entry = logger
            .log_action(
                "api_call",
                "system",
                None,
                serde_json::json!({
                    "api_key": "sk-abc123456789xyzABCDEF",
                    "password": "password=my_secret_password123",
                }),
            )
            .await
            .unwrap();

        // 验证敏感信息已被脱敏
        let details_str = serde_json::to_string(&entry.details).unwrap();
        assert!(
            !details_str.contains("sk-abc123456789xyzABCDEF"),
            "API 密钥应被脱敏"
        );
        assert!(
            !details_str.contains("my_secret_password123"),
            "密码应被脱敏"
        );
        assert!(details_str.contains("[REDACTED]"));
    }

    /// 测试：日志轮转功能
    #[tokio::test]
    async fn test_log_rotation() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("test_rotate.jsonl");

        // 设置极小的文件大小限制以触发轮转
        let config = AuditLoggerConfig {
            log_path: log_path.clone(),
            max_file_size: 50, // 50 字节即触发轮转
            batch_size: 1,
            sanitize: false,
        };
        let logger = AuditLogger::new(config);

        // 写入足够多的数据以触发轮转
        logger
            .log_action(
                "test",
                "user",
                None,
                serde_json::json!({"data": "第一批数据，需要足够长以触发轮转"}),
            )
            .await
            .unwrap();
        logger.flush().await.unwrap();

        // 再写入一次，应该触发轮转
        logger
            .log_action(
                "test2",
                "user",
                None,
                serde_json::json!({"data": "第二批数据"}),
            )
            .await
            .unwrap();
        logger.flush().await.unwrap();

        // 验证目录中存在备份文件
        let mut entries = tokio::fs::read_dir(dir.path()).await.unwrap();
        let mut file_count = 0;
        while let Some(_entry) = entries.next_entry().await.unwrap() {
            file_count += 1;
        }
        // 应该有原始文件和至少一个备份文件
        assert!(
            file_count >= 2,
            "轮转后应至少有2个文件，实际有 {} 个",
            file_count
        );
    }

    /// 测试：批量写入触发自动刷新
    #[tokio::test]
    async fn test_batch_auto_flush() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("test_batch.jsonl");

        // 设置批量大小为 3
        let config = AuditLoggerConfig {
            log_path: log_path.clone(),
            max_file_size: 1024 * 1024,
            batch_size: 3,
            sanitize: false,
        };
        let logger = AuditLogger::new(config);

        // 写入 3 条记录，应自动触发刷新
        for i in 0..3 {
            logger
                .log_action(
                    &format!("action_{}", i),
                    "user",
                    None,
                    serde_json::json!({}),
                )
                .await
                .unwrap();
        }

        // 验证文件已被写入（无需手动 flush）
        let content = tokio::fs::read_to_string(&log_path).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3, "批量刷新后应有3行记录");
    }
}

//! # Bash 工具
//!
//! 在 shell 中执行命令并返回输出结果。
//! 支持超时控制、工作目录切换和输出截断。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::process::Command;
use tracing::debug;

/// 输出最大字节数（50KB）
const MAX_OUTPUT_BYTES: usize = 50 * 1024;

/// 危险命令关键词列表
const DANGEROUS_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf /*",
    "mkfs.",
    "dd if=/dev/zero of=/dev/",
    ":(){:|:&};:",
    "> /dev/sda",
];

/// Bash 工具 — 在 shell 中执行命令
#[derive(Debug)]
pub struct BashTool {
    /// 默认超时时间（秒）
    default_timeout: u64,
    /// 工作目录
    working_dir: Option<PathBuf>,
}

impl BashTool {
    /// 创建新的 Bash 工具实例
    pub fn new() -> Self {
        Self {
            default_timeout: 30,
            working_dir: None,
        }
    }

    /// 设置默认超时时间
    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// 设置默认工作目录
    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// 检查命令是否包含危险模式
    fn is_dangerous_command(command: &str) -> bool {
        let normalized = command.to_lowercase();
        DANGEROUS_PATTERNS
            .iter()
            .any(|pattern| normalized.contains(pattern))
    }

    /// 截断过长的输出，保留最后 MAX_OUTPUT_BYTES 字节
    fn truncate_output(output: &str) -> String {
        if output.len() <= MAX_OUTPUT_BYTES {
            return output.to_string();
        }
        let truncated = &output[output.len() - MAX_OUTPUT_BYTES..];
        format!(
            "[输出已截断，仅显示最后 {} 字节]\n{}",
            MAX_OUTPUT_BYTES, truncated
        )
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "在 shell 中执行命令，返回输出结果"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "要执行的 shell 命令"
                },
                "timeout": {
                    "type": "number",
                    "description": "超时时间（秒），默认 30，范围 1-3600",
                    "minimum": 1,
                    "maximum": 3600,
                    "default": 30
                },
                "cwd": {
                    "type": "string",
                    "description": "命令执行的工作目录（可选）"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        // 提取命令参数
        let command = params
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: command".to_string()))?;

        // 空命令检查
        if command.trim().is_empty() {
            return Err(ToolError::InvalidParams(
                "command 参数不能为空".to_string(),
            ));
        }

        // 危险命令检查
        if Self::is_dangerous_command(command) {
            return Err(ToolError::SecurityViolation(format!(
                "检测到危险命令: {}",
                command
            )));
        }

        // 解析超时参数
        let timeout_secs = params
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.default_timeout);

        // 限制超时范围
        let timeout_secs = timeout_secs.clamp(1, 3600);

        // 解析工作目录
        let cwd = params
            .get("cwd")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .or_else(|| self.working_dir.clone());

        // 验证工作目录存在
        if let Some(ref dir) = cwd {
            if !dir.exists() {
                return Err(ToolError::InvalidParams(format!(
                    "工作目录不存在: {}",
                    dir.display()
                )));
            }
            if !dir.is_dir() {
                return Err(ToolError::InvalidParams(format!(
                    "指定路径不是目录: {}",
                    dir.display()
                )));
            }
        }

        debug!("执行命令: {} (超时: {}秒)", command, timeout_secs);

        // 构建子进程
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(command);
            c
        } else {
            let mut c = Command::new("/bin/sh");
            c.arg("-c").arg(command);
            c
        };

        if let Some(ref dir) = cwd {
            cmd.current_dir(dir);
        }

        // 捕获标准输出和标准错误
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // 启动进程并应用超时
        let timeout_duration = std::time::Duration::from_secs(timeout_secs);

        match tokio::time::timeout(timeout_duration, cmd.output()).await {
            Ok(Ok(output)) => {
                let exit_code = output.status.code().unwrap_or(-1);
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                let stdout_display = Self::truncate_output(&stdout);
                let stderr_display = Self::truncate_output(&stderr);

                let result = format!(
                    "Exit code: {}\nstdout:\n{}\nstderr:\n{}",
                    exit_code, stdout_display, stderr_display
                );
                Ok(result)
            }
            Ok(Err(e)) => Err(ToolError::ExecutionError(format!(
                "启动进程失败: {}",
                e
            ))),
            Err(_) => Err(ToolError::ExecutionError(format!(
                "命令执行超时（{}秒）: {}",
                timeout_secs, command
            ))),
        }
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 测试执行简单命令
    #[tokio::test]
    async fn test_execute_simple_command() {
        let tool = BashTool::new();
        let params = json!({"command": "echo hello"});
        let result = tool.execute(params).await.unwrap();

        assert!(result.contains("Exit code: 0"));
        assert!(result.contains("hello"));
    }

    /// 测试命令非零退出码
    #[tokio::test]
    async fn test_execute_nonzero_exit_code() {
        let tool = BashTool::new();
        let params = json!({"command": "exit 42"});
        let result = tool.execute(params).await.unwrap();

        assert!(result.contains("Exit code: 42"));
    }

    /// 测试命令超时处理
    #[tokio::test]
    async fn test_execute_timeout() {
        let tool = BashTool::new();
        let params = json!({"command": "sleep 10", "timeout": 1});
        let result = tool.execute(params).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ExecutionError(msg) => {
                assert!(msg.contains("超时"));
            }
            other => panic!("期望执行超时错误，得到: {:?}", other),
        }
    }

    /// 测试大输出截断
    #[tokio::test]
    async fn test_large_output_truncation() {
        let tool = BashTool::new();
        // 生成超过 50KB 的输出
        let params = json!({"command": "python3 -c \"print('A' * 60000)\"", "timeout": 10});
        let result = tool.execute(params).await.unwrap();

        // 检查输出包含截断提示
        assert!(result.contains("截断") || result.len() <= MAX_OUTPUT_BYTES + 500);
    }

    /// 测试工作目录切换
    #[tokio::test]
    async fn test_working_directory() {
        let temp = tempfile::tempdir().unwrap();
        let temp_path = temp.path().to_str().unwrap().to_string();

        let tool = BashTool::new();
        let params = json!({"command": "pwd", "cwd": temp_path});
        let result = tool.execute(params).await.unwrap();

        // macOS 下 /var 可能解析为 /private/var，使用 realpath 比较
        let canonical = temp.path().canonicalize().unwrap();
        let canonical_str = canonical.to_str().unwrap();
        assert!(
            result.contains(&temp_path) || result.contains(canonical_str),
            "输出不包含工作目录路径: {}",
            result
        );
    }

    /// 测试标准错误捕获
    #[tokio::test]
    async fn test_stderr_capture() {
        let tool = BashTool::new();
        let params = json!({"command": "echo error_msg >&2"});
        let result = tool.execute(params).await.unwrap();

        assert!(result.contains("stderr:"));
        assert!(result.contains("error_msg"));
    }

    /// 测试空命令处理
    #[tokio::test]
    async fn test_empty_command() {
        let tool = BashTool::new();
        let params = json!({"command": ""});
        let result = tool.execute(params).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::InvalidParams(msg) => {
                assert!(msg.contains("空"));
            }
            other => panic!("期望参数错误，得到: {:?}", other),
        }
    }

    /// 测试缺少命令参数
    #[tokio::test]
    async fn test_missing_command_param() {
        let tool = BashTool::new();
        let params = json!({});
        let result = tool.execute(params).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::InvalidParams(msg) => {
                assert!(msg.contains("command"));
            }
            other => panic!("期望参数错误，得到: {:?}", other),
        }
    }

    /// 测试参数 Schema 结构
    #[test]
    fn test_parameters_schema() {
        let tool = BashTool::new();
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["command"].is_object());
        assert!(schema["properties"]["timeout"].is_object());
        assert!(schema["properties"]["cwd"].is_object());

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("command")));
    }

    /// 测试特殊字符命令
    #[tokio::test]
    async fn test_command_with_special_characters() {
        let tool = BashTool::new();
        let params = json!({"command": "echo 'hello world' && echo \"foo bar\""});
        let result = tool.execute(params).await.unwrap();

        assert!(result.contains("hello world"));
        assert!(result.contains("foo bar"));
    }

    /// 测试危险命令检测
    #[tokio::test]
    async fn test_dangerous_command_detection() {
        let tool = BashTool::new();
        let params = json!({"command": "rm -rf /"});
        let result = tool.execute(params).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::SecurityViolation(msg) => {
                assert!(msg.contains("危险"));
            }
            other => panic!("期望安全违规错误，得到: {:?}", other),
        }
    }

    /// 测试不存在的工作目录
    #[tokio::test]
    async fn test_nonexistent_cwd() {
        let tool = BashTool::new();
        let params = json!({"command": "echo test", "cwd": "/nonexistent_dir_12345"});
        let result = tool.execute(params).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::InvalidParams(msg) => {
                assert!(msg.contains("不存在"));
            }
            other => panic!("期望参数错误，得到: {:?}", other),
        }
    }

    /// 测试工具名称和描述
    #[test]
    fn test_name_and_description() {
        let tool = BashTool::new();
        assert_eq!(tool.name(), "bash");
        assert!(!tool.description().is_empty());
    }

    /// 测试默认超时设置
    #[test]
    fn test_default_timeout() {
        let tool = BashTool::new();
        assert_eq!(tool.default_timeout, 30);
    }

    /// 测试自定义超时设置
    #[test]
    fn test_custom_timeout() {
        let tool = BashTool::with_timeout(BashTool::new(), 60);
        assert_eq!(tool.default_timeout, 60);
    }

    /// 测试截断函数
    #[test]
    fn test_truncate_output() {
        // 短输出不截断
        let short = "hello";
        assert_eq!(BashTool::truncate_output(short), "hello");

        // 长输出应被截断
        let long = "A".repeat(MAX_OUTPUT_BYTES + 1000);
        let truncated = BashTool::truncate_output(&long);
        assert!(truncated.contains("截断"));
        assert!(truncated.len() <= MAX_OUTPUT_BYTES + 200);
    }

    /// 测试危险命令模式列表
    #[test]
    fn test_dangerous_patterns() {
        assert!(BashTool::is_dangerous_command("rm -rf /"));
        assert!(BashTool::is_dangerous_command("rm -rf /*"));
        assert!(BashTool::is_dangerous_command("sudo rm -rf /"));
        assert!(!BashTool::is_dangerous_command("rm -rf ./temp"));
        assert!(!BashTool::is_dangerous_command("echo hello"));
    }
}

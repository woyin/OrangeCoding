//! # SSH 工具
//!
//! 远程命令执行工具，通过系统 SSH 客户端连接远程服务器并执行命令。
//! 支持多种认证方式：密钥、密码、SSH Agent。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::debug;

// ============================================================
// 数据类型定义
// ============================================================

/// SSH 认证方式
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SshAuth {
    /// 使用密钥文件认证
    Key(String),
    /// 使用密码认证
    Password(String),
    /// 使用 SSH Agent 认证
    Agent,
}

/// SSH 连接配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SshConnection {
    /// 远程主机地址
    pub host: String,
    /// SSH 端口号
    pub port: u16,
    /// 登录用户名
    pub user: String,
    /// 认证方式
    pub auth: SshAuth,
}

/// SSH 命令执行结果
#[derive(Clone, Debug)]
pub struct SshResult {
    /// 标准输出
    pub stdout: String,
    /// 标准错误
    pub stderr: String,
    /// 退出码
    pub exit_code: i32,
}

// ============================================================
// SshTool — 远程命令执行工具
// ============================================================

/// SSH 工具 — 远程命令执行
#[derive(Debug)]
pub struct SshTool;

impl SshTool {
    /// 解析 SSH 连接字符串
    ///
    /// 支持格式：
    /// - `user@host:port`
    /// - `user@host`（默认端口 22）
    /// - `host`（使用当前用户名，默认端口 22）
    pub fn parse_connection_string(s: &str) -> Result<SshConnection, String> {
        let s = s.trim();
        if s.is_empty() {
            return Err("连接字符串不能为空".to_string());
        }

        let (user, host_port) = if let Some(at_pos) = s.find('@') {
            let user = &s[..at_pos];
            if user.is_empty() {
                return Err("用户名不能为空".to_string());
            }
            (user.to_string(), &s[at_pos + 1..])
        } else {
            // 没有 @ 符号，使用当前用户名
            let current_user = std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_else(|_| "root".to_string());
            (current_user, s)
        };

        // 解析主机和端口
        let (host, port) = if let Some(colon_pos) = host_port.rfind(':') {
            let host = &host_port[..colon_pos];
            let port_str = &host_port[colon_pos + 1..];
            let port = port_str
                .parse::<u16>()
                .map_err(|_| format!("无效的端口号: {}", port_str))?;
            (host.to_string(), port)
        } else {
            (host_port.to_string(), 22)
        };

        if host.is_empty() {
            return Err("主机名不能为空".to_string());
        }

        Ok(SshConnection {
            host,
            port,
            user,
            auth: SshAuth::Agent,
        })
    }

    /// 构建 SSH 命令行参数
    ///
    /// 生成可供 `std::process::Command` 使用的参数列表。
    pub fn build_ssh_command(conn: &SshConnection, command: &str) -> Vec<String> {
        let mut args = vec!["ssh".to_string()];

        // 添加端口参数（非默认端口时）
        if conn.port != 22 {
            args.push("-p".to_string());
            args.push(conn.port.to_string());
        }

        // 添加认证参数
        match &conn.auth {
            SshAuth::Key(key_path) => {
                args.push("-i".to_string());
                args.push(key_path.clone());
            }
            SshAuth::Password(_) => {
                // 密码认证通过 sshpass 或其他方式处理，此处不添加额外参数
            }
            SshAuth::Agent => {
                // 使用 SSH Agent，无需额外参数
            }
        }

        // 默认启用严格主机密钥检查（首次连接时自动接受新密钥）
        args.push("-o".to_string());
        args.push("StrictHostKeyChecking=accept-new".to_string());

        // 添加用户@主机
        args.push(format!("{}@{}", conn.user, conn.host));

        // 添加要执行的命令
        args.push(command.to_string());

        args
    }
}

#[async_trait]
impl Tool for SshTool {
    fn name(&self) -> &str {
        "ssh"
    }

    fn description(&self) -> &str {
        "通过 SSH 在远程服务器上执行命令"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "host": {
                    "type": "string",
                    "description": "SSH 主机地址（支持 user@host:port 格式）"
                },
                "command": {
                    "type": "string",
                    "description": "要在远程服务器上执行的命令"
                },
                "timeout": {
                    "type": "number",
                    "description": "超时时间（秒），默认 30",
                    "default": 30
                }
            },
            "required": ["host", "command"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        let host = params
            .get("host")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: host".to_string()))?;

        let command = params
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: command".to_string()))?;

        let timeout = params.get("timeout").and_then(|v| v.as_u64()).unwrap_or(30);

        debug!("SSH 连接: {}，执行命令: {}", host, command);

        // 解析连接字符串
        let conn = Self::parse_connection_string(host)
            .map_err(|e| ToolError::InvalidParams(format!("无效的连接字符串: {}", e)))?;

        // 构建 SSH 命令
        let args = Self::build_ssh_command(&conn, command);

        // 执行 SSH 命令
        let mut cmd = tokio::process::Command::new(&args[0]);
        for arg in &args[1..] {
            cmd.arg(arg);
        }
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let timeout_duration = std::time::Duration::from_secs(timeout);

        match tokio::time::timeout(timeout_duration, cmd.output()).await {
            Ok(Ok(output)) => {
                let exit_code = output.status.code().unwrap_or(-1);
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                Ok(format!(
                    "Exit code: {}\nstdout:\n{}\nstderr:\n{}",
                    exit_code, stdout, stderr
                ))
            }
            Ok(Err(e)) => Err(ToolError::ExecutionError(format!(
                "SSH 命令启动失败: {}",
                e
            ))),
            Err(_) => Err(ToolError::ExecutionError(format!(
                "SSH 命令执行超时（{}秒）",
                timeout
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

    /// 测试解析简单连接字符串 — user@host
    #[test]
    fn test_parse_connection_simple() {
        let conn = SshTool::parse_connection_string("admin@example.com").unwrap();
        assert_eq!(conn.user, "admin");
        assert_eq!(conn.host, "example.com");
        assert_eq!(conn.port, 22);
    }

    /// 测试解析带端口的连接字符串 — user@host:port
    #[test]
    fn test_parse_connection_with_port() {
        let conn = SshTool::parse_connection_string("deploy@server.io:2222").unwrap();
        assert_eq!(conn.user, "deploy");
        assert_eq!(conn.host, "server.io");
        assert_eq!(conn.port, 2222);
    }

    /// 测试解析无用户名的连接字符串 — host
    #[test]
    fn test_parse_connection_no_user() {
        let conn = SshTool::parse_connection_string("myhost.local").unwrap();
        // 应使用当前系统用户名
        assert!(!conn.user.is_empty());
        assert_eq!(conn.host, "myhost.local");
        assert_eq!(conn.port, 22);
    }

    /// 测试构建基本 SSH 命令
    #[test]
    fn test_build_ssh_command() {
        let conn = SshConnection {
            host: "example.com".to_string(),
            port: 22,
            user: "admin".to_string(),
            auth: SshAuth::Agent,
        };

        let args = SshTool::build_ssh_command(&conn, "ls -la");

        assert_eq!(args[0], "ssh");
        // 默认端口不应有 -p 参数
        assert!(!args.contains(&"-p".to_string()));
        assert!(args.contains(&"admin@example.com".to_string()));
        assert!(args.contains(&"ls -la".to_string()));
    }

    /// 测试构建带密钥的 SSH 命令
    #[test]
    fn test_build_ssh_command_with_key() {
        let conn = SshConnection {
            host: "server.io".to_string(),
            port: 2222,
            user: "deploy".to_string(),
            auth: SshAuth::Key("/home/user/.ssh/id_rsa".to_string()),
        };

        let args = SshTool::build_ssh_command(&conn, "whoami");

        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"2222".to_string()));
        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"/home/user/.ssh/id_rsa".to_string()));
        assert!(args.contains(&"deploy@server.io".to_string()));
    }

    /// 测试参数 Schema 结构
    #[test]
    fn test_parameter_schema() {
        let tool = SshTool;
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["host"].is_object());
        assert!(schema["properties"]["command"].is_object());
        assert!(schema["properties"]["timeout"].is_object());

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("host")));
        assert!(required.contains(&json!("command")));
    }

    /// 测试工具名称
    #[test]
    fn test_tool_name() {
        let tool = SshTool;
        assert_eq!(tool.name(), "ssh");
        assert!(!tool.description().is_empty());
    }
}

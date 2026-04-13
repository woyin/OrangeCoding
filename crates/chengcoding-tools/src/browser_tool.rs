//! # 浏览器工具
//!
//! 网页交互与截图工具，通过浏览器自动化框架（Playwright/Puppeteer）
//! 实现页面导航、截图、文本提取、元素操作等功能。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::debug;

// ============================================================
// 数据类型定义
// ============================================================

/// 浏览器动作
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserAction {
    /// 导航到 URL
    Navigate { url: String },
    /// 截图
    Screenshot { url: String, full_page: bool },
    /// 获取页面文本
    GetText {
        url: String,
        selector: Option<String>,
    },
    /// 点击元素
    Click { url: String, selector: String },
    /// 输入文本
    Type {
        url: String,
        selector: String,
        text: String,
    },
    /// 执行 JavaScript
    Evaluate { url: String, script: String },
    /// 等待元素
    WaitFor {
        url: String,
        selector: String,
        timeout_ms: u64,
    },
}

/// 浏览器结果
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrowserResult {
    /// 执行的动作名称
    pub action: String,
    /// 是否成功
    pub success: bool,
    /// 返回内容
    pub content: Option<String>,
    /// 截图文件路径
    pub screenshot_path: Option<String>,
    /// 错误信息
    pub error: Option<String>,
}

// ============================================================
// BrowserTool — 浏览器自动化工具
// ============================================================

/// 浏览器工具 — 网页交互与截图
#[derive(Debug)]
pub struct BrowserTool {
    /// 是否使用无头模式
    headless: bool,
    /// 超时时间（秒）
    timeout_secs: u64,
}

impl BrowserTool {
    /// 创建默认的浏览器工具实例（无头模式，30 秒超时）
    pub fn new() -> Self {
        Self {
            headless: true,
            timeout_secs: 30,
        }
    }

    /// 构建浏览器命令
    ///
    /// 根据动作类型生成对应的 npx playwright 命令行参数。
    pub fn build_command(&self, action: &BrowserAction) -> Vec<String> {
        let mut args = vec!["npx".to_string(), "playwright".to_string()];

        match action {
            BrowserAction::Navigate { url } => {
                args.push("open".to_string());
                if self.headless {
                    args.push("--headless".to_string());
                }
                args.push(url.clone());
            }
            BrowserAction::Screenshot { url, full_page } => {
                args.push("screenshot".to_string());
                if self.headless {
                    args.push("--headless".to_string());
                }
                if *full_page {
                    args.push("--full-page".to_string());
                }
                args.push(url.clone());
            }
            BrowserAction::GetText { url, selector } => {
                args.push("eval".to_string());
                if self.headless {
                    args.push("--headless".to_string());
                }
                let script = if let Some(sel) = selector {
                    format!("document.querySelector('{}')?.textContent || ''", sel)
                } else {
                    "document.body.innerText".to_string()
                };
                args.push(url.clone());
                args.push(script);
            }
            BrowserAction::Click { url, selector } => {
                args.push("click".to_string());
                if self.headless {
                    args.push("--headless".to_string());
                }
                args.push(url.clone());
                args.push(selector.clone());
            }
            BrowserAction::Type {
                url,
                selector,
                text,
            } => {
                args.push("fill".to_string());
                if self.headless {
                    args.push("--headless".to_string());
                }
                args.push(url.clone());
                args.push(selector.clone());
                args.push(text.clone());
            }
            BrowserAction::Evaluate { url, script } => {
                args.push("eval".to_string());
                if self.headless {
                    args.push("--headless".to_string());
                }
                args.push(url.clone());
                args.push(script.clone());
            }
            BrowserAction::WaitFor {
                url,
                selector,
                timeout_ms,
            } => {
                args.push("wait-for".to_string());
                if self.headless {
                    args.push("--headless".to_string());
                }
                args.push(url.clone());
                args.push(selector.clone());
                args.push("--timeout".to_string());
                args.push(timeout_ms.to_string());
            }
        }

        args
    }

    /// 检测可用的浏览器自动化工具
    ///
    /// 按优先级依次检测 playwright、puppeteer。
    pub fn detect_browser_tool() -> Option<String> {
        let candidates = [
            ("npx", &["playwright", "--version"] as &[&str]),
            ("npx", &["puppeteer", "--version"]),
        ];

        for (cmd, check_args) in &candidates {
            let mut command = std::process::Command::new(cmd);
            for arg in *check_args {
                command.arg(arg);
            }
            if command.output().is_ok() {
                return Some(check_args[0].to_string());
            }
        }
        None
    }

    /// 格式化浏览器结果
    pub fn format_result(result: &BrowserResult) -> String {
        let mut output = String::new();

        let status = if result.success { "成功" } else { "失败" };
        output.push_str(&format!("[{}] {}\n", status, result.action));

        if let Some(ref content) = result.content {
            output.push_str(&format!("内容:\n{}\n", content));
        }

        if let Some(ref path) = result.screenshot_path {
            output.push_str(&format!("截图: {}\n", path));
        }

        if let Some(ref error) = result.error {
            output.push_str(&format!("错误: {}\n", error));
        }

        output
    }
}

impl Default for BrowserTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser"
    }

    fn description(&self) -> &str {
        "浏览器自动化工具，支持页面导航、截图、文本提取和元素交互"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "浏览器动作: navigate, screenshot, get_text, click, type, evaluate, wait_for",
                    "enum": ["navigate", "screenshot", "get_text", "click", "type", "evaluate", "wait_for"]
                },
                "url": {
                    "type": "string",
                    "description": "目标 URL"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS 选择器"
                },
                "text": {
                    "type": "string",
                    "description": "输入文本"
                },
                "script": {
                    "type": "string",
                    "description": "JavaScript 脚本"
                },
                "full_page": {
                    "type": "boolean",
                    "description": "是否截取全页面",
                    "default": false
                },
                "timeout": {
                    "type": "number",
                    "description": "超时时间（秒），默认 30",
                    "default": 30
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        let action_str = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: action".to_string()))?;

        debug!("浏览器操作: {}", action_str);

        // 构建浏览器动作
        let browser_action =
            match action_str {
                "navigate" => {
                    let url = params.get("url").and_then(|v| v.as_str()).ok_or_else(|| {
                        ToolError::InvalidParams("navigate 需要 url 参数".to_string())
                    })?;
                    BrowserAction::Navigate {
                        url: url.to_string(),
                    }
                }
                "screenshot" => {
                    let url = params.get("url").and_then(|v| v.as_str()).ok_or_else(|| {
                        ToolError::InvalidParams("screenshot 需要 url 参数".to_string())
                    })?;
                    let full_page = params
                        .get("full_page")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    BrowserAction::Screenshot {
                        url: url.to_string(),
                        full_page,
                    }
                }
                "get_text" => {
                    let url = params.get("url").and_then(|v| v.as_str()).ok_or_else(|| {
                        ToolError::InvalidParams("get_text 需要 url 参数".to_string())
                    })?;
                    let selector = params
                        .get("selector")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    BrowserAction::GetText {
                        url: url.to_string(),
                        selector,
                    }
                }
                "click" => {
                    let url = params.get("url").and_then(|v| v.as_str()).ok_or_else(|| {
                        ToolError::InvalidParams("click 需要 url 参数".to_string())
                    })?;
                    let selector =
                        params
                            .get("selector")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                ToolError::InvalidParams("click 需要 selector 参数".to_string())
                            })?;
                    BrowserAction::Click {
                        url: url.to_string(),
                        selector: selector.to_string(),
                    }
                }
                "type" => {
                    let url = params.get("url").and_then(|v| v.as_str()).ok_or_else(|| {
                        ToolError::InvalidParams("type 需要 url 参数".to_string())
                    })?;
                    let selector =
                        params
                            .get("selector")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                ToolError::InvalidParams("type 需要 selector 参数".to_string())
                            })?;
                    let text = params.get("text").and_then(|v| v.as_str()).ok_or_else(|| {
                        ToolError::InvalidParams("type 需要 text 参数".to_string())
                    })?;
                    BrowserAction::Type {
                        url: url.to_string(),
                        selector: selector.to_string(),
                        text: text.to_string(),
                    }
                }
                "evaluate" => {
                    let url = params.get("url").and_then(|v| v.as_str()).ok_or_else(|| {
                        ToolError::InvalidParams("evaluate 需要 url 参数".to_string())
                    })?;
                    let script =
                        params
                            .get("script")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                ToolError::InvalidParams("evaluate 需要 script 参数".to_string())
                            })?;
                    BrowserAction::Evaluate {
                        url: url.to_string(),
                        script: script.to_string(),
                    }
                }
                "wait_for" => {
                    let url = params.get("url").and_then(|v| v.as_str()).ok_or_else(|| {
                        ToolError::InvalidParams("wait_for 需要 url 参数".to_string())
                    })?;
                    let selector =
                        params
                            .get("selector")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                ToolError::InvalidParams("wait_for 需要 selector 参数".to_string())
                            })?;
                    let timeout_ms = params
                        .get("timeout")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(5000);
                    BrowserAction::WaitFor {
                        url: url.to_string(),
                        selector: selector.to_string(),
                        timeout_ms,
                    }
                }
                other => {
                    return Err(ToolError::InvalidParams(format!(
                        "未知的浏览器动作: {}",
                        other
                    )))
                }
            };

        // 构建命令
        let args = self.build_command(&browser_action);

        let timeout = params
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.timeout_secs);

        // 执行命令
        let mut cmd = tokio::process::Command::new(&args[0]);
        for arg in &args[1..] {
            cmd.arg(arg);
        }
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let timeout_duration = std::time::Duration::from_secs(timeout);

        match tokio::time::timeout(timeout_duration, cmd.output()).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let success = output.status.success();

                let result = BrowserResult {
                    action: action_str.to_string(),
                    success,
                    content: if stdout.is_empty() {
                        None
                    } else {
                        Some(stdout)
                    },
                    screenshot_path: None,
                    error: if stderr.is_empty() {
                        None
                    } else {
                        Some(stderr)
                    },
                };

                Ok(Self::format_result(&result))
            }
            Ok(Err(e)) => Err(ToolError::ExecutionError(format!(
                "启动浏览器进程失败: {}",
                e
            ))),
            Err(_) => Err(ToolError::ExecutionError(format!(
                "浏览器操作超时（{}秒）",
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

    /// 测试导航动作构建
    #[test]
    fn test_navigate_action() {
        let action = BrowserAction::Navigate {
            url: "https://example.com".to_string(),
        };
        // 验证动作可序列化
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["type"], "navigate");
        assert_eq!(json["url"], "https://example.com");
    }

    /// 测试截图动作构建
    #[test]
    fn test_screenshot_action() {
        let action = BrowserAction::Screenshot {
            url: "https://example.com".to_string(),
            full_page: true,
        };
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["type"], "screenshot");
        assert_eq!(json["full_page"], true);
    }

    /// 测试获取文本动作
    #[test]
    fn test_get_text_action() {
        let action = BrowserAction::GetText {
            url: "https://example.com".to_string(),
            selector: Some("#main".to_string()),
        };
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["type"], "get_text");
        assert_eq!(json["selector"], "#main");
    }

    /// 测试点击动作
    #[test]
    fn test_click_action() {
        let action = BrowserAction::Click {
            url: "https://example.com".to_string(),
            selector: "button.submit".to_string(),
        };
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["type"], "click");
        assert_eq!(json["url"], "https://example.com");
        assert_eq!(json["selector"], "button.submit");
    }

    /// 测试输入文本动作
    #[test]
    fn test_type_action() {
        let action = BrowserAction::Type {
            url: "https://example.com".to_string(),
            selector: "#search".to_string(),
            text: "查询内容".to_string(),
        };
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["type"], "type");
        assert_eq!(json["url"], "https://example.com");
        assert_eq!(json["selector"], "#search");
        assert_eq!(json["text"], "查询内容");
    }

    /// 测试 JavaScript 执行动作
    #[test]
    fn test_evaluate_action() {
        let action = BrowserAction::Evaluate {
            url: "https://example.com".to_string(),
            script: "document.title".to_string(),
        };
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["type"], "evaluate");
        assert_eq!(json["url"], "https://example.com");
        assert_eq!(json["script"], "document.title");
    }

    /// 测试构建命令
    #[test]
    fn test_build_command() {
        let tool = BrowserTool::new();

        // 测试导航命令
        let action = BrowserAction::Navigate {
            url: "https://example.com".to_string(),
        };
        let args = tool.build_command(&action);
        assert_eq!(args[0], "npx");
        assert_eq!(args[1], "playwright");
        assert_eq!(args[2], "open");
        assert!(args.contains(&"--headless".to_string()));
        assert!(args.contains(&"https://example.com".to_string()));

        // 测试截图命令（全页面）
        let action = BrowserAction::Screenshot {
            url: "https://example.com".to_string(),
            full_page: true,
        };
        let args = tool.build_command(&action);
        assert!(args.contains(&"screenshot".to_string()));
        assert!(args.contains(&"--full-page".to_string()));
    }

    /// 测试格式化成功结果
    #[test]
    fn test_format_result_success() {
        let result = BrowserResult {
            action: "navigate".to_string(),
            success: true,
            content: Some("页面已加载".to_string()),
            screenshot_path: None,
            error: None,
        };
        let formatted = BrowserTool::format_result(&result);

        assert!(formatted.contains("成功"));
        assert!(formatted.contains("navigate"));
        assert!(formatted.contains("页面已加载"));
    }

    /// 测试格式化错误结果
    #[test]
    fn test_format_result_error() {
        let result = BrowserResult {
            action: "navigate".to_string(),
            success: false,
            content: None,
            screenshot_path: None,
            error: Some("连接超时".to_string()),
        };
        let formatted = BrowserTool::format_result(&result);

        assert!(formatted.contains("失败"));
        assert!(formatted.contains("错误"));
        assert!(formatted.contains("连接超时"));
    }

    /// 测试参数 Schema 结构
    #[test]
    fn test_parameter_schema() {
        let tool = BrowserTool::new();
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["action"].is_object());
        assert!(schema["properties"]["url"].is_object());
        assert!(schema["properties"]["selector"].is_object());

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("action")));
    }

    /// 测试工具名称和描述
    #[test]
    fn test_tool_name() {
        let tool = BrowserTool::new();
        assert_eq!(tool.name(), "browser");
        assert!(!tool.description().is_empty());
    }
}

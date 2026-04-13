//! # LSP 工具
//!
//! 语言服务器协议（Language Server Protocol）集成工具。
//! 支持跳转到定义、查找引用、悬浮信息、代码补全、诊断和符号搜索。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::debug;

// ============================================================
// 数据类型定义
// ============================================================

/// LSP 请求类型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LspRequest {
    /// 跳转到定义
    GotoDefinition {
        file: String,
        line: u32,
        column: u32,
    },
    /// 查找引用
    FindReferences {
        file: String,
        line: u32,
        column: u32,
    },
    /// 悬浮信息
    Hover {
        file: String,
        line: u32,
        column: u32,
    },
    /// 代码补全
    Completion {
        file: String,
        line: u32,
        column: u32,
    },
    /// 诊断信息
    Diagnostics { file: String },
    /// 符号搜索
    WorkspaceSymbol { query: String },
}

impl LspRequest {
    /// 从动作名称和参数构造请求
    pub fn from_params(action: &str, params: &Value) -> Result<Self, ToolError> {
        match action {
            "goto_definition" => {
                let (file, line, column) = Self::extract_position(params)?;
                Ok(LspRequest::GotoDefinition { file, line, column })
            }
            "find_references" => {
                let (file, line, column) = Self::extract_position(params)?;
                Ok(LspRequest::FindReferences { file, line, column })
            }
            "hover" => {
                let (file, line, column) = Self::extract_position(params)?;
                Ok(LspRequest::Hover { file, line, column })
            }
            "completion" => {
                let (file, line, column) = Self::extract_position(params)?;
                Ok(LspRequest::Completion { file, line, column })
            }
            "diagnostics" => {
                let file = params
                    .get("file")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: file".to_string()))?
                    .to_string();
                Ok(LspRequest::Diagnostics { file })
            }
            "workspace_symbol" => {
                let query = params
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: query".to_string()))?
                    .to_string();
                Ok(LspRequest::WorkspaceSymbol { query })
            }
            other => Err(ToolError::InvalidParams(format!(
                "未知的 LSP 操作: {}",
                other
            ))),
        }
    }

    /// 从参数中提取文件位置信息
    fn extract_position(params: &Value) -> Result<(String, u32, u32), ToolError> {
        let file = params
            .get("file")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: file".to_string()))?
            .to_string();

        let line = params
            .get("line")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: line".to_string()))?
            as u32;

        let column = params
            .get("column")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: column".to_string()))?
            as u32;

        Ok((file, line, column))
    }

    /// 返回请求类型名称
    pub fn request_type(&self) -> &str {
        match self {
            LspRequest::GotoDefinition { .. } => "goto_definition",
            LspRequest::FindReferences { .. } => "find_references",
            LspRequest::Hover { .. } => "hover",
            LspRequest::Completion { .. } => "completion",
            LspRequest::Diagnostics { .. } => "diagnostics",
            LspRequest::WorkspaceSymbol { .. } => "workspace_symbol",
        }
    }
}

/// LSP 结果项
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LspResultItem {
    /// 文件路径
    pub file: Option<String>,
    /// 行号
    pub line: Option<u32>,
    /// 列号
    pub column: Option<u32>,
    /// 内容
    pub content: String,
    /// 符号类型
    pub kind: Option<String>,
}

/// LSP 响应
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LspResponse {
    /// 请求类型
    pub request_type: String,
    /// 结果列表
    pub results: Vec<LspResultItem>,
}

impl LspResponse {
    /// 格式化响应为可读字符串
    pub fn format(&self) -> String {
        if self.results.is_empty() {
            return format!("[{}] 未找到结果", self.request_type);
        }

        let mut output = format!(
            "[{}] 找到 {} 个结果:\n",
            self.request_type,
            self.results.len()
        );

        for (i, item) in self.results.iter().enumerate() {
            let location = match (&item.file, item.line, item.column) {
                (Some(f), Some(l), Some(c)) => format!("{}:{}:{}", f, l, c),
                (Some(f), Some(l), None) => format!("{}:{}", f, l),
                (Some(f), None, None) => f.clone(),
                _ => "未知位置".to_string(),
            };

            let kind_str = item
                .kind
                .as_ref()
                .map(|k| format!(" [{}]", k))
                .unwrap_or_default();

            output.push_str(&format!(
                "  {}. {}{}: {}\n",
                i + 1,
                location,
                kind_str,
                item.content
            ));
        }

        output
    }
}

// ============================================================
// LspTool — 语言服务器协议集成工具
// ============================================================

/// LSP 工具 — 语言服务器协议集成
#[derive(Debug)]
pub struct LspTool;

#[async_trait]
impl Tool for LspTool {
    fn name(&self) -> &str {
        "lsp"
    }

    fn description(&self) -> &str {
        "语言服务器协议集成，支持跳转到定义、查找引用、悬浮信息等"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "LSP 操作类型",
                    "enum": [
                        "goto_definition",
                        "find_references",
                        "hover",
                        "completion",
                        "diagnostics",
                        "workspace_symbol"
                    ]
                },
                "file": {
                    "type": "string",
                    "description": "文件路径（大多数操作必需）"
                },
                "line": {
                    "type": "number",
                    "description": "行号（从 1 开始）"
                },
                "column": {
                    "type": "number",
                    "description": "列号（从 1 开始）"
                },
                "query": {
                    "type": "string",
                    "description": "搜索查询（用于 workspace_symbol）"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: action".to_string()))?;

        debug!("LSP 操作: {}", action);

        // 解析请求
        let request = LspRequest::from_params(action, &params)?;

        // 目前返回占位响应，待 LSP 客户端集成后实现实际通信
        let response = LspResponse {
            request_type: request.request_type().to_string(),
            results: vec![],
        };

        Ok(response.format())
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 测试从参数解析 LSP 请求
    #[test]
    fn test_lsp_request_parsing() {
        let params = json!({
            "file": "src/main.rs",
            "line": 10,
            "column": 5
        });

        let request = LspRequest::from_params("goto_definition", &params).unwrap();
        assert_eq!(request.request_type(), "goto_definition");
    }

    /// 测试跳转到定义请求
    #[test]
    fn test_goto_definition_request() {
        let params = json!({
            "file": "src/lib.rs",
            "line": 42,
            "column": 10
        });

        let request = LspRequest::from_params("goto_definition", &params).unwrap();
        match request {
            LspRequest::GotoDefinition { file, line, column } => {
                assert_eq!(file, "src/lib.rs");
                assert_eq!(line, 42);
                assert_eq!(column, 10);
            }
            other => panic!("期望 GotoDefinition，得到: {:?}", other),
        }
    }

    /// 测试查找引用请求
    #[test]
    fn test_find_references_request() {
        let params = json!({
            "file": "src/utils.rs",
            "line": 15,
            "column": 3
        });

        let request = LspRequest::from_params("find_references", &params).unwrap();
        match request {
            LspRequest::FindReferences { file, line, column } => {
                assert_eq!(file, "src/utils.rs");
                assert_eq!(line, 15);
                assert_eq!(column, 3);
            }
            other => panic!("期望 FindReferences，得到: {:?}", other),
        }
    }

    /// 测试悬浮信息请求
    #[test]
    fn test_hover_request() {
        let params = json!({
            "file": "src/api.rs",
            "line": 20,
            "column": 8
        });

        let request = LspRequest::from_params("hover", &params).unwrap();
        assert_eq!(request.request_type(), "hover");
    }

    /// 测试诊断请求
    #[test]
    fn test_diagnostics_request() {
        let params = json!({
            "file": "src/main.rs"
        });

        let request = LspRequest::from_params("diagnostics", &params).unwrap();
        match request {
            LspRequest::Diagnostics { file } => {
                assert_eq!(file, "src/main.rs");
            }
            other => panic!("期望 Diagnostics，得到: {:?}", other),
        }
    }

    /// 测试符号搜索请求
    #[test]
    fn test_workspace_symbol_request() {
        let params = json!({
            "query": "MyStruct"
        });

        let request = LspRequest::from_params("workspace_symbol", &params).unwrap();
        match request {
            LspRequest::WorkspaceSymbol { query } => {
                assert_eq!(query, "MyStruct");
            }
            other => panic!("期望 WorkspaceSymbol，得到: {:?}", other),
        }
    }

    /// 测试响应格式化
    #[test]
    fn test_lsp_response_formatting() {
        let response = LspResponse {
            request_type: "goto_definition".to_string(),
            results: vec![LspResultItem {
                file: Some("src/lib.rs".to_string()),
                line: Some(42),
                column: Some(5),
                content: "pub struct MyStruct".to_string(),
                kind: Some("struct".to_string()),
            }],
        };

        let formatted = response.format();
        assert!(formatted.contains("goto_definition"));
        assert!(formatted.contains("src/lib.rs:42:5"));
        assert!(formatted.contains("[struct]"));
        assert!(formatted.contains("pub struct MyStruct"));

        // 测试空结果
        let empty_response = LspResponse {
            request_type: "hover".to_string(),
            results: vec![],
        };
        let empty_formatted = empty_response.format();
        assert!(empty_formatted.contains("未找到结果"));
    }

    /// 测试参数 Schema 结构
    #[test]
    fn test_parameter_schema() {
        let tool = LspTool;
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["action"].is_object());
        assert!(schema["properties"]["file"].is_object());
        assert!(schema["properties"]["line"].is_object());
        assert!(schema["properties"]["column"].is_object());
        assert!(schema["properties"]["query"].is_object());

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("action")));
    }

    /// 测试工具名称
    #[test]
    fn test_tool_name() {
        let tool = LspTool;
        assert_eq!(tool.name(), "lsp");
        assert!(!tool.description().is_empty());
    }
}

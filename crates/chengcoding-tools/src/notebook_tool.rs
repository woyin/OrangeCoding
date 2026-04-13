//! # Notebook 工具
//!
//! Jupyter Notebook (.ipynb) 文件的解析、创建和编辑操作。
//! 支持读取/写入 nbformat v4 格式，以及对单元格的增删改查。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::debug;

// ============================================================
// 数据类型定义
// ============================================================

/// 单元格类型
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellType {
    /// 代码单元格
    Code,
    /// Markdown 单元格
    Markdown,
    /// 原始文本单元格
    Raw,
}

/// 输出类型
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputType {
    /// 流式输出（stdout/stderr）
    Stream,
    /// 显示数据
    DisplayData,
    /// 执行结果
    ExecuteResult,
    /// 错误输出
    Error,
}

/// 单元格输出
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CellOutput {
    /// 输出类型
    pub output_type: OutputType,
    /// 文本内容
    pub text: Option<String>,
    /// 结构化数据
    pub data: Option<Value>,
}

/// Notebook 单元格
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotebookCell {
    /// 单元格类型
    pub cell_type: CellType,
    /// 源代码/文本
    pub source: String,
    /// 输出列表
    pub outputs: Vec<CellOutput>,
    /// 执行计数
    pub execution_count: Option<u32>,
}

/// Notebook 元数据
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotebookMetadata {
    /// 内核名称
    pub kernel: String,
    /// 编程语言
    pub language: String,
}

/// Notebook 文档
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notebook {
    /// 单元格列表
    pub cells: Vec<NotebookCell>,
    /// 元数据
    pub metadata: NotebookMetadata,
}

/// Notebook 操作
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum NotebookAction {
    /// 读取 notebook
    Read { path: String },
    /// 添加单元格
    AddCell {
        path: String,
        cell_type: CellType,
        source: String,
        position: Option<usize>,
    },
    /// 编辑单元格
    EditCell {
        path: String,
        index: usize,
        source: String,
    },
    /// 删除单元格
    DeleteCell { path: String, index: usize },
    /// 创建新 notebook
    Create { path: String, kernel: String },
}

// ============================================================
// NotebookTool — Notebook 操作工具
// ============================================================

/// Notebook 工具 — Jupyter Notebook 操作
#[derive(Debug)]
pub struct NotebookTool;

impl NotebookTool {
    /// 解析 .ipynb 文件内容为 Notebook 结构
    ///
    /// 支持 nbformat v4 格式。
    pub fn parse_notebook(content: &str) -> Result<Notebook, String> {
        let raw: Value =
            serde_json::from_str(content).map_err(|e| format!("JSON 解析失败: {}", e))?;

        let cells_raw = raw
            .get("cells")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "缺少 cells 字段".to_string())?;

        let mut cells = Vec::new();
        for cell_val in cells_raw {
            let cell_type_str = cell_val
                .get("cell_type")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "单元格缺少 cell_type".to_string())?;

            let cell_type = match cell_type_str {
                "code" => CellType::Code,
                "markdown" => CellType::Markdown,
                "raw" => CellType::Raw,
                other => return Err(format!("未知的单元格类型: {}", other)),
            };

            // source 可以是字符串或字符串数组
            let source = match cell_val.get("source") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(""),
                _ => String::new(),
            };

            let execution_count = cell_val
                .get("execution_count")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);

            let outputs = cell_val
                .get("outputs")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|o| {
                            let output_type_str = o.get("output_type")?.as_str()?;
                            let output_type = match output_type_str {
                                "stream" => OutputType::Stream,
                                "display_data" => OutputType::DisplayData,
                                "execute_result" => OutputType::ExecuteResult,
                                "error" => OutputType::Error,
                                _ => return None,
                            };
                            let text = o.get("text").and_then(|v| match v {
                                Value::String(s) => Some(s.clone()),
                                Value::Array(arr) => Some(
                                    arr.iter()
                                        .filter_map(|v| v.as_str())
                                        .collect::<Vec<_>>()
                                        .join(""),
                                ),
                                _ => None,
                            });
                            let data = o.get("data").cloned();
                            Some(CellOutput {
                                output_type,
                                text,
                                data,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            cells.push(NotebookCell {
                cell_type,
                source,
                outputs,
                execution_count,
            });
        }

        // 解析元数据
        let metadata_val = raw.get("metadata");
        let kernel = metadata_val
            .and_then(|m| m.get("kernelspec"))
            .and_then(|k| k.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("python3")
            .to_string();

        let language = metadata_val
            .and_then(|m| m.get("language_info"))
            .and_then(|l| l.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("python")
            .to_string();

        Ok(Notebook {
            cells,
            metadata: NotebookMetadata { kernel, language },
        })
    }

    /// 序列化为 .ipynb (nbformat v4) JSON 格式
    pub fn serialize_notebook(notebook: &Notebook) -> Result<String, String> {
        let cells: Vec<Value> = notebook
            .cells
            .iter()
            .map(|cell| {
                let cell_type = match cell.cell_type {
                    CellType::Code => "code",
                    CellType::Markdown => "markdown",
                    CellType::Raw => "raw",
                };

                let outputs: Vec<Value> = cell
                    .outputs
                    .iter()
                    .map(|o| {
                        let output_type = match o.output_type {
                            OutputType::Stream => "stream",
                            OutputType::DisplayData => "display_data",
                            OutputType::ExecuteResult => "execute_result",
                            OutputType::Error => "error",
                        };
                        let mut obj = json!({ "output_type": output_type });
                        if let Some(ref text) = o.text {
                            obj["text"] = json!(text);
                        }
                        if let Some(ref data) = o.data {
                            obj["data"] = data.clone();
                        }
                        obj
                    })
                    .collect();

                let mut cell_obj = json!({
                    "cell_type": cell_type,
                    "source": cell.source,
                    "metadata": {},
                });

                if cell.cell_type == CellType::Code {
                    cell_obj["outputs"] = json!(outputs);
                    cell_obj["execution_count"] = match cell.execution_count {
                        Some(n) => json!(n),
                        None => Value::Null,
                    };
                }

                cell_obj
            })
            .collect();

        let notebook_json = json!({
            "nbformat": 4,
            "nbformat_minor": 5,
            "metadata": {
                "kernelspec": {
                    "name": notebook.metadata.kernel,
                    "display_name": notebook.metadata.kernel,
                    "language": notebook.metadata.language,
                },
                "language_info": {
                    "name": notebook.metadata.language,
                }
            },
            "cells": cells,
        });

        serde_json::to_string_pretty(&notebook_json).map_err(|e| format!("JSON 序列化失败: {}", e))
    }

    /// 格式化 notebook 为可读文本
    pub fn format_notebook(notebook: &Notebook) -> String {
        let mut output = String::new();
        output.push_str(&format!(
            "Notebook (kernel: {}, language: {})\n",
            notebook.metadata.kernel, notebook.metadata.language
        ));
        output.push_str(&format!("共 {} 个单元格\n", notebook.cells.len()));
        output.push_str("---\n");

        for (i, cell) in notebook.cells.iter().enumerate() {
            let type_label = match cell.cell_type {
                CellType::Code => "Code",
                CellType::Markdown => "Markdown",
                CellType::Raw => "Raw",
            };

            let exec_info = match cell.execution_count {
                Some(n) => format!(" [{}]", n),
                None => String::new(),
            };

            output.push_str(&format!("Cell {} ({}{})\n", i, type_label, exec_info));
            output.push_str(&cell.source);
            if !cell.source.ends_with('\n') {
                output.push('\n');
            }

            // 显示输出
            for o in &cell.outputs {
                if let Some(ref text) = o.text {
                    output.push_str(&format!("=> {}\n", text));
                }
            }

            output.push_str("---\n");
        }

        output
    }

    /// 创建新的空 notebook
    pub fn create_notebook(kernel: &str) -> Notebook {
        Notebook {
            cells: Vec::new(),
            metadata: NotebookMetadata {
                kernel: kernel.to_string(),
                language: "python".to_string(),
            },
        }
    }

    /// 添加单元格
    ///
    /// 如果指定了 `position`，则在该位置插入；否则追加到末尾。
    pub fn add_cell(
        notebook: &mut Notebook,
        cell_type: CellType,
        source: &str,
        position: Option<usize>,
    ) {
        let cell = NotebookCell {
            cell_type,
            source: source.to_string(),
            outputs: Vec::new(),
            execution_count: None,
        };

        match position {
            Some(pos) if pos < notebook.cells.len() => {
                notebook.cells.insert(pos, cell);
            }
            _ => {
                notebook.cells.push(cell);
            }
        }
    }
}

#[async_trait]
impl Tool for NotebookTool {
    fn name(&self) -> &str {
        "notebook"
    }

    fn description(&self) -> &str {
        "Jupyter Notebook 操作，支持读取、创建和编辑 .ipynb 文件"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "操作类型: read, add_cell, edit_cell, delete_cell, create",
                    "enum": ["read", "add_cell", "edit_cell", "delete_cell", "create"]
                },
                "path": {
                    "type": "string",
                    "description": ".ipynb 文件路径"
                },
                "cell_type": {
                    "type": "string",
                    "description": "单元格类型: code, markdown, raw",
                    "enum": ["code", "markdown", "raw"]
                },
                "source": {
                    "type": "string",
                    "description": "单元格源代码/文本"
                },
                "index": {
                    "type": "number",
                    "description": "单元格索引（从 0 开始）"
                },
                "position": {
                    "type": "number",
                    "description": "插入位置（可选）"
                },
                "kernel": {
                    "type": "string",
                    "description": "内核名称（创建时使用），默认 python3",
                    "default": "python3"
                }
            },
            "required": ["action", "path"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: action".to_string()))?;

        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: path".to_string()))?;

        debug!("Notebook 操作: {} on {}", action, path);

        match action {
            "read" => {
                let content = tokio::fs::read_to_string(path)
                    .await
                    .map_err(|e| ToolError::Io(e))?;
                let notebook =
                    Self::parse_notebook(&content).map_err(|e| ToolError::ExecutionError(e))?;
                Ok(Self::format_notebook(&notebook))
            }
            "create" => {
                let kernel = params
                    .get("kernel")
                    .and_then(|v| v.as_str())
                    .unwrap_or("python3");
                let notebook = Self::create_notebook(kernel);
                let content = Self::serialize_notebook(&notebook)
                    .map_err(|e| ToolError::ExecutionError(e))?;
                tokio::fs::write(path, &content)
                    .await
                    .map_err(|e| ToolError::Io(e))?;
                Ok(format!("已创建 Notebook: {}", path))
            }
            "add_cell" => {
                let content = tokio::fs::read_to_string(path)
                    .await
                    .map_err(|e| ToolError::Io(e))?;
                let mut notebook =
                    Self::parse_notebook(&content).map_err(|e| ToolError::ExecutionError(e))?;

                let cell_type_str = params
                    .get("cell_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("code");
                let cell_type = match cell_type_str {
                    "code" => CellType::Code,
                    "markdown" => CellType::Markdown,
                    "raw" => CellType::Raw,
                    other => {
                        return Err(ToolError::InvalidParams(format!(
                            "未知的单元格类型: {}",
                            other
                        )))
                    }
                };

                let source = params.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let position = params
                    .get("position")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);

                Self::add_cell(&mut notebook, cell_type, source, position);

                let serialized = Self::serialize_notebook(&notebook)
                    .map_err(|e| ToolError::ExecutionError(e))?;
                tokio::fs::write(path, &serialized)
                    .await
                    .map_err(|e| ToolError::Io(e))?;

                Ok(format!("已添加 {} 单元格", cell_type_str))
            }
            "edit_cell" => {
                let content = tokio::fs::read_to_string(path)
                    .await
                    .map_err(|e| ToolError::Io(e))?;
                let mut notebook =
                    Self::parse_notebook(&content).map_err(|e| ToolError::ExecutionError(e))?;

                let index = params
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: index".to_string()))?
                    as usize;

                let source = params
                    .get("source")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: source".to_string()))?;

                if index >= notebook.cells.len() {
                    return Err(ToolError::InvalidParams(format!(
                        "单元格索引越界: {}（共 {} 个单元格）",
                        index,
                        notebook.cells.len()
                    )));
                }

                notebook.cells[index].source = source.to_string();

                let serialized = Self::serialize_notebook(&notebook)
                    .map_err(|e| ToolError::ExecutionError(e))?;
                tokio::fs::write(path, &serialized)
                    .await
                    .map_err(|e| ToolError::Io(e))?;

                Ok(format!("已编辑第 {} 个单元格", index))
            }
            "delete_cell" => {
                let content = tokio::fs::read_to_string(path)
                    .await
                    .map_err(|e| ToolError::Io(e))?;
                let mut notebook =
                    Self::parse_notebook(&content).map_err(|e| ToolError::ExecutionError(e))?;

                let index = params
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: index".to_string()))?
                    as usize;

                if index >= notebook.cells.len() {
                    return Err(ToolError::InvalidParams(format!(
                        "单元格索引越界: {}（共 {} 个单元格）",
                        index,
                        notebook.cells.len()
                    )));
                }

                notebook.cells.remove(index);

                let serialized = Self::serialize_notebook(&notebook)
                    .map_err(|e| ToolError::ExecutionError(e))?;
                tokio::fs::write(path, &serialized)
                    .await
                    .map_err(|e| ToolError::Io(e))?;

                Ok(format!("已删除第 {} 个单元格", index))
            }
            other => Err(ToolError::InvalidParams(format!("未知操作: {}", other))),
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

    /// 测试创建新的空 notebook
    #[test]
    fn test_create_notebook() {
        let nb = NotebookTool::create_notebook("python3");

        assert!(nb.cells.is_empty());
        assert_eq!(nb.metadata.kernel, "python3");
        assert_eq!(nb.metadata.language, "python");
    }

    /// 测试添加代码单元格
    #[test]
    fn test_add_code_cell() {
        let mut nb = NotebookTool::create_notebook("python3");
        NotebookTool::add_cell(&mut nb, CellType::Code, "print('hello')", None);

        assert_eq!(nb.cells.len(), 1);
        assert_eq!(nb.cells[0].cell_type, CellType::Code);
        assert_eq!(nb.cells[0].source, "print('hello')");
        assert!(nb.cells[0].outputs.is_empty());
        assert!(nb.cells[0].execution_count.is_none());
    }

    /// 测试添加 Markdown 单元格
    #[test]
    fn test_add_markdown_cell() {
        let mut nb = NotebookTool::create_notebook("python3");
        NotebookTool::add_cell(&mut nb, CellType::Markdown, "# 标题", None);

        assert_eq!(nb.cells.len(), 1);
        assert_eq!(nb.cells[0].cell_type, CellType::Markdown);
        assert_eq!(nb.cells[0].source, "# 标题");
    }

    /// 测试在指定位置插入单元格
    #[test]
    fn test_add_cell_at_position() {
        let mut nb = NotebookTool::create_notebook("python3");
        NotebookTool::add_cell(&mut nb, CellType::Code, "cell_0", None);
        NotebookTool::add_cell(&mut nb, CellType::Code, "cell_2", None);
        // 在位置 1 插入
        NotebookTool::add_cell(&mut nb, CellType::Code, "cell_1", Some(1));

        assert_eq!(nb.cells.len(), 3);
        assert_eq!(nb.cells[0].source, "cell_0");
        assert_eq!(nb.cells[1].source, "cell_1");
        assert_eq!(nb.cells[2].source, "cell_2");
    }

    /// 测试解析 .ipynb 格式
    #[test]
    fn test_parse_ipynb_format() {
        let ipynb = json!({
            "nbformat": 4,
            "nbformat_minor": 5,
            "metadata": {
                "kernelspec": {
                    "name": "python3",
                    "display_name": "Python 3",
                    "language": "python"
                },
                "language_info": {
                    "name": "python"
                }
            },
            "cells": [
                {
                    "cell_type": "markdown",
                    "source": "# Hello",
                    "metadata": {}
                },
                {
                    "cell_type": "code",
                    "source": ["print(", "'hello')"],
                    "metadata": {},
                    "execution_count": 1,
                    "outputs": [
                        {
                            "output_type": "stream",
                            "text": "hello\n"
                        }
                    ]
                }
            ]
        });

        let nb = NotebookTool::parse_notebook(&ipynb.to_string()).unwrap();

        assert_eq!(nb.cells.len(), 2);
        assert_eq!(nb.cells[0].cell_type, CellType::Markdown);
        assert_eq!(nb.cells[0].source, "# Hello");
        assert_eq!(nb.cells[1].cell_type, CellType::Code);
        assert_eq!(nb.cells[1].source, "print('hello')");
        assert_eq!(nb.cells[1].execution_count, Some(1));
        assert_eq!(nb.cells[1].outputs.len(), 1);
        assert_eq!(nb.cells[1].outputs[0].output_type, OutputType::Stream);
        assert_eq!(nb.metadata.kernel, "python3");
        assert_eq!(nb.metadata.language, "python");
    }

    /// 测试序列化再解析的往返一致性
    #[test]
    fn test_serialize_roundtrip() {
        let mut nb = NotebookTool::create_notebook("python3");
        NotebookTool::add_cell(&mut nb, CellType::Code, "x = 1", None);
        NotebookTool::add_cell(&mut nb, CellType::Markdown, "# 说明", None);

        let serialized = NotebookTool::serialize_notebook(&nb).unwrap();
        let parsed = NotebookTool::parse_notebook(&serialized).unwrap();

        assert_eq!(parsed.cells.len(), 2);
        assert_eq!(parsed.cells[0].cell_type, CellType::Code);
        assert_eq!(parsed.cells[0].source, "x = 1");
        assert_eq!(parsed.cells[1].cell_type, CellType::Markdown);
        assert_eq!(parsed.cells[1].source, "# 说明");
        assert_eq!(parsed.metadata.kernel, "python3");
    }

    /// 测试格式化 notebook 显示
    #[test]
    fn test_format_notebook_display() {
        let mut nb = NotebookTool::create_notebook("python3");
        NotebookTool::add_cell(&mut nb, CellType::Code, "print('hi')", None);
        NotebookTool::add_cell(&mut nb, CellType::Markdown, "# 标题", None);

        let display = NotebookTool::format_notebook(&nb);

        assert!(display.contains("python3"));
        assert!(display.contains("2 个单元格"));
        assert!(display.contains("Code"));
        assert!(display.contains("Markdown"));
        assert!(display.contains("print('hi')"));
        assert!(display.contains("# 标题"));
    }

    /// 测试单元格类型枚举
    #[test]
    fn test_cell_types() {
        assert_ne!(CellType::Code, CellType::Markdown);
        assert_ne!(CellType::Code, CellType::Raw);
        assert_ne!(CellType::Markdown, CellType::Raw);

        // 序列化/反序列化
        let code_json = serde_json::to_string(&CellType::Code).unwrap();
        assert_eq!(code_json, "\"code\"");
        let md_json = serde_json::to_string(&CellType::Markdown).unwrap();
        assert_eq!(md_json, "\"markdown\"");
    }

    /// 测试输出类型枚举
    #[test]
    fn test_output_types() {
        assert_ne!(OutputType::Stream, OutputType::Error);
        assert_ne!(OutputType::DisplayData, OutputType::ExecuteResult);

        let stream_json = serde_json::to_string(&OutputType::Stream).unwrap();
        assert_eq!(stream_json, "\"stream\"");
        let error_json = serde_json::to_string(&OutputType::Error).unwrap();
        assert_eq!(error_json, "\"error\"");
    }

    /// 测试参数 Schema 结构
    #[test]
    fn test_parameter_schema() {
        let tool = NotebookTool;
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["action"].is_object());
        assert!(schema["properties"]["path"].is_object());
        assert!(schema["properties"]["cell_type"].is_object());
        assert!(schema["properties"]["source"].is_object());

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("action")));
        assert!(required.contains(&json!("path")));
    }

    /// 测试工具名称和描述
    #[test]
    fn test_tool_name() {
        let tool = NotebookTool;
        assert_eq!(tool.name(), "notebook");
        assert!(!tool.description().is_empty());
    }
}

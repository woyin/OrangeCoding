//! # Ask 工具
//!
//! 向用户提出结构化问题，支持多选、多问题、自由输入等交互模式。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

// ============================================================
// 数据类型定义
// ============================================================

/// 问题定义
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Question {
    /// 问题文本
    pub text: String,
    /// 选项列表（为空时为自由输入）
    pub choices: Vec<Choice>,
    /// 是否允许多选
    pub multi_select: bool,
}

/// 选项
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Choice {
    pub label: String,
    pub value: String,
    pub description: Option<String>,
}

/// 用户回答
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AskResponse {
    /// 对应的问题索引
    pub question_index: usize,
    /// 选中的选项值列表
    pub selected: Vec<String>,
    /// 自由输入文本
    pub text: Option<String>,
}

// ============================================================
// Ask 工具
// ============================================================

/// Ask 工具 — 向用户提出结构化问题
///
/// 支持多选、多问题、自由输入等交互模式。
/// 在非交互模式下返回待回答的问题列表。
#[derive(Debug)]
pub struct AskTool {
    /// 回答发送通道（测试中使用模拟回答）
    #[allow(dead_code)]
    response_tx: Option<mpsc::Sender<AskResponse>>,
    /// 回答接收通道
    response_rx: Option<Arc<Mutex<mpsc::Receiver<AskResponse>>>>,
}

impl AskTool {
    /// 创建非交互模式的 Ask 工具
    pub fn new() -> Self {
        Self {
            response_tx: None,
            response_rx: None,
        }
    }

    /// 创建带回答通道的 Ask 工具（用于测试或交互模式）
    pub fn with_channel() -> (Self, mpsc::Sender<AskResponse>) {
        let (tx, rx) = mpsc::channel(32);
        let tool = Self {
            response_tx: Some(tx.clone()),
            response_rx: Some(Arc::new(Mutex::new(rx))),
        };
        (tool, tx)
    }

    /// 从 JSON 参数解析问题列表
    fn parse_questions(params: &Value) -> ToolResult<Vec<Question>> {
        let questions_val = params
            .get("questions")
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: questions".to_string()))?;

        let questions: Vec<Question> = serde_json::from_value(questions_val.clone())
            .map_err(|e| ToolError::InvalidParams(format!("questions 参数格式错误: {}", e)))?;

        if questions.is_empty() {
            return Err(ToolError::InvalidParams("questions 不能为空".to_string()));
        }

        Ok(questions)
    }

    /// 格式化问题列表为可读文本
    fn format_questions(questions: &[Question]) -> String {
        let mut output = String::new();
        for (i, q) in questions.iter().enumerate() {
            output.push_str(&format!("Question {}: {}\n", i + 1, q.text));
            if q.choices.is_empty() {
                output.push_str("  (自由输入)\n");
            } else {
                for choice in &q.choices {
                    if let Some(desc) = &choice.description {
                        output.push_str(&format!(
                            "  - {} ({}): {}\n",
                            choice.label, choice.value, desc
                        ));
                    } else {
                        output.push_str(&format!("  - {} ({})\n", choice.label, choice.value));
                    }
                }
                if q.multi_select {
                    output.push_str("  (允许多选)\n");
                }
            }
        }
        output
    }

    /// 格式化回答为可读文本
    fn format_responses(questions: &[Question], responses: &[AskResponse]) -> String {
        let mut output = String::new();
        for resp in responses {
            if resp.question_index >= questions.len() {
                continue;
            }
            let q = &questions[resp.question_index];
            output.push_str(&format!(
                "Question {}: {}\n",
                resp.question_index + 1,
                q.text
            ));

            if let Some(text) = &resp.text {
                output.push_str(&format!("Answer: {}\n", text));
            } else if !resp.selected.is_empty() {
                output.push_str(&format!("Answer: {}\n", resp.selected.join(", ")));
            }
            output.push('\n');
        }
        output.trim_end().to_string()
    }
}

impl Default for AskTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for AskTool {
    fn name(&self) -> &str {
        "ask"
    }

    fn description(&self) -> &str {
        "向用户提出结构化问题，支持选择和自由输入"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "questions": {
                    "type": "array",
                    "description": "问题列表",
                    "items": {
                        "type": "object",
                        "properties": {
                            "text": {
                                "type": "string",
                                "description": "问题文本"
                            },
                            "choices": {
                                "type": "array",
                                "description": "选项列表（为空时为自由输入）",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": {
                                            "type": "string",
                                            "description": "选项显示标签"
                                        },
                                        "value": {
                                            "type": "string",
                                            "description": "选项值"
                                        },
                                        "description": {
                                            "type": "string",
                                            "description": "选项描述（可选）"
                                        }
                                    },
                                    "required": ["label", "value"]
                                }
                            },
                            "multi_select": {
                                "type": "boolean",
                                "description": "是否允许多选",
                                "default": false
                            }
                        },
                        "required": ["text"]
                    }
                }
            },
            "required": ["questions"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        let questions = Self::parse_questions(&params)?;

        // 有接收通道时尝试收集回答
        if let Some(rx) = &self.response_rx {
            let mut rx_guard = rx.lock().await;
            let mut responses = Vec::new();

            for _ in 0..questions.len() {
                match rx_guard.try_recv() {
                    Ok(resp) => responses.push(resp),
                    Err(_) => break,
                }
            }

            if responses.len() == questions.len() {
                return Ok(Self::format_responses(&questions, &responses));
            }
        }

        // 非交互模式或未收到全部回答：返回待回答问题
        let formatted = Self::format_questions(&questions);
        Ok(format!("Questions pending user response:\n\n{}", formatted))
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 测试问题序列化与反序列化
    #[test]
    fn test_question_serialization() {
        let question = Question {
            text: "选择数据库".to_string(),
            choices: vec![
                Choice {
                    label: "PostgreSQL".to_string(),
                    value: "pg".to_string(),
                    description: Some("关系型数据库".to_string()),
                },
                Choice {
                    label: "MongoDB".to_string(),
                    value: "mongo".to_string(),
                    description: None,
                },
            ],
            multi_select: false,
        };

        // 序列化
        let json_str = serde_json::to_string(&question).unwrap();
        assert!(json_str.contains("选择数据库"));
        assert!(json_str.contains("PostgreSQL"));

        // 反序列化
        let deserialized: Question = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.text, "选择数据库");
        assert_eq!(deserialized.choices.len(), 2);
        assert_eq!(deserialized.choices[0].value, "pg");
        assert!(!deserialized.multi_select);
    }

    /// 测试单选问题交互
    #[tokio::test]
    async fn test_single_choice_question() {
        let (tool, tx) = AskTool::with_channel();

        // 预先发送回答
        tx.send(AskResponse {
            question_index: 0,
            selected: vec!["pg".to_string()],
            text: None,
        })
        .await
        .unwrap();

        let params = json!({
            "questions": [{
                "text": "What database should I use?",
                "choices": [
                    {"label": "PostgreSQL", "value": "pg"},
                    {"label": "MySQL", "value": "mysql"}
                ],
                "multi_select": false
            }]
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("What database should I use?"));
        assert!(result.contains("pg"));
    }

    /// 测试多选问题交互
    #[tokio::test]
    async fn test_multi_select_question() {
        let (tool, tx) = AskTool::with_channel();

        tx.send(AskResponse {
            question_index: 0,
            selected: vec!["Yes".to_string(), "No".to_string()],
            text: None,
        })
        .await
        .unwrap();

        let params = json!({
            "questions": [{
                "text": "Enable caching?",
                "choices": [
                    {"label": "Yes", "value": "Yes"},
                    {"label": "No", "value": "No"}
                ],
                "multi_select": true
            }]
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("Enable caching?"));
        assert!(result.contains("Yes, No"));
    }

    /// 测试自由文本输入
    #[tokio::test]
    async fn test_free_text_question() {
        let (tool, tx) = AskTool::with_channel();

        tx.send(AskResponse {
            question_index: 0,
            selected: vec![],
            text: Some("My awesome project".to_string()),
        })
        .await
        .unwrap();

        let params = json!({
            "questions": [{
                "text": "What is your project name?",
                "choices": [],
                "multi_select": false
            }]
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("What is your project name?"));
        assert!(result.contains("My awesome project"));
    }

    /// 测试多个问题
    #[tokio::test]
    async fn test_multiple_questions() {
        let (tool, tx) = AskTool::with_channel();

        // 发送两个回答
        tx.send(AskResponse {
            question_index: 0,
            selected: vec!["pg".to_string()],
            text: None,
        })
        .await
        .unwrap();

        tx.send(AskResponse {
            question_index: 1,
            selected: vec!["Yes".to_string()],
            text: None,
        })
        .await
        .unwrap();

        let params = json!({
            "questions": [
                {
                    "text": "What database should I use?",
                    "choices": [{"label": "PostgreSQL", "value": "pg"}],
                    "multi_select": false
                },
                {
                    "text": "Enable caching?",
                    "choices": [{"label": "Yes", "value": "Yes"}, {"label": "No", "value": "No"}],
                    "multi_select": false
                }
            ]
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("Question 1: What database should I use?"));
        assert!(result.contains("Question 2: Enable caching?"));
    }

    /// 测试空选项列表（自由输入模式）
    #[tokio::test]
    async fn test_empty_choices() {
        let tool = AskTool::new();

        let params = json!({
            "questions": [{
                "text": "Describe your requirements",
                "choices": [],
                "multi_select": false
            }]
        });

        let result = tool.execute(params).await.unwrap();
        // 非交互模式返回待回答
        assert!(result.contains("Questions pending user response"));
        assert!(result.contains("Describe your requirements"));
        assert!(result.contains("自由输入"));
    }

    /// 测试参数 Schema
    #[test]
    fn test_parameter_schema() {
        let tool = AskTool::new();
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["questions"].is_object());
        assert_eq!(schema["properties"]["questions"]["type"], "array");

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("questions")));
    }

    /// 测试工具名称和描述
    #[test]
    fn test_tool_name_and_description() {
        let tool = AskTool::new();
        assert_eq!(tool.name(), "ask");
        assert!(!tool.description().is_empty());
        assert!(tool.description().contains("问题") || tool.description().contains("用户"));
    }
}

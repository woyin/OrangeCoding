//! # 任务代理工具
//!
//! 委派子任务给独立代理执行，支持任务状态跟踪和结果收集。
//! 可配置最大执行步数和可用工具列表。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::debug;
use uuid::Uuid;

// ============================================================
// 数据类型定义
// ============================================================

/// 任务请求 — 描述要委派的子任务
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRequest {
    /// 任务描述
    pub description: String,
    /// 附加上下文信息
    pub context: Option<String>,
    /// 可用工具列表
    pub tools: Vec<String>,
    /// 最大执行步数
    pub max_steps: Option<usize>,
}

/// 任务状态
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    /// 等待执行
    Pending,
    /// 正在执行
    Running,
    /// 执行完成
    Completed,
    /// 执行失败
    Failed,
    /// 已取消
    Cancelled,
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskState::Pending => write!(f, "pending"),
            TaskState::Running => write!(f, "running"),
            TaskState::Completed => write!(f, "completed"),
            TaskState::Failed => write!(f, "failed"),
            TaskState::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// 任务结果 — 包含任务执行的状态和输出
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskResult {
    /// 任务唯一标识
    pub task_id: String,
    /// 当前状态
    pub state: TaskState,
    /// 输出结果
    pub output: Option<String>,
    /// 已执行步数
    pub steps_taken: usize,
}

// ============================================================
// TaskTool — 任务代理委派工具
// ============================================================

/// 任务代理工具 — 委派子任务给独立代理
#[derive(Debug)]
pub struct TaskTool;

impl TaskTool {
    /// 创建新的任务请求
    pub fn create_request(
        description: &str,
        context: Option<&str>,
        max_steps: Option<usize>,
    ) -> TaskRequest {
        TaskRequest {
            description: description.to_string(),
            context: context.map(|s| s.to_string()),
            tools: Vec::new(),
            max_steps,
        }
    }

    /// 生成唯一任务 ID
    fn generate_task_id() -> String {
        format!(
            "task-{}",
            Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("unknown")
        )
    }
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "task"
    }

    fn description(&self) -> &str {
        "委派子任务给独立代理执行"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "任务描述 — 代理应该执行什么"
                },
                "context": {
                    "type": "string",
                    "description": "附加上下文信息（可选）"
                },
                "max_steps": {
                    "type": "number",
                    "description": "最大执行步数，默认 20",
                    "default": 20
                }
            },
            "required": ["description"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        let description = params
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: description".to_string()))?;

        let _context = params.get("context").and_then(|v| v.as_str());

        let max_steps = params
            .get("max_steps")
            .and_then(|v| v.as_u64())
            .unwrap_or(20) as usize;

        debug!("创建任务: {}（最大步数: {}）", description, max_steps);

        let task_id = Self::generate_task_id();

        // 创建任务结果（目前返回占位结果，待代理系统集成后实现实际委派）
        let result = TaskResult {
            task_id: task_id.clone(),
            state: TaskState::Pending,
            output: None,
            steps_taken: 0,
        };

        let result_json = serde_json::to_string_pretty(&result)
            .map_err(|e| ToolError::ExecutionError(format!("序列化任务结果失败: {}", e)))?;

        Ok(format!(
            "任务已创建: {}\n描述: {}\n最大步数: {}\n\n{}",
            task_id, description, max_steps, result_json
        ))
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 测试任务请求创建
    #[test]
    fn test_task_request_creation() {
        let request = TaskTool::create_request("分析代码库", Some("检查 src/ 目录"), Some(10));

        assert_eq!(request.description, "分析代码库");
        assert_eq!(request.context, Some("检查 src/ 目录".to_string()));
        assert_eq!(request.max_steps, Some(10));
        assert!(request.tools.is_empty());

        // 测试无上下文的请求
        let request2 = TaskTool::create_request("简单任务", None, None);
        assert!(request2.context.is_none());
        assert!(request2.max_steps.is_none());
    }

    /// 测试任务状态枚举
    #[test]
    fn test_task_states() {
        assert_eq!(TaskState::Pending.to_string(), "pending");
        assert_eq!(TaskState::Running.to_string(), "running");
        assert_eq!(TaskState::Completed.to_string(), "completed");
        assert_eq!(TaskState::Failed.to_string(), "failed");
        assert_eq!(TaskState::Cancelled.to_string(), "cancelled");

        // 验证相等性比较
        assert_eq!(TaskState::Pending, TaskState::Pending);
        assert_ne!(TaskState::Pending, TaskState::Running);
    }

    /// 测试任务结果序列化
    #[test]
    fn test_task_result_serialization() {
        let result = TaskResult {
            task_id: "task-abc123".to_string(),
            state: TaskState::Completed,
            output: Some("任务执行成功".to_string()),
            steps_taken: 5,
        };

        // 序列化
        let json_str = serde_json::to_string(&result).unwrap();
        assert!(json_str.contains("task-abc123"));
        assert!(json_str.contains("Completed"));

        // 反序列化
        let deserialized: TaskResult = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.task_id, "task-abc123");
        assert_eq!(deserialized.state, TaskState::Completed);
        assert_eq!(deserialized.output, Some("任务执行成功".to_string()));
        assert_eq!(deserialized.steps_taken, 5);
    }

    /// 测试参数 Schema 结构
    #[test]
    fn test_parameter_schema() {
        let tool = TaskTool;
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["description"].is_object());
        assert!(schema["properties"]["context"].is_object());
        assert!(schema["properties"]["max_steps"].is_object());

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("description")));
    }

    /// 测试工具名称
    #[test]
    fn test_tool_name() {
        let tool = TaskTool;
        assert_eq!(tool.name(), "task");
        assert!(!tool.description().is_empty());
    }
}

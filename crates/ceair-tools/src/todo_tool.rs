//! # Todo 工具
//!
//! 分阶段任务跟踪，支持多阶段组织任务并跟踪进度。
//! 每次变更后自动规范化：确保恰好一个任务处于进行中状态。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================
// 数据类型定义
// ============================================================

/// 任务状态
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Abandoned,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Abandoned => write!(f, "abandoned"),
        }
    }
}

impl TaskStatus {
    /// 从字符串解析任务状态
    fn from_str(s: &str) -> ToolResult<Self> {
        match s {
            "pending" => Ok(TaskStatus::Pending),
            "in_progress" => Ok(TaskStatus::InProgress),
            "completed" => Ok(TaskStatus::Completed),
            "abandoned" => Ok(TaskStatus::Abandoned),
            other => Err(ToolError::InvalidParams(format!(
                "未知任务状态: {}",
                other
            ))),
        }
    }

    /// 返回状态对应的图标
    fn icon(&self) -> &str {
        match self {
            TaskStatus::Pending => "⬜",
            TaskStatus::InProgress => "🔄",
            TaskStatus::Completed => "✅",
            TaskStatus::Abandoned => "🚫",
        }
    }
}

/// 单个任务
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TodoTask {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
}

/// 任务阶段
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Phase {
    pub name: String,
    pub tasks: Vec<TodoTask>,
}

/// Todo 状态
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TodoState {
    /// 所有阶段
    pub phases: Vec<Phase>,
}

impl TodoState {
    /// 自动规范化：确保恰好一个任务处于进行中状态
    ///
    /// - 若无任务处于 InProgress，则将第一个 Pending 任务设为 InProgress
    /// - 若多个任务处于 InProgress，仅保留最后一个
    fn normalize(&mut self) {
        // 收集所有 InProgress 任务的位置
        let mut in_progress: Vec<(usize, usize)> = Vec::new();
        for (pi, phase) in self.phases.iter().enumerate() {
            for (ti, task) in phase.tasks.iter().enumerate() {
                if task.status == TaskStatus::InProgress {
                    in_progress.push((pi, ti));
                }
            }
        }

        if in_progress.len() > 1 {
            // 保留最后一个，其余改为 Pending
            for &(pi, ti) in &in_progress[..in_progress.len() - 1] {
                self.phases[pi].tasks[ti].status = TaskStatus::Pending;
            }
        } else if in_progress.is_empty() {
            // 将第一个 Pending 任务设为 InProgress
            for phase in &mut self.phases {
                for task in &mut phase.tasks {
                    if task.status == TaskStatus::Pending {
                        task.status = TaskStatus::InProgress;
                        return;
                    }
                }
            }
        }
    }

    /// 格式化输出当前状态
    fn format(&self) -> String {
        if self.phases.is_empty() {
            return "📋 Todo List\n──────────────\n(空)".to_string();
        }

        let mut output = String::from("📋 Todo List\n──────────────\n");

        // 统计进度
        let mut total = 0usize;
        let mut completed = 0usize;
        for phase in &self.phases {
            for task in &phase.tasks {
                total += 1;
                if task.status == TaskStatus::Completed {
                    completed += 1;
                }
            }
        }

        for (i, phase) in self.phases.iter().enumerate() {
            if i > 0 {
                output.push('\n');
            }
            output.push_str(&format!("Phase: {}\n", phase.name));
            if phase.tasks.is_empty() {
                output.push_str("  (无任务)\n");
            } else {
                for task in &phase.tasks {
                    let suffix = if task.status == TaskStatus::InProgress {
                        " (in progress)"
                    } else {
                        ""
                    };
                    output.push_str(&format!(
                        "  {} [{}] {}{}\n",
                        task.status.icon(),
                        task.id,
                        task.title,
                        suffix
                    ));
                }
            }
        }

        if total > 0 {
            output.push_str(&format!("\nProgress: {}/{} completed", completed, total));
        }

        output
    }

    /// 根据 ID 查找任务（返回阶段索引和任务索引）
    fn find_task(&self, task_id: &str) -> Option<(usize, usize)> {
        for (pi, phase) in self.phases.iter().enumerate() {
            for (ti, task) in phase.tasks.iter().enumerate() {
                if task.id == task_id {
                    return Some((pi, ti));
                }
            }
        }
        None
    }

    /// 根据名称查找阶段索引
    fn find_phase(&self, name: &str) -> Option<usize> {
        self.phases.iter().position(|p| p.name == name)
    }
}

// ============================================================
// Todo 工具
// ============================================================

/// Todo 工具 — 分阶段任务跟踪
///
/// 支持分阶段组织任务，跟踪进度。
/// 每次变更后自动规范化任务状态。
#[derive(Debug)]
pub struct TodoTool {
    state: Arc<RwLock<TodoState>>,
}

impl TodoTool {
    /// 创建新的 Todo 工具
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(TodoState::default())),
        }
    }

    /// 使用初始状态创建 Todo 工具
    pub fn with_state(state: TodoState) -> Self {
        Self {
            state: Arc::new(RwLock::new(state)),
        }
    }

    /// 获取当前状态的快照
    pub async fn get_state(&self) -> TodoState {
        self.state.read().await.clone()
    }

    /// 处理 replace 操作：替换所有阶段
    fn handle_replace(state: &mut TodoState, params: &Value) -> ToolResult<()> {
        let phases_val = params
            .get("phases")
            .ok_or_else(|| ToolError::InvalidParams("replace 操作需要 phases 参数".to_string()))?;

        let phases: Vec<Phase> = serde_json::from_value(phases_val.clone()).map_err(|e| {
            ToolError::InvalidParams(format!("phases 参数格式错误: {}", e))
        })?;

        state.phases = phases;

        // 确保每个任务有默认状态（serde 反序列化时由 JSON 解析保证）
        for phase in &mut state.phases {
            for _task in &mut phase.tasks {}
        }

        Ok(())
    }

    /// 处理 add_phase 操作：添加新阶段
    fn handle_add_phase(state: &mut TodoState, params: &Value) -> ToolResult<()> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidParams("add_phase 操作需要 name 参数".to_string())
            })?;

        let tasks: Vec<TodoTask> = if let Some(tasks_val) = params.get("tasks") {
            serde_json::from_value(tasks_val.clone())
                .map_err(|e| ToolError::InvalidParams(format!("tasks 参数格式错误: {}", e)))?
        } else {
            Vec::new()
        };

        state.phases.push(Phase {
            name: name.to_string(),
            tasks,
        });

        Ok(())
    }

    /// 处理 add_task 操作：向指定阶段添加任务
    fn handle_add_task(state: &mut TodoState, params: &Value) -> ToolResult<()> {
        let phase_name = params
            .get("phase")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidParams("add_task 操作需要 phase 参数".to_string())
            })?;

        let task_val = params.get("task").ok_or_else(|| {
            ToolError::InvalidParams("add_task 操作需要 task 参数".to_string())
        })?;

        let id = task_val
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("task 需要 id 字段".to_string()))?;

        let title = task_val
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("task 需要 title 字段".to_string()))?;

        let phase_idx = state.find_phase(phase_name).ok_or_else(|| {
            ToolError::ExecutionError(format!("阶段不存在: {}", phase_name))
        })?;

        state.phases[phase_idx].tasks.push(TodoTask {
            id: id.to_string(),
            title: title.to_string(),
            status: TaskStatus::Pending,
        });

        Ok(())
    }

    /// 处理 update 操作：更新任务状态
    fn handle_update(state: &mut TodoState, params: &Value) -> ToolResult<()> {
        let task_id = params
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidParams("update 操作需要 task_id 参数".to_string())
            })?;

        let status_str = params
            .get("status")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidParams("update 操作需要 status 参数".to_string())
            })?;

        let new_status = TaskStatus::from_str(status_str)?;

        let (pi, ti) = state.find_task(task_id).ok_or_else(|| {
            ToolError::ExecutionError(format!("任务不存在: {}", task_id))
        })?;

        state.phases[pi].tasks[ti].status = new_status;

        Ok(())
    }

    /// 处理 remove_task 操作：移除任务
    fn handle_remove_task(state: &mut TodoState, params: &Value) -> ToolResult<()> {
        let task_id = params
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidParams("remove_task 操作需要 task_id 参数".to_string())
            })?;

        let (pi, ti) = state.find_task(task_id).ok_or_else(|| {
            ToolError::ExecutionError(format!("任务不存在: {}", task_id))
        })?;

        state.phases[pi].tasks.remove(ti);

        Ok(())
    }
}

impl Default for TodoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        "todo"
    }

    fn description(&self) -> &str {
        "分阶段任务跟踪工具，支持创建、更新和管理任务"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "操作类型",
                    "enum": ["replace", "add_phase", "add_task", "update", "remove_task"]
                },
                "phases": {
                    "type": "array",
                    "description": "阶段列表（replace 操作使用）",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": {"type": "string"},
                            "tasks": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "id": {"type": "string"},
                                        "title": {"type": "string"},
                                        "status": {"type": "string", "enum": ["pending", "in_progress", "completed", "abandoned"]}
                                    },
                                    "required": ["id", "title"]
                                }
                            }
                        },
                        "required": ["name", "tasks"]
                    }
                },
                "name": {
                    "type": "string",
                    "description": "阶段名称（add_phase 操作使用）"
                },
                "tasks": {
                    "type": "array",
                    "description": "任务列表（add_phase 操作使用，可选）"
                },
                "phase": {
                    "type": "string",
                    "description": "目标阶段名称（add_task 操作使用）"
                },
                "task": {
                    "type": "object",
                    "description": "任务定义（add_task 操作使用）",
                    "properties": {
                        "id": {"type": "string"},
                        "title": {"type": "string"}
                    },
                    "required": ["id", "title"]
                },
                "task_id": {
                    "type": "string",
                    "description": "任务 ID（update/remove_task 操作使用）"
                },
                "status": {
                    "type": "string",
                    "description": "新的任务状态（update 操作使用）",
                    "enum": ["pending", "in_progress", "completed", "abandoned"]
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

        let mut state = self.state.write().await;

        match action {
            "replace" => Self::handle_replace(&mut state, &params)?,
            "add_phase" => Self::handle_add_phase(&mut state, &params)?,
            "add_task" => Self::handle_add_task(&mut state, &params)?,
            "update" => Self::handle_update(&mut state, &params)?,
            "remove_task" => Self::handle_remove_task(&mut state, &params)?,
            other => {
                return Err(ToolError::InvalidParams(format!(
                    "未知操作: {}",
                    other
                )));
            }
        }

        // 每次变更后自动规范化
        state.normalize();

        Ok(state.format())
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 测试替换所有阶段
    #[tokio::test]
    async fn test_replace_phases() {
        let tool = TodoTool::new();

        let params = json!({
            "action": "replace",
            "phases": [
                {
                    "name": "Setup",
                    "tasks": [
                        {"id": "setup-1", "title": "Initialize project", "status": "Completed"},
                        {"id": "setup-2", "title": "Configure database", "status": "Pending"}
                    ]
                },
                {
                    "name": "Implementation",
                    "tasks": [
                        {"id": "impl-1", "title": "Create API routes", "status": "Pending"}
                    ]
                }
            ]
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("Phase: Setup"));
        assert!(result.contains("Phase: Implementation"));
        assert!(result.contains("[setup-1]"));
        assert!(result.contains("[impl-1]"));

        // 验证状态
        let state = tool.get_state().await;
        assert_eq!(state.phases.len(), 2);
        assert_eq!(state.phases[0].tasks.len(), 2);
    }

    /// 测试添加新阶段
    #[tokio::test]
    async fn test_add_phase() {
        let tool = TodoTool::new();

        let params = json!({
            "action": "add_phase",
            "name": "Testing",
            "tasks": [
                {"id": "test-1", "title": "Write unit tests", "status": "Pending"}
            ]
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("Phase: Testing"));
        assert!(result.contains("[test-1]"));

        let state = tool.get_state().await;
        assert_eq!(state.phases.len(), 1);
        assert_eq!(state.phases[0].name, "Testing");
    }

    /// 测试向阶段添加任务
    #[tokio::test]
    async fn test_add_task_to_phase() {
        let tool = TodoTool::new();

        // 先创建阶段
        tool.execute(json!({
            "action": "add_phase",
            "name": "Development"
        }))
        .await
        .unwrap();

        // 添加任务
        let result = tool
            .execute(json!({
                "action": "add_task",
                "phase": "Development",
                "task": {"id": "dev-1", "title": "Build feature"}
            }))
            .await
            .unwrap();

        assert!(result.contains("[dev-1]"));
        assert!(result.contains("Build feature"));

        let state = tool.get_state().await;
        assert_eq!(state.phases[0].tasks.len(), 1);
        assert_eq!(state.phases[0].tasks[0].id, "dev-1");
    }

    /// 测试更新任务状态
    #[tokio::test]
    async fn test_update_task_status() {
        let tool = TodoTool::new();

        // 初始化
        tool.execute(json!({
            "action": "replace",
            "phases": [{
                "name": "Work",
                "tasks": [
                    {"id": "w-1", "title": "Task A", "status": "Pending"},
                    {"id": "w-2", "title": "Task B", "status": "Pending"}
                ]
            }]
        }))
        .await
        .unwrap();

        // 将 w-1 标记为完成
        let result = tool
            .execute(json!({
                "action": "update",
                "task_id": "w-1",
                "status": "completed"
            }))
            .await
            .unwrap();

        assert!(result.contains("✅"));

        let state = tool.get_state().await;
        assert_eq!(state.phases[0].tasks[0].status, TaskStatus::Completed);
    }

    /// 测试移除任务
    #[tokio::test]
    async fn test_remove_task() {
        let tool = TodoTool::new();

        tool.execute(json!({
            "action": "replace",
            "phases": [{
                "name": "Work",
                "tasks": [
                    {"id": "w-1", "title": "Task A", "status": "Pending"},
                    {"id": "w-2", "title": "Task B", "status": "Pending"}
                ]
            }]
        }))
        .await
        .unwrap();

        let result = tool
            .execute(json!({
                "action": "remove_task",
                "task_id": "w-1"
            }))
            .await
            .unwrap();

        assert!(!result.contains("[w-1]"));
        assert!(result.contains("[w-2]"));

        let state = tool.get_state().await;
        assert_eq!(state.phases[0].tasks.len(), 1);
    }

    /// 测试自动规范化：无进行中任务时设置第一个 Pending
    #[tokio::test]
    async fn test_auto_normalize_sets_first_pending() {
        let tool = TodoTool::new();

        tool.execute(json!({
            "action": "replace",
            "phases": [{
                "name": "Work",
                "tasks": [
                    {"id": "w-1", "title": "First", "status": "Pending"},
                    {"id": "w-2", "title": "Second", "status": "Pending"}
                ]
            }]
        }))
        .await
        .unwrap();

        let state = tool.get_state().await;
        // 第一个 Pending 任务应被自动设为 InProgress
        assert_eq!(state.phases[0].tasks[0].status, TaskStatus::InProgress);
        assert_eq!(state.phases[0].tasks[1].status, TaskStatus::Pending);
    }

    /// 测试自动规范化：多个 InProgress 仅保留最后一个
    #[tokio::test]
    async fn test_auto_normalize_single_in_progress() {
        let tool = TodoTool::new();

        tool.execute(json!({
            "action": "replace",
            "phases": [{
                "name": "Work",
                "tasks": [
                    {"id": "w-1", "title": "First", "status": "InProgress"},
                    {"id": "w-2", "title": "Second", "status": "InProgress"},
                    {"id": "w-3", "title": "Third", "status": "InProgress"}
                ]
            }]
        }))
        .await
        .unwrap();

        let state = tool.get_state().await;
        // 仅最后一个应保持 InProgress
        assert_eq!(state.phases[0].tasks[0].status, TaskStatus::Pending);
        assert_eq!(state.phases[0].tasks[1].status, TaskStatus::Pending);
        assert_eq!(state.phases[0].tasks[2].status, TaskStatus::InProgress);
    }

    /// 测试进度摘要
    #[tokio::test]
    async fn test_progress_summary() {
        let tool = TodoTool::new();

        let result = tool
            .execute(json!({
                "action": "replace",
                "phases": [{
                    "name": "Work",
                    "tasks": [
                        {"id": "w-1", "title": "Done task", "status": "Completed"},
                        {"id": "w-2", "title": "Pending task", "status": "Pending"}
                    ]
                }]
            }))
            .await
            .unwrap();

        assert!(result.contains("Progress: 1/2 completed"));
    }

    /// 测试已完成阶段
    #[tokio::test]
    async fn test_completed_phase() {
        let tool = TodoTool::new();

        let result = tool
            .execute(json!({
                "action": "replace",
                "phases": [{
                    "name": "Done Phase",
                    "tasks": [
                        {"id": "d-1", "title": "Task A", "status": "Completed"},
                        {"id": "d-2", "title": "Task B", "status": "Completed"}
                    ]
                }]
            }))
            .await
            .unwrap();

        assert!(result.contains("✅ [d-1]"));
        assert!(result.contains("✅ [d-2]"));
        assert!(result.contains("Progress: 2/2 completed"));
    }

    /// 测试向不存在的阶段添加任务
    #[tokio::test]
    async fn test_unknown_phase_error() {
        let tool = TodoTool::new();

        let result = tool
            .execute(json!({
                "action": "add_task",
                "phase": "Nonexistent",
                "task": {"id": "x-1", "title": "Test"}
            }))
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{}", err).contains("阶段不存在"));
    }

    /// 测试更新不存在的任务
    #[tokio::test]
    async fn test_unknown_task_error() {
        let tool = TodoTool::new();

        let result = tool
            .execute(json!({
                "action": "update",
                "task_id": "nonexistent",
                "status": "completed"
            }))
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{}", err).contains("任务不存在"));
    }

    /// 测试任务状态显示
    #[test]
    fn test_task_status_display() {
        assert_eq!(TaskStatus::Pending.to_string(), "pending");
        assert_eq!(TaskStatus::InProgress.to_string(), "in_progress");
        assert_eq!(TaskStatus::Completed.to_string(), "completed");
        assert_eq!(TaskStatus::Abandoned.to_string(), "abandoned");

        assert_eq!(TaskStatus::Pending.icon(), "⬜");
        assert_eq!(TaskStatus::InProgress.icon(), "🔄");
        assert_eq!(TaskStatus::Completed.icon(), "✅");
        assert_eq!(TaskStatus::Abandoned.icon(), "🚫");
    }

    /// 测试空状态
    #[tokio::test]
    async fn test_empty_state() {
        let tool = TodoTool::new();

        let result = tool
            .execute(json!({
                "action": "replace",
                "phases": []
            }))
            .await
            .unwrap();

        assert!(result.contains("(空)"));
    }

    /// 测试参数 Schema
    #[test]
    fn test_parameter_schema() {
        let tool = TodoTool::new();
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["action"].is_object());
        assert!(schema["properties"]["phases"].is_object());
        assert!(schema["properties"]["task_id"].is_object());
        assert!(schema["properties"]["status"].is_object());

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("action")));
    }

    /// 测试多阶段与进度
    #[tokio::test]
    async fn test_multiple_phases_with_progress() {
        let tool = TodoTool::new();

        // 创建多个阶段
        tool.execute(json!({
            "action": "replace",
            "phases": [
                {
                    "name": "Setup",
                    "tasks": [
                        {"id": "s-1", "title": "Init", "status": "Completed"},
                        {"id": "s-2", "title": "Config", "status": "Completed"}
                    ]
                },
                {
                    "name": "Build",
                    "tasks": [
                        {"id": "b-1", "title": "Routes", "status": "Pending"},
                        {"id": "b-2", "title": "Models", "status": "Pending"}
                    ]
                }
            ]
        }))
        .await
        .unwrap();

        let state = tool.get_state().await;

        // Setup 阶段全部完成
        assert_eq!(state.phases[0].tasks[0].status, TaskStatus::Completed);
        assert_eq!(state.phases[0].tasks[1].status, TaskStatus::Completed);

        // Build 阶段第一个 Pending 被自动设为 InProgress
        assert_eq!(state.phases[1].tasks[0].status, TaskStatus::InProgress);
        assert_eq!(state.phases[1].tasks[1].status, TaskStatus::Pending);

        // 完成 b-1 后 b-2 应自动变为 InProgress
        tool.execute(json!({
            "action": "update",
            "task_id": "b-1",
            "status": "completed"
        }))
        .await
        .unwrap();

        let state = tool.get_state().await;
        assert_eq!(state.phases[1].tasks[0].status, TaskStatus::Completed);
        assert_eq!(state.phases[1].tasks[1].status, TaskStatus::InProgress);

        // 进度应为 3/4
        let result = tool
            .execute(json!({
                "action": "update",
                "task_id": "b-2",
                "status": "completed"
            }))
            .await
            .unwrap();

        assert!(result.contains("Progress: 4/4 completed"));
    }
}

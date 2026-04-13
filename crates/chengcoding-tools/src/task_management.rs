//! # 任务管理工具模块
//!
//! 提供任务创建、查询、更新、列表等工具，支持依赖追踪和并行执行。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

// ============================================================
// 数据模型
// ============================================================

/// 任务状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// 待处理
    Pending,
    /// 进行中
    InProgress,
    /// 已完成
    Completed,
    /// 已删除
    Deleted,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Deleted => write!(f, "deleted"),
        }
    }
}

impl TaskStatus {
    /// 从字符串解析任务状态
    pub fn from_str(s: &str) -> Result<Self, ToolError> {
        match s {
            "pending" => Ok(TaskStatus::Pending),
            "in_progress" => Ok(TaskStatus::InProgress),
            "completed" => Ok(TaskStatus::Completed),
            "deleted" => Ok(TaskStatus::Deleted),
            other => Err(ToolError::InvalidParams(format!("未知任务状态: {}", other))),
        }
    }
}

/// 任务条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    /// 任务唯一标识（格式：T-{uuid}）
    pub id: String,
    /// 任务主题
    pub subject: String,
    /// 任务详细描述
    pub description: String,
    /// 当前状态
    pub status: TaskStatus,
    /// 活动表单（可选，用于交互式输入）
    pub active_form: Option<String>,
    /// 此任务阻塞的其他任务 ID 列表
    pub blocks: Vec<String>,
    /// 阻塞此任务的其他任务 ID 列表
    pub blocked_by: Vec<String>,
    /// 任务负责人（可选）
    pub owner: Option<String>,
    /// 所属线程 ID
    pub thread_id: String,
}

/// 任务更新参数
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskUpdate {
    /// 更新主题（可选）
    pub subject: Option<String>,
    /// 更新描述（可选）
    pub description: Option<String>,
    /// 更新状态（可选）
    pub status: Option<TaskStatus>,
    /// 更新负责人（可选）
    pub owner: Option<String>,
    /// 添加阻塞的任务 ID（可选）
    pub add_blocks: Option<Vec<String>>,
    /// 添加被阻塞的任务 ID（可选）
    pub add_blocked_by: Option<Vec<String>>,
}

// ============================================================
// 任务存储
// ============================================================

/// 任务存储管理器
///
/// 提供任务的增删改查和依赖关系管理。
#[derive(Debug, Clone)]
pub struct TaskStore {
    /// 任务映射（ID → TaskItem）
    tasks: Arc<RwLock<HashMap<String, TaskItem>>>,
    /// 存储目录（用于持久化）
    #[allow(dead_code)]
    storage_dir: PathBuf,
}

impl TaskStore {
    /// 创建新的任务存储
    pub fn new(storage_dir: PathBuf) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            storage_dir,
        }
    }

    /// 创建新任务
    pub fn create(&self, subject: String, description: String) -> TaskItem {
        let id = format!("T-{}", Uuid::new_v4());
        let task = TaskItem {
            id: id.clone(),
            subject,
            description,
            status: TaskStatus::Pending,
            active_form: None,
            blocks: Vec::new(),
            blocked_by: Vec::new(),
            owner: None,
            thread_id: Uuid::new_v4().to_string(),
        };
        self.tasks.write().insert(id, task.clone());
        task
    }

    /// 获取指定 ID 的任务
    pub fn get(&self, id: &str) -> Option<TaskItem> {
        self.tasks.read().get(id).cloned()
    }

    /// 列出所有未删除的任务
    pub fn list(&self) -> Vec<TaskItem> {
        self.tasks
            .read()
            .values()
            .filter(|t| t.status != TaskStatus::Deleted)
            .cloned()
            .collect()
    }

    /// 更新指定任务的字段
    pub fn update(&self, id: &str, updates: TaskUpdate) -> Result<TaskItem, ToolError> {
        let mut tasks = self.tasks.write();
        let task = tasks
            .get_mut(id)
            .ok_or_else(|| ToolError::NotFound(format!("任务未找到: {}", id)))?;

        if let Some(subject) = updates.subject {
            task.subject = subject;
        }
        if let Some(description) = updates.description {
            task.description = description;
        }
        if let Some(status) = updates.status {
            task.status = status;
        }
        if let Some(owner) = updates.owner {
            task.owner = Some(owner);
        }
        if let Some(blocks) = updates.add_blocks {
            task.blocks.extend(blocks);
        }
        if let Some(blocked_by) = updates.add_blocked_by {
            task.blocked_by.extend(blocked_by);
        }

        Ok(task.clone())
    }

    /// 软删除任务（将状态设为 Deleted）
    pub fn delete(&self, id: &str) -> Result<(), ToolError> {
        let mut tasks = self.tasks.write();
        let task = tasks
            .get_mut(id)
            .ok_or_else(|| ToolError::NotFound(format!("任务未找到: {}", id)))?;
        task.status = TaskStatus::Deleted;
        Ok(())
    }

    /// 获取可执行的任务列表（所有阻塞者均已完成或已删除的待处理任务）
    ///
    /// 返回状态为 Pending 且所有 `blocked_by` 中的任务都已完成/删除的任务。
    pub fn get_ready_tasks(&self) -> Vec<TaskItem> {
        let tasks = self.tasks.read();

        tasks
            .values()
            .filter(|task| {
                if task.status != TaskStatus::Pending {
                    return false;
                }
                // 检查所有阻塞者是否都已完成或已删除
                task.blocked_by.iter().all(|blocker_id| {
                    tasks
                        .get(blocker_id)
                        .map(|b| {
                            b.status == TaskStatus::Completed || b.status == TaskStatus::Deleted
                        })
                        .unwrap_or(true) // 找不到的阻塞者视为已解除
                })
            })
            .cloned()
            .collect()
    }
}

// ============================================================
// 任务创建工具
// ============================================================

/// 任务创建工具 —— 创建新的任务条目
#[derive(Debug)]
pub struct TaskCreateTool {
    store: TaskStore,
}

impl TaskCreateTool {
    /// 创建任务创建工具
    pub fn new(store: TaskStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str {
        "task_create"
    }

    fn description(&self) -> &str {
        "创建一个新的任务条目"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "subject": {
                    "type": "string",
                    "description": "任务主题"
                },
                "description": {
                    "type": "string",
                    "description": "任务详细描述"
                }
            },
            "required": ["subject", "description"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        let subject = params
            .get("subject")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少 subject 参数".to_string()))?;

        let description = params
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少 description 参数".to_string()))?;

        let task = self
            .store
            .create(subject.to_string(), description.to_string());

        serde_json::to_string_pretty(&task)
            .map_err(|e| ToolError::ExecutionError(format!("序列化任务失败: {}", e)))
    }
}

// ============================================================
// 任务查询工具
// ============================================================

/// 任务查询工具 —— 获取指定任务的详细信息
#[derive(Debug)]
pub struct TaskGetTool {
    store: TaskStore,
}

impl TaskGetTool {
    /// 创建任务查询工具
    pub fn new(store: TaskStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &str {
        "task_get"
    }

    fn description(&self) -> &str {
        "获取指定任务的详细信息"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "任务唯一标识"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        let task_id = params
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少 task_id 参数".to_string()))?;

        let task = self
            .store
            .get(task_id)
            .ok_or_else(|| ToolError::NotFound(format!("任务未找到: {}", task_id)))?;

        serde_json::to_string_pretty(&task)
            .map_err(|e| ToolError::ExecutionError(format!("序列化任务失败: {}", e)))
    }
}

// ============================================================
// 任务列表工具
// ============================================================

/// 任务列表工具 —— 列出所有活跃的任务
#[derive(Debug)]
pub struct TaskListTool {
    store: TaskStore,
}

impl TaskListTool {
    /// 创建任务列表工具
    pub fn new(store: TaskStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str {
        "task_list"
    }

    fn description(&self) -> &str {
        "列出所有活跃的任务（不含已删除的任务）"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _params: Value) -> ToolResult<String> {
        let tasks = self.store.list();
        serde_json::to_string_pretty(&tasks)
            .map_err(|e| ToolError::ExecutionError(format!("序列化任务列表失败: {}", e)))
    }
}

// ============================================================
// 任务更新工具
// ============================================================

/// 任务更新工具 —— 更新指定任务的字段
#[derive(Debug)]
pub struct TaskUpdateTool {
    store: TaskStore,
}

impl TaskUpdateTool {
    /// 创建任务更新工具
    pub fn new(store: TaskStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &str {
        "task_update"
    }

    fn description(&self) -> &str {
        "更新指定任务的状态、主题、描述或负责人"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "任务唯一标识"
                },
                "subject": {
                    "type": "string",
                    "description": "更新后的主题（可选）"
                },
                "description": {
                    "type": "string",
                    "description": "更新后的描述（可选）"
                },
                "status": {
                    "type": "string",
                    "enum": ["pending", "in_progress", "completed", "deleted"],
                    "description": "更新后的状态（可选）"
                },
                "owner": {
                    "type": "string",
                    "description": "更新后的负责人（可选）"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        let task_id = params
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少 task_id 参数".to_string()))?;

        let mut updates = TaskUpdate::default();

        if let Some(subject) = params.get("subject").and_then(|v| v.as_str()) {
            updates.subject = Some(subject.to_string());
        }
        if let Some(description) = params.get("description").and_then(|v| v.as_str()) {
            updates.description = Some(description.to_string());
        }
        if let Some(status_str) = params.get("status").and_then(|v| v.as_str()) {
            updates.status = Some(TaskStatus::from_str(status_str)?);
        }
        if let Some(owner) = params.get("owner").and_then(|v| v.as_str()) {
            updates.owner = Some(owner.to_string());
        }

        let task = self.store.update(task_id, updates)?;

        serde_json::to_string_pretty(&task)
            .map_err(|e| ToolError::ExecutionError(format!("序列化任务失败: {}", e)))
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用的任务存储
    fn create_test_store() -> TaskStore {
        TaskStore::new(PathBuf::from("./test_tasks"))
    }

    /// 测试创建任务
    #[test]
    fn 测试创建任务() {
        let store = create_test_store();
        let task = store.create("测试任务".to_string(), "这是一个测试".to_string());

        assert!(task.id.starts_with("T-"));
        assert_eq!(task.subject, "测试任务");
        assert_eq!(task.description, "这是一个测试");
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.blocks.is_empty());
        assert!(task.blocked_by.is_empty());
    }

    /// 测试获取任务
    #[test]
    fn 测试获取任务() {
        let store = create_test_store();
        let created = store.create("查询测试".to_string(), "描述".to_string());

        let fetched = store.get(&created.id).unwrap();
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.subject, "查询测试");
    }

    /// 测试获取不存在的任务
    #[test]
    fn 测试获取不存在的任务() {
        let store = create_test_store();
        assert!(store.get("T-nonexistent").is_none());
    }

    /// 测试列出任务（不含已删除）
    #[test]
    fn 测试列出任务() {
        let store = create_test_store();
        store.create("任务一".to_string(), "描述一".to_string());
        let task2 = store.create("任务二".to_string(), "描述二".to_string());
        store.create("任务三".to_string(), "描述三".to_string());

        // 删除任务二
        store.delete(&task2.id).unwrap();

        let list = store.list();
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|t| t.id != task2.id));
    }

    /// 测试更新任务
    #[test]
    fn 测试更新任务() {
        let store = create_test_store();
        let task = store.create("原始主题".to_string(), "原始描述".to_string());

        let updated = store
            .update(
                &task.id,
                TaskUpdate {
                    subject: Some("新主题".to_string()),
                    status: Some(TaskStatus::InProgress),
                    owner: Some("张三".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.subject, "新主题");
        assert_eq!(updated.status, TaskStatus::InProgress);
        assert_eq!(updated.owner.as_deref(), Some("张三"));
        // 描述未更新，应保持不变
        assert_eq!(updated.description, "原始描述");
    }

    /// 测试删除任务
    #[test]
    fn 测试删除任务() {
        let store = create_test_store();
        let task = store.create("待删除".to_string(), "描述".to_string());
        store.delete(&task.id).unwrap();

        let fetched = store.get(&task.id).unwrap();
        assert_eq!(fetched.status, TaskStatus::Deleted);
    }

    /// 测试删除不存在的任务
    #[test]
    fn 测试删除不存在的任务() {
        let store = create_test_store();
        let result = store.delete("T-bad-id");
        assert!(result.is_err());
    }

    /// 测试获取就绪任务（无阻塞者）
    #[test]
    fn 测试获取就绪任务无阻塞() {
        let store = create_test_store();
        store.create("就绪任务一".to_string(), "描述".to_string());
        store.create("就绪任务二".to_string(), "描述".to_string());

        let ready = store.get_ready_tasks();
        assert_eq!(ready.len(), 2);
    }

    /// 测试获取就绪任务（有阻塞依赖）
    #[test]
    fn 测试获取就绪任务有阻塞() {
        let store = create_test_store();
        let blocker = store.create("前置任务".to_string(), "先完成这个".to_string());
        let blocked = store.create("后续任务".to_string(), "需要等前置任务完成".to_string());

        // 手动设置依赖关系
        store
            .update(
                &blocked.id,
                TaskUpdate {
                    add_blocked_by: Some(vec![blocker.id.clone()]),
                    ..Default::default()
                },
            )
            .unwrap();

        // 前置任务未完成，后续任务不应出现在就绪列表中
        let ready = store.get_ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, blocker.id);

        // 完成前置任务
        store
            .update(
                &blocker.id,
                TaskUpdate {
                    status: Some(TaskStatus::Completed),
                    ..Default::default()
                },
            )
            .unwrap();

        // 现在后续任务应该就绪
        let ready = store.get_ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, blocked.id);
    }

    /// 测试任务创建工具通过 Tool trait 执行
    #[tokio::test]
    async fn 测试任务创建工具() {
        let store = create_test_store();
        let tool = TaskCreateTool::new(store);

        let result = tool
            .execute(json!({
                "subject": "工具测试任务",
                "description": "通过 Tool trait 创建"
            }))
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("工具测试任务"));
        assert!(output.contains("T-"));
    }

    /// 测试任务状态的序列化和反序列化
    #[test]
    fn 测试任务状态序列化() {
        let status = TaskStatus::InProgress;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"in_progress\"");

        let parsed: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, TaskStatus::InProgress);
    }

    /// 测试任务项的完整序列化和反序列化
    #[test]
    fn 测试任务项序列化() {
        let task = TaskItem {
            id: "T-test-123".to_string(),
            subject: "序列化测试".to_string(),
            description: "测试描述".to_string(),
            status: TaskStatus::Pending,
            active_form: None,
            blocks: vec!["T-other".to_string()],
            blocked_by: Vec::new(),
            owner: Some("测试员".to_string()),
            thread_id: "thread-1".to_string(),
        };

        let json = serde_json::to_string(&task).unwrap();
        let parsed: TaskItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "T-test-123");
        assert_eq!(parsed.blocks.len(), 1);
        assert_eq!(parsed.owner.as_deref(), Some("测试员"));
    }
}

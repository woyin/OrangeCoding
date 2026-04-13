//! # 会话管理工具模块
//!
//! 提供会话列表、读取、搜索、信息查询等工具。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================
// 会话数据模型
// ============================================================

/// 会话消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    /// 消息角色（user / assistant / system）
    pub role: String,
    /// 消息内容
    pub content: String,
    /// 时间戳（ISO 8601 格式）
    pub timestamp: String,
}

/// 会话元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// 会话唯一标识
    pub session_id: String,
    /// 会话标题
    pub title: String,
    /// 创建时间
    pub created_at: String,
    /// 最后更新时间
    pub updated_at: String,
    /// 消息总数
    pub message_count: usize,
}

/// 会话存储（内存中的模拟存储）
#[derive(Debug, Clone)]
pub struct SessionStore {
    /// 会话元数据映射
    sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
    /// 会话消息映射
    messages: Arc<RwLock<HashMap<String, Vec<SessionMessage>>>>,
}

impl SessionStore {
    /// 创建空的会话存储
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            messages: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 添加会话
    pub fn add_session(&self, info: SessionInfo, msgs: Vec<SessionMessage>) {
        let id = info.session_id.clone();
        self.sessions.write().insert(id.clone(), info);
        self.messages.write().insert(id, msgs);
    }

    /// 列出所有会话
    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        self.sessions.read().values().cloned().collect()
    }

    /// 读取指定会话的消息
    pub fn read_messages(&self, session_id: &str) -> Option<Vec<SessionMessage>> {
        self.messages.read().get(session_id).cloned()
    }

    /// 获取指定会话的元数据
    pub fn get_session_info(&self, session_id: &str) -> Option<SessionInfo> {
        self.sessions.read().get(session_id).cloned()
    }

    /// 全文搜索所有会话的消息内容
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();
        let messages = self.messages.read();
        let sessions = self.sessions.read();
        let mut results = Vec::new();

        for (sid, msgs) in messages.iter() {
            let title = sessions
                .get(sid)
                .map(|s| s.title.clone())
                .unwrap_or_default();

            for (idx, msg) in msgs.iter().enumerate() {
                if msg.content.to_lowercase().contains(&query_lower) {
                    results.push(SearchResult {
                        session_id: sid.clone(),
                        session_title: title.clone(),
                        message_index: idx,
                        role: msg.role.clone(),
                        snippet: extract_snippet(&msg.content, &query_lower),
                    });
                }
            }
        }

        results
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

/// 搜索结果条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// 匹配所在的会话 ID
    pub session_id: String,
    /// 会话标题
    pub session_title: String,
    /// 消息在会话中的索引
    pub message_index: usize,
    /// 消息角色
    pub role: String,
    /// 匹配上下文片段
    pub snippet: String,
}

/// 从内容中提取包含关键词的上下文片段
fn extract_snippet(content: &str, query: &str) -> String {
    let lower = content.to_lowercase();
    if let Some(pos) = lower.find(query) {
        // 向前查找 10 个字符作为上下文
        let start_chars: Vec<(usize, char)> = content[..pos].char_indices().collect();
        let start = if start_chars.len() > 10 {
            start_chars[start_chars.len() - 10].0
        } else {
            0
        };

        // 向后查找关键词结束位置 + 10 个字符
        let after_match = pos + query.len();
        let end = content[after_match..]
            .char_indices()
            .nth(10)
            .map(|(i, _)| after_match + i)
            .unwrap_or(content.len());

        format!("...{}...", &content[start..end])
    } else {
        content.chars().take(60).collect::<String>()
    }
}

// ============================================================
// 会话列表工具
// ============================================================

/// 会话列表工具 —— 列出所有可用会话
#[derive(Debug)]
pub struct SessionListTool {
    store: SessionStore,
}

impl SessionListTool {
    /// 创建会话列表工具
    pub fn new(store: SessionStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for SessionListTool {
    fn name(&self) -> &str {
        "session_list"
    }

    fn description(&self) -> &str {
        "列出所有可用的对话会话"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _params: Value) -> ToolResult<String> {
        let sessions = self.store.list_sessions();
        serde_json::to_string_pretty(&sessions)
            .map_err(|e| ToolError::ExecutionError(format!("序列化会话列表失败: {}", e)))
    }
}

// ============================================================
// 会话读取工具
// ============================================================

/// 会话读取工具 —— 读取指定会话的消息
#[derive(Debug)]
pub struct SessionReadTool {
    store: SessionStore,
}

impl SessionReadTool {
    /// 创建会话读取工具
    pub fn new(store: SessionStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for SessionReadTool {
    fn name(&self) -> &str {
        "session_read"
    }

    fn description(&self) -> &str {
        "读取指定会话的全部消息"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "会话唯一标识"
                }
            },
            "required": ["session_id"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        let session_id = params
            .get("session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少 session_id 参数".to_string()))?;

        let messages = self
            .store
            .read_messages(session_id)
            .ok_or_else(|| ToolError::NotFound(format!("会话未找到: {}", session_id)))?;

        serde_json::to_string_pretty(&messages)
            .map_err(|e| ToolError::ExecutionError(format!("序列化消息失败: {}", e)))
    }
}

// ============================================================
// 会话搜索工具
// ============================================================

/// 会话搜索工具 —— 在所有会话中进行全文搜索
#[derive(Debug)]
pub struct SessionSearchTool {
    store: SessionStore,
}

impl SessionSearchTool {
    /// 创建会话搜索工具
    pub fn new(store: SessionStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for SessionSearchTool {
    fn name(&self) -> &str {
        "session_search"
    }

    fn description(&self) -> &str {
        "在所有会话中搜索包含指定关键词的消息"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "搜索关键词"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        let query = params
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少 query 参数".to_string()))?;

        let results = self.store.search(query);

        serde_json::to_string_pretty(&results)
            .map_err(|e| ToolError::ExecutionError(format!("序列化搜索结果失败: {}", e)))
    }
}

// ============================================================
// 会话信息工具
// ============================================================

/// 会话信息工具 —— 获取指定会话的元数据
#[derive(Debug)]
pub struct SessionInfoTool {
    store: SessionStore,
}

impl SessionInfoTool {
    /// 创建会话信息工具
    pub fn new(store: SessionStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for SessionInfoTool {
    fn name(&self) -> &str {
        "session_info"
    }

    fn description(&self) -> &str {
        "获取指定会话的元数据信息（标题、创建时间、消息数等）"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "会话唯一标识"
                }
            },
            "required": ["session_id"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        let session_id = params
            .get("session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少 session_id 参数".to_string()))?;

        let info = self
            .store
            .get_session_info(session_id)
            .ok_or_else(|| ToolError::NotFound(format!("会话未找到: {}", session_id)))?;

        serde_json::to_string_pretty(&info)
            .map_err(|e| ToolError::ExecutionError(format!("序列化会话信息失败: {}", e)))
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建用于测试的示例会话存储
    fn create_test_store() -> SessionStore {
        let store = SessionStore::new();

        let info1 = SessionInfo {
            session_id: "sess-001".to_string(),
            title: "Rust 项目讨论".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T01:00:00Z".to_string(),
            message_count: 2,
        };
        let msgs1 = vec![
            SessionMessage {
                role: "user".to_string(),
                content: "如何使用 Rust 实现并发？".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
            },
            SessionMessage {
                role: "assistant".to_string(),
                content: "你可以使用 tokio 库来实现异步并发编程。".to_string(),
                timestamp: "2024-01-01T00:01:00Z".to_string(),
            },
        ];

        let info2 = SessionInfo {
            session_id: "sess-002".to_string(),
            title: "Python 数据分析".to_string(),
            created_at: "2024-01-02T00:00:00Z".to_string(),
            updated_at: "2024-01-02T01:00:00Z".to_string(),
            message_count: 1,
        };
        let msgs2 = vec![SessionMessage {
            role: "user".to_string(),
            content: "推荐几个 Python 数据分析库".to_string(),
            timestamp: "2024-01-02T00:00:00Z".to_string(),
        }];

        store.add_session(info1, msgs1);
        store.add_session(info2, msgs2);
        store
    }

    /// 测试会话列表
    #[tokio::test]
    async fn 测试会话列表() {
        let store = create_test_store();
        let tool = SessionListTool::new(store);
        let result = tool.execute(json!({})).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("sess-001"));
        assert!(output.contains("sess-002"));
    }

    /// 测试读取指定会话的消息
    #[tokio::test]
    async fn 测试读取会话消息() {
        let store = create_test_store();
        let tool = SessionReadTool::new(store);
        let result = tool.execute(json!({"session_id": "sess-001"})).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("如何使用 Rust 实现并发"));
        assert!(output.contains("tokio"));
    }

    /// 测试读取不存在的会话
    #[tokio::test]
    async fn 测试读取不存在的会话() {
        let store = create_test_store();
        let tool = SessionReadTool::new(store);
        let result = tool.execute(json!({"session_id": "nonexistent"})).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::NotFound(msg) => assert!(msg.contains("nonexistent")),
            other => panic!("期望 NotFound 错误，实际: {:?}", other),
        }
    }

    /// 测试会话搜索——匹配关键词
    #[tokio::test]
    async fn 测试会话搜索命中() {
        let store = create_test_store();
        let tool = SessionSearchTool::new(store);
        let result = tool.execute(json!({"query": "tokio"})).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("sess-001"));
        assert!(output.contains("tokio"));
    }

    /// 测试会话搜索——无匹配
    #[tokio::test]
    async fn 测试会话搜索无结果() {
        let store = create_test_store();
        let tool = SessionSearchTool::new(store);
        let result = tool.execute(json!({"query": "量子计算"})).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        // 应返回空数组
        assert!(output.contains("[]"));
    }

    /// 测试获取会话信息
    #[tokio::test]
    async fn 测试获取会话信息() {
        let store = create_test_store();
        let tool = SessionInfoTool::new(store);
        let result = tool.execute(json!({"session_id": "sess-001"})).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Rust 项目讨论"));
        assert!(output.contains("sess-001"));
    }

    /// 测试获取不存在的会话信息
    #[tokio::test]
    async fn 测试获取不存在的会话信息() {
        let store = create_test_store();
        let tool = SessionInfoTool::new(store);
        let result = tool.execute(json!({"session_id": "bad-id"})).await;
        assert!(result.is_err());
    }

    /// 测试缺少必要参数的错误处理
    #[tokio::test]
    async fn 测试缺少参数错误() {
        let store = create_test_store();
        let tool = SessionReadTool::new(store);
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::InvalidParams(msg) => assert!(msg.contains("session_id")),
            other => panic!("期望 InvalidParams 错误，实际: {:?}", other),
        }
    }
}

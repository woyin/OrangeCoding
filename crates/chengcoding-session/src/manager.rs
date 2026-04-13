//! 会话管理器模块
//!
//! 本模块实现了会话的生命周期管理，包括创建、恢复、列表、分支和清理。
//! `SessionManager` 管理所有会话文件的发现和操作。
//! `Session` 封装了活跃会话的读写操作。

use std::path::{Path, PathBuf};

use chengcoding_core::message::{Message, Role};
use chengcoding_core::TokenUsage;
use chrono::{DateTime, Utc};

use crate::entry::{CompactionEntry, EntryId, MessageEntry, SessionEntry};
use crate::error::SessionError;
use crate::storage::{encode_cwd, SessionHeader, SessionStorage};
use crate::tree::SessionTree;

// ---------------------------------------------------------------------------
// 会话信息
// ---------------------------------------------------------------------------

/// 会话信息 - 用于列表展示
#[derive(Clone, Debug)]
pub struct SessionInfo {
    /// 会话 ID
    pub id: String,
    /// 会话标题
    pub title: Option<String>,
    /// 关联的工作目录
    pub cwd: PathBuf,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后更新时间
    pub updated_at: DateTime<Utc>,
    /// 条目数量
    pub entry_count: usize,
}

// ---------------------------------------------------------------------------
// 会话管理器
// ---------------------------------------------------------------------------

/// 会话管理器 - 管理所有会话文件的发现、创建和恢复
pub struct SessionManager {
    /// 会话存储的基础目录
    base_dir: PathBuf,
    /// 是否为内存模式（不保存到磁盘）
    in_memory: bool,
}

impl SessionManager {
    /// 创建会话管理器
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            in_memory: false,
        }
    }

    /// 创建内存模式的会话管理器（不保存到磁盘）
    pub fn in_memory() -> Self {
        Self {
            base_dir: PathBuf::from("/dev/null"),
            in_memory: true,
        }
    }

    /// 创建新会话
    pub async fn create_session(&self, cwd: &Path) -> Result<Session, SessionError> {
        if self.in_memory {
            return Ok(Session::in_memory(cwd));
        }

        let storage = SessionStorage::create(&self.base_dir, cwd).await?;
        let tree = SessionTree::new();
        Ok(Session {
            storage: Some(storage),
            tree,
            cwd: cwd.to_path_buf(),
        })
    }

    /// 恢复最近的会话（按工作目录筛选）
    ///
    /// 扫描指定工作目录对应的会话子目录，找到最新的会话文件。
    pub async fn resume_latest(&self, cwd: &Path) -> Result<Option<Session>, SessionError> {
        if self.in_memory {
            return Ok(None);
        }

        let cwd_dir = self.base_dir.join(encode_cwd(cwd));
        if !cwd_dir.exists() {
            return Ok(None);
        }

        let mut latest: Option<(PathBuf, DateTime<Utc>)> = None;
        let mut dir = tokio::fs::read_dir(&cwd_dir)
            .await
            .map_err(|e| SessionError::Io(format!("无法读取目录 {:?}: {}", cwd_dir, e)))?;

        while let Some(entry) = dir
            .next_entry()
            .await
            .map_err(|e| SessionError::Io(format!("读取目录条目失败: {}", e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            // 尝试打开并读取时间戳
            if let Ok(storage) = SessionStorage::open(&path).await {
                let ts = storage.header().timestamp;
                if latest.as_ref().map_or(true, |(_, t)| ts > *t) {
                    latest = Some((path, ts));
                }
            }
        }

        match latest {
            Some((path, _)) => {
                let storage = SessionStorage::open(&path).await?;
                let entries = storage.read_entries().await?;
                let tree = SessionTree::from_entries(entries);
                Ok(Some(Session {
                    cwd: storage.header().cwd.clone(),
                    storage: Some(storage),
                    tree,
                }))
            }
            None => Ok(None),
        }
    }

    /// 按 ID 前缀恢复会话
    ///
    /// 在所有会话目录中搜索 ID 以指定前缀开头的会话。
    pub async fn resume_by_id(&self, id_prefix: &str) -> Result<Option<Session>, SessionError> {
        if self.in_memory {
            return Ok(None);
        }

        if !self.base_dir.exists() {
            return Ok(None);
        }

        // 遍历所有子目录
        let mut base_dir = tokio::fs::read_dir(&self.base_dir)
            .await
            .map_err(|e| SessionError::Io(format!("无法读取基础目录: {}", e)))?;

        while let Some(cwd_entry) = base_dir
            .next_entry()
            .await
            .map_err(|e| SessionError::Io(format!("读取目录条目失败: {}", e)))?
        {
            let cwd_path = cwd_entry.path();
            if !cwd_path.is_dir() {
                continue;
            }

            let mut dir = match tokio::fs::read_dir(&cwd_path).await {
                Ok(d) => d,
                Err(_) => continue,
            };

            while let Some(file_entry) = dir
                .next_entry()
                .await
                .map_err(|e| SessionError::Io(format!("读取文件条目失败: {}", e)))?
            {
                let path = file_entry.path();
                let file_stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();

                if file_stem.starts_with(id_prefix) {
                    let storage = SessionStorage::open(&path).await?;
                    let entries = storage.read_entries().await?;
                    let tree = SessionTree::from_entries(entries);
                    return Ok(Some(Session {
                        cwd: storage.header().cwd.clone(),
                        storage: Some(storage),
                        tree,
                    }));
                }
            }
        }

        Ok(None)
    }

    /// 列出指定工作目录下的所有会话
    pub async fn list_sessions(&self, cwd: &Path) -> Result<Vec<SessionInfo>, SessionError> {
        if self.in_memory {
            return Ok(Vec::new());
        }

        let cwd_dir = self.base_dir.join(encode_cwd(cwd));
        if !cwd_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        let mut dir = tokio::fs::read_dir(&cwd_dir)
            .await
            .map_err(|e| SessionError::Io(format!("无法读取目录: {}", e)))?;

        while let Some(entry) = dir
            .next_entry()
            .await
            .map_err(|e| SessionError::Io(format!("读取目录条目失败: {}", e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }

            if let Ok(storage) = SessionStorage::open(&path).await {
                let header = storage.header();
                let entries = storage.read_entries().await.unwrap_or_default();

                // 获取最后条目的时间作为更新时间
                let updated_at = entries
                    .last()
                    .map(|e| e.timestamp)
                    .unwrap_or(header.timestamp);

                sessions.push(SessionInfo {
                    id: header.id.clone(),
                    title: header.title.clone(),
                    cwd: header.cwd.clone(),
                    created_at: header.timestamp,
                    updated_at,
                    entry_count: entries.len(),
                });
            }
        }

        // 按创建时间降序排列
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(sessions)
    }

    /// 删除旧会话
    ///
    /// 删除超过指定天数的会话文件，返回删除的数量。
    pub async fn cleanup_old_sessions(&self, max_age_days: u32) -> Result<u32, SessionError> {
        if self.in_memory || !self.base_dir.exists() {
            return Ok(0);
        }

        let cutoff = Utc::now() - chrono::Duration::days(max_age_days as i64);
        let mut deleted = 0u32;

        let mut base_dir = tokio::fs::read_dir(&self.base_dir)
            .await
            .map_err(|e| SessionError::Io(format!("无法读取基础目录: {}", e)))?;

        while let Some(cwd_entry) = base_dir
            .next_entry()
            .await
            .map_err(|e| SessionError::Io(format!("读取目录条目失败: {}", e)))?
        {
            let cwd_path = cwd_entry.path();
            if !cwd_path.is_dir() {
                continue;
            }

            let mut dir = match tokio::fs::read_dir(&cwd_path).await {
                Ok(d) => d,
                Err(_) => continue,
            };

            while let Some(file_entry) = dir
                .next_entry()
                .await
                .map_err(|e| SessionError::Io(format!("读取文件条目失败: {}", e)))?
            {
                let path = file_entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }

                if let Ok(storage) = SessionStorage::open(&path).await {
                    if storage.header().timestamp < cutoff {
                        if tokio::fs::remove_file(&path).await.is_ok() {
                            deleted += 1;
                        }
                    }
                }
            }
        }

        Ok(deleted)
    }

    /// 获取基础目录路径
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

// ---------------------------------------------------------------------------
// 活跃会话
// ---------------------------------------------------------------------------

/// 活跃会话 - 封装会话的读写操作
pub struct Session {
    /// 存储后端（内存模式为 None）
    storage: Option<SessionStorage>,
    /// 会话树
    tree: SessionTree,
    /// 工作目录
    cwd: PathBuf,
}

impl Session {
    /// 创建内存模式的会话（不保存到磁盘）
    fn in_memory(cwd: &Path) -> Self {
        Self {
            storage: None,
            tree: SessionTree::new(),
            cwd: cwd.to_path_buf(),
        }
    }

    /// 添加用户消息
    pub async fn add_user_message(&mut self, content: &str) -> Result<EntryId, SessionError> {
        let parent_id = self.tree.current_leaf().map(|e| e.id.clone());
        let entry = SessionEntry::message(
            parent_id,
            MessageEntry {
                role: Role::User,
                content: content.to_string(),
                tool_calls: vec![],
                tool_call_id: None,
                model: None,
                token_usage: None,
            },
        );
        let id = entry.id.clone();

        if let Some(ref storage) = self.storage {
            storage.append_entry(&entry).await?;
        }
        self.tree.add_entry(entry);

        Ok(id)
    }

    /// 添加助手消息
    pub async fn add_assistant_message(
        &mut self,
        content: &str,
        model: &str,
        usage: Option<TokenUsage>,
    ) -> Result<EntryId, SessionError> {
        let parent_id = self.tree.current_leaf().map(|e| e.id.clone());
        let entry = SessionEntry::message(
            parent_id,
            MessageEntry {
                role: Role::Assistant,
                content: content.to_string(),
                tool_calls: vec![],
                tool_call_id: None,
                model: Some(model.to_string()),
                token_usage: usage,
            },
        );
        let id = entry.id.clone();

        if let Some(ref storage) = self.storage {
            storage.append_entry(&entry).await?;
        }
        self.tree.add_entry(entry);

        Ok(id)
    }

    /// 添加工具调用结果
    pub async fn add_tool_result(
        &mut self,
        tool_call_id: &str,
        content: &str,
        is_error: bool,
    ) -> Result<EntryId, SessionError> {
        let parent_id = self.tree.current_leaf().map(|e| e.id.clone());
        let role_content = if is_error {
            format!("[错误] {}", content)
        } else {
            content.to_string()
        };
        let entry = SessionEntry::message(
            parent_id,
            MessageEntry {
                role: Role::Tool,
                content: role_content,
                tool_calls: vec![],
                tool_call_id: Some(tool_call_id.to_string()),
                model: None,
                token_usage: None,
            },
        );
        let id = entry.id.clone();

        if let Some(ref storage) = self.storage {
            storage.append_entry(&entry).await?;
        }
        self.tree.add_entry(entry);

        Ok(id)
    }

    /// 获取上下文消息（用于发送给 AI）
    ///
    /// 从活跃分支提取消息条目，转换为 ceair-core 的 Message 类型。
    pub fn context_messages(&self) -> Vec<Message> {
        self.tree
            .context_messages()
            .into_iter()
            .map(|m| {
                let mut msg = match m.role {
                    Role::User => Message::user(&m.content),
                    Role::Assistant => Message::assistant(&m.content),
                    Role::System => Message::system(&m.content),
                    Role::Tool => {
                        Message::tool(m.tool_call_id.as_deref().unwrap_or(""), &m.content)
                    }
                };
                // 复制工具调用信息
                msg.tool_calls = m
                    .tool_calls
                    .iter()
                    .map(|tc| {
                        chengcoding_core::message::ToolCall::new(
                            &tc.id,
                            &tc.name,
                            tc.arguments.clone(),
                        )
                    })
                    .collect();
                msg
            })
            .collect()
    }

    /// 获取会话树的引用
    pub fn tree(&self) -> &SessionTree {
        &self.tree
    }

    /// 从指定条目分支出新会话
    pub async fn branch(
        &self,
        from_entry: &EntryId,
        manager: &SessionManager,
    ) -> Result<Session, SessionError> {
        if manager.in_memory {
            return Ok(Session::in_memory(&self.cwd));
        }

        // 创建带父会话引用的新存储
        let parent_id = self
            .storage
            .as_ref()
            .map(|s| s.header().id.clone())
            .unwrap_or_default();

        let header = SessionHeader {
            version: 3,
            id: crate::storage::generate_session_id(),
            timestamp: Utc::now(),
            cwd: self.cwd.clone(),
            title: None,
            parent_session: Some(parent_id),
        };

        let storage = SessionStorage::create_with_header(manager.base_dir(), header).await?;

        // 复制从根到 from_entry 的所有条目到新会话
        let mut tree = SessionTree::new();

        // 获取从根到指定条目的路径
        // 先临时导航到 from_entry 获取路径
        let mut temp_tree = SessionTree::from_entries(Vec::new());
        for entry in self.tree.active_branch() {
            temp_tree.add_entry(entry.clone());
        }

        // 简化处理：如果 from_entry 存在于当前树中，复制活跃分支到该点
        if let Some(_) = self.tree.get_entry(from_entry) {
            // 从原树获取到 from_entry 的路径
            let mut nav_tree = self.tree_clone();
            nav_tree.navigate_to(from_entry);
            for entry in nav_tree.active_branch() {
                let cloned = entry.clone();
                storage.append_entry(&cloned).await?;
                tree.add_entry(cloned);
            }
        }

        Ok(Session {
            cwd: self.cwd.clone(),
            storage: Some(storage),
            tree,
        })
    }

    /// 克隆树结构（内部辅助方法）
    fn tree_clone(&self) -> SessionTree {
        let entries: Vec<SessionEntry> = self
            .tree
            .active_branch()
            .iter()
            .map(|e| (*e).clone())
            .collect();
        // 重建整棵树需要所有条目
        let mut tree = SessionTree::new();
        for e in &entries {
            tree.add_entry(e.clone());
        }
        tree
    }

    /// 添加压缩条目
    pub async fn compact(
        &mut self,
        summary: &str,
        first_kept: &EntryId,
        tokens_before: u64,
    ) -> Result<EntryId, SessionError> {
        let parent_id = self.tree.current_leaf().map(|e| e.id.clone());
        let entry = SessionEntry::compaction(
            parent_id,
            CompactionEntry {
                summary: summary.to_string(),
                short_summary: None,
                first_kept_entry_id: first_kept.clone(),
                tokens_before,
            },
        );
        let id = entry.id.clone();

        if let Some(ref storage) = self.storage {
            storage.append_entry(&entry).await?;
        }
        self.tree.add_entry(entry);

        Ok(id)
    }

    /// 获取会话信息
    pub fn info(&self) -> SessionInfo {
        let (id, title, created_at) = match &self.storage {
            Some(s) => {
                let h = s.header();
                (h.id.clone(), h.title.clone(), h.timestamp)
            }
            None => ("in-memory".to_string(), None, Utc::now()),
        };

        let updated_at = self
            .tree
            .current_leaf()
            .map(|e| e.timestamp)
            .unwrap_or(created_at);

        SessionInfo {
            id,
            title,
            cwd: self.cwd.clone(),
            created_at,
            updated_at,
            entry_count: self.tree.len(),
        }
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn 测试创建会话并添加消息() {
        let tmp = TempDir::new().unwrap();
        let manager = SessionManager::new(tmp.path().to_path_buf());
        let cwd = PathBuf::from("/test/project");

        let mut session = manager.create_session(&cwd).await.unwrap();

        let user_id = session.add_user_message("你好").await.unwrap();
        assert!(!user_id.as_str().is_empty());

        let assistant_id = session
            .add_assistant_message("你好！", "gpt-4", Some(TokenUsage::new(10, 20)))
            .await
            .unwrap();
        assert!(!assistant_id.as_str().is_empty());

        assert_eq!(session.tree().len(), 2);
    }

    #[tokio::test]
    async fn 测试恢复最近的会话() {
        let tmp = TempDir::new().unwrap();
        let manager = SessionManager::new(tmp.path().to_path_buf());
        let cwd = PathBuf::from("/test/resume");

        // 创建会话并添加消息
        let mut session = manager.create_session(&cwd).await.unwrap();
        session.add_user_message("测试消息").await.unwrap();
        session
            .add_assistant_message("回复", "model", None)
            .await
            .unwrap();
        drop(session);

        // 恢复
        let resumed = manager.resume_latest(&cwd).await.unwrap();
        assert!(resumed.is_some());
        let resumed = resumed.unwrap();
        assert_eq!(resumed.tree().len(), 2);
    }

    #[tokio::test]
    async fn 测试按ID前缀恢复() {
        let tmp = TempDir::new().unwrap();
        let manager = SessionManager::new(tmp.path().to_path_buf());
        let cwd = PathBuf::from("/test/id_resume");

        let mut session = manager.create_session(&cwd).await.unwrap();
        let session_id = session.info().id.clone();
        session.add_user_message("测试").await.unwrap();
        drop(session);

        // 使用完整 ID 恢复
        let resumed = manager.resume_by_id(&session_id).await.unwrap();
        assert!(resumed.is_some());

        // 使用前缀恢复
        let prefix = &session_id[..6];
        let resumed = manager.resume_by_id(prefix).await.unwrap();
        assert!(resumed.is_some());
    }

    #[tokio::test]
    async fn 测试列出会话() {
        let tmp = TempDir::new().unwrap();
        let manager = SessionManager::new(tmp.path().to_path_buf());
        let cwd = PathBuf::from("/test/list");

        // 创建两个会话
        let mut s1 = manager.create_session(&cwd).await.unwrap();
        s1.add_user_message("会话1").await.unwrap();
        drop(s1);

        let mut s2 = manager.create_session(&cwd).await.unwrap();
        s2.add_user_message("会话2").await.unwrap();
        drop(s2);

        let sessions = manager.list_sessions(&cwd).await.unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[tokio::test]
    async fn 测试上下文消息提取() {
        let tmp = TempDir::new().unwrap();
        let manager = SessionManager::new(tmp.path().to_path_buf());
        let cwd = PathBuf::from("/test/context");

        let mut session = manager.create_session(&cwd).await.unwrap();
        session.add_user_message("问题").await.unwrap();
        session
            .add_assistant_message("回答", "gpt-4", None)
            .await
            .unwrap();

        let messages = session.context_messages();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[1].role, Role::Assistant);
        assert_eq!(messages[0].content.as_deref(), Some("问题"));
    }

    #[tokio::test]
    async fn 测试带压缩的上下文消息() {
        let tmp = TempDir::new().unwrap();
        let manager = SessionManager::new(tmp.path().to_path_buf());
        let cwd = PathBuf::from("/test/compact");

        let mut session = manager.create_session(&cwd).await.unwrap();
        session.add_user_message("旧消息1").await.unwrap();
        session
            .add_assistant_message("旧回复1", "model", None)
            .await
            .unwrap();

        // 添加压缩
        let kept_id = EntryId::from_string("kept_entry");
        session
            .compact("之前的对话摘要", &kept_id, 5000)
            .await
            .unwrap();

        // 压缩后添加新消息
        session.add_user_message("新消息").await.unwrap();

        let messages = session.context_messages();
        // 压缩之后只有新消息
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content.as_deref(), Some("新消息"));
    }

    #[tokio::test]
    async fn 测试工具结果添加() {
        let tmp = TempDir::new().unwrap();
        let manager = SessionManager::new(tmp.path().to_path_buf());
        let cwd = PathBuf::from("/test/tool");

        let mut session = manager.create_session(&cwd).await.unwrap();
        session.add_user_message("执行命令").await.unwrap();

        let tool_id = session
            .add_tool_result("call_001", "命令执行成功", false)
            .await
            .unwrap();
        assert!(!tool_id.as_str().is_empty());

        // 验证工具结果消息
        let messages = session.context_messages();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, Role::Tool);
    }

    #[tokio::test]
    async fn 测试工具错误结果() {
        let tmp = TempDir::new().unwrap();
        let manager = SessionManager::new(tmp.path().to_path_buf());
        let cwd = PathBuf::from("/test/tool_error");

        let mut session = manager.create_session(&cwd).await.unwrap();
        session.add_user_message("执行命令").await.unwrap();
        session
            .add_tool_result("call_002", "权限不足", true)
            .await
            .unwrap();

        let messages = session.context_messages();
        assert_eq!(messages[1].role, Role::Tool);
        assert!(messages[1].content.as_deref().unwrap().contains("错误"));
    }

    #[tokio::test]
    async fn 测试内存模式会话() {
        let manager = SessionManager::in_memory();
        let cwd = PathBuf::from("/test/memory");

        let mut session = manager.create_session(&cwd).await.unwrap();
        session.add_user_message("内存消息").await.unwrap();
        session
            .add_assistant_message("内存回复", "model", None)
            .await
            .unwrap();

        assert_eq!(session.tree().len(), 2);
        let info = session.info();
        assert_eq!(info.id, "in-memory");
    }

    #[tokio::test]
    async fn 测试会话信息生成() {
        let tmp = TempDir::new().unwrap();
        let manager = SessionManager::new(tmp.path().to_path_buf());
        let cwd = PathBuf::from("/test/info");

        let mut session = manager.create_session(&cwd).await.unwrap();
        session.add_user_message("测试").await.unwrap();

        let info = session.info();
        assert!(!info.id.is_empty());
        assert_eq!(info.cwd, cwd);
        assert_eq!(info.entry_count, 1);
    }

    #[tokio::test]
    async fn 测试恢复不存在的工作目录() {
        let tmp = TempDir::new().unwrap();
        let manager = SessionManager::new(tmp.path().to_path_buf());
        let cwd = PathBuf::from("/nonexistent/path");

        let result = manager.resume_latest(&cwd).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn 测试按不存在的ID前缀恢复() {
        let tmp = TempDir::new().unwrap();
        let manager = SessionManager::new(tmp.path().to_path_buf());

        let result = manager.resume_by_id("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn 测试空工作目录的会话列表() {
        let tmp = TempDir::new().unwrap();
        let manager = SessionManager::new(tmp.path().to_path_buf());
        let cwd = PathBuf::from("/empty/dir");

        let sessions = manager.list_sessions(&cwd).await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn 测试分支会话() {
        let tmp = TempDir::new().unwrap();
        let manager = SessionManager::new(tmp.path().to_path_buf());
        let cwd = PathBuf::from("/test/branch");

        let mut session = manager.create_session(&cwd).await.unwrap();
        let id1 = session.add_user_message("消息1").await.unwrap();
        session
            .add_assistant_message("回复1", "model", None)
            .await
            .unwrap();

        // 从第一个条目分支
        let branched = session.branch(&id1, &manager).await.unwrap();
        assert!(branched.tree().len() > 0);
    }

    #[tokio::test]
    async fn 测试清理旧会话() {
        let tmp = TempDir::new().unwrap();
        let manager = SessionManager::new(tmp.path().to_path_buf());
        let cwd = PathBuf::from("/test/cleanup");

        // 创建一个会话
        let mut session = manager.create_session(&cwd).await.unwrap();
        session.add_user_message("测试").await.unwrap();
        drop(session);

        // 清理 0 天以上的会话不应删除刚创建的
        let deleted = manager.cleanup_old_sessions(1).await.unwrap();
        assert_eq!(deleted, 0);
    }
}

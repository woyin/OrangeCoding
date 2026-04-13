//! JSONL 会话存储模块
//!
//! 本模块实现了基于 JSONL（每行一个 JSON 对象）格式的会话文件存储。
//! 文件格式：
//! - 第 1 行：JSON 头部信息（版本、ID、时间戳、工作目录等）
//! - 后续行：每行一个会话条目（追加写入，永不重写整个文件）

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tracing::warn;

use crate::entry::SessionEntry;
use crate::error::SessionError;

/// 当前文件格式版本号
const FORMAT_VERSION: u32 = 3;

// ---------------------------------------------------------------------------
// 会话头部
// ---------------------------------------------------------------------------

/// 会话头部信息 - 存储在 JSONL 文件的第一行
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionHeader {
    /// 文件格式版本号
    pub version: u32,
    /// 会话唯一标识（十六进制时间戳）
    pub id: String,
    /// 会话创建时间
    pub timestamp: DateTime<Utc>,
    /// 会话关联的工作目录
    pub cwd: PathBuf,
    /// 会话标题（可后续设置）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// 父会话 ID（分支时使用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_session: Option<String>,
}

// ---------------------------------------------------------------------------
// 会话存储
// ---------------------------------------------------------------------------

/// JSONL 会话存储
///
/// 负责会话文件的创建、打开、读写操作。
/// 使用追加写入模式，保证数据不会丢失。
pub struct SessionStorage {
    /// 文件路径
    path: PathBuf,
    /// 会话头部信息（可变：标题可更新）
    header: RwLock<SessionHeader>,
}

/// 生成十六进制时间戳 ID
pub fn generate_session_id() -> String {
    let ts = Utc::now().timestamp_millis() as u64;
    let rng = ring::rand::SystemRandom::new();
    let mut buf = [0u8; 4];
    ring::rand::SecureRandom::fill(&rng, &mut buf).expect("系统随机数生成失败");
    let rand = u32::from_le_bytes(buf);
    format!("{:012x}{:08x}", ts, rand)
}

/// 将工作目录编码为安全的目录名（替换 / 为 --）
pub fn encode_cwd(cwd: &Path) -> String {
    let s = cwd.to_string_lossy();
    let trimmed = s.trim_start_matches('/');
    if trimmed.is_empty() {
        "root".to_string()
    } else {
        trimmed.replace('/', "--")
    }
}

impl SessionStorage {
    /// 创建新会话文件
    ///
    /// 在指定目录下创建以工作目录编码命名的子目录，
    /// 然后在其中创建新的 JSONL 会话文件。
    pub async fn create(dir: &Path, cwd: &Path) -> Result<Self, SessionError> {
        let session_id = generate_session_id();
        let header = SessionHeader {
            version: FORMAT_VERSION,
            id: session_id.clone(),
            timestamp: Utc::now(),
            cwd: cwd.to_path_buf(),
            title: None,
            parent_session: None,
        };

        // 创建会话目录
        let cwd_dir = dir.join(encode_cwd(cwd));
        tokio::fs::create_dir_all(&cwd_dir)
            .await
            .map_err(|e| SessionError::Io(format!("无法创建会话目录 {:?}: {}", cwd_dir, e)))?;

        // 创建会话文件
        let file_path = cwd_dir.join(format!("{}.jsonl", session_id));
        let header_json = serde_json::to_string(&header)
            .map_err(|e| SessionError::Serialization(format!("序列化头部失败: {}", e)))?;

        tokio::fs::write(&file_path, format!("{}\n", header_json))
            .await
            .map_err(|e| SessionError::Io(format!("写入头部失败: {}", e)))?;

        Ok(Self {
            path: file_path,
            header: RwLock::new(header),
        })
    }

    /// 使用自定义头部创建新会话文件（用于分支等场景）
    pub async fn create_with_header(
        dir: &Path,
        header: SessionHeader,
    ) -> Result<Self, SessionError> {
        let cwd_dir = dir.join(encode_cwd(&header.cwd));
        tokio::fs::create_dir_all(&cwd_dir)
            .await
            .map_err(|e| SessionError::Io(format!("无法创建会话目录 {:?}: {}", cwd_dir, e)))?;

        let file_path = cwd_dir.join(format!("{}.jsonl", header.id));
        let header_json = serde_json::to_string(&header)
            .map_err(|e| SessionError::Serialization(format!("序列化头部失败: {}", e)))?;

        tokio::fs::write(&file_path, format!("{}\n", header_json))
            .await
            .map_err(|e| SessionError::Io(format!("写入头部失败: {}", e)))?;

        Ok(Self {
            path: file_path,
            header: RwLock::new(header),
        })
    }

    /// 打开已有会话文件
    ///
    /// 读取文件第一行解析头部，验证格式版本。
    pub async fn open(path: &Path) -> Result<Self, SessionError> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| SessionError::Io(format!("无法读取会话文件 {:?}: {}", path, e)))?;

        let first_line = content
            .lines()
            .next()
            .ok_or_else(|| SessionError::Format("会话文件为空".to_string()))?;

        let header: SessionHeader = serde_json::from_str(first_line)
            .map_err(|e| SessionError::Format(format!("无法解析会话头部: {}", e)))?;

        Ok(Self {
            path: path.to_path_buf(),
            header: RwLock::new(header),
        })
    }

    /// 追加条目到文件末尾
    pub async fn append_entry(&self, entry: &SessionEntry) -> Result<(), SessionError> {
        let json = serde_json::to_string(entry)
            .map_err(|e| SessionError::Serialization(format!("序列化条目失败: {}", e)))?;

        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(&self.path)
            .await
            .map_err(|e| SessionError::Io(format!("无法打开文件追加: {}", e)))?;

        file.write_all(format!("{}\n", json).as_bytes())
            .await
            .map_err(|e| SessionError::Io(format!("写入条目失败: {}", e)))?;

        file.flush()
            .await
            .map_err(|e| SessionError::Io(format!("刷新文件失败: {}", e)))?;

        Ok(())
    }

    /// 读取所有条目（跳过头部行和损坏行）
    pub async fn read_entries(&self) -> Result<Vec<SessionEntry>, SessionError> {
        let content = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(|e| SessionError::Io(format!("无法读取会话文件: {}", e)))?;

        let mut entries = Vec::new();
        for (i, line) in content.lines().enumerate() {
            // 跳过第一行（头部）
            if i == 0 {
                continue;
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<SessionEntry>(trimmed) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    // 跳过损坏的行，记录警告
                    warn!("跳过损坏的会话条目（第 {} 行）: {}", i + 1, e);
                }
            }
        }
        Ok(entries)
    }

    /// 读取会话头部
    pub fn header(&self) -> SessionHeader {
        self.header.read().clone()
    }

    /// 获取会话文件路径
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 设置会话标题
    ///
    /// 更新内存中的头部并重写文件第一行。
    pub async fn set_title(&self, title: &str) -> Result<(), SessionError> {
        // 更新内存中的头部
        {
            let mut h = self.header.write();
            h.title = Some(title.to_string());
        }

        // 读取当前文件内容，替换第一行
        let content = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(|e| SessionError::Io(format!("读取文件失败: {}", e)))?;

        let header = self.header.read().clone();
        let header_json = serde_json::to_string(&header)
            .map_err(|e| SessionError::Serialization(format!("序列化头部失败: {}", e)))?;

        let mut lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return Err(SessionError::Format("会话文件为空".to_string()));
        }

        // 替换第一行
        let new_content = std::iter::once(header_json.as_str())
            .chain(lines.drain(1..))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";

        tokio::fs::write(&self.path, new_content)
            .await
            .map_err(|e| SessionError::Io(format!("写入文件失败: {}", e)))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::entry::{EntryId, MessageEntry};
    use chengcoding_core::message::Role;
    use tempfile::TempDir;

    /// 创建测试用消息条目
    fn make_user_msg(content: &str) -> SessionEntry {
        SessionEntry::message(
            None,
            MessageEntry {
                role: Role::User,
                content: content.to_string(),
                tool_calls: vec![],
                tool_call_id: None,
                model: None,
                token_usage: None,
            },
        )
    }

    /// 创建带父 ID 的测试消息条目
    fn make_reply(parent_id: &EntryId, content: &str) -> SessionEntry {
        SessionEntry::message(
            Some(parent_id.clone()),
            MessageEntry {
                role: Role::Assistant,
                content: content.to_string(),
                tool_calls: vec![],
                tool_call_id: None,
                model: Some("test-model".to_string()),
                token_usage: None,
            },
        )
    }

    #[tokio::test]
    async fn 测试创建新会话文件并验证头部() {
        let tmp = TempDir::new().unwrap();
        let cwd = PathBuf::from("/home/user/project");
        let storage = SessionStorage::create(tmp.path(), &cwd).await.unwrap();

        let header = storage.header();
        assert_eq!(header.version, FORMAT_VERSION);
        assert_eq!(header.cwd, cwd);
        assert!(header.title.is_none());
        assert!(header.parent_session.is_none());
        // 会话文件路径必须存在
        assert!(storage.path().exists());
    }

    #[tokio::test]
    async fn 测试追加条目并读取() {
        let tmp = TempDir::new().unwrap();
        let cwd = PathBuf::from("/test/cwd");
        let storage = SessionStorage::create(tmp.path(), &cwd).await.unwrap();

        // 追加两个条目
        let entry1 = make_user_msg("你好");
        let entry2 = make_reply(&entry1.id, "你好！有什么可以帮你的？");
        storage.append_entry(&entry1).await.unwrap();
        storage.append_entry(&entry2).await.unwrap();

        // 读回
        let entries = storage.read_entries().await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].as_message().unwrap().content, "你好");
        assert_eq!(
            entries[1].as_message().unwrap().content,
            "你好！有什么可以帮你的？"
        );
    }

    #[tokio::test]
    async fn 测试打开已有会话文件() {
        let tmp = TempDir::new().unwrap();
        let cwd = PathBuf::from("/test/open");
        let storage = SessionStorage::create(tmp.path(), &cwd).await.unwrap();
        let session_path = storage.path().to_path_buf();
        let session_id = storage.header().id.clone();

        // 追加一个条目
        storage.append_entry(&make_user_msg("测试")).await.unwrap();

        // 重新打开
        let reopened = SessionStorage::open(&session_path).await.unwrap();
        assert_eq!(reopened.header().id, session_id);
        let entries = reopened.read_entries().await.unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[tokio::test]
    async fn 测试处理损坏的行() {
        let tmp = TempDir::new().unwrap();
        let cwd = PathBuf::from("/test/corrupt");
        let storage = SessionStorage::create(tmp.path(), &cwd).await.unwrap();

        // 追加一个正常条目
        storage.append_entry(&make_user_msg("正常")).await.unwrap();

        // 手动写入一行损坏数据
        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(storage.path())
            .await
            .unwrap();
        file.write_all("not valid json\n".as_bytes()).await.unwrap();
        file.flush().await.unwrap();

        // 再追加一个正常条目
        storage
            .append_entry(&make_user_msg("也正常"))
            .await
            .unwrap();

        // 读取时应跳过损坏行，只返回两个正常条目
        let entries = storage.read_entries().await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn 测试设置和获取标题() {
        let tmp = TempDir::new().unwrap();
        let cwd = PathBuf::from("/test/title");
        let storage = SessionStorage::create(tmp.path(), &cwd).await.unwrap();

        assert!(storage.header().title.is_none());

        storage.set_title("我的会话").await.unwrap();
        assert_eq!(storage.header().title.as_deref(), Some("我的会话"));

        // 重新打开文件验证标题已持久化
        let reopened = SessionStorage::open(storage.path()).await.unwrap();
        assert_eq!(reopened.header().title.as_deref(), Some("我的会话"));
    }

    #[tokio::test]
    async fn 测试空会话只有头部() {
        let tmp = TempDir::new().unwrap();
        let cwd = PathBuf::from("/test/empty");
        let storage = SessionStorage::create(tmp.path(), &cwd).await.unwrap();

        let entries = storage.read_entries().await.unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn 测试设置标题后条目不丢失() {
        let tmp = TempDir::new().unwrap();
        let cwd = PathBuf::from("/test/title_entries");
        let storage = SessionStorage::create(tmp.path(), &cwd).await.unwrap();

        storage.append_entry(&make_user_msg("条目1")).await.unwrap();
        storage.append_entry(&make_user_msg("条目2")).await.unwrap();
        storage.set_title("标题").await.unwrap();

        let entries = storage.read_entries().await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].as_message().unwrap().content, "条目1");
        assert_eq!(entries[1].as_message().unwrap().content, "条目2");
    }

    #[test]
    fn 测试工作目录编码() {
        assert_eq!(
            encode_cwd(Path::new("/home/user/project")),
            "home--user--project"
        );
        assert_eq!(encode_cwd(Path::new("/tmp")), "tmp");
        assert_eq!(encode_cwd(Path::new("/")), "root");
    }
}

//! Blob 存储模块
//!
//! 本模块实现了基于内容寻址的 Blob 存储系统。
//! 使用 SHA-256 哈希作为文件标识，相同内容自动去重。
//! 主要用于外部化大型工具输出，避免会话文件过大。

use std::collections::HashSet;
use std::path::PathBuf;

use ring::digest;
use tracing::debug;

use crate::error::SessionError;

// ---------------------------------------------------------------------------
// Blob 存储
// ---------------------------------------------------------------------------

/// Blob 存储 - 基于 SHA-256 内容寻址的文件存储
///
/// 每个 Blob 以其 SHA-256 哈希值命名存储在指定目录下。
/// 相同内容只存储一份（去重）。
pub struct BlobStore {
    /// Blob 存储目录
    blob_dir: PathBuf,
}

impl BlobStore {
    /// 创建 BlobStore 实例
    pub fn new(blob_dir: PathBuf) -> Self {
        Self { blob_dir }
    }

    /// 计算数据的 SHA-256 哈希（十六进制字符串）
    fn sha256_hex(data: &[u8]) -> String {
        let digest = digest::digest(&digest::SHA256, data);
        hex_encode(digest.as_ref())
    }

    /// 获取 Blob 的文件路径
    fn blob_path(&self, sha256: &str) -> PathBuf {
        // 使用前两个字符作为子目录，减少单目录文件数
        let (prefix, _rest) = if sha256.len() >= 2 {
            sha256.split_at(2)
        } else {
            (sha256, "")
        };
        self.blob_dir.join(prefix).join(sha256)
    }

    /// 存储数据并返回 SHA-256 标识
    ///
    /// 如果相同内容已存在，直接返回哈希值，不重复写入。
    pub async fn store(&self, data: &[u8]) -> Result<String, SessionError> {
        let sha256 = Self::sha256_hex(data);
        let path = self.blob_path(&sha256);

        // 内容寻址去重：如果已存在则跳过
        if path.exists() {
            debug!("Blob 已存在: {}", sha256);
            return Ok(sha256);
        }

        // 创建子目录
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| SessionError::Io(format!("无法创建 Blob 目录 {:?}: {}", parent, e)))?;
        }

        tokio::fs::write(&path, data)
            .await
            .map_err(|e| SessionError::Io(format!("写入 Blob 失败: {}", e)))?;

        Ok(sha256)
    }

    /// 按 SHA-256 标识读取 Blob 数据
    pub async fn read(&self, sha256: &str) -> Result<Vec<u8>, SessionError> {
        let path = self.blob_path(sha256);
        tokio::fs::read(&path)
            .await
            .map_err(|e| SessionError::Io(format!("读取 Blob {} 失败: {}", sha256, e)))
    }

    /// 检查 Blob 是否存在
    pub async fn exists(&self, sha256: &str) -> bool {
        self.blob_path(sha256).exists()
    }

    /// 垃圾回收 - 清理未被引用的 Blob
    ///
    /// 遍历所有 Blob 文件，删除不在 `referenced` 集合中的文件。
    /// 返回删除的文件数。
    pub async fn gc(&self, referenced: &HashSet<String>) -> Result<u32, SessionError> {
        let mut deleted = 0u32;

        if !self.blob_dir.exists() {
            return Ok(0);
        }

        // 遍历子目录
        let mut dir = tokio::fs::read_dir(&self.blob_dir)
            .await
            .map_err(|e| SessionError::Io(format!("无法读取 Blob 目录: {}", e)))?;

        while let Some(prefix_entry) = dir
            .next_entry()
            .await
            .map_err(|e| SessionError::Io(format!("读取目录条目失败: {}", e)))?
        {
            let prefix_path = prefix_entry.path();
            if !prefix_path.is_dir() {
                continue;
            }

            let mut sub_dir = tokio::fs::read_dir(&prefix_path)
                .await
                .map_err(|e| SessionError::Io(format!("无法读取子目录: {}", e)))?;

            while let Some(blob_entry) = sub_dir
                .next_entry()
                .await
                .map_err(|e| SessionError::Io(format!("读取 Blob 条目失败: {}", e)))?
            {
                let file_name = blob_entry.file_name().to_string_lossy().to_string();
                if !referenced.contains(&file_name) {
                    tokio::fs::remove_file(blob_entry.path())
                        .await
                        .map_err(|e| SessionError::Io(format!("删除 Blob 失败: {}", e)))?;
                    deleted += 1;
                }
            }
        }

        Ok(deleted)
    }
}

/// 字节数组转十六进制字符串
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
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
    async fn 测试存储和读取数据() {
        let tmp = TempDir::new().unwrap();
        let store = BlobStore::new(tmp.path().to_path_buf());

        let data = b"Hello, World!";
        let sha = store.store(data).await.unwrap();

        // 读回数据
        let read_back = store.read(&sha).await.unwrap();
        assert_eq!(read_back, data);
    }

    #[tokio::test]
    async fn 测试内容寻址_相同内容相同哈希() {
        let tmp = TempDir::new().unwrap();
        let store = BlobStore::new(tmp.path().to_path_buf());

        let data = b"same content";
        let sha1 = store.store(data).await.unwrap();
        let sha2 = store.store(data).await.unwrap();

        // 相同内容必须产生相同的哈希
        assert_eq!(sha1, sha2);
    }

    #[tokio::test]
    async fn 测试不同内容不同哈希() {
        let tmp = TempDir::new().unwrap();
        let store = BlobStore::new(tmp.path().to_path_buf());

        let sha1 = store.store(b"content A").await.unwrap();
        let sha2 = store.store(b"content B").await.unwrap();

        assert_ne!(sha1, sha2);
    }

    #[tokio::test]
    async fn 测试存在性检查() {
        let tmp = TempDir::new().unwrap();
        let store = BlobStore::new(tmp.path().to_path_buf());

        let sha = store.store(b"test data").await.unwrap();

        assert!(store.exists(&sha).await);
        assert!(
            !store
                .exists("0000000000000000000000000000000000000000000000000000000000000000")
                .await
        );
    }

    #[tokio::test]
    async fn 测试垃圾回收_删除未引用的blob() {
        let tmp = TempDir::new().unwrap();
        let store = BlobStore::new(tmp.path().to_path_buf());

        let sha1 = store.store(b"keep this").await.unwrap();
        let sha2 = store.store(b"remove this").await.unwrap();
        let sha3 = store.store(b"also remove").await.unwrap();

        // 只引用 sha1
        let mut referenced = HashSet::new();
        referenced.insert(sha1.clone());

        let deleted = store.gc(&referenced).await.unwrap();
        assert_eq!(deleted, 2);

        // sha1 仍然存在
        assert!(store.exists(&sha1).await);
        // sha2、sha3 已被删除
        assert!(!store.exists(&sha2).await);
        assert!(!store.exists(&sha3).await);
    }

    #[tokio::test]
    async fn 测试垃圾回收_空目录() {
        let tmp = TempDir::new().unwrap();
        let store = BlobStore::new(tmp.path().join("blobs"));

        let referenced = HashSet::new();
        let deleted = store.gc(&referenced).await.unwrap();
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn 测试大数据存储() {
        let tmp = TempDir::new().unwrap();
        let store = BlobStore::new(tmp.path().to_path_buf());

        // 1MB 的数据
        let data: Vec<u8> = (0..1_000_000).map(|i| (i % 256) as u8).collect();
        let sha = store.store(&data).await.unwrap();
        let read_back = store.read(&sha).await.unwrap();
        assert_eq!(read_back, data);
    }

    #[tokio::test]
    async fn 测试读取不存在的blob() {
        let tmp = TempDir::new().unwrap();
        let store = BlobStore::new(tmp.path().to_path_buf());

        let result = store.read("nonexistent").await;
        assert!(result.is_err());
    }

    #[test]
    fn 测试sha256计算() {
        // 已知的 SHA-256 值验证
        let hash = BlobStore::sha256_hex(b"hello");
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }
}

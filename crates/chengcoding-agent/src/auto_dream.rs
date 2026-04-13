//! # AutoDream 门控与锁
//!
//! AutoDream 是记忆整合的自动触发机制。
//!
//! # 设计思想
//! 参考 reference 中 autoDream 的设计：
//! - 门控条件链按成本从低到高排列（cheapest gate first）
//! - 分布式锁防止多进程同时执行整合
//! - 锁过期机制防止进程崩溃后死锁

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

// ---------------------------------------------------------------------------
// 门控条件
// ---------------------------------------------------------------------------

/// AutoDream 门控条件快照
///
/// 收集判断是否应触发 dream 的所有信息
#[derive(Clone, Debug)]
pub struct DreamGateSnapshot {
    /// 记忆系统是否已启用
    pub memory_enabled: bool,
    /// 距上次整合的时间
    pub since_last_consolidation: Duration,
    /// 距上次扫描的时间
    pub since_last_scan: Duration,
    /// 上次整合以来的新会话数
    pub new_sessions: usize,
}

/// AutoDream 门控配置
#[derive(Clone, Debug)]
pub struct DreamGateConfig {
    /// 整合间隔最小时间（默认 24 小时）
    pub min_consolidation_interval: Duration,
    /// 扫描间隔最小时间（默认 10 分钟）
    pub min_scan_interval: Duration,
    /// 触发整合的最少新会话数（默认 5）
    pub min_new_sessions: usize,
}

impl Default for DreamGateConfig {
    fn default() -> Self {
        Self {
            min_consolidation_interval: Duration::from_secs(24 * 60 * 60),
            min_scan_interval: Duration::from_secs(10 * 60),
            min_new_sessions: 5,
        }
    }
}

/// AutoDream 门控判断器
///
/// 按成本从低到高检查门控条件：
/// 1. 记忆系统是否启用（最便宜，内存查询）
/// 2. 距上次整合是否超过阈值（时间比较）
/// 3. 距上次扫描是否超过阈值（时间比较）
/// 4. 新会话数是否足够（计数查询）
pub struct DreamGate {
    config: DreamGateConfig,
}

impl DreamGate {
    pub fn new(config: DreamGateConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(DreamGateConfig::default())
    }

    /// 判断是否应触发 dream 整合
    ///
    /// 门控条件按成本从低到高排列（cheapest gate first），
    /// 任一条件不满足立即返回 false
    pub fn should_dream(&self, snapshot: &DreamGateSnapshot) -> bool {
        // 门控 1: 记忆系统必须已启用
        if !snapshot.memory_enabled {
            return false;
        }

        // 门控 2: 距上次整合必须超过最小间隔
        if snapshot.since_last_consolidation < self.config.min_consolidation_interval {
            return false;
        }

        // 门控 3: 距上次扫描必须超过最小间隔
        if snapshot.since_last_scan < self.config.min_scan_interval {
            return false;
        }

        // 门控 4: 新会话数必须达到阈值
        if snapshot.new_sessions < self.config.min_new_sessions {
            return false;
        }

        true
    }
}

impl Default for DreamGate {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ---------------------------------------------------------------------------
// 分布式锁
// ---------------------------------------------------------------------------

/// 锁文件过期时间（1 小时）
const LOCK_EXPIRY: Duration = Duration::from_secs(60 * 60);

/// DreamLock 操作结果
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LockResult {
    /// 成功获取锁
    Acquired,
    /// 锁已被其他进程持有
    AlreadyHeld(String),
    /// IO 错误
    Error(String),
}

/// 分布式文件锁
///
/// 使用文件系统实现的简单分布式锁：
/// - 写入 PID 标识所有权
/// - 过期时间防止死锁
/// - 可抢占过期锁
pub struct DreamLock {
    lock_path: PathBuf,
}

impl DreamLock {
    /// 创建锁实例
    pub fn new(lock_path: impl Into<PathBuf>) -> Self {
        Self {
            lock_path: lock_path.into(),
        }
    }

    /// 使用默认路径创建
    pub fn with_default_path() -> Self {
        let mut path = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        path.push(".chengcoding");
        path.push("memory");
        path.push(".consolidate-lock");
        Self::new(path)
    }

    /// 尝试获取锁
    ///
    /// 流程：
    /// 1. 检查锁文件是否存在
    /// 2. 如果存在且未过期 → AlreadyHeld
    /// 3. 如果存在但已过期 → 删除后重新获取（可抢占）
    /// 4. 如果不存在 → 创建锁文件并写入 PID
    /// 5. 重读验证 PID 确认所有权
    pub fn try_acquire(&self) -> LockResult {
        // 检查现有锁
        if self.lock_path.exists() {
            match self.check_existing_lock() {
                ExistingLock::Valid(holder) => {
                    // 如果是当前进程持有的锁，视为已获取
                    let my_pid = std::process::id().to_string();
                    if holder == my_pid {
                        return LockResult::Acquired;
                    }
                    return LockResult::AlreadyHeld(holder);
                }
                ExistingLock::Expired => {
                    // 过期锁可抢占
                    if let Err(e) = std::fs::remove_file(&self.lock_path) {
                        return LockResult::Error(format!("删除过期锁失败: {}", e));
                    }
                }
                ExistingLock::Invalid => {
                    // 无效锁文件，删除后重新获取
                    let _ = std::fs::remove_file(&self.lock_path);
                }
            }
        }

        // 确保父目录存在
        if let Some(parent) = self.lock_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return LockResult::Error(format!("创建锁目录失败: {}", e));
            }
        }

        // 写入 PID
        let pid = std::process::id().to_string();
        if let Err(e) = std::fs::write(&self.lock_path, &pid) {
            return LockResult::Error(format!("写入锁文件失败: {}", e));
        }

        // 重读验证所有权（防止竞态）
        match std::fs::read_to_string(&self.lock_path) {
            Ok(content) if content.trim() == pid => LockResult::Acquired,
            Ok(content) => LockResult::AlreadyHeld(content.trim().to_string()),
            Err(e) => LockResult::Error(format!("验证锁失败: {}", e)),
        }
    }

    /// 释放锁
    pub fn release(&self) -> Result<(), String> {
        if self.lock_path.exists() {
            // 只释放自己持有的锁
            if let Ok(content) = std::fs::read_to_string(&self.lock_path) {
                let pid = std::process::id().to_string();
                if content.trim() != pid {
                    return Err(format!(
                        "锁由其他进程持有 (PID: {}), 无法释放",
                        content.trim()
                    ));
                }
            }
            std::fs::remove_file(&self.lock_path).map_err(|e| format!("删除锁文件失败: {}", e))
        } else {
            Ok(())
        }
    }

    /// 锁文件路径
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    /// 检查现有锁状态
    fn check_existing_lock(&self) -> ExistingLock {
        // 检查修改时间是否过期
        if let Ok(metadata) = std::fs::metadata(&self.lock_path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(elapsed) = SystemTime::now().duration_since(modified) {
                    if elapsed > LOCK_EXPIRY {
                        return ExistingLock::Expired;
                    }
                }
            }
        }

        // 读取持有者信息
        match std::fs::read_to_string(&self.lock_path) {
            Ok(content) if !content.trim().is_empty() => {
                ExistingLock::Valid(content.trim().to_string())
            }
            _ => ExistingLock::Invalid,
        }
    }
}

/// 现有锁状态
enum ExistingLock {
    /// 有效锁，附带持有者信息
    Valid(String),
    /// 过期锁
    Expired,
    /// 无效锁文件
    Invalid,
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn all_pass_snapshot() -> DreamGateSnapshot {
        DreamGateSnapshot {
            memory_enabled: true,
            since_last_consolidation: Duration::from_secs(25 * 60 * 60), // 25 小时
            since_last_scan: Duration::from_secs(15 * 60),               // 15 分钟
            new_sessions: 10,
        }
    }

    // --- 门控条件测试 ---

    #[test]
    fn test_all_gates_pass() {
        let gate = DreamGate::with_defaults();
        assert!(gate.should_dream(&all_pass_snapshot()));
    }

    #[test]
    fn test_memory_disabled_fails() {
        let gate = DreamGate::with_defaults();
        let mut snap = all_pass_snapshot();
        snap.memory_enabled = false;
        assert!(!gate.should_dream(&snap));
    }

    #[test]
    fn test_recent_consolidation_fails() {
        let gate = DreamGate::with_defaults();
        let mut snap = all_pass_snapshot();
        snap.since_last_consolidation = Duration::from_secs(60 * 60); // 1 小时
        assert!(!gate.should_dream(&snap));
    }

    #[test]
    fn test_recent_scan_fails() {
        let gate = DreamGate::with_defaults();
        let mut snap = all_pass_snapshot();
        snap.since_last_scan = Duration::from_secs(5 * 60); // 5 分钟
        assert!(!gate.should_dream(&snap));
    }

    #[test]
    fn test_insufficient_sessions_fails() {
        let gate = DreamGate::with_defaults();
        let mut snap = all_pass_snapshot();
        snap.new_sessions = 3;
        assert!(!gate.should_dream(&snap));
    }

    #[test]
    fn test_exact_threshold_sessions() {
        let gate = DreamGate::with_defaults();
        let mut snap = all_pass_snapshot();
        snap.new_sessions = 5; // 刚好等于阈值
        assert!(gate.should_dream(&snap));
    }

    #[test]
    fn test_custom_config() {
        let config = DreamGateConfig {
            min_consolidation_interval: Duration::from_secs(60),
            min_scan_interval: Duration::from_secs(30),
            min_new_sessions: 2,
        };
        let gate = DreamGate::new(config);
        let snap = DreamGateSnapshot {
            memory_enabled: true,
            since_last_consolidation: Duration::from_secs(120),
            since_last_scan: Duration::from_secs(60),
            new_sessions: 3,
        };
        assert!(gate.should_dream(&snap));
    }

    // --- 锁测试 ---

    #[test]
    fn test_lock_acquire_and_release() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join(".consolidate-lock");
        let lock = DreamLock::new(&lock_path);

        let result = lock.try_acquire();
        assert_eq!(result, LockResult::Acquired);
        assert!(lock_path.exists());

        let release = lock.release();
        assert!(release.is_ok());
        assert!(!lock_path.exists());
    }

    #[test]
    fn test_lock_double_acquire() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join(".consolidate-lock");
        let lock = DreamLock::new(&lock_path);

        let r1 = lock.try_acquire();
        assert_eq!(r1, LockResult::Acquired);

        // 同一进程再次获取应成功（PID 相同）
        let r2 = lock.try_acquire();
        assert_eq!(r2, LockResult::Acquired);

        lock.release().unwrap();
    }

    #[test]
    fn test_lock_held_by_other() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join(".consolidate-lock");

        // 模拟另一个进程写入 PID
        std::fs::write(&lock_path, "99999999").unwrap();

        let lock = DreamLock::new(&lock_path);
        let result = lock.try_acquire();
        assert!(matches!(result, LockResult::AlreadyHeld(_)));

        // 清理
        std::fs::remove_file(&lock_path).ok();
    }

    #[test]
    fn test_release_nonexistent_lock() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join(".nonexistent-lock");
        let lock = DreamLock::new(&lock_path);
        // 释放不存在的锁应该成功
        assert!(lock.release().is_ok());
    }

    #[test]
    fn test_release_others_lock_fails() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join(".consolidate-lock");

        // 模拟另一个进程的锁
        std::fs::write(&lock_path, "99999999").unwrap();

        let lock = DreamLock::new(&lock_path);
        let result = lock.release();
        assert!(result.is_err());

        // 清理
        std::fs::remove_file(&lock_path).ok();
    }

    #[test]
    fn test_lock_path() {
        let lock = DreamLock::new("/tmp/test-lock");
        assert_eq!(lock.lock_path(), Path::new("/tmp/test-lock"));
    }

    #[test]
    fn test_lock_invalid_file_gets_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join(".consolidate-lock");

        // 写入空内容（无效锁文件）
        std::fs::write(&lock_path, "").unwrap();

        let lock = DreamLock::new(&lock_path);
        let result = lock.try_acquire();
        assert_eq!(result, LockResult::Acquired);

        lock.release().unwrap();
    }
}

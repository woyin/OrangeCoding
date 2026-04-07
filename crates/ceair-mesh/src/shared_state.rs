//! 共享状态存储模块
//!
//! 提供线程安全的键值存储，基于 `DashMap` 实现无锁并发读写。
//! 支持可选的 TTL（生存时间），条目过期后自动失效。

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde_json::Value;
use tracing::debug;

// ---------------------------------------------------------------------------
// 带过期时间的条目
// ---------------------------------------------------------------------------

/// 共享状态条目 - 存储值及其可选的过期时间
#[derive(Debug, Clone)]
struct StateEntry {
    /// 存储的 JSON 值
    value: Value,
    /// 可选的过期时间，超过此时间后条目视为失效
    expires_at: Option<DateTime<Utc>>,
}

impl StateEntry {
    /// 创建一个不过期的条目
    fn new(value: Value) -> Self {
        Self {
            value,
            expires_at: None,
        }
    }

    /// 创建一个带 TTL 的条目
    fn with_ttl(value: Value, ttl: chrono::Duration) -> Self {
        Self {
            value,
            expires_at: Some(Utc::now() + ttl),
        }
    }

    /// 检查条目是否已过期
    fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires_at) => Utc::now() > expires_at,
            None => false,
        }
    }
}

// ---------------------------------------------------------------------------
// 共享状态存储
// ---------------------------------------------------------------------------

/// 共享状态存储 - 线程安全的键值对存储
///
/// 底层使用 `DashMap` 实现，支持多线程并发读写而无需外部加锁。
/// 所有值均以 `serde_json::Value` 存储，适合异构数据场景。
///
/// # 特性
///
/// - 无锁并发读写
/// - 可选的 TTL 过期机制
/// - 快照功能（获取当前所有有效条目的副本）
///
/// # 示例
///
/// ```rust
/// use ceair_mesh::shared_state::SharedState;
///
/// let state = SharedState::new();
/// state.set("key", serde_json::json!("value"));
/// assert_eq!(state.get("key"), Some(serde_json::json!("value")));
/// ```
#[derive(Debug)]
pub struct SharedState {
    /// 内部存储映射
    store: DashMap<String, StateEntry>,
}

impl SharedState {
    /// 创建一个空的共享状态存储
    pub fn new() -> Self {
        debug!("创建新的共享状态存储");
        Self {
            store: DashMap::new(),
        }
    }

    /// 设置键值对（不过期）
    ///
    /// 如果键已存在，则覆盖旧值。
    pub fn set(&self, key: impl Into<String>, value: Value) {
        let key = key.into();
        debug!(key = %key, "设置共享状态条目");
        self.store.insert(key, StateEntry::new(value));
    }

    /// 设置键值对并指定 TTL（生存时间）
    ///
    /// 超过 TTL 后，条目在下次访问时自动失效并被移除。
    pub fn set_with_ttl(&self, key: impl Into<String>, value: Value, ttl: chrono::Duration) {
        let key = key.into();
        debug!(key = %key, ttl_secs = ttl.num_seconds(), "设置带TTL的共享状态条目");
        self.store.insert(key, StateEntry::with_ttl(value, ttl));
    }

    /// 获取指定键的值
    ///
    /// 如果键不存在或已过期，返回 `None`。
    /// 过期条目在被访问时会自动移除。
    pub fn get(&self, key: &str) -> Option<Value> {
        // 先检查条目是否存在并判断过期
        let entry = self.store.get(key)?;
        if entry.is_expired() {
            // 释放读锁后再移除过期条目
            drop(entry);
            self.store.remove(key);
            debug!(key = %key, "共享状态条目已过期，自动移除");
            return None;
        }
        Some(entry.value.clone())
    }

    /// 移除指定键的条目
    ///
    /// 返回被移除的值，如果键不存在则返回 `None`。
    pub fn remove(&self, key: &str) -> Option<Value> {
        debug!(key = %key, "移除共享状态条目");
        self.store
            .remove(key)
            .map(|(_, entry)| entry.value)
    }

    /// 检查是否包含指定的键
    ///
    /// 注意：如果条目已过期，也会返回 `false` 并清理过期条目。
    pub fn contains(&self, key: &str) -> bool {
        match self.store.get(key) {
            Some(entry) => {
                if entry.is_expired() {
                    drop(entry);
                    self.store.remove(key);
                    false
                } else {
                    true
                }
            }
            None => false,
        }
    }

    /// 获取所有有效（未过期）的键列表
    pub fn keys(&self) -> Vec<String> {
        let mut expired_keys = Vec::new();
        let mut valid_keys = Vec::new();

        for entry in self.store.iter() {
            if entry.value().is_expired() {
                expired_keys.push(entry.key().clone());
            } else {
                valid_keys.push(entry.key().clone());
            }
        }

        // 清理过期条目
        for key in &expired_keys {
            self.store.remove(key);
        }

        valid_keys
    }

    /// 清除所有条目
    pub fn clear(&self) {
        debug!("清除所有共享状态条目");
        self.store.clear();
    }

    /// 获取当前所有有效条目的快照（深拷贝）
    ///
    /// 返回一个 `HashMap`，包含所有未过期的键值对。
    /// 快照是一个独立的副本，对快照的修改不会影响原始存储。
    pub fn snapshot(&self) -> HashMap<String, Value> {
        let mut result = HashMap::new();
        let mut expired_keys = Vec::new();

        for entry in self.store.iter() {
            if entry.value().is_expired() {
                expired_keys.push(entry.key().clone());
            } else {
                result.insert(entry.key().clone(), entry.value().value.clone());
            }
        }

        // 清理过期条目
        for key in &expired_keys {
            self.store.remove(key);
        }

        result
    }

    /// 获取当前有效条目数量
    pub fn len(&self) -> usize {
        // 简单返回存储大小（包括可能的过期条目）
        // 精确计数需要遍历，此处为近似值以保证性能
        self.store.len()
    }

    /// 检查存储是否为空
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn 测试基本的设置和获取() {
        let state = SharedState::new();
        state.set("name", json!("CEAIR"));
        assert_eq!(state.get("name"), Some(json!("CEAIR")));
    }

    #[test]
    fn 测试获取不存在的键() {
        let state = SharedState::new();
        assert_eq!(state.get("不存在"), None);
    }

    #[test]
    fn 测试覆盖已有的值() {
        let state = SharedState::new();
        state.set("count", json!(1));
        state.set("count", json!(2));
        assert_eq!(state.get("count"), Some(json!(2)));
    }

    #[test]
    fn 测试移除条目() {
        let state = SharedState::new();
        state.set("temp", json!("临时数据"));

        let removed = state.remove("temp");
        assert_eq!(removed, Some(json!("临时数据")));
        assert_eq!(state.get("temp"), None);
    }

    #[test]
    fn 测试移除不存在的条目() {
        let state = SharedState::new();
        assert_eq!(state.remove("不存在"), None);
    }

    #[test]
    fn 测试包含检查() {
        let state = SharedState::new();
        state.set("exists", json!(true));

        assert!(state.contains("exists"));
        assert!(!state.contains("not_exists"));
    }

    #[test]
    fn 测试获取所有键() {
        let state = SharedState::new();
        state.set("a", json!(1));
        state.set("b", json!(2));
        state.set("c", json!(3));

        let mut keys = state.keys();
        keys.sort();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn 测试清除所有条目() {
        let state = SharedState::new();
        state.set("a", json!(1));
        state.set("b", json!(2));

        state.clear();
        assert!(state.is_empty());
        assert_eq!(state.len(), 0);
    }

    #[test]
    fn 测试快照功能() {
        let state = SharedState::new();
        state.set("x", json!(10));
        state.set("y", json!(20));

        let snap = state.snapshot();
        assert_eq!(snap.len(), 2);
        assert_eq!(snap.get("x"), Some(&json!(10)));
        assert_eq!(snap.get("y"), Some(&json!(20)));

        // 修改原始存储不影响快照（快照是深拷贝）
        state.set("x", json!(999));
        assert_eq!(snap.get("x"), Some(&json!(10)));
    }

    #[test]
    fn 测试带TTL的条目立即可用() {
        let state = SharedState::new();
        // 设置一个 1 小时后过期的条目
        state.set_with_ttl("long_lived", json!("存活"), chrono::Duration::hours(1));
        assert_eq!(state.get("long_lived"), Some(json!("存活")));
    }

    #[test]
    fn 测试TTL过期的条目自动移除() {
        let state = SharedState::new();
        // 设置一个已经过期的条目（TTL 为负数）
        state.set_with_ttl("expired", json!("过期数据"), chrono::Duration::seconds(-1));

        // 访问已过期的条目应返回 None
        assert_eq!(state.get("expired"), None);
        // contains 检查也应返回 false
        assert!(!state.contains("expired"));
    }

    #[test]
    fn 测试过期条目不出现在keys中() {
        let state = SharedState::new();
        state.set("valid", json!(1));
        state.set_with_ttl("expired", json!(2), chrono::Duration::seconds(-1));

        let keys = state.keys();
        assert_eq!(keys, vec!["valid"]);
    }

    #[test]
    fn 测试过期条目不出现在快照中() {
        let state = SharedState::new();
        state.set("valid", json!("有效"));
        state.set_with_ttl("expired", json!("过期"), chrono::Duration::seconds(-1));

        let snap = state.snapshot();
        assert_eq!(snap.len(), 1);
        assert!(snap.contains_key("valid"));
        assert!(!snap.contains_key("expired"));
    }

    #[test]
    fn 测试长度和空判断() {
        let state = SharedState::new();
        assert!(state.is_empty());
        assert_eq!(state.len(), 0);

        state.set("a", json!(1));
        assert!(!state.is_empty());
        assert_eq!(state.len(), 1);
    }

    #[test]
    fn 测试存储各种JSON类型() {
        let state = SharedState::new();

        // 字符串
        state.set("string", json!("hello"));
        // 数字
        state.set("number", json!(42));
        // 布尔值
        state.set("bool", json!(true));
        // 数组
        state.set("array", json!([1, 2, 3]));
        // 对象
        state.set("object", json!({"key": "value"}));
        // null
        state.set("null", json!(null));

        assert_eq!(state.get("string"), Some(json!("hello")));
        assert_eq!(state.get("number"), Some(json!(42)));
        assert_eq!(state.get("bool"), Some(json!(true)));
        assert_eq!(state.get("array"), Some(json!([1, 2, 3])));
        assert_eq!(state.get("object"), Some(json!({"key": "value"})));
        assert_eq!(state.get("null"), Some(json!(null)));
    }

    #[test]
    fn 测试默认构造() {
        let state = SharedState::default();
        assert!(state.is_empty());
    }
}

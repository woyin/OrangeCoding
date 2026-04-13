//! # 哈希链防篡改模块
//!
//! 使用 SHA-256 哈希链来保证审计日志的完整性和不可篡改性。
//! 每个条目的哈希值 = SHA-256(前一个条目的哈希值 + 当前条目数据)，
//! 形成链式结构，任何中间条目的篡改都会导致后续所有哈希值失效。

use chrono::Utc;
use ring::digest::{self, SHA256};
use serde_json;
use uuid::Uuid;

use crate::logger::AuditEntry;

/// 哈希链结构
///
/// 维护一个有序的审计条目列表，每个条目通过哈希值与前一个条目关联，
/// 构成不可篡改的链式结构。
#[derive(Debug)]
pub struct HashChain {
    /// 链中的所有审计条目
    entries: Vec<AuditEntry>,

    /// 最新条目的哈希值，用于计算下一个条目的哈希
    latest_hash: String,
}

/// 创世区块（链的第一个条目）的前置哈希值
const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

impl HashChain {
    /// 创建新的空哈希链
    ///
    /// 初始化时最新哈希值设为创世哈希（全零）。
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            latest_hash: GENESIS_HASH.to_string(),
        }
    }

    /// 向哈希链追加新条目
    ///
    /// 根据前一个条目的哈希值和当前条目数据计算新的哈希值，
    /// 然后创建审计条目并追加到链中。
    ///
    /// # 参数
    /// - `action`: 操作类型
    /// - `actor`: 操作者
    /// - `target`: 操作目标（可选）
    /// - `details`: 详细信息
    ///
    /// # 返回
    /// 包含哈希值的新审计条目
    pub fn append(
        &mut self,
        action: String,
        actor: String,
        target: Option<String>,
        details: serde_json::Value,
    ) -> AuditEntry {
        let id = Uuid::new_v4();
        let timestamp = Utc::now();
        let previous_hash = self.latest_hash.clone();

        // 构造待哈希的数据：前一个哈希 + 条目核心字段的序列化
        let hash_input = Self::build_hash_input(
            &previous_hash,
            &id,
            &timestamp,
            &action,
            &actor,
            &target,
            &details,
        );

        // 计算 SHA-256 哈希值
        let hash = Self::compute_hash(&hash_input);

        // 创建审计条目
        let entry = AuditEntry {
            id,
            timestamp,
            action,
            actor,
            target,
            details,
            hash: hash.clone(),
            previous_hash,
        };

        // 更新链状态
        self.latest_hash = hash;
        self.entries.push(entry.clone());

        entry
    }

    /// 验证整个哈希链的完整性
    ///
    /// 从链的起点开始，逐个验证每个条目的哈希值是否正确，
    /// 以及前置哈希值是否与前一个条目的哈希值匹配。
    ///
    /// # 返回
    /// 如果整个链完整无损返回 `true`，否则返回 `false`
    pub fn verify_chain(&self) -> bool {
        let mut expected_previous_hash = GENESIS_HASH.to_string();

        for entry in &self.entries {
            // 检查前置哈希值是否匹配
            if entry.previous_hash != expected_previous_hash {
                tracing::error!(
                    entry_id = %entry.id,
                    expected = %expected_previous_hash,
                    actual = %entry.previous_hash,
                    "哈希链验证失败：前置哈希不匹配"
                );
                return false;
            }

            // 重新计算哈希值并比较
            if !self.verify_entry(entry) {
                return false;
            }

            // 更新预期的前置哈希为当前条目的哈希
            expected_previous_hash = entry.hash.clone();
        }

        true
    }

    /// 验证单个条目的哈希值是否正确
    ///
    /// 使用条目的数据重新计算哈希值，与存储的哈希值进行比较。
    ///
    /// # 参数
    /// - `entry`: 要验证的审计条目
    ///
    /// # 返回
    /// 哈希值匹配返回 `true`，不匹配返回 `false`
    pub fn verify_entry(&self, entry: &AuditEntry) -> bool {
        let hash_input = Self::build_hash_input(
            &entry.previous_hash,
            &entry.id,
            &entry.timestamp,
            &entry.action,
            &entry.actor,
            &entry.target,
            &entry.details,
        );

        let computed_hash = Self::compute_hash(&hash_input);

        if computed_hash != entry.hash {
            tracing::error!(
                entry_id = %entry.id,
                expected = %entry.hash,
                computed = %computed_hash,
                "哈希链验证失败：条目哈希不匹配"
            );
            return false;
        }

        true
    }

    /// 获取最新条目的哈希值
    ///
    /// 如果链为空，返回创世哈希值。
    pub fn get_latest_hash(&self) -> &str {
        &self.latest_hash
    }

    /// 获取链中条目的数量
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 判断哈希链是否为空
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 获取链中所有条目的只读引用
    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    /// 构建用于哈希计算的输入字符串
    ///
    /// 将前置哈希和条目核心字段拼接为统一的字符串格式。
    fn build_hash_input(
        previous_hash: &str,
        id: &Uuid,
        timestamp: &chrono::DateTime<Utc>,
        action: &str,
        actor: &str,
        target: &Option<String>,
        details: &serde_json::Value,
    ) -> String {
        // 将目标字段序列化为字符串
        let target_str = target.as_deref().unwrap_or("");

        // 将详细信息序列化为紧凑的 JSON 字符串
        let details_str = serde_json::to_string(details).unwrap_or_default();

        // 拼接所有字段：前置哈希 | ID | 时间戳 | 操作 | 操作者 | 目标 | 详情
        format!(
            "{}|{}|{}|{}|{}|{}|{}",
            previous_hash,
            id,
            timestamp.to_rfc3339(),
            action,
            actor,
            target_str,
            details_str
        )
    }

    /// 计算 SHA-256 哈希值
    ///
    /// 使用 ring 库计算输入数据的 SHA-256 摘要，返回十六进制字符串。
    fn compute_hash(input: &str) -> String {
        let digest = digest::digest(&SHA256, input.as_bytes());

        // 将哈希字节转换为十六进制字符串
        digest
            .as_ref()
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect()
    }
}

impl Default for HashChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试：创建空哈希链
    #[test]
    fn test_new_chain() {
        let chain = HashChain::new();

        // 新链应该为空
        assert!(chain.is_empty(), "新创建的哈希链应为空");
        assert_eq!(chain.len(), 0);
        assert_eq!(chain.get_latest_hash(), GENESIS_HASH);
    }

    /// 测试：向哈希链追加条目
    #[test]
    fn test_append_entry() {
        let mut chain = HashChain::new();

        // 追加第一个条目
        let entry = chain.append(
            "test_action".to_string(),
            "user1".to_string(),
            Some("target1".to_string()),
            serde_json::json!({"key": "value"}),
        );

        // 验证链的状态
        assert_eq!(chain.len(), 1);
        assert!(!chain.is_empty());

        // 验证条目字段
        assert_eq!(entry.action, "test_action");
        assert_eq!(entry.actor, "user1");
        assert_eq!(entry.target, Some("target1".to_string()));

        // 第一个条目的前置哈希应为创世哈希
        assert_eq!(entry.previous_hash, GENESIS_HASH);

        // 哈希值不应为空
        assert!(!entry.hash.is_empty(), "哈希值不应为空");

        // 最新哈希应更新为当前条目的哈希
        assert_eq!(chain.get_latest_hash(), entry.hash);
    }

    /// 测试：多个条目形成哈希链
    #[test]
    fn test_chain_linkage() {
        let mut chain = HashChain::new();

        // 追加三个条目
        let entry1 = chain.append(
            "action1".to_string(),
            "user".to_string(),
            None,
            serde_json::json!({}),
        );

        let entry2 = chain.append(
            "action2".to_string(),
            "user".to_string(),
            None,
            serde_json::json!({}),
        );

        let entry3 = chain.append(
            "action3".to_string(),
            "user".to_string(),
            None,
            serde_json::json!({}),
        );

        // 验证链的连接关系
        assert_eq!(
            entry1.previous_hash, GENESIS_HASH,
            "第一个条目的前置哈希应为创世哈希"
        );
        assert_eq!(
            entry2.previous_hash, entry1.hash,
            "第二个条目的前置哈希应为第一个条目的哈希"
        );
        assert_eq!(
            entry3.previous_hash, entry2.hash,
            "第三个条目的前置哈希应为第二个条目的哈希"
        );

        // 每个条目的哈希值应该不同
        assert_ne!(entry1.hash, entry2.hash, "不同条目的哈希值不应相同");
        assert_ne!(entry2.hash, entry3.hash, "不同条目的哈希值不应相同");
    }

    /// 测试：验证完整哈希链
    #[test]
    fn test_verify_chain_valid() {
        let mut chain = HashChain::new();

        // 添加多个条目
        for i in 0..5 {
            chain.append(
                format!("action_{}", i),
                "user".to_string(),
                None,
                serde_json::json!({"index": i}),
            );
        }

        // 未被篡改的链应验证通过
        assert!(chain.verify_chain(), "未篡改的哈希链应验证通过");
    }

    /// 测试：检测篡改的哈希链
    #[test]
    fn test_verify_chain_tampered() {
        let mut chain = HashChain::new();

        // 添加条目
        chain.append(
            "action1".to_string(),
            "user".to_string(),
            None,
            serde_json::json!({}),
        );
        chain.append(
            "action2".to_string(),
            "user".to_string(),
            None,
            serde_json::json!({}),
        );
        chain.append(
            "action3".to_string(),
            "user".to_string(),
            None,
            serde_json::json!({}),
        );

        // 篡改中间条目的数据
        chain.entries[1].action = "tampered_action".to_string();

        // 篡改后的链应验证失败
        assert!(!chain.verify_chain(), "篡改后的哈希链应验证失败");
    }

    /// 测试：验证单个条目
    #[test]
    fn test_verify_single_entry() {
        let mut chain = HashChain::new();

        let entry = chain.append(
            "test".to_string(),
            "user".to_string(),
            None,
            serde_json::json!({"data": "测试数据"}),
        );

        // 未篡改的条目应验证通过
        assert!(chain.verify_entry(&entry), "未篡改的条目应验证通过");

        // 篡改条目后应验证失败
        let mut tampered = entry.clone();
        tampered.details = serde_json::json!({"data": "被修改的数据"});
        assert!(!chain.verify_entry(&tampered), "篡改后的条目应验证失败");
    }

    /// 测试：空链的验证
    #[test]
    fn test_verify_empty_chain() {
        let chain = HashChain::new();

        // 空链应该验证通过
        assert!(chain.verify_chain(), "空链应验证通过");
    }

    /// 测试：哈希值的确定性
    #[test]
    fn test_hash_deterministic() {
        // 相同输入应产生相同的哈希值
        let hash1 = HashChain::compute_hash("测试数据");
        let hash2 = HashChain::compute_hash("测试数据");
        assert_eq!(hash1, hash2, "相同输入应产生相同的哈希值");

        // 不同输入应产生不同的哈希值
        let hash3 = HashChain::compute_hash("不同的数据");
        assert_ne!(hash1, hash3, "不同输入应产生不同的哈希值");
    }

    /// 测试：哈希值格式（64个十六进制字符）
    #[test]
    fn test_hash_format() {
        let hash = HashChain::compute_hash("test");

        // SHA-256 产生 32 字节 = 64 个十六进制字符
        assert_eq!(hash.len(), 64, "SHA-256 哈希值应为64个十六进制字符");

        // 验证所有字符都是十六进制
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "哈希值应只包含十六进制字符"
        );
    }

    /// 测试：获取链中的所有条目
    #[test]
    fn test_entries_access() {
        let mut chain = HashChain::new();

        chain.append(
            "action1".to_string(),
            "user".to_string(),
            None,
            serde_json::json!({}),
        );
        chain.append(
            "action2".to_string(),
            "user".to_string(),
            None,
            serde_json::json!({}),
        );

        let entries = chain.entries();
        assert_eq!(entries.len(), 2, "应返回2个条目");
        assert_eq!(entries[0].action, "action1");
        assert_eq!(entries[1].action, "action2");
    }

    /// 测试：篡改哈希值本身也会被检测
    #[test]
    fn test_detect_hash_tampering() {
        let mut chain = HashChain::new();

        chain.append(
            "action1".to_string(),
            "user".to_string(),
            None,
            serde_json::json!({}),
        );
        chain.append(
            "action2".to_string(),
            "user".to_string(),
            None,
            serde_json::json!({}),
        );

        // 篡改第一个条目的哈希值
        chain.entries[0].hash =
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();

        // 链应验证失败（第二个条目的前置哈希与第一个条目的哈希不匹配）
        assert!(!chain.verify_chain(), "哈希值被篡改后链应验证失败");
    }
}

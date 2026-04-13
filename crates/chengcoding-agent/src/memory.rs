//! # 记忆系统模块
//!
//! 提供按会话的记忆提取和跨会话整合两个阶段。
//! 默认禁用，需要显式启用。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// 记忆类型分类
// ---------------------------------------------------------------------------

/// 记忆类型分类
///
/// 参考 reference 中 memdir 系统的 4 类分类法：
/// - 每种类型对应不同的记忆生命周期和使用场景
/// - 类型决定了记忆在上下文注入时的优先级
/// - 分类法帮助 autoDream 做出合理的整合决策
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryType {
    /// 项目规范与约定（如编码风格、架构决策）
    ///
    /// 生命周期长、变动少，适合持久化到 MEMORY.md
    Guideline,

    /// 错误模式与修复策略（如常见 bug、踩坑记录）
    ///
    /// 有时效性，过期后可降级或清理
    Bugfix,

    /// 系统知识（如架构设计、技术选型、数据流）
    ///
    /// 跨会话共享价值高，autoDream 整合的重点对象
    Knowledge,

    /// 用户偏好（如交互风格、工具选择、输出格式）
    ///
    /// 优先级最高，在上下文受限时最后被裁剪
    Preference,
}

impl MemoryType {
    /// 获取类型的中文显示名
    pub fn display_name(&self) -> &'static str {
        match self {
            MemoryType::Guideline => "规范",
            MemoryType::Bugfix => "修复",
            MemoryType::Knowledge => "知识",
            MemoryType::Preference => "偏好",
        }
    }

    /// 获取类型的英文标识（用于序列化和文件名）
    pub fn slug(&self) -> &'static str {
        match self {
            MemoryType::Guideline => "guideline",
            MemoryType::Bugfix => "bugfix",
            MemoryType::Knowledge => "knowledge",
            MemoryType::Preference => "preference",
        }
    }

    /// 从英文标识解析
    pub fn from_slug(s: &str) -> Option<Self> {
        match s {
            "guideline" => Some(MemoryType::Guideline),
            "bugfix" => Some(MemoryType::Bugfix),
            "knowledge" => Some(MemoryType::Knowledge),
            "preference" => Some(MemoryType::Preference),
            _ => None,
        }
    }

    /// 获取在上下文注入时的优先级权重
    ///
    /// 权重越高，在 token 受限时越不容易被裁剪。
    /// 设计依据：用户偏好 > 项目规范 > 系统知识 > 修复记录
    pub fn priority_weight(&self) -> u8 {
        match self {
            MemoryType::Preference => 4,
            MemoryType::Guideline => 3,
            MemoryType::Knowledge => 2,
            MemoryType::Bugfix => 1,
        }
    }

    /// 列出所有类型
    pub fn all() -> &'static [MemoryType] {
        &[
            MemoryType::Guideline,
            MemoryType::Bugfix,
            MemoryType::Knowledge,
            MemoryType::Preference,
        ]
    }
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.slug())
    }
}

/// 记忆系统配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// 是否启用（默认 false）
    pub enabled: bool,
    /// 存储路径
    pub storage_path: PathBuf,
    /// 最大记忆条目数
    pub max_entries: usize,
    /// 过期天数（0 = 不过期）
    pub expire_days: u32,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            storage_path: PathBuf::from("~/.chengcoding/memory"),
            max_entries: 1000,
            expire_days: 0,
        }
    }
}

/// 记忆条目
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// 唯一标识
    pub id: String,
    /// 记忆类型分类
    pub memory_type: MemoryType,
    /// 记忆内容
    pub content: String,
    /// 来源会话 ID
    pub source_session: Option<String>,
    /// 标签列表
    pub tags: Vec<String>,
    /// 重要性评分（0.0 ~ 1.0）
    pub importance: f32,
    /// 创建时间戳（秒）
    pub created_at: u64,
    /// 最后访问时间戳（秒）
    pub last_accessed: u64,
}

/// 记忆存储
pub struct MemoryStore {
    config: MemoryConfig,
    entries: Vec<MemoryEntry>,
}

/// 获取当前 UNIX 时间戳（秒）
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl MemoryStore {
    /// 创建新的记忆存储
    pub fn new(config: MemoryConfig) -> Self {
        Self {
            config,
            entries: Vec::new(),
        }
    }

    /// 添加记忆条目，返回生成的 ID
    pub fn add(
        &mut self,
        content: &str,
        memory_type: MemoryType,
        tags: Vec<String>,
        importance: f32,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let ts = now_secs();
        let entry = MemoryEntry {
            id: id.clone(),
            content: content.to_string(),
            memory_type,
            source_session: None,
            tags,
            importance,
            created_at: ts,
            last_accessed: ts,
        };
        self.entries.push(entry);

        // 超过最大条目数时淘汰最不重要的
        if self.entries.len() > self.config.max_entries {
            self.entries.sort_by(|a, b| {
                b.importance
                    .partial_cmp(&a.importance)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            self.entries.truncate(self.config.max_entries);
        }

        id
    }

    /// 按关键词搜索（大小写不敏感），返回匹配的条目引用
    pub fn search(&self, query: &str, limit: usize) -> Vec<&MemoryEntry> {
        if !self.config.enabled {
            return Vec::new();
        }
        let q = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.content.to_lowercase().contains(&q))
            .take(limit)
            .collect()
    }

    /// 按标签搜索
    pub fn search_by_tag(&self, tag: &str) -> Vec<&MemoryEntry> {
        if !self.config.enabled {
            return Vec::new();
        }
        self.entries
            .iter()
            .filter(|e| e.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// 按记忆类型搜索
    pub fn search_by_type(&self, memory_type: MemoryType) -> Vec<&MemoryEntry> {
        if !self.config.enabled {
            return Vec::new();
        }
        self.entries
            .iter()
            .filter(|e| e.memory_type == memory_type)
            .collect()
    }

    /// 按优先级权重排序返回记忆（用于 token 受限时的裁剪决策）
    ///
    /// 先按 MemoryType 优先级降序，再按 importance 降序
    pub fn prioritized(&self, limit: usize) -> Vec<&MemoryEntry> {
        if !self.config.enabled {
            return Vec::new();
        }
        let mut sorted: Vec<&MemoryEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| {
            let type_cmp = b
                .memory_type
                .priority_weight()
                .cmp(&a.memory_type.priority_weight());
            type_cmp.then(
                b.importance
                    .partial_cmp(&a.importance)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        sorted.into_iter().take(limit).collect()
    }

    /// 获取最近的记忆（按创建时间降序）
    pub fn recent(&self, limit: usize) -> Vec<&MemoryEntry> {
        if !self.config.enabled {
            return Vec::new();
        }
        let mut sorted: Vec<&MemoryEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        sorted.into_iter().take(limit).collect()
    }

    /// 获取最重要的记忆（按 importance 降序）
    pub fn top_important(&self, limit: usize) -> Vec<&MemoryEntry> {
        if !self.config.enabled {
            return Vec::new();
        }
        let mut sorted: Vec<&MemoryEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.into_iter().take(limit).collect()
    }

    /// 删除过期记忆，返回删除数量
    pub fn cleanup_expired(&mut self) -> usize {
        if self.config.expire_days == 0 {
            return 0;
        }
        let cutoff = now_secs().saturating_sub(self.config.expire_days as u64 * 86400);
        let before = self.entries.len();
        self.entries.retain(|e| e.created_at >= cutoff);
        before - self.entries.len()
    }

    /// 合并外部记忆条目（跨会话整合），去重基于 ID
    pub fn consolidate(&mut self, entries: Vec<MemoryEntry>) {
        for entry in entries {
            if !self.entries.iter().any(|e| e.id == entry.id) {
                self.entries.push(entry);
            }
        }
        // 超出限制时淘汰低重要性条目
        if self.entries.len() > self.config.max_entries {
            self.entries.sort_by(|a, b| {
                b.importance
                    .partial_cmp(&a.importance)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            self.entries.truncate(self.config.max_entries);
        }
    }

    /// 保存到 JSON 文件
    pub fn save(&self) -> Result<(), std::io::Error> {
        if let Some(parent) = self.config.storage_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(&self.entries)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&self.config.storage_path, data)
    }

    /// 从 JSON 文件加载
    pub fn load(config: MemoryConfig) -> Result<Self, std::io::Error> {
        if !config.storage_path.exists() {
            return Ok(Self::new(config));
        }
        let data = std::fs::read_to_string(&config.storage_path)?;
        let entries: Vec<MemoryEntry> = serde_json::from_str(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(Self { config, entries })
    }

    /// 总条目数
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// 清除所有记忆
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// 检查记忆系统是否启用
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建启用的测试配置
    fn enabled_config() -> MemoryConfig {
        MemoryConfig {
            enabled: true,
            storage_path: PathBuf::from("test_memory.json"),
            max_entries: 100,
            expire_days: 0,
        }
    }

    #[test]
    fn test_default_config_disabled() {
        // 默认配置应处于禁用状态
        let cfg = MemoryConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.max_entries, 1000);
        assert_eq!(cfg.expire_days, 0);
    }

    #[test]
    fn test_add_memory() {
        let mut store = MemoryStore::new(enabled_config());
        let id = store.add("测试记忆", MemoryType::Knowledge, vec!["tag1".into()], 0.5);
        assert!(!id.is_empty());
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn test_search_by_keyword() {
        let mut store = MemoryStore::new(enabled_config());
        store.add("Rust 编程语言", MemoryType::Knowledge, vec![], 0.5);
        store.add("Python 脚本", MemoryType::Knowledge, vec![], 0.3);
        store.add("Rust 异步编程", MemoryType::Knowledge, vec![], 0.8);

        let results = store.search("rust", 10);
        assert_eq!(results.len(), 2);

        // 未匹配
        let results = store.search("java", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_by_tag() {
        let mut store = MemoryStore::new(enabled_config());
        store.add(
            "条目1",
            MemoryType::Guideline,
            vec!["重要".into(), "工作".into()],
            0.5,
        );
        store.add("条目2", MemoryType::Bugfix, vec!["个人".into()], 0.3);
        store.add("条目3", MemoryType::Guideline, vec!["重要".into()], 0.8);

        let results = store.search_by_tag("重要");
        assert_eq!(results.len(), 2);

        let results = store.search_by_tag("不存在");
        assert!(results.is_empty());
    }

    #[test]
    fn test_recent_entries() {
        let mut store = MemoryStore::new(enabled_config());
        // 手动构造不同时间戳的条目
        store.entries.push(MemoryEntry {
            id: "old".into(),
            content: "旧条目".into(),
            memory_type: MemoryType::Knowledge,
            source_session: None,
            tags: vec![],
            importance: 0.5,
            created_at: 1000,
            last_accessed: 1000,
        });
        store.entries.push(MemoryEntry {
            id: "new".into(),
            content: "新条目".into(),
            memory_type: MemoryType::Knowledge,
            source_session: None,
            tags: vec![],
            importance: 0.5,
            created_at: 2000,
            last_accessed: 2000,
        });

        let recent = store.recent(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, "new");
    }

    #[test]
    fn test_top_important() {
        let mut store = MemoryStore::new(enabled_config());
        store.add("低重要性", MemoryType::Bugfix, vec![], 0.1);
        store.add("高重要性", MemoryType::Preference, vec![], 0.9);
        store.add("中重要性", MemoryType::Knowledge, vec![], 0.5);

        let top = store.top_important(2);
        assert_eq!(top.len(), 2);
        assert!(top[0].importance >= top[1].importance);
        assert_eq!(top[0].importance, 0.9);
    }

    #[test]
    fn test_max_entries_limit() {
        // 超过限制时淘汰低重要性条目
        let mut cfg = enabled_config();
        cfg.max_entries = 3;
        let mut store = MemoryStore::new(cfg);

        store.add("一", MemoryType::Bugfix, vec![], 0.1);
        store.add("二", MemoryType::Preference, vec![], 0.9);
        store.add("三", MemoryType::Knowledge, vec![], 0.5);
        assert_eq!(store.count(), 3);

        // 再添加一条，应淘汰最低重要性的
        store.add("四", MemoryType::Guideline, vec![], 0.8);
        assert_eq!(store.count(), 3);

        // 最低重要性（0.1）的条目应被淘汰
        let has_low = store.entries.iter().any(|e| e.importance == 0.1);
        assert!(!has_low, "低重要性条目应被淘汰");
    }

    #[test]
    fn test_cleanup_expired() {
        let mut cfg = enabled_config();
        cfg.expire_days = 1; // 1 天过期
        let mut store = MemoryStore::new(cfg);

        // 添加一个过期条目（时间戳设为很久以前）
        store.entries.push(MemoryEntry {
            id: "expired".into(),
            content: "过期".into(),
            memory_type: MemoryType::Bugfix,
            source_session: None,
            tags: vec![],
            importance: 0.5,
            created_at: 0, // 1970 年，一定过期
            last_accessed: 0,
        });

        // 添加一个新鲜条目
        store.add("新鲜", MemoryType::Knowledge, vec![], 0.5);

        assert_eq!(store.count(), 2);
        let removed = store.cleanup_expired();
        assert_eq!(removed, 1);
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn test_consolidate() {
        let mut store = MemoryStore::new(enabled_config());
        store.add("本地记忆", MemoryType::Knowledge, vec![], 0.5);

        // 外部记忆
        let external = vec![
            MemoryEntry {
                id: "ext1".into(),
                content: "外部记忆1".into(),
                memory_type: MemoryType::Guideline,
                source_session: Some("session-2".into()),
                tags: vec![],
                importance: 0.7,
                created_at: now_secs(),
                last_accessed: now_secs(),
            },
            MemoryEntry {
                id: "ext2".into(),
                content: "外部记忆2".into(),
                memory_type: MemoryType::Bugfix,
                source_session: Some("session-3".into()),
                tags: vec![],
                importance: 0.3,
                created_at: now_secs(),
                last_accessed: now_secs(),
            },
        ];

        store.consolidate(external);
        assert_eq!(store.count(), 3);

        // 重复整合同 ID 不应增加
        let dup = vec![MemoryEntry {
            id: "ext1".into(),
            content: "重复".into(),
            memory_type: MemoryType::Guideline,
            source_session: None,
            tags: vec![],
            importance: 0.7,
            created_at: now_secs(),
            last_accessed: now_secs(),
        }];
        store.consolidate(dup);
        assert_eq!(store.count(), 3);
    }

    #[test]
    fn test_save_and_load() {
        // 使用临时目录进行文件读写测试
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");

        let mut cfg = enabled_config();
        cfg.storage_path = path.clone();

        let mut store = MemoryStore::new(cfg.clone());
        store.add(
            "持久化测试",
            MemoryType::Preference,
            vec!["测试".into()],
            0.8,
        );
        store.add("第二条", MemoryType::Knowledge, vec![], 0.3);
        store.save().unwrap();

        // 从文件加载
        let loaded = MemoryStore::load(cfg).unwrap();
        assert_eq!(loaded.count(), 2);
        assert!(loaded.entries.iter().any(|e| e.content == "持久化测试"));
    }

    #[test]
    fn test_disabled_returns_empty() {
        // 禁用时搜索方法应返回空
        let cfg = MemoryConfig::default(); // enabled = false
        let mut store = MemoryStore::new(cfg);
        store.add("内容", MemoryType::Knowledge, vec!["tag".into()], 0.5);

        assert!(store.search("内容", 10).is_empty());
        assert!(store.search_by_tag("tag").is_empty());
        assert!(store.recent(10).is_empty());
        assert!(store.top_important(10).is_empty());
    }

    #[test]
    fn test_clear() {
        let mut store = MemoryStore::new(enabled_config());
        store.add("a", MemoryType::Knowledge, vec![], 0.5);
        store.add("b", MemoryType::Knowledge, vec![], 0.5);
        assert_eq!(store.count(), 2);

        store.clear();
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_count() {
        let mut store = MemoryStore::new(enabled_config());
        assert_eq!(store.count(), 0);
        store.add("x", MemoryType::Knowledge, vec![], 0.5);
        assert_eq!(store.count(), 1);
        store.add("y", MemoryType::Bugfix, vec![], 0.5);
        assert_eq!(store.count(), 2);
    }

    #[test]
    fn test_unique_ids() {
        // 每次 add 应生成唯一 ID
        let mut store = MemoryStore::new(enabled_config());
        let id1 = store.add("a", MemoryType::Knowledge, vec![], 0.5);
        let id2 = store.add("b", MemoryType::Bugfix, vec![], 0.5);
        let id3 = store.add("c", MemoryType::Guideline, vec![], 0.5);
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    // -----------------------------------------------------------------------
    // MemoryType 测试
    // -----------------------------------------------------------------------

    /// 测试所有类型的 slug 往返转换
    #[test]
    fn test_memory_type_slug_roundtrip() {
        for mt in MemoryType::all() {
            let slug = mt.slug();
            let parsed = MemoryType::from_slug(slug);
            assert_eq!(parsed, Some(*mt), "slug '{}' 应往返转换", slug);
        }
    }

    /// 测试未知 slug 返回 None
    #[test]
    fn test_memory_type_unknown_slug() {
        assert!(MemoryType::from_slug("unknown").is_none());
        assert!(MemoryType::from_slug("").is_none());
    }

    /// 测试优先级权重递增
    #[test]
    fn test_memory_type_priority_ordering() {
        assert!(MemoryType::Preference.priority_weight() > MemoryType::Guideline.priority_weight());
        assert!(MemoryType::Guideline.priority_weight() > MemoryType::Knowledge.priority_weight());
        assert!(MemoryType::Knowledge.priority_weight() > MemoryType::Bugfix.priority_weight());
    }

    /// 测试 Display 实现
    #[test]
    fn test_memory_type_display() {
        assert_eq!(format!("{}", MemoryType::Guideline), "guideline");
        assert_eq!(format!("{}", MemoryType::Bugfix), "bugfix");
        assert_eq!(format!("{}", MemoryType::Knowledge), "knowledge");
        assert_eq!(format!("{}", MemoryType::Preference), "preference");
    }

    /// 测试 display_name 返回中文
    #[test]
    fn test_memory_type_display_name() {
        assert_eq!(MemoryType::Guideline.display_name(), "规范");
        assert_eq!(MemoryType::Bugfix.display_name(), "修复");
        assert_eq!(MemoryType::Knowledge.display_name(), "知识");
        assert_eq!(MemoryType::Preference.display_name(), "偏好");
    }

    /// 测试 all() 包含所有 4 种类型
    #[test]
    fn test_memory_type_all() {
        let all = MemoryType::all();
        assert_eq!(all.len(), 4);
        assert!(all.contains(&MemoryType::Guideline));
        assert!(all.contains(&MemoryType::Bugfix));
        assert!(all.contains(&MemoryType::Knowledge));
        assert!(all.contains(&MemoryType::Preference));
    }

    /// 测试按类型搜索
    #[test]
    fn test_search_by_type() {
        let mut store = MemoryStore::new(enabled_config());
        store.add("规范1", MemoryType::Guideline, vec![], 0.5);
        store.add("修复1", MemoryType::Bugfix, vec![], 0.5);
        store.add("规范2", MemoryType::Guideline, vec![], 0.5);
        store.add("知识1", MemoryType::Knowledge, vec![], 0.5);

        let guidelines = store.search_by_type(MemoryType::Guideline);
        assert_eq!(guidelines.len(), 2);

        let bugfixes = store.search_by_type(MemoryType::Bugfix);
        assert_eq!(bugfixes.len(), 1);

        let preferences = store.search_by_type(MemoryType::Preference);
        assert_eq!(preferences.len(), 0);
    }

    /// 测试优先级排序
    #[test]
    fn test_prioritized() {
        let mut store = MemoryStore::new(enabled_config());
        store.add("bug", MemoryType::Bugfix, vec![], 0.9);
        store.add("pref", MemoryType::Preference, vec![], 0.3);
        store.add("guide", MemoryType::Guideline, vec![], 0.5);
        store.add("knowledge", MemoryType::Knowledge, vec![], 0.7);

        let sorted = store.prioritized(4);
        assert_eq!(sorted.len(), 4);

        // Preference 类型优先级最高，应排在最前
        assert_eq!(sorted[0].memory_type, MemoryType::Preference);
        // Guideline 第二
        assert_eq!(sorted[1].memory_type, MemoryType::Guideline);
        // Knowledge 第三
        assert_eq!(sorted[2].memory_type, MemoryType::Knowledge);
        // Bugfix 最低（虽然 importance 0.9 最高，但类型优先级低）
        assert_eq!(sorted[3].memory_type, MemoryType::Bugfix);
    }

    /// 测试同类型内按 importance 排序
    #[test]
    fn test_prioritized_same_type() {
        let mut store = MemoryStore::new(enabled_config());
        store.add("低", MemoryType::Knowledge, vec![], 0.2);
        store.add("高", MemoryType::Knowledge, vec![], 0.9);
        store.add("中", MemoryType::Knowledge, vec![], 0.5);

        let sorted = store.prioritized(3);
        assert!(sorted[0].importance >= sorted[1].importance);
        assert!(sorted[1].importance >= sorted[2].importance);
    }
}

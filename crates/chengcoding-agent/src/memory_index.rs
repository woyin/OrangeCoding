//! # MEMORY.md 索引文件
//!
//! 维护记忆目录的索引文件，方便 Agent 快速查找可用记忆。
//!
//! # 设计思想
//! 参考 reference 中 MEMORY.md 的设计：
//! - 索引文件是记忆系统的入口点
//! - 限制行数和大小，防止索引本身消耗过多 token
//! - 按类型分组排序，方便定位
//! - 每次记忆变更后自动重建

use super::memdir::MemdirEntry;

/// 索引配置
pub const MAX_INDEX_LINES: usize = 200;
pub const MAX_INDEX_BYTES: usize = 25 * 1024; // 25KB

/// 索引行格式
///
/// 格式: `- [{type}] {name} — {description}`
fn format_index_line(entry: &MemdirEntry) -> String {
    if entry.description.is_empty() {
        format!("- [{}] {}", entry.entry_type, entry.name)
    } else {
        format!(
            "- [{}] {} — {}",
            entry.entry_type, entry.name, entry.description
        )
    }
}

/// 构建索引内容
///
/// 流程：
/// 1. 按类型分组
/// 2. 每组内按名称排序
/// 3. 输出分组标题和条目
/// 4. 截断到 MAX_INDEX_LINES 行
/// 5. 截断到 MAX_INDEX_BYTES 大小
pub fn build_index(entries: &[MemdirEntry]) -> String {
    if entries.is_empty() {
        return "# 记忆索引\n\n暂无记忆。\n".to_string();
    }

    // 按类型分组
    let mut groups: std::collections::BTreeMap<String, Vec<&MemdirEntry>> =
        std::collections::BTreeMap::new();

    for entry in entries {
        groups
            .entry(entry.entry_type.clone())
            .or_default()
            .push(entry);
    }

    // 每组内按名称排序
    for group in groups.values_mut() {
        group.sort_by(|a, b| a.name.cmp(&b.name));
    }

    let mut lines = Vec::new();
    lines.push("# 记忆索引".to_string());
    lines.push(String::new());
    lines.push(format!("共 {} 条记忆。", entries.len()));
    lines.push(String::new());

    for (type_name, group) in &groups {
        lines.push(format!("## {}", type_name));
        lines.push(String::new());
        for entry in group {
            lines.push(format_index_line(entry));
        }
        lines.push(String::new());
    }

    // 行数限制
    if lines.len() > MAX_INDEX_LINES {
        lines.truncate(MAX_INDEX_LINES);
        lines.push("...(已截断)".to_string());
    }

    let mut result = lines.join("\n");

    // 大小限制
    if result.len() > MAX_INDEX_BYTES {
        // 在字节边界内找到最后一个完整行
        let mut end = MAX_INDEX_BYTES;
        while end > 0 && !result.is_char_boundary(end) {
            end -= 1;
        }
        // 回退到最后一个换行符
        if let Some(last_nl) = result[..end].rfind('\n') {
            end = last_nl;
        }
        result.truncate(end);
        result.push_str("\n...(已截断至 25KB)");
    }

    result
}

/// 计算索引统计
pub fn index_stats(entries: &[MemdirEntry]) -> IndexStats {
    let mut types: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for entry in entries {
        *types.entry(entry.entry_type.clone()).or_default() += 1;
    }
    IndexStats {
        total_entries: entries.len(),
        by_type: types,
    }
}

/// 索引统计
#[derive(Clone, Debug)]
pub struct IndexStats {
    pub total_entries: usize,
    pub by_type: std::collections::HashMap<String, usize>,
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_entry(name: &str, entry_type: &str, desc: &str) -> MemdirEntry {
        MemdirEntry {
            name: name.to_string(),
            description: desc.to_string(),
            entry_type: entry_type.to_string(),
            content: String::new(),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_empty_index() {
        let index = build_index(&[]);
        assert!(index.contains("暂无记忆"));
    }

    #[test]
    fn test_single_entry_index() {
        let entries = vec![make_entry("规范", "guideline", "编码规范")];
        let index = build_index(&entries);
        assert!(index.contains("# 记忆索引"));
        assert!(index.contains("[guideline] 规范 — 编码规范"));
        assert!(index.contains("共 1 条"));
    }

    #[test]
    fn test_multiple_types_grouped() {
        let entries = vec![
            make_entry("A", "user", "用户A"),
            make_entry("B", "guideline", "规范B"),
            make_entry("C", "user", "用户C"),
        ];
        let index = build_index(&entries);
        assert!(index.contains("## guideline"));
        assert!(index.contains("## user"));
    }

    #[test]
    fn test_sorted_within_group() {
        let entries = vec![
            make_entry("Zebra", "user", "z"),
            make_entry("Apple", "user", "a"),
            make_entry("Mango", "user", "m"),
        ];
        let index = build_index(&entries);
        let apple_pos = index.find("Apple").unwrap();
        let mango_pos = index.find("Mango").unwrap();
        let zebra_pos = index.find("Zebra").unwrap();
        assert!(apple_pos < mango_pos);
        assert!(mango_pos < zebra_pos);
    }

    #[test]
    fn test_max_lines_limit() {
        let entries: Vec<MemdirEntry> = (0..300)
            .map(|i| make_entry(&format!("记忆{}", i), "user", "描述"))
            .collect();
        let index = build_index(&entries);
        let lines: Vec<&str> = index.lines().collect();
        assert!(lines.len() <= MAX_INDEX_LINES + 2); // +1 for truncation notice
    }

    #[test]
    fn test_max_bytes_limit() {
        let entries: Vec<MemdirEntry> = (0..500)
            .map(|i| {
                make_entry(
                    &format!("很长的记忆名称_{}", i),
                    "user",
                    &"非常长的描述".repeat(50),
                )
            })
            .collect();
        let index = build_index(&entries);
        // 允许截断标记额外字符
        assert!(
            index.len() < MAX_INDEX_BYTES + 100,
            "索引大小 {} 超过限制",
            index.len()
        );
    }

    #[test]
    fn test_empty_description() {
        let entries = vec![make_entry("简单", "user", "")];
        let index = build_index(&entries);
        assert!(index.contains("- [user] 简单"));
        assert!(!index.contains("—"));
    }

    #[test]
    fn test_index_stats() {
        let entries = vec![
            make_entry("A", "user", ""),
            make_entry("B", "guideline", ""),
            make_entry("C", "user", ""),
        ];
        let stats = index_stats(&entries);
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.by_type["user"], 2);
        assert_eq!(stats.by_type["guideline"], 1);
    }

    #[test]
    fn test_format_index_line() {
        let entry = make_entry("测试", "user", "描述");
        assert_eq!(format_index_line(&entry), "- [user] 测试 — 描述");
    }

    #[test]
    fn test_format_index_line_no_description() {
        let entry = make_entry("测试", "user", "");
        assert_eq!(format_index_line(&entry), "- [user] 测试");
    }
}

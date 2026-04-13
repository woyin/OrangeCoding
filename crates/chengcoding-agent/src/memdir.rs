//! # Markdown + Frontmatter 存储格式
//!
//! 每个记忆保存为一个 Markdown 文件，元数据存放在 Frontmatter 中。
//!
//! # 设计思想
//! 参考 reference 中 Memdir 的设计：
//! - 人类可读：直接用编辑器查看和修改记忆
//! - 版本控制友好：每个记忆一个文件，易于 diff
//! - Frontmatter 使用 `---` 分隔符手动解析（不引入 YAML 库）
//! - 文件名安全化：过滤特殊字符，防止路径注入

use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 记忆条目
// ---------------------------------------------------------------------------

/// Memdir 记忆条目
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemdirEntry {
    /// 记忆名称
    pub name: String,
    /// 简要描述
    pub description: String,
    /// 记忆类型（如 "user", "system", "guideline"）
    pub entry_type: String,
    /// 详细内容（Markdown 格式）
    pub content: String,
    /// 额外元数据
    pub metadata: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Frontmatter 解析
// ---------------------------------------------------------------------------

/// 解析 Markdown 文件的 Frontmatter 和内容
///
/// 格式：
/// ```text
/// ---
/// key: "value"
/// ---
/// 正文内容
/// ```
///
/// 为什么手动解析而非使用 YAML 库：
/// - Frontmatter 格式简单（key: value 对）
/// - 避免引入重量级依赖
/// - 保持解析逻辑透明可调试
pub fn parse_frontmatter(text: &str) -> (HashMap<String, String>, String) {
    let mut metadata = HashMap::new();

    // 检查是否以 --- 开头
    let trimmed = text.trim_start();
    if !trimmed.starts_with("---") {
        return (metadata, text.to_string());
    }

    // 找到第二个 ---
    let after_first = &trimmed[3..].trim_start_matches('\n');
    if let Some(end_pos) = after_first.find("\n---") {
        let frontmatter = &after_first[..end_pos];
        let content_start = end_pos + 4; // skip "\n---"
        let content = after_first[content_start..].trim_start_matches('\n');

        // 解析 key: value 对
        for line in frontmatter.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim().to_string();
                let value = value.trim().trim_matches('"').to_string();
                metadata.insert(key, value);
            }
        }

        (metadata, content.to_string())
    } else {
        // 没有结束分隔符，视为无 frontmatter
        (metadata, text.to_string())
    }
}

/// 将 Frontmatter 和内容序列化为 Markdown 文件格式
pub fn serialize_frontmatter(metadata: &HashMap<String, String>, content: &str) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());

    // 按 key 排序保证输出稳定
    let mut keys: Vec<&String> = metadata.keys().collect();
    keys.sort();

    for key in keys {
        let value = &metadata[key];
        lines.push(format!("{}: \"{}\"", key, value));
    }

    lines.push("---".to_string());
    lines.push(String::new());
    lines.push(content.to_string());

    lines.join("\n")
}

/// 将 MemdirEntry 转为 Markdown 文件内容
pub fn entry_to_markdown(entry: &MemdirEntry) -> String {
    let mut meta = entry.metadata.clone();
    meta.insert("name".to_string(), entry.name.clone());
    meta.insert("description".to_string(), entry.description.clone());
    meta.insert("type".to_string(), entry.entry_type.clone());

    serialize_frontmatter(&meta, &entry.content)
}

/// 从 Markdown 文件内容解析 MemdirEntry
pub fn markdown_to_entry(text: &str) -> Option<MemdirEntry> {
    let (mut meta, content) = parse_frontmatter(text);

    let name = meta.remove("name")?;
    let description = meta.remove("description").unwrap_or_default();
    let entry_type = meta.remove("type").unwrap_or_else(|| "user".to_string());

    Some(MemdirEntry {
        name,
        description,
        entry_type,
        content,
        metadata: meta,
    })
}

// ---------------------------------------------------------------------------
// 文件名安全化
// ---------------------------------------------------------------------------

/// 将名称转为安全的文件名
///
/// 过滤规则：
/// - 只保留字母、数字、中文字符、连字符、下划线
/// - 空格替换为连字符
/// - 连续连字符合并
/// - 最大长度 100 字符
pub fn sanitize_filename(name: &str) -> String {
    let mut result = String::new();

    for c in name.chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' || c >= '\u{4e00}' && c <= '\u{9fff}' {
            result.push(c);
        } else if c == ' ' {
            result.push('-');
        }
        // 其他字符直接丢弃
    }

    // 合并连续连字符
    while result.contains("--") {
        result = result.replace("--", "-");
    }

    // 去除首尾连字符
    let result = result.trim_matches('-').to_string();

    // 截断长度
    if result.chars().count() > 100 {
        result.chars().take(100).collect()
    } else if result.is_empty() {
        "unnamed".to_string()
    } else {
        result
    }
}

/// 生成记忆文件路径
pub fn memory_file_path(base_dir: &Path, name: &str) -> PathBuf {
    let filename = format!("{}.md", sanitize_filename(name));
    base_dir.join(filename)
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Frontmatter 解析测试 ---

    #[test]
    fn test_parse_simple_frontmatter() {
        let text = "---\nname: \"测试记忆\"\ntype: \"user\"\n---\n\n## 内容\n正文";
        let (meta, content) = parse_frontmatter(text);
        assert_eq!(meta.get("name").unwrap(), "测试记忆");
        assert_eq!(meta.get("type").unwrap(), "user");
        assert!(content.contains("正文"));
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let text = "这是普通文本";
        let (meta, content) = parse_frontmatter(text);
        assert!(meta.is_empty());
        assert_eq!(content, "这是普通文本");
    }

    #[test]
    fn test_parse_empty_frontmatter() {
        let text = "---\n---\n\n内容";
        let (meta, content) = parse_frontmatter(text);
        assert!(meta.is_empty());
        assert!(content.contains("内容"));
    }

    #[test]
    fn test_parse_unquoted_values() {
        let text = "---\nname: 无引号值\n---\n\n内容";
        let (meta, _) = parse_frontmatter(text);
        assert_eq!(meta.get("name").unwrap(), "无引号值");
    }

    // --- 序列化测试 ---

    #[test]
    fn test_serialize_frontmatter() {
        let mut meta = HashMap::new();
        meta.insert("name".to_string(), "test".to_string());
        meta.insert("type".to_string(), "user".to_string());
        let result = serialize_frontmatter(&meta, "内容");
        assert!(result.starts_with("---\n"));
        assert!(result.contains("name: \"test\""));
        assert!(result.contains("type: \"user\""));
        assert!(result.ends_with("内容"));
    }

    #[test]
    fn test_roundtrip() {
        let entry = MemdirEntry {
            name: "测试".into(),
            description: "描述".into(),
            entry_type: "guideline".into(),
            content: "## 详细内容\n这是正文".into(),
            metadata: HashMap::new(),
        };
        let md = entry_to_markdown(&entry);
        let parsed = markdown_to_entry(&md).unwrap();
        assert_eq!(parsed.name, entry.name);
        assert_eq!(parsed.description, entry.description);
        assert_eq!(parsed.entry_type, entry.entry_type);
        assert_eq!(parsed.content, entry.content);
    }

    #[test]
    fn test_markdown_to_entry_missing_name() {
        let text = "---\ntype: \"user\"\n---\n\n内容";
        let result = markdown_to_entry(text);
        assert!(result.is_none());
    }

    #[test]
    fn test_markdown_to_entry_defaults() {
        let text = "---\nname: \"记忆\"\n---\n\n内容";
        let entry = markdown_to_entry(text).unwrap();
        assert_eq!(entry.name, "记忆");
        assert_eq!(entry.description, "");
        assert_eq!(entry.entry_type, "user"); // 默认类型
    }

    // --- 文件名安全化测试 ---

    #[test]
    fn test_sanitize_simple() {
        assert_eq!(sanitize_filename("hello world"), "hello-world");
    }

    #[test]
    fn test_sanitize_chinese() {
        assert_eq!(sanitize_filename("代码规范"), "代码规范");
    }

    #[test]
    fn test_sanitize_special_chars() {
        assert_eq!(sanitize_filename("a/b\\c:d"), "abcd");
    }

    #[test]
    fn test_sanitize_empty() {
        assert_eq!(sanitize_filename(""), "unnamed");
    }

    #[test]
    fn test_sanitize_only_special() {
        assert_eq!(sanitize_filename("///"), "unnamed");
    }

    #[test]
    fn test_sanitize_consecutive_dashes() {
        assert_eq!(sanitize_filename("a  b   c"), "a-b-c");
    }

    #[test]
    fn test_sanitize_long_name() {
        let long = "a".repeat(200);
        let result = sanitize_filename(&long);
        assert!(result.chars().count() <= 100);
    }

    // --- memory_file_path 测试 ---

    #[test]
    fn test_memory_file_path() {
        let path = memory_file_path(Path::new("/home/user/.chengcoding/memory"), "代码规范");
        assert_eq!(
            path,
            PathBuf::from("/home/user/.chengcoding/memory/代码规范.md")
        );
    }

    // --- MemdirEntry 额外元数据测试 ---

    #[test]
    fn test_extra_metadata_preserved() {
        let mut meta = HashMap::new();
        meta.insert("tags".to_string(), "rust,testing".to_string());

        let entry = MemdirEntry {
            name: "test".into(),
            description: "desc".into(),
            entry_type: "user".into(),
            content: "content".into(),
            metadata: meta,
        };

        let md = entry_to_markdown(&entry);
        let parsed = markdown_to_entry(&md).unwrap();
        assert_eq!(parsed.metadata.get("tags").unwrap(), "rust,testing");
    }
}

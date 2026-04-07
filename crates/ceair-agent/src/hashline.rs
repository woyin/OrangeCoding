//! # Hashline 编辑模块
//!
//! 基于内容哈希的精确代码编辑定位。每行内容生成唯一哈希锚点，
//! 实现无歧义的行级编辑操作。

use ring::digest;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 数据结构
// ---------------------------------------------------------------------------

/// 行哈希 — 包含行号、内容哈希和原始内容
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LineHash {
    /// 行号（从 1 开始）
    pub line_number: usize,
    /// 内容哈希（SHA-256 前 8 个十六进制字符）
    pub hash: String,
    /// 行内容
    pub content: String,
}

/// 编辑操作
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HashlineEdit {
    /// 目标行的哈希锚点
    pub anchor_hash: String,
    /// 操作类型
    pub operation: EditOperation,
}

/// 编辑操作类型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EditOperation {
    /// 替换行内容
    Replace(String),
    /// 在行后插入
    InsertAfter(String),
    /// 在行前插入
    InsertBefore(String),
    /// 删除行
    Delete,
}

/// Hashline 编辑错误
#[derive(Debug, thiserror::Error)]
pub enum HashlineError {
    /// 找不到指定的哈希锚点
    #[error("找不到哈希锚点: {0}")]
    AnchorNotFound(String),
    /// 多行具有相同的哈希值
    #[error("哈希冲突: {0}")]
    HashCollision(String),
}

// ---------------------------------------------------------------------------
// Hashline 编辑器
// ---------------------------------------------------------------------------

/// Hashline 编辑器 — 基于内容哈希的精确行定位与编辑
pub struct HashlineEditor;

impl HashlineEditor {
    /// 计算行内容的 SHA-256 哈希（取前 8 个十六进制字符）
    pub fn hash_line(content: &str) -> String {
        let hash = digest::digest(&digest::SHA256, content.as_bytes());
        hash.as_ref()
            .iter()
            .take(4) // 4 字节 = 8 个十六进制字符
            .map(|b| format!("{:02x}", b))
            .collect()
    }

    /// 为文件内容生成所有行哈希
    pub fn hash_file(content: &str) -> Vec<LineHash> {
        content
            .lines()
            .enumerate()
            .map(|(i, line)| LineHash {
                line_number: i + 1,
                hash: Self::hash_line(line),
                content: line.to_string(),
            })
            .collect()
    }

    /// 查找匹配哈希的行
    pub fn find_line<'a>(lines: &'a [LineHash], hash: &str) -> Option<&'a LineHash> {
        lines.iter().find(|l| l.hash == hash)
    }

    /// 应用编辑操作到文件内容
    ///
    /// 从后向前处理以避免行号偏移问题。
    pub fn apply_edits(content: &str, edits: &[HashlineEdit]) -> Result<String, HashlineError> {
        let line_hashes = Self::hash_file(content);
        let mut lines: Vec<String> = content.lines().map(String::from).collect();

        // 收集每个编辑对应的行索引，检查锚点有效性
        let mut indexed_edits: Vec<(usize, &HashlineEdit)> = Vec::new();

        for edit in edits {
            let matching: Vec<usize> = line_hashes
                .iter()
                .filter(|lh| lh.hash == edit.anchor_hash)
                .map(|lh| lh.line_number - 1) // 转为 0-based 索引
                .collect();

            match matching.len() {
                0 => return Err(HashlineError::AnchorNotFound(edit.anchor_hash.clone())),
                1 => indexed_edits.push((matching[0], edit)),
                _ => return Err(HashlineError::HashCollision(edit.anchor_hash.clone())),
            }
        }

        // 从后向前排序，避免索引偏移
        indexed_edits.sort_by(|a, b| b.0.cmp(&a.0));

        for (idx, edit) in indexed_edits {
            match &edit.operation {
                EditOperation::Replace(new_content) => {
                    lines[idx] = new_content.clone();
                }
                EditOperation::InsertAfter(new_content) => {
                    lines.insert(idx + 1, new_content.clone());
                }
                EditOperation::InsertBefore(new_content) => {
                    lines.insert(idx, new_content.clone());
                }
                EditOperation::Delete => {
                    lines.remove(idx);
                }
            }
        }

        Ok(lines.join("\n"))
    }

    /// 格式化带哈希的文件预览
    pub fn format_with_hashes(content: &str) -> String {
        let hashes = Self::hash_file(content);
        hashes
            .iter()
            .map(|lh| format!("[{}] {}: {}", lh.hash, lh.line_number, lh.content))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // 哈希计算测试
    // -----------------------------------------------------------------------

    /// 测试相同输入产生相同哈希（确定性）
    #[test]
    fn test_hash_line_deterministic() {
        let h1 = HashlineEditor::hash_line("fn main() {}");
        let h2 = HashlineEditor::hash_line("fn main() {}");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 8); // SHA-256 前 8 个十六进制字符
    }

    /// 测试不同输入产生不同哈希
    #[test]
    fn test_hash_line_different() {
        let h1 = HashlineEditor::hash_line("fn main() {}");
        let h2 = HashlineEditor::hash_line("fn test() {}");
        assert_ne!(h1, h2);
    }

    /// 测试文件行哈希的行号正确性
    #[test]
    fn test_hash_file_line_numbers() {
        let content = "line one\nline two\nline three";
        let hashes = HashlineEditor::hash_file(content);

        assert_eq!(hashes.len(), 3);
        assert_eq!(hashes[0].line_number, 1);
        assert_eq!(hashes[1].line_number, 2);
        assert_eq!(hashes[2].line_number, 3);
        assert_eq!(hashes[0].content, "line one");
        assert_eq!(hashes[2].content, "line three");
    }

    // -----------------------------------------------------------------------
    // 行查找测试
    // -----------------------------------------------------------------------

    /// 测试通过哈希查找行
    #[test]
    fn test_find_line_by_hash() {
        let content = "alpha\nbeta\ngamma";
        let hashes = HashlineEditor::hash_file(content);
        let target_hash = HashlineEditor::hash_line("beta");

        let found = HashlineEditor::find_line(&hashes, &target_hash);
        assert!(found.is_some());
        assert_eq!(found.unwrap().content, "beta");
        assert_eq!(found.unwrap().line_number, 2);
    }

    /// 测试查找不存在的哈希
    #[test]
    fn test_find_line_not_found() {
        let content = "alpha\nbeta";
        let hashes = HashlineEditor::hash_file(content);

        let found = HashlineEditor::find_line(&hashes, "deadbeef");
        assert!(found.is_none());
    }

    // -----------------------------------------------------------------------
    // 编辑操作测试
    // -----------------------------------------------------------------------

    /// 测试替换操作
    #[test]
    fn test_apply_replace() {
        let content = "fn old() {}\nfn keep() {}";
        let hash = HashlineEditor::hash_line("fn old() {}");

        let edits = vec![HashlineEdit {
            anchor_hash: hash,
            operation: EditOperation::Replace("fn new() {}".to_string()),
        }];

        let result = HashlineEditor::apply_edits(content, &edits).unwrap();
        assert!(result.contains("fn new() {}"));
        assert!(result.contains("fn keep() {}"));
        assert!(!result.contains("fn old() {}"));
    }

    /// 测试在行后插入
    #[test]
    fn test_apply_insert_after() {
        let content = "use std::io;\nfn main() {}";
        let hash = HashlineEditor::hash_line("use std::io;");

        let edits = vec![HashlineEdit {
            anchor_hash: hash,
            operation: EditOperation::InsertAfter("use std::fs;".to_string()),
        }];

        let result = HashlineEditor::apply_edits(content, &edits).unwrap();
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "use std::io;");
        assert_eq!(lines[1], "use std::fs;");
        assert_eq!(lines[2], "fn main() {}");
    }

    /// 测试在行前插入
    #[test]
    fn test_apply_insert_before() {
        let content = "fn main() {}\n    println!(\"hello\");";
        let hash = HashlineEditor::hash_line("fn main() {}");

        let edits = vec![HashlineEdit {
            anchor_hash: hash,
            operation: EditOperation::InsertBefore("// 主函数".to_string()),
        }];

        let result = HashlineEditor::apply_edits(content, &edits).unwrap();
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "// 主函数");
        assert_eq!(lines[1], "fn main() {}");
    }

    /// 测试删除操作
    #[test]
    fn test_apply_delete() {
        let content = "keep this\ndelete this\nalso keep";
        let hash = HashlineEditor::hash_line("delete this");

        let edits = vec![HashlineEdit {
            anchor_hash: hash,
            operation: EditOperation::Delete,
        }];

        let result = HashlineEditor::apply_edits(content, &edits).unwrap();
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "keep this");
        assert_eq!(lines[1], "also keep");
    }

    /// 测试多个编辑同时应用
    #[test]
    fn test_apply_multiple_edits() {
        let content = "aaa\nbbb\nccc\nddd";
        let hash_bbb = HashlineEditor::hash_line("bbb");
        let hash_ddd = HashlineEditor::hash_line("ddd");

        let edits = vec![
            HashlineEdit {
                anchor_hash: hash_bbb,
                operation: EditOperation::Replace("BBB".to_string()),
            },
            HashlineEdit {
                anchor_hash: hash_ddd,
                operation: EditOperation::Delete,
            },
        ];

        let result = HashlineEditor::apply_edits(content, &edits).unwrap();
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "aaa");
        assert_eq!(lines[1], "BBB");
        assert_eq!(lines[2], "ccc");
    }

    // -----------------------------------------------------------------------
    // 格式化测试
    // -----------------------------------------------------------------------

    /// 测试带哈希的文件预览格式
    #[test]
    fn test_format_with_hashes() {
        let content = "hello\nworld";
        let formatted = HashlineEditor::format_with_hashes(content);

        let hash_hello = HashlineEditor::hash_line("hello");
        let hash_world = HashlineEditor::hash_line("world");

        assert!(formatted.contains(&format!("[{}] 1: hello", hash_hello)));
        assert!(formatted.contains(&format!("[{}] 2: world", hash_world)));
    }

    // -----------------------------------------------------------------------
    // 错误处理测试
    // -----------------------------------------------------------------------

    /// 测试锚点不存在时返回错误
    #[test]
    fn test_anchor_not_found_error() {
        let content = "existing line";
        let edits = vec![HashlineEdit {
            anchor_hash: "deadbeef".to_string(),
            operation: EditOperation::Delete,
        }];

        let result = HashlineEditor::apply_edits(content, &edits);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, HashlineError::AnchorNotFound(_)));
        assert!(err.to_string().contains("deadbeef"));
    }

    // -----------------------------------------------------------------------
    // 边界情况测试
    // -----------------------------------------------------------------------

    /// 测试空文件
    #[test]
    fn test_empty_file() {
        let hashes = HashlineEditor::hash_file("");
        // 空字符串经过 lines() 会产生一个空行
        // 但空内容不应有任何哈希行
        assert!(hashes.is_empty() || hashes.len() == 1);

        // 空编辑列表应成功
        let result = HashlineEditor::apply_edits("some content", &[]);
        assert!(result.is_ok());
    }
}

//! # 压缩后重注入
//!
//! 压缩完成后自动恢复关键上下文：
//! - 最近读取的文件内容
//! - 活跃的工作计划
//!
//! # 设计思想
//! 参考 reference 中 compaction 后的 re-injection 策略：
//! - 压缩会丢失具体文件内容，但 Agent 需要这些信息继续工作
//! - 按时间排序（最新优先），确保最相关的文件被恢复
//! - 多级预算控制：单文件上限 + 总量上限 + 文件数上限

/// 重注入配置
#[derive(Clone, Debug)]
pub struct ReinjectionConfig {
    /// 最多注入的文件数
    pub max_files: usize,
    /// 每个文件的最大 token 数（粗估：1 token ≈ 4 字符）
    pub max_tokens_per_file: usize,
    /// 总注入预算（token 数）
    pub total_budget: usize,
}

impl Default for ReinjectionConfig {
    fn default() -> Self {
        Self {
            max_files: 5,
            max_tokens_per_file: 5_000,
            total_budget: 50_000,
        }
    }
}

/// 可注入的文件内容
#[derive(Clone, Debug)]
pub struct FileContent {
    /// 文件路径
    pub path: String,
    /// 文件内容
    pub content: String,
    /// 最后读取时间戳（Unix 秒）
    pub last_read_ts: u64,
}

/// 重注入项
#[derive(Clone, Debug)]
pub struct ReinjectionItem {
    /// 文件路径
    pub path: String,
    /// 截断后的内容
    pub content: String,
    /// 预估 token 数
    pub estimated_tokens: usize,
}

/// 估算 token 数（粗估：1 token ≈ 4 字符）
fn estimate_tokens(text: &str) -> usize {
    (text.len() + 3) / 4
}

/// 截断文本到指定 token 数
fn truncate_to_tokens(text: &str, max_tokens: usize) -> String {
    let max_chars = max_tokens * 4;
    if text.len() <= max_chars {
        text.to_string()
    } else {
        // 在字符边界截断
        let truncated: String = text.chars().take(max_chars).collect();
        format!("{}...\n[内容已截断至约 {} tokens]", truncated, max_tokens)
    }
}

/// 执行重注入
///
/// 流程：
/// 1. 按最后读取时间排序（最新优先）
/// 2. 取前 max_files 个文件
/// 3. 每个文件截断到 max_tokens_per_file
/// 4. 总量不超过 total_budget
pub fn reinject(files: &[FileContent], config: &ReinjectionConfig) -> Vec<ReinjectionItem> {
    if files.is_empty() {
        return Vec::new();
    }

    // 按时间排序（最新优先）
    let mut sorted: Vec<&FileContent> = files.iter().collect();
    sorted.sort_by(|a, b| b.last_read_ts.cmp(&a.last_read_ts));

    let mut result = Vec::new();
    let mut total_tokens = 0;

    for file in sorted.iter().take(config.max_files) {
        // 截断到单文件上限
        let truncated = truncate_to_tokens(&file.content, config.max_tokens_per_file);
        let tokens = estimate_tokens(&truncated);

        // 检查总预算
        if total_tokens + tokens > config.total_budget {
            break;
        }

        total_tokens += tokens;
        result.push(ReinjectionItem {
            path: file.path.clone(),
            content: truncated,
            estimated_tokens: tokens,
        });
    }

    result
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_file(path: &str, content: &str, ts: u64) -> FileContent {
        FileContent {
            path: path.to_string(),
            content: content.to_string(),
            last_read_ts: ts,
        }
    }

    #[test]
    fn test_no_files() {
        let result = reinject(&[], &ReinjectionConfig::default());
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_small_file() {
        let files = vec![make_file("src/main.rs", "fn main() {}", 100)];
        let result = reinject(&files, &ReinjectionConfig::default());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "src/main.rs");
        assert_eq!(result[0].content, "fn main() {}");
    }

    #[test]
    fn test_max_files_limit() {
        let files: Vec<FileContent> = (0..10)
            .map(|i| make_file(&format!("file{}.rs", i), "content", i as u64))
            .collect();

        let config = ReinjectionConfig {
            max_files: 3,
            ..Default::default()
        };
        let result = reinject(&files, &config);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_sorted_by_time_newest_first() {
        let files = vec![
            make_file("old.rs", "old", 10),
            make_file("new.rs", "new", 100),
            make_file("mid.rs", "mid", 50),
        ];
        let result = reinject(&files, &ReinjectionConfig::default());
        assert_eq!(result[0].path, "new.rs");
        assert_eq!(result[1].path, "mid.rs");
        assert_eq!(result[2].path, "old.rs");
    }

    #[test]
    fn test_truncate_large_file() {
        // 生成一个超大文件内容
        let content = "x".repeat(100_000);
        let files = vec![make_file("big.rs", &content, 100)];

        let config = ReinjectionConfig {
            max_tokens_per_file: 100, // 约 400 字符
            ..Default::default()
        };
        let result = reinject(&files, &config);
        assert_eq!(result.len(), 1);
        assert!(result[0].content.len() < content.len());
        assert!(result[0].content.contains("[内容已截断"));
    }

    #[test]
    fn test_total_budget_limit() {
        // 每个文件约 25 tokens (100 字符)
        let files: Vec<FileContent> = (0..20)
            .map(|i| make_file(&format!("f{}.rs", i), &"x".repeat(100), i as u64))
            .collect();

        let config = ReinjectionConfig {
            max_files: 20,
            max_tokens_per_file: 5_000,
            total_budget: 100, // 只允许约 4 个文件
        };
        let result = reinject(&files, &config);
        // 不应包含所有 20 个文件
        assert!(result.len() < 20);
        // 总 token 不超过预算
        let total: usize = result.iter().map(|r| r.estimated_tokens).sum();
        assert!(total <= config.total_budget);
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("hi"), 1); // (2+3)/4 = 1
        assert_eq!(estimate_tokens("abcd"), 1); // (4+3)/4 = 1
        assert_eq!(estimate_tokens("abcde"), 2); // (5+3)/4 = 2
    }

    #[test]
    fn test_truncate_to_tokens_no_truncation() {
        let result = truncate_to_tokens("short", 1000);
        assert_eq!(result, "short");
    }

    #[test]
    fn test_truncate_to_tokens_truncated() {
        let long = "a".repeat(1000);
        let result = truncate_to_tokens(&long, 10); // 约 40 字符
        assert!(result.len() < 1000);
        assert!(result.contains("[内容已截断"));
    }

    #[test]
    fn test_default_config() {
        let config = ReinjectionConfig::default();
        assert_eq!(config.max_files, 5);
        assert_eq!(config.max_tokens_per_file, 5_000);
        assert_eq!(config.total_budget, 50_000);
    }

    #[test]
    fn test_reinjection_item_estimated_tokens() {
        let files = vec![make_file("test.rs", "hello world", 100)];
        let result = reinject(&files, &ReinjectionConfig::default());
        assert_eq!(result[0].estimated_tokens, estimate_tokens("hello world"));
    }
}

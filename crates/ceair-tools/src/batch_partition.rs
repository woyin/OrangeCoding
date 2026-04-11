//! # 工具调用并发执行分区
//!
//! 将工具调用列表按读写安全性分区：
//! - 只读/并发安全工具组成并发批次
//! - 写入/非安全工具独立成单元素串行批次
//!
//! # 设计思想
//! 参考 reference 中 execute_batch 的设计：
//! - 利用 ToolMetadata.is_concurrency_safe 判断工具是否可并发
//! - 连续的并发安全工具合并到同一批次
//! - 非安全工具切断并发链，独立执行
//! - 保持原始顺序不变（稳定分区）

/// 工具调用信息（用于分区判断）
#[derive(Clone, Debug)]
pub struct ToolCallInfo {
    /// 工具名称
    pub tool_name: String,
    /// 调用 ID
    pub call_id: String,
    /// 是否并发安全
    pub is_concurrency_safe: bool,
}

/// 执行批次
///
/// 一组工具调用，标记为并发或串行执行
#[derive(Clone, Debug)]
pub struct ExecutionBatch {
    /// 批次内的工具调用
    pub calls: Vec<ToolCallInfo>,
    /// 是否可并发执行
    pub concurrent: bool,
}

impl ExecutionBatch {
    /// 批次大小
    pub fn len(&self) -> usize {
        self.calls.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }
}

/// 将工具调用列表分区为执行批次
///
/// 分区规则：
/// - 按原始顺序扫描
/// - 连续的 concurrency_safe 工具合并为一个并发批次
/// - 非安全工具单独成一个串行批次（每批只有一个调用）
/// - 保证执行顺序语义正确：写入操作不会与相邻操作并发
pub fn partition_tool_calls(calls: Vec<ToolCallInfo>) -> Vec<ExecutionBatch> {
    if calls.is_empty() {
        return Vec::new();
    }

    let mut batches: Vec<ExecutionBatch> = Vec::new();
    let mut current_concurrent: Vec<ToolCallInfo> = Vec::new();

    for call in calls {
        if call.is_concurrency_safe {
            // 并发安全工具，加入当前并发组
            current_concurrent.push(call);
        } else {
            // 非安全工具：先刷出之前的并发组，再独立成批
            if !current_concurrent.is_empty() {
                batches.push(ExecutionBatch {
                    calls: std::mem::take(&mut current_concurrent),
                    concurrent: true,
                });
            }
            batches.push(ExecutionBatch {
                calls: vec![call],
                concurrent: false,
            });
        }
    }

    // 刷出最后的并发组
    if !current_concurrent.is_empty() {
        batches.push(ExecutionBatch {
            calls: current_concurrent,
            concurrent: true,
        });
    }

    batches
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn safe_call(name: &str) -> ToolCallInfo {
        ToolCallInfo {
            tool_name: name.to_string(),
            call_id: format!("id-{}", name),
            is_concurrency_safe: true,
        }
    }

    fn unsafe_call(name: &str) -> ToolCallInfo {
        ToolCallInfo {
            tool_name: name.to_string(),
            call_id: format!("id-{}", name),
            is_concurrency_safe: false,
        }
    }

    #[test]
    fn test_empty_list() {
        let result = partition_tool_calls(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_all_readonly_single_concurrent_batch() {
        let calls = vec![
            safe_call("read_file"),
            safe_call("grep"),
            safe_call("glob"),
        ];
        let batches = partition_tool_calls(calls);
        assert_eq!(batches.len(), 1);
        assert!(batches[0].concurrent);
        assert_eq!(batches[0].len(), 3);
    }

    #[test]
    fn test_all_write_separate_batches() {
        let calls = vec![
            unsafe_call("edit_file"),
            unsafe_call("bash"),
            unsafe_call("write_file"),
        ];
        let batches = partition_tool_calls(calls);
        assert_eq!(batches.len(), 3);
        for batch in &batches {
            assert!(!batch.concurrent);
            assert_eq!(batch.len(), 1);
        }
    }

    #[test]
    fn test_mixed_tools_partition() {
        let calls = vec![
            safe_call("read_file"),
            safe_call("grep"),
            unsafe_call("edit_file"),
            safe_call("read_file"),
            unsafe_call("bash"),
        ];
        let batches = partition_tool_calls(calls);
        // [read_file, grep](并发) → [edit_file](串行) → [read_file](并发) → [bash](串行)
        assert_eq!(batches.len(), 4);
        assert!(batches[0].concurrent);
        assert_eq!(batches[0].len(), 2);
        assert!(!batches[1].concurrent);
        assert_eq!(batches[1].len(), 1);
        assert!(batches[2].concurrent);
        assert_eq!(batches[2].len(), 1);
        assert!(!batches[3].concurrent);
        assert_eq!(batches[3].len(), 1);
    }

    #[test]
    fn test_single_safe_call() {
        let calls = vec![safe_call("read_file")];
        let batches = partition_tool_calls(calls);
        assert_eq!(batches.len(), 1);
        assert!(batches[0].concurrent);
        assert_eq!(batches[0].len(), 1);
    }

    #[test]
    fn test_single_unsafe_call() {
        let calls = vec![unsafe_call("bash")];
        let batches = partition_tool_calls(calls);
        assert_eq!(batches.len(), 1);
        assert!(!batches[0].concurrent);
        assert_eq!(batches[0].len(), 1);
    }

    #[test]
    fn test_safe_at_end() {
        let calls = vec![
            unsafe_call("bash"),
            safe_call("read_file"),
            safe_call("grep"),
        ];
        let batches = partition_tool_calls(calls);
        assert_eq!(batches.len(), 2);
        assert!(!batches[0].concurrent);
        assert_eq!(batches[0].calls[0].tool_name, "bash");
        assert!(batches[1].concurrent);
        assert_eq!(batches[1].len(), 2);
    }

    #[test]
    fn test_preserves_order() {
        let calls = vec![
            safe_call("a"),
            safe_call("b"),
            unsafe_call("c"),
            safe_call("d"),
        ];
        let batches = partition_tool_calls(calls);
        assert_eq!(batches[0].calls[0].tool_name, "a");
        assert_eq!(batches[0].calls[1].tool_name, "b");
        assert_eq!(batches[1].calls[0].tool_name, "c");
        assert_eq!(batches[2].calls[0].tool_name, "d");
    }

    #[test]
    fn test_batch_is_empty() {
        let batch = ExecutionBatch {
            calls: vec![],
            concurrent: true,
        };
        assert!(batch.is_empty());
    }

    #[test]
    fn test_alternating_safe_unsafe() {
        let calls = vec![
            safe_call("a"),
            unsafe_call("b"),
            safe_call("c"),
            unsafe_call("d"),
            safe_call("e"),
        ];
        let batches = partition_tool_calls(calls);
        // [a](并发) → [b](串行) → [c](并发) → [d](串行) → [e](并发)
        assert_eq!(batches.len(), 5);
        assert!(batches[0].concurrent);
        assert!(!batches[1].concurrent);
        assert!(batches[2].concurrent);
        assert!(!batches[3].concurrent);
        assert!(batches[4].concurrent);
    }
}

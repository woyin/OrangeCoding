//! # 取消令牌层级系统
//!
//! 提供父子级联的任务取消机制。
//!
//! # 设计思想
//! 参考 reference 中的 AbortController 层级：
//! - 父任务取消时自动取消所有子任务
//! - 子任务取消不影响父任务
//! - 使用 AtomicBool + 回调链实现零开销检查
//! - 适用于 Agent → SubAgent → Tool 的取消传播

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// CancellationToken
// ---------------------------------------------------------------------------

/// 取消令牌 — 线程安全的取消信号传播器
///
/// 支持父子层级：父令牌取消时自动传播到所有子令牌。
/// 使用 AtomicBool 实现极低开销的取消状态检查。
#[derive(Clone)]
pub struct CancellationToken {
    inner: Arc<CancellationTokenInner>,
}

struct CancellationTokenInner {
    /// 是否已取消
    cancelled: AtomicBool,
    /// 子令牌列表（取消时级联通知）
    children: Mutex<Vec<CancellationToken>>,
    /// 取消原因
    reason: Mutex<Option<String>>,
}

impl CancellationToken {
    /// 创建新的根令牌
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CancellationTokenInner {
                cancelled: AtomicBool::new(false),
                children: Mutex::new(Vec::new()),
                reason: Mutex::new(None),
            }),
        }
    }

    /// 创建子令牌
    ///
    /// 子令牌继承父令牌的取消状态：
    /// - 如果父已取消，子令牌立即标记为已取消
    /// - 父令牌取消时，子令牌自动取消
    /// - 子令牌取消不影响父令牌
    pub fn child(&self) -> CancellationToken {
        let child = CancellationToken::new();

        // 如果父已取消，子立即取消
        if self.is_cancelled() {
            child.inner.cancelled.store(true, Ordering::SeqCst);
            let reason = self.inner.reason.lock().unwrap().clone();
            *child.inner.reason.lock().unwrap() = reason;
        }

        // 注册到父的子列表
        self.inner.children.lock().unwrap().push(child.clone());

        child
    }

    /// 取消此令牌及所有子令牌
    pub fn cancel(&self) {
        self.cancel_with_reason("cancelled");
    }

    /// 带原因取消
    pub fn cancel_with_reason(&self, reason: &str) {
        // 使用 compare_exchange 确保只执行一次
        if self
            .inner
            .cancelled
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            *self.inner.reason.lock().unwrap() = Some(reason.to_string());

            // 级联取消所有子令牌
            let children = self.inner.children.lock().unwrap();
            for child in children.iter() {
                child.cancel_with_reason(reason);
            }
        }
    }

    /// 检查是否已取消（极低开销，适合热循环调用）
    #[inline]
    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::SeqCst)
    }

    /// 获取取消原因
    pub fn reason(&self) -> Option<String> {
        self.inner.reason.lock().unwrap().clone()
    }

    /// 获取子令牌数量
    pub fn children_count(&self) -> usize {
        self.inner.children.lock().unwrap().len()
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for CancellationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancellationToken")
            .field("cancelled", &self.is_cancelled())
            .field("children", &self.children_count())
            .finish()
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试新令牌默认未取消
    #[test]
    fn test_new_token_not_cancelled() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
        assert!(token.reason().is_none());
    }

    /// 测试取消令牌
    #[test]
    fn test_cancel() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(token.is_cancelled());
    }

    /// 测试带原因取消
    #[test]
    fn test_cancel_with_reason() {
        let token = CancellationToken::new();
        token.cancel_with_reason("用户中断");
        assert!(token.is_cancelled());
        assert_eq!(token.reason(), Some("用户中断".to_string()));
    }

    /// 测试重复取消幂等
    #[test]
    fn test_cancel_idempotent() {
        let token = CancellationToken::new();
        token.cancel_with_reason("第一次");
        token.cancel_with_reason("第二次");
        // 原因应保持第一次的
        assert_eq!(token.reason(), Some("第一次".to_string()));
    }

    /// 测试父取消级联到子
    #[test]
    fn test_parent_cancel_cascades() {
        let parent = CancellationToken::new();
        let child1 = parent.child();
        let child2 = parent.child();

        assert!(!child1.is_cancelled());
        assert!(!child2.is_cancelled());

        parent.cancel_with_reason("超时");

        assert!(child1.is_cancelled());
        assert!(child2.is_cancelled());
        assert_eq!(child1.reason(), Some("超时".to_string()));
        assert_eq!(child2.reason(), Some("超时".to_string()));
    }

    /// 测试子取消不影响父
    #[test]
    fn test_child_cancel_no_cascade_up() {
        let parent = CancellationToken::new();
        let child = parent.child();

        child.cancel();

        assert!(child.is_cancelled());
        assert!(!parent.is_cancelled(), "子取消不应影响父");
    }

    /// 测试子取消不影响兄弟
    #[test]
    fn test_sibling_independence() {
        let parent = CancellationToken::new();
        let child1 = parent.child();
        let child2 = parent.child();

        child1.cancel();

        assert!(child1.is_cancelled());
        assert!(!child2.is_cancelled(), "兄弟令牌应独立");
    }

    /// 测试三级级联
    #[test]
    fn test_three_level_cascade() {
        let root = CancellationToken::new();
        let mid = root.child();
        let leaf = mid.child();

        root.cancel_with_reason("根取消");

        assert!(root.is_cancelled());
        assert!(mid.is_cancelled());
        assert!(leaf.is_cancelled());
        assert_eq!(leaf.reason(), Some("根取消".to_string()));
    }

    /// 测试已取消的父创建子令牌
    #[test]
    fn test_child_of_cancelled_parent() {
        let parent = CancellationToken::new();
        parent.cancel_with_reason("已取消");

        let child = parent.child();
        assert!(child.is_cancelled(), "已取消父的子令牌应立即取消");
        assert_eq!(child.reason(), Some("已取消".to_string()));
    }

    /// 测试 children_count
    #[test]
    fn test_children_count() {
        let parent = CancellationToken::new();
        assert_eq!(parent.children_count(), 0);

        let _c1 = parent.child();
        assert_eq!(parent.children_count(), 1);

        let _c2 = parent.child();
        assert_eq!(parent.children_count(), 2);
    }

    /// 测试 Clone 共享状态
    #[test]
    fn test_clone_shared_state() {
        let token = CancellationToken::new();
        let clone = token.clone();

        token.cancel();
        assert!(clone.is_cancelled(), "clone 应共享取消状态");
    }

    /// 测试 Debug 输出
    #[test]
    fn test_debug_format() {
        let token = CancellationToken::new();
        let debug = format!("{:?}", token);
        assert!(debug.contains("CancellationToken"));
        assert!(debug.contains("cancelled"));
    }

    /// 测试多线程取消
    #[test]
    fn test_thread_safety() {
        let token = CancellationToken::new();
        let token_clone = token.clone();

        let handle = std::thread::spawn(move || {
            token_clone.cancel_with_reason("从另一个线程");
        });

        handle.join().unwrap();
        assert!(token.is_cancelled());
        assert_eq!(token.reason(), Some("从另一个线程".to_string()));
    }
}

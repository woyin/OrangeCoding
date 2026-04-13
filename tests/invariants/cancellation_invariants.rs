//! # Cancellation Invariant Tests
//!
//! INV-CANCEL-01: 取消信号必须向下传播
//! INV-CANCEL-02: 取消后必须可重置

use chengcoding_agent::cancellation::CancellationToken;
use chengcoding_worker::SessionSupervisor;

// =========================================================================
// INV-CANCEL-01: 取消信号必须向下传播
// =========================================================================

#[test]
fn inv_cancel_01_parent_cancel_propagates_to_child() {
    let parent = CancellationToken::new();
    let child = parent.child();

    assert!(!child.is_cancelled(), "子令牌初始应未取消");

    parent.cancel();

    assert!(parent.is_cancelled(), "父令牌应已取消");
    assert!(child.is_cancelled(), "父取消后子令牌必须被取消");
}

#[test]
fn inv_cancel_01_cancel_propagates_deep_nesting() {
    let root = CancellationToken::new();
    let level1 = root.child();
    let level2 = level1.child();
    let level3 = level2.child();
    let level4 = level3.child();

    assert!(!level4.is_cancelled(), "四级嵌套令牌初始应未取消");

    root.cancel();

    assert!(level1.is_cancelled(), "一级子令牌必须被取消");
    assert!(level2.is_cancelled(), "二级子令牌必须被取消");
    assert!(level3.is_cancelled(), "三级子令牌必须被取消");
    assert!(level4.is_cancelled(), "四级子令牌必须被取消");
}

#[test]
fn inv_cancel_01_child_cancel_does_not_propagate_up() {
    let parent = CancellationToken::new();
    let child = parent.child();

    child.cancel();

    assert!(child.is_cancelled());
    assert!(!parent.is_cancelled(), "子取消不得向上传播到父令牌");
}

#[test]
fn inv_cancel_01_cancel_reason_propagates() {
    let parent = CancellationToken::new();
    let child = parent.child();

    parent.cancel_with_reason("用户取消");

    assert_eq!(child.reason(), Some("用户取消".to_string()));
}

#[test]
fn inv_cancel_01_supervisor_cancel_cancels_token() {
    let sv = SessionSupervisor::new();
    let session = sv.create_session(Some("test".into()), None);
    let token = sv.get_cancel_token(&session.id).unwrap();

    assert!(!token.is_cancelled());

    assert!(sv.cancel_task(&session.id), "cancel_task 应返回 true");
    assert!(
        token.is_cancelled(),
        "cancel_task 后 token 必须被标记为 cancelled"
    );
}

// =========================================================================
// INV-CANCEL-02: 取消后必须可重置
// =========================================================================

#[test]
fn inv_cancel_02_reset_after_cancel_creates_fresh_token() {
    let sv = SessionSupervisor::new();
    let session = sv.create_session(None, None);

    // Cancel
    sv.cancel_task(&session.id);
    let old_token = sv.get_cancel_token(&session.id).unwrap();
    assert!(old_token.is_cancelled(), "取消后 token 应为 cancelled");

    // Reset
    assert!(sv.reset_cancel_token(&session.id), "reset 应返回 true");

    let new_token = sv.get_cancel_token(&session.id).unwrap();
    assert!(!new_token.is_cancelled(), "重置后新 token 不得为 cancelled");
}

#[test]
fn inv_cancel_02_reset_nonexistent_returns_false() {
    let sv = SessionSupervisor::new();
    assert!(
        !sv.reset_cancel_token("nonexistent-session-id"),
        "重置不存在的 session 应返回 false"
    );
}

#[test]
fn inv_cancel_02_multiple_cancel_reset_cycles() {
    let sv = SessionSupervisor::new();
    let session = sv.create_session(Some("cycle-test".into()), None);

    for i in 0..5 {
        sv.cancel_task(&session.id);
        let token = sv.get_cancel_token(&session.id).unwrap();
        assert!(token.is_cancelled(), "cycle {} cancel failed", i);

        sv.reset_cancel_token(&session.id);
        let token = sv.get_cancel_token(&session.id).unwrap();
        assert!(!token.is_cancelled(), "cycle {} reset failed", i);
    }
}

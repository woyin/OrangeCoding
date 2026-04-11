//! # Approval Invariant Tests
//!
//! INV-APPROVAL-01: 审批请求必须可等待
//! INV-APPROVAL-02: 审批结果必须送达请求方

use ceair_control_protocol::{ApprovalDecision, RiskLevel};
use ceair_worker::ApprovalBridge;

// =========================================================================
// INV-APPROVAL-01: 审批请求必须可等待
// =========================================================================

#[tokio::test]
async fn inv_approval_01_request_then_approve_delivers_decision() {
    let bridge = ApprovalBridge::new();
    let (req, rx) = bridge
        .request_approval(
            "session-1".into(),
            "bash".into(),
            RiskLevel::High,
            "run rm -rf".into(),
            serde_json::json!({"cmd": "rm -rf /"}),
        )
        .await;

    assert_eq!(bridge.pending_count(), 1, "应有 1 个待处理审批");

    let resolved = bridge.resolve(&req.id, ApprovalDecision::Approved);
    assert!(resolved, "resolve 应返回 true");

    let decision = rx.await.expect("receiver 应收到决策");
    assert_eq!(decision, ApprovalDecision::Approved);
    assert_eq!(bridge.pending_count(), 0, "resolve 后应无待处理审批");
}

#[tokio::test]
async fn inv_approval_01_request_then_deny_delivers_decision() {
    let bridge = ApprovalBridge::new();
    let (req, rx) = bridge
        .request_approval(
            "session-2".into(),
            "delete_file".into(),
            RiskLevel::Critical,
            "delete production db".into(),
            serde_json::json!({}),
        )
        .await;

    let deny = ApprovalDecision::Denied {
        reason: Some("too dangerous".into()),
    };
    bridge.resolve(&req.id, deny.clone());

    let decision = rx.await.expect("receiver 应收到 deny 决策");
    assert_eq!(decision, deny);
}

#[tokio::test]
async fn inv_approval_01_multiple_concurrent_requests_independent() {
    let bridge = ApprovalBridge::new();

    let (req1, rx1) = bridge
        .request_approval(
            "s1".into(),
            "tool_a".into(),
            RiskLevel::Medium,
            "op1".into(),
            serde_json::json!({}),
        )
        .await;

    let (req2, rx2) = bridge
        .request_approval(
            "s2".into(),
            "tool_b".into(),
            RiskLevel::Low,
            "op2".into(),
            serde_json::json!({}),
        )
        .await;

    assert_eq!(bridge.pending_count(), 2);

    // Resolve in reverse order
    bridge.resolve(
        &req2.id,
        ApprovalDecision::Denied {
            reason: Some("no".into()),
        },
    );
    bridge.resolve(&req1.id, ApprovalDecision::Approved);

    let d1 = rx1.await.unwrap();
    let d2 = rx2.await.unwrap();

    assert_eq!(d1, ApprovalDecision::Approved);
    assert_eq!(
        d2,
        ApprovalDecision::Denied {
            reason: Some("no".into())
        }
    );
}

// =========================================================================
// INV-APPROVAL-02: 审批结果必须送达请求方
// =========================================================================

#[tokio::test]
async fn inv_approval_02_resolve_sends_through_channel() {
    let bridge = ApprovalBridge::new();
    let (req, rx) = bridge
        .request_approval(
            "s1".into(),
            "write_file".into(),
            RiskLevel::High,
            "overwrite config".into(),
            serde_json::json!({"path": "/etc/conf"}),
        )
        .await;

    assert!(bridge.resolve(&req.id, ApprovalDecision::Approved));

    // The receiver must get the value
    let result = rx.await;
    assert!(result.is_ok(), "receiver 必须收到审批结果");
    assert_eq!(result.unwrap(), ApprovalDecision::Approved);
}

#[tokio::test]
async fn inv_approval_02_receiver_dropped_resolve_returns_false() {
    let bridge = ApprovalBridge::new();
    let (req, rx) = bridge
        .request_approval(
            "s1".into(),
            "bash".into(),
            RiskLevel::Medium,
            "test".into(),
            serde_json::json!({}),
        )
        .await;

    // Drop the receiver before resolving
    drop(rx);

    let resolved = bridge.resolve(&req.id, ApprovalDecision::Approved);
    assert!(
        !resolved,
        "receiver 已 drop 时 resolve 应返回 false"
    );
}

#[tokio::test]
async fn inv_approval_02_resolve_nonexistent_returns_false() {
    let bridge = ApprovalBridge::new();
    assert!(
        !bridge.resolve("nonexistent-id", ApprovalDecision::Approved),
        "resolve 不存在的 approval 应返回 false"
    );
}

//! # Session Invariant Tests
//!
//! INV-SESSION-01: 会话上下文必须跨 turn 持久化
//! INV-SESSION-02: 关闭的会话不可继续使用
//! INV-SESSION-03: 会话 ID 必须全局唯一

use ceair_control_protocol::SessionState;
use ceair_worker::SessionSupervisor;
use std::collections::HashSet;

// =========================================================================
// INV-SESSION-01: 会话上下文必须跨 turn 持久化
// =========================================================================

#[test]
fn inv_session_01_state_persists_across_updates() {
    let sv = SessionSupervisor::new();
    let session = sv.create_session(Some("persistent".into()), None);

    assert_eq!(
        sv.get_session(&session.id).unwrap().state,
        SessionState::Idle,
        "新会话应为 Idle"
    );

    // Simulate turn 1: Running
    sv.update_state(&session.id, SessionState::Running);
    assert_eq!(
        sv.get_session(&session.id).unwrap().state,
        SessionState::Running,
        "第一次更新后应为 Running"
    );

    // Simulate turn 2: Completed
    sv.update_state(&session.id, SessionState::Completed);
    let info = sv.get_session(&session.id).unwrap();
    assert_eq!(
        info.state,
        SessionState::Completed,
        "第二次更新后应为 Completed"
    );
    assert_eq!(
        info.title,
        Some("persistent".into()),
        "标题应在多次状态更新后保持不变"
    );
}

#[test]
fn inv_session_01_updated_at_advances() {
    let sv = SessionSupervisor::new();
    let session = sv.create_session(Some("time-test".into()), None);
    let t0 = sv.get_session(&session.id).unwrap().updated_at;

    // Small delay to ensure time difference
    std::thread::sleep(std::time::Duration::from_millis(2));

    sv.update_state(&session.id, SessionState::Running);
    let t1 = sv.get_session(&session.id).unwrap().updated_at;
    assert!(t1 >= t0, "updated_at 应在状态更新后递增");
}

// =========================================================================
// INV-SESSION-02: 关闭的会话不可继续使用
// =========================================================================

#[test]
fn inv_session_02_closed_session_get_returns_none() {
    let sv = SessionSupervisor::new();
    let session = sv.create_session(Some("closable".into()), None);
    let id = session.id.clone();

    assert!(sv.close_session(&id), "关闭应返回 true");
    assert!(
        sv.get_session(&id).is_none(),
        "关闭后 get_session 必须返回 None"
    );
}

#[test]
fn inv_session_02_closed_session_cancel_returns_false() {
    let sv = SessionSupervisor::new();
    let session = sv.create_session(None, None);
    let id = session.id.clone();

    sv.close_session(&id);
    assert!(
        !sv.cancel_task(&id),
        "关闭后 cancel_task 必须返回 false"
    );
}

#[test]
fn inv_session_02_closed_session_update_returns_false() {
    let sv = SessionSupervisor::new();
    let session = sv.create_session(None, None);
    let id = session.id.clone();

    sv.close_session(&id);
    assert!(
        !sv.update_state(&id, SessionState::Running),
        "关闭后 update_state 必须返回 false"
    );
}

#[test]
fn inv_session_02_closed_session_not_in_list() {
    let sv = SessionSupervisor::new();
    let s1 = sv.create_session(Some("keep".into()), None);
    let s2 = sv.create_session(Some("close".into()), None);

    sv.close_session(&s2.id);

    let list = sv.list_sessions();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, s1.id, "关闭的会话不得出现在列表中");
}

#[test]
fn inv_session_02_double_close_returns_false() {
    let sv = SessionSupervisor::new();
    let session = sv.create_session(None, None);
    let id = session.id.clone();

    assert!(sv.close_session(&id), "第一次关闭应返回 true");
    assert!(!sv.close_session(&id), "第二次关闭应返回 false");
}

// =========================================================================
// INV-SESSION-03: 会话 ID 必须全局唯一
// =========================================================================

#[test]
fn inv_session_03_ids_are_unique() {
    let sv = SessionSupervisor::new();
    let mut ids = HashSet::new();

    for _ in 0..1000 {
        let session = sv.create_session(None, None);
        assert!(
            ids.insert(session.id.clone()),
            "重复的 session ID: {}",
            session.id
        );
    }

    assert_eq!(ids.len(), 1000);
    assert_eq!(sv.count(), 1000);
}

#[test]
fn inv_session_03_ids_are_valid_uuid_format() {
    let sv = SessionSupervisor::new();

    for _ in 0..10 {
        let session = sv.create_session(None, None);
        // UUID v4 format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
        assert_eq!(
            session.id.len(),
            36,
            "session ID 长度应为 36 (UUID 格式)"
        );
        assert!(
            session.id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'),
            "session ID 应只含十六进制字符和连字符: {}",
            session.id
        );
        // Check the version nibble (position 14 should be '4')
        assert_eq!(
            session.id.as_bytes()[14],
            b'4',
            "session ID 应为 UUID v4: {}",
            session.id
        );
    }
}

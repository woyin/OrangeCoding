//! # Event Invariant Tests
//!
//! INV-EVENT-01: 事件序列必须保持时间顺序

use ceair_control_protocol::ServerEvent;
use ceair_worker::WorkerRuntime;
use chrono::Utc;

// =========================================================================
// INV-EVENT-01: 事件序列必须保持时间顺序
// =========================================================================

#[tokio::test]
async fn inv_event_01_events_received_in_publish_order() {
    let runtime = WorkerRuntime::new();
    let mut rx = runtime.subscribe_events();

    let events = vec![
        ServerEvent::AgentStatus {
            session_id: "s1".into(),
            status: "A".into(),
            message: Some("first".into()),
            timestamp: Utc::now(),
        },
        ServerEvent::AgentStatus {
            session_id: "s1".into(),
            status: "B".into(),
            message: Some("second".into()),
            timestamp: Utc::now(),
        },
        ServerEvent::AgentStatus {
            session_id: "s1".into(),
            status: "C".into(),
            message: Some("third".into()),
            timestamp: Utc::now(),
        },
    ];

    for event in &events {
        runtime.publish_event(event.clone());
    }

    let mut received = Vec::new();
    for _ in 0..3 {
        let event = rx.recv().await.expect("应收到事件");
        received.push(event);
    }

    // Verify order: A, B, C
    for (i, event) in received.iter().enumerate() {
        match event {
            ServerEvent::AgentStatus { status, .. } => {
                let expected = &["A", "B", "C"][i];
                assert_eq!(
                    status, expected,
                    "事件 {} 应为 '{}' 但实际为 '{}'",
                    i, expected, status
                );
            }
            other => panic!("期望 AgentStatus，收到 {:?}", other),
        }
    }
}

#[tokio::test]
async fn inv_event_01_multiple_subscribers_same_order() {
    let runtime = WorkerRuntime::new();
    let mut rx1 = runtime.subscribe_events();
    let mut rx2 = runtime.subscribe_events();

    let labels = ["X", "Y", "Z"];
    for label in &labels {
        runtime.publish_event(ServerEvent::AgentStatus {
            session_id: "s1".into(),
            status: label.to_string(),
            message: None,
            timestamp: Utc::now(),
        });
    }

    for label in &labels {
        match rx1.recv().await.unwrap() {
            ServerEvent::AgentStatus { status, .. } => {
                assert_eq!(&status, label, "subscriber 1 顺序错误");
            }
            _ => panic!("unexpected event type"),
        }
        match rx2.recv().await.unwrap() {
            ServerEvent::AgentStatus { status, .. } => {
                assert_eq!(&status, label, "subscriber 2 顺序错误");
            }
            _ => panic!("unexpected event type"),
        }
    }
}

#[tokio::test]
async fn inv_event_01_mixed_event_types_maintain_order() {
    let runtime = WorkerRuntime::new();
    let mut rx = runtime.subscribe_events();

    runtime.publish_event(ServerEvent::Pong {
        timestamp: Utc::now(),
    });
    runtime.publish_event(ServerEvent::SessionCreated {
        session_id: "s1".into(),
        title: Some("test".into()),
        timestamp: Utc::now(),
    });
    runtime.publish_event(ServerEvent::Error {
        session_id: None,
        code: "test_error".into(),
        message: "something failed".into(),
        timestamp: Utc::now(),
    });

    // Event 1: Pong
    match rx.recv().await.unwrap() {
        ServerEvent::Pong { .. } => {}
        other => panic!("期望 Pong，收到 {:?}", other),
    }

    // Event 2: SessionCreated
    match rx.recv().await.unwrap() {
        ServerEvent::SessionCreated { session_id, .. } => {
            assert_eq!(session_id, "s1");
        }
        other => panic!("期望 SessionCreated，收到 {:?}", other),
    }

    // Event 3: Error
    match rx.recv().await.unwrap() {
        ServerEvent::Error { code, .. } => {
            assert_eq!(code, "test_error");
        }
        other => panic!("期望 Error，收到 {:?}", other),
    }
}

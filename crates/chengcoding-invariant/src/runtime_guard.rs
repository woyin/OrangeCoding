//! Runtime Guard: 运行时不变量拦截器
//!
//! 在运行时拦截违反不变量的操作：
//! - 未鉴权 WebSocket
//! - 未授权工具调用
//! - 未传播的取消信号
//! - 丢失的会话状态

use crate::report::Violation;
use crate::rules::Severity;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 运行时拦截动作
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardAction {
    /// 允许操作继续
    Allow,
    /// 拒绝操作
    Deny(String),
    /// 需要审批后继续
    RequireApproval(String),
}

/// 工具调用请求上下文
#[derive(Debug, Clone)]
pub struct ToolCallContext {
    /// 工具名称
    pub tool_name: String,
    /// 会话 ID
    pub session_id: String,
    /// 调用方是否已认证
    pub authenticated: bool,
    /// 工具风险级别
    pub risk_level: RiskLevel,
}

/// 工具风险级别
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Safe,
    Low,
    Medium,
    High,
    Critical,
}

/// WebSocket 连接请求上下文
#[derive(Debug, Clone)]
pub struct WsConnectionContext {
    /// 是否携带有效 token
    pub has_valid_token: bool,
    /// 来源 IP
    pub source_ip: String,
    /// 是否本地连接
    pub is_local: bool,
}

/// 会话操作上下文
#[derive(Debug, Clone)]
pub struct SessionOpContext {
    /// 会话 ID
    pub session_id: String,
    /// 会话是否存在
    pub session_exists: bool,
    /// 会话是否已关闭
    pub session_closed: bool,
}

/// 取消操作上下文
#[derive(Debug, Clone)]
pub struct CancelContext {
    /// 父 token 是否已取消
    pub parent_cancelled: bool,
    /// 子 token 数量
    pub child_count: usize,
    /// 已传播到的子 token 数量
    pub propagated_count: usize,
}

/// 运行时守卫 — 在操作执行前进行不变量校验
pub struct RuntimeGuard {
    /// 高危工具名单（默认需要审批）
    high_risk_tools: HashSet<String>,
    /// 是否强制认证
    require_auth: bool,
}

impl RuntimeGuard {
    /// 创建默认守卫（高危工具列表：bash, edit, delete, ssh, web_fetch）
    pub fn new() -> Self {
        let high_risk_tools: HashSet<String> = ["bash", "edit", "delete", "ssh", "web_fetch"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        Self {
            high_risk_tools,
            require_auth: true,
        }
    }

    /// 自定义高危工具列表
    pub fn with_high_risk_tools(tools: Vec<String>) -> Self {
        Self {
            high_risk_tools: tools.into_iter().collect(),
            require_auth: true,
        }
    }

    /// 设置是否强制认证
    pub fn set_require_auth(&mut self, require: bool) {
        self.require_auth = require;
    }

    /// 检查 WebSocket 连接是否允许 — INV-AUTH-01
    pub fn check_ws_connection(&self, ctx: &WsConnectionContext) -> GuardAction {
        if self.require_auth && !ctx.has_valid_token && !ctx.is_local {
            return GuardAction::Deny("INV-AUTH-01: WebSocket 连接未鉴权".into());
        }
        // 本地连接无需认证
        GuardAction::Allow
    }

    /// 检查工具调用是否允许 — INV-TOOL-01, INV-TOOL-02
    pub fn check_tool_call(&self, ctx: &ToolCallContext) -> GuardAction {
        if !ctx.authenticated {
            return GuardAction::Deny("INV-AUTH-02: 未认证用户不可调用工具".into());
        }
        // 高危工具 + 高/关键风险级别 → 需要审批
        if self.high_risk_tools.contains(&ctx.tool_name)
            && matches!(ctx.risk_level, RiskLevel::High | RiskLevel::Critical)
        {
            return GuardAction::RequireApproval(format!(
                "INV-TOOL-01: 高危工具 {} 需要审批",
                ctx.tool_name
            ));
        }
        GuardAction::Allow
    }

    /// 检查会话操作是否允许 — INV-SESSION-01, INV-SESSION-02
    pub fn check_session_op(&self, ctx: &SessionOpContext) -> GuardAction {
        if !ctx.session_exists {
            return GuardAction::Deny("INV-SESSION-01: 会话不存在".into());
        }
        if ctx.session_closed {
            return GuardAction::Deny("INV-SESSION-02: 会话已关闭".into());
        }
        GuardAction::Allow
    }

    /// 检查取消信号传播 — INV-CANCEL-01
    pub fn check_cancel_propagation(&self, ctx: &CancelContext) -> GuardAction {
        if ctx.parent_cancelled && ctx.propagated_count < ctx.child_count {
            return GuardAction::Deny("INV-CANCEL-01: 取消信号未完全传播".into());
        }
        GuardAction::Allow
    }

    /// 收集所有拒绝为 Violation 记录
    pub fn to_violation(
        &self,
        action: &GuardAction,
        rule_id: &str,
        rule_name: &str,
    ) -> Option<Violation> {
        match action {
            GuardAction::Deny(msg) => Some(Violation {
                rule_id: rule_id.to_string(),
                rule_name: rule_name.to_string(),
                severity: Severity::Critical,
                message: msg.clone(),
                timestamp: Utc::now(),
            }),
            _ => None,
        }
    }
}

impl Default for RuntimeGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_deny_unauthenticated_remote() {
        let guard = RuntimeGuard::new();
        let ctx = WsConnectionContext {
            has_valid_token: false,
            source_ip: "203.0.113.1".into(),
            is_local: false,
        };
        assert_eq!(
            guard.check_ws_connection(&ctx),
            GuardAction::Deny("INV-AUTH-01: WebSocket 连接未鉴权".into())
        );
    }

    #[test]
    fn ws_allow_local_without_token() {
        let guard = RuntimeGuard::new();
        let ctx = WsConnectionContext {
            has_valid_token: false,
            source_ip: "127.0.0.1".into(),
            is_local: true,
        };
        assert_eq!(guard.check_ws_connection(&ctx), GuardAction::Allow);
    }

    #[test]
    fn ws_allow_authenticated() {
        let guard = RuntimeGuard::new();
        let ctx = WsConnectionContext {
            has_valid_token: true,
            source_ip: "203.0.113.1".into(),
            is_local: false,
        };
        assert_eq!(guard.check_ws_connection(&ctx), GuardAction::Allow);
    }

    #[test]
    fn tool_deny_unauthenticated() {
        let guard = RuntimeGuard::new();
        let ctx = ToolCallContext {
            tool_name: "bash".into(),
            session_id: "sess-1".into(),
            authenticated: false,
            risk_level: RiskLevel::High,
        };
        assert_eq!(
            guard.check_tool_call(&ctx),
            GuardAction::Deny("INV-AUTH-02: 未认证用户不可调用工具".into())
        );
    }

    #[test]
    fn tool_require_approval_high_risk() {
        let guard = RuntimeGuard::new();
        let ctx = ToolCallContext {
            tool_name: "bash".into(),
            session_id: "sess-1".into(),
            authenticated: true,
            risk_level: RiskLevel::High,
        };
        assert!(matches!(
            guard.check_tool_call(&ctx),
            GuardAction::RequireApproval(_)
        ));
    }

    #[test]
    fn tool_allow_safe() {
        let guard = RuntimeGuard::new();
        let ctx = ToolCallContext {
            tool_name: "read".into(),
            session_id: "sess-1".into(),
            authenticated: true,
            risk_level: RiskLevel::Safe,
        };
        assert_eq!(guard.check_tool_call(&ctx), GuardAction::Allow);
    }

    #[test]
    fn session_deny_closed() {
        let guard = RuntimeGuard::new();
        let ctx = SessionOpContext {
            session_id: "sess-1".into(),
            session_exists: true,
            session_closed: true,
        };
        assert_eq!(
            guard.check_session_op(&ctx),
            GuardAction::Deny("INV-SESSION-02: 会话已关闭".into())
        );
    }

    #[test]
    fn session_deny_nonexistent() {
        let guard = RuntimeGuard::new();
        let ctx = SessionOpContext {
            session_id: "sess-404".into(),
            session_exists: false,
            session_closed: false,
        };
        assert_eq!(
            guard.check_session_op(&ctx),
            GuardAction::Deny("INV-SESSION-01: 会话不存在".into())
        );
    }

    #[test]
    fn cancel_deny_partial_propagation() {
        let guard = RuntimeGuard::new();
        let ctx = CancelContext {
            parent_cancelled: true,
            child_count: 5,
            propagated_count: 3,
        };
        assert_eq!(
            guard.check_cancel_propagation(&ctx),
            GuardAction::Deny("INV-CANCEL-01: 取消信号未完全传播".into())
        );
    }

    #[test]
    fn cancel_allow_full_propagation() {
        let guard = RuntimeGuard::new();
        let ctx = CancelContext {
            parent_cancelled: true,
            child_count: 5,
            propagated_count: 5,
        };
        assert_eq!(guard.check_cancel_propagation(&ctx), GuardAction::Allow);
    }

    #[test]
    fn to_violation_from_deny() {
        let guard = RuntimeGuard::new();
        let action = GuardAction::Deny("test failure".into());
        let violation = guard.to_violation(&action, "INV-AUTH-01", "WebSocket 连接必须鉴权");
        assert!(violation.is_some());
        let v = violation.unwrap();
        assert_eq!(v.rule_id, "INV-AUTH-01");
        assert_eq!(v.severity, Severity::Critical);
        assert_eq!(v.message, "test failure");
    }

    #[test]
    fn to_violation_from_allow() {
        let guard = RuntimeGuard::new();
        let action = GuardAction::Allow;
        assert!(guard.to_violation(&action, "INV-AUTH-01", "test").is_none());
    }
}

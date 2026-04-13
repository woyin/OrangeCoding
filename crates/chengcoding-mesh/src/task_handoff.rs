//! # 任务重分配模块
//!
//! 当Agent过载或任务不匹配时，支持将任务交接给更合适的Agent。

use std::collections::HashMap;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// 交接原因
// ---------------------------------------------------------------------------

/// 任务交接原因 - 说明为何需要将任务转交给其他Agent
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum HandoffReason {
    /// Agent当前负载过高
    Overloaded,
    /// Agent能力与任务需求不匹配
    CapabilityMismatch,
    /// 任务处理超时
    Timeout,
    /// Agent主动放弃任务
    Voluntary,
    /// 发生错误
    Error(String),
}

impl fmt::Display for HandoffReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandoffReason::Overloaded => write!(f, "负载过高"),
            HandoffReason::CapabilityMismatch => write!(f, "能力不匹配"),
            HandoffReason::Timeout => write!(f, "处理超时"),
            HandoffReason::Voluntary => write!(f, "主动放弃"),
            HandoffReason::Error(msg) => write!(f, "错误: {}", msg),
        }
    }
}

// ---------------------------------------------------------------------------
// 交接请求
// ---------------------------------------------------------------------------

/// 任务交接请求 - 描述一次任务转交的详细信息
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandoffRequest {
    /// 发起交接的Agent标识
    pub from_agent: String,
    /// 期望接收任务的Agent标识（`None` 表示由系统自动选择）
    pub to_agent: Option<String>,
    /// 被交接的任务 ID
    pub task_id: String,
    /// 交接原因
    pub reason: HandoffReason,
    /// 任务上下文信息
    pub context: String,
    /// 已有的部分结果（可选）
    pub partial_results: Option<String>,
    /// 请求创建时间
    pub created_at: DateTime<Utc>,
}

impl HandoffRequest {
    /// 创建一个新的交接请求
    pub fn new(
        from_agent: impl Into<String>,
        task_id: impl Into<String>,
        reason: HandoffReason,
        context: impl Into<String>,
    ) -> Self {
        Self {
            from_agent: from_agent.into(),
            to_agent: None,
            task_id: task_id.into(),
            reason,
            context: context.into(),
            partial_results: None,
            created_at: Utc::now(),
        }
    }

    /// 指定目标接收Agent
    pub fn with_target(mut self, to_agent: impl Into<String>) -> Self {
        self.to_agent = Some(to_agent.into());
        self
    }

    /// 附加部分结果
    pub fn with_partial_results(mut self, results: impl Into<String>) -> Self {
        self.partial_results = Some(results.into());
        self
    }
}

impl fmt::Display for HandoffRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let target = self.to_agent.as_deref().unwrap_or("自动选择");
        write!(
            f,
            "交接请求[{}] {} -> {} (原因: {})",
            self.task_id, self.from_agent, target, self.reason
        )
    }
}

// ---------------------------------------------------------------------------
// 交接结果
// ---------------------------------------------------------------------------

/// 任务交接结果
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum HandoffResult {
    /// 交接成功
    Success {
        /// 接手任务的新Agent标识
        new_agent: String,
    },
    /// 交接失败
    Failed {
        /// 失败原因
        reason: String,
    },
    /// 交接待处理
    Pending,
}

impl fmt::Display for HandoffResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandoffResult::Success { new_agent } => {
                write!(f, "交接成功: 新Agent为 {}", new_agent)
            }
            HandoffResult::Failed { reason } => {
                write!(f, "交接失败: {}", reason)
            }
            HandoffResult::Pending => write!(f, "交接待处理"),
        }
    }
}

// ---------------------------------------------------------------------------
// 交接管理器
// ---------------------------------------------------------------------------

/// 任务交接管理器 - 管理任务的重分配和交接流程
///
/// 维护待处理的交接请求队列，提供发起、接受和拒绝交接的接口。
pub struct HandoffManager {
    /// 待处理的交接请求，按任务 ID 索引
    pending: HashMap<String, HandoffRequest>,
    /// 已完成的交接记录
    completed: Vec<(String, HandoffResult)>,
}

impl HandoffManager {
    /// 创建一个新的交接管理器
    pub fn new() -> Self {
        debug!("创建任务交接管理器");
        Self {
            pending: HashMap::new(),
            completed: Vec::new(),
        }
    }

    /// 发起一个任务交接请求
    ///
    /// 如果指定了目标Agent，则交接请求进入待处理状态；
    /// 否则返回 `Pending` 状态等待系统分配。
    pub fn initiate_handoff(&mut self, request: HandoffRequest) -> HandoffResult {
        let task_id = request.task_id.clone();
        info!(
            task_id = %task_id,
            from = %request.from_agent,
            reason = %request.reason,
            "发起任务交接"
        );

        // 检查是否已有该任务的交接请求
        if self.pending.contains_key(&task_id) {
            warn!(task_id = %task_id, "该任务已有待处理的交接请求");
            return HandoffResult::Failed {
                reason: "该任务已有待处理的交接请求".to_string(),
            };
        }

        // 如果指定了目标Agent，记录待处理请求
        let result = if request.to_agent.is_some() {
            self.pending.insert(task_id.clone(), request);
            HandoffResult::Pending
        } else {
            // 未指定目标，标记为待处理
            self.pending.insert(task_id.clone(), request);
            HandoffResult::Pending
        };

        result
    }

    /// 接受一个交接请求
    ///
    /// 将指定任务的交接请求标记为成功，并记录接手的Agent。
    pub fn accept_handoff(&mut self, agent_id: &str, task_id: &str) -> HandoffResult {
        match self.pending.remove(task_id) {
            Some(_request) => {
                info!(
                    task_id = %task_id,
                    agent_id = %agent_id,
                    "Agent接受了任务交接"
                );
                let result = HandoffResult::Success {
                    new_agent: agent_id.to_string(),
                };
                self.completed.push((task_id.to_string(), result.clone()));
                result
            }
            None => {
                warn!(task_id = %task_id, "未找到待处理的交接请求");
                HandoffResult::Failed {
                    reason: format!("未找到任务 {} 的交接请求", task_id),
                }
            }
        }
    }

    /// 拒绝一个交接请求
    pub fn reject_handoff(&mut self, agent_id: &str, task_id: &str, reason: &str) -> HandoffResult {
        match self.pending.remove(task_id) {
            Some(_request) => {
                info!(
                    task_id = %task_id,
                    agent_id = %agent_id,
                    reason = %reason,
                    "Agent拒绝了任务交接"
                );
                let result = HandoffResult::Failed {
                    reason: format!("Agent {} 拒绝: {}", agent_id, reason),
                };
                self.completed.push((task_id.to_string(), result.clone()));
                result
            }
            None => {
                warn!(task_id = %task_id, "未找到待处理的交接请求");
                HandoffResult::Failed {
                    reason: format!("未找到任务 {} 的交接请求", task_id),
                }
            }
        }
    }

    /// 获取所有待处理的交接请求
    pub fn get_pending_handoffs(&self) -> Vec<HandoffRequest> {
        self.pending.values().cloned().collect()
    }

    /// 获取待处理交接请求的数量
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// 获取已完成的交接记录数量
    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }

    /// 检查指定任务是否有待处理的交接请求
    pub fn has_pending(&self, task_id: &str) -> bool {
        self.pending.contains_key(task_id)
    }
}

impl Default for HandoffManager {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for HandoffManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandoffManager")
            .field("pending_count", &self.pending.len())
            .field("completed_count", &self.completed.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 测试交接原因显示() {
        assert_eq!(format!("{}", HandoffReason::Overloaded), "负载过高");
        assert_eq!(
            format!("{}", HandoffReason::CapabilityMismatch),
            "能力不匹配"
        );
        assert_eq!(format!("{}", HandoffReason::Timeout), "处理超时");
        assert_eq!(format!("{}", HandoffReason::Voluntary), "主动放弃");

        let err = HandoffReason::Error("内存不足".to_string());
        assert!(format!("{err}").contains("内存不足"));
    }

    #[test]
    fn 测试创建交接请求() {
        let req = HandoffRequest::new(
            "agent-a",
            "task-001",
            HandoffReason::Overloaded,
            "正在处理大型文件解析",
        );
        assert_eq!(req.from_agent, "agent-a");
        assert_eq!(req.task_id, "task-001");
        assert!(req.to_agent.is_none());
        assert!(req.partial_results.is_none());
    }

    #[test]
    fn 测试带目标的交接请求() {
        let req = HandoffRequest::new(
            "agent-a",
            "task-002",
            HandoffReason::CapabilityMismatch,
            "需要Python技能",
        )
        .with_target("agent-b")
        .with_partial_results("已完成数据收集");

        assert_eq!(req.to_agent, Some("agent-b".to_string()));
        assert_eq!(req.partial_results, Some("已完成数据收集".to_string()));
    }

    #[test]
    fn 测试发起交接() {
        let mut manager = HandoffManager::new();
        let req = HandoffRequest::new("agent-a", "task-001", HandoffReason::Timeout, "任务超时了")
            .with_target("agent-b");

        let result = manager.initiate_handoff(req);
        assert_eq!(result, HandoffResult::Pending);
        assert_eq!(manager.pending_count(), 1);
        assert!(manager.has_pending("task-001"));
    }

    #[test]
    fn 测试重复发起交接被拒绝() {
        let mut manager = HandoffManager::new();

        let req1 = HandoffRequest::new(
            "agent-a",
            "task-001",
            HandoffReason::Overloaded,
            "第一次请求",
        );
        manager.initiate_handoff(req1);

        let req2 = HandoffRequest::new(
            "agent-a",
            "task-001",
            HandoffReason::Voluntary,
            "第二次请求",
        );
        let result = manager.initiate_handoff(req2);
        match result {
            HandoffResult::Failed { reason } => {
                assert!(reason.contains("已有待处理"));
            }
            other => panic!("期望 Failed，实际得到 {:?}", other),
        }
    }

    #[test]
    fn 测试接受交接() {
        let mut manager = HandoffManager::new();
        let req = HandoffRequest::new("agent-a", "task-001", HandoffReason::Overloaded, "需要转交");

        manager.initiate_handoff(req);
        let result = manager.accept_handoff("agent-b", "task-001");

        assert_eq!(
            result,
            HandoffResult::Success {
                new_agent: "agent-b".to_string()
            }
        );
        assert_eq!(manager.pending_count(), 0);
        assert_eq!(manager.completed_count(), 1);
    }

    #[test]
    fn 测试接受不存在的交接() {
        let mut manager = HandoffManager::new();
        let result = manager.accept_handoff("agent-b", "task-999");

        match result {
            HandoffResult::Failed { reason } => {
                assert!(reason.contains("task-999"));
            }
            other => panic!("期望 Failed，实际得到 {:?}", other),
        }
    }

    #[test]
    fn 测试拒绝交接() {
        let mut manager = HandoffManager::new();
        let req = HandoffRequest::new(
            "agent-a",
            "task-001",
            HandoffReason::CapabilityMismatch,
            "需要其他技能",
        );

        manager.initiate_handoff(req);
        let result = manager.reject_handoff("agent-b", "task-001", "我也不会这个技能");

        match result {
            HandoffResult::Failed { reason } => {
                assert!(reason.contains("agent-b"));
                assert!(reason.contains("我也不会这个技能"));
            }
            other => panic!("期望 Failed，实际得到 {:?}", other),
        }
        assert_eq!(manager.pending_count(), 0);
        assert_eq!(manager.completed_count(), 1);
    }

    #[test]
    fn 测试获取待处理列表() {
        let mut manager = HandoffManager::new();

        manager.initiate_handoff(HandoffRequest::new(
            "agent-a",
            "task-001",
            HandoffReason::Overloaded,
            "上下文1",
        ));
        manager.initiate_handoff(HandoffRequest::new(
            "agent-b",
            "task-002",
            HandoffReason::Voluntary,
            "上下文2",
        ));

        let pending = manager.get_pending_handoffs();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn 测试交接结果显示() {
        let success = HandoffResult::Success {
            new_agent: "agent-x".to_string(),
        };
        assert!(format!("{success}").contains("agent-x"));

        let failed = HandoffResult::Failed {
            reason: "无可用Agent".to_string(),
        };
        assert!(format!("{failed}").contains("无可用Agent"));

        let pending = HandoffResult::Pending;
        assert!(format!("{pending}").contains("待处理"));
    }

    #[test]
    fn 测试交接请求显示() {
        let req = HandoffRequest::new("agent-a", "task-001", HandoffReason::Overloaded, "上下文");
        let display = format!("{req}");
        assert!(display.contains("task-001"));
        assert!(display.contains("agent-a"));
        assert!(display.contains("自动选择"));

        let req_with_target = req.with_target("agent-b");
        let display2 = format!("{req_with_target}");
        assert!(display2.contains("agent-b"));
    }

    #[test]
    fn 测试默认构造() {
        let manager = HandoffManager::default();
        assert_eq!(manager.pending_count(), 0);
        assert_eq!(manager.completed_count(), 0);
    }
}

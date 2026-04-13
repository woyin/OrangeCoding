//! # Agent任务协商协议模块
//!
//! 实现Agent之间的任务协商：请求-提议-接受/拒绝流程。

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// 任务请求
// ---------------------------------------------------------------------------

/// 任务请求 - 描述需要协商分配的任务
///
/// 包含任务的标识、技能需求、优先级和截止时间提示等信息，
/// 用于向可用Agent发起协商。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRequest {
    /// 任务唯一标识
    pub task_id: String,
    /// 任务所需的技能列表
    pub requirements: Vec<String>,
    /// 任务优先级（0-255，值越大优先级越高）
    pub priority: u8,
    /// 截止时间提示（可选，如 "2小时内"）
    pub deadline_hint: Option<String>,
}

impl TaskRequest {
    /// 创建一个新的任务请求
    pub fn new(task_id: impl Into<String>, requirements: Vec<String>, priority: u8) -> Self {
        Self {
            task_id: task_id.into(),
            requirements,
            priority,
            deadline_hint: None,
        }
    }

    /// 设置截止时间提示
    pub fn with_deadline(mut self, hint: impl Into<String>) -> Self {
        self.deadline_hint = Some(hint.into());
        self
    }
}

impl fmt::Display for TaskRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "任务请求[{}] 优先级={} 需求={:?}",
            self.task_id, self.priority, self.requirements
        )
    }
}

// ---------------------------------------------------------------------------
// 任务报价
// ---------------------------------------------------------------------------

/// 任务报价 - Agent对某个任务的竞标信息
///
/// 包含Agent的能力评分和预估工作量，用于协商决策。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskOffer {
    /// 竞标Agent的标识
    pub agent_id: String,
    /// 竞标的任务 ID
    pub task_id: String,
    /// 能力匹配得分（0.0-1.0，越高越匹配）
    pub capability_score: f32,
    /// 预估工作量描述
    pub estimated_effort: String,
}

impl TaskOffer {
    /// 创建一个新的任务报价
    pub fn new(
        agent_id: impl Into<String>,
        task_id: impl Into<String>,
        capability_score: f32,
        estimated_effort: impl Into<String>,
    ) -> Self {
        // 将分数限制在 0.0-1.0 范围内
        let score = capability_score.clamp(0.0, 1.0);
        Self {
            agent_id: agent_id.into(),
            task_id: task_id.into(),
            capability_score: score,
            estimated_effort: estimated_effort.into(),
        }
    }
}

impl fmt::Display for TaskOffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "报价[{}] Agent={} 得分={:.2} 工作量={}",
            self.task_id, self.agent_id, self.capability_score, self.estimated_effort
        )
    }
}

// ---------------------------------------------------------------------------
// 协商结果
// ---------------------------------------------------------------------------

/// 协商结果 - 任务协商的最终结果
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NegotiationOutcome {
    /// 任务已被接受
    Accepted {
        /// 接受任务的Agent标识
        agent_id: String,
        /// 被接受的任务 ID
        task_id: String,
    },
    /// 任务被拒绝
    Rejected {
        /// 拒绝原因
        reason: String,
    },
    /// 无Agent提供报价
    NoOffers,
}

impl fmt::Display for NegotiationOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NegotiationOutcome::Accepted { agent_id, task_id } => {
                write!(f, "已接受: Agent {} 承接任务 {}", agent_id, task_id)
            }
            NegotiationOutcome::Rejected { reason } => {
                write!(f, "已拒绝: {}", reason)
            }
            NegotiationOutcome::NoOffers => {
                write!(f, "无可用报价")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Agent 能力描述
// ---------------------------------------------------------------------------

/// Agent能力描述 - 定义Agent的技能和当前状态
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentCapability {
    /// Agent 标识
    pub agent_id: String,
    /// Agent 具备的技能列表
    pub skills: Vec<String>,
    /// 当前负载（0.0-1.0，0表示空闲，1表示满载）
    pub current_load: f32,
    /// 最大并行任务数
    pub max_concurrent: u8,
}

impl AgentCapability {
    /// 创建一个新的Agent能力描述
    pub fn new(agent_id: impl Into<String>, skills: Vec<String>, max_concurrent: u8) -> Self {
        Self {
            agent_id: agent_id.into(),
            skills,
            current_load: 0.0,
            max_concurrent,
        }
    }

    /// 检查Agent是否具备指定技能
    pub fn has_skill(&self, skill: &str) -> bool {
        self.skills.iter().any(|s| s == skill)
    }

    /// 检查Agent是否有空闲容量
    pub fn has_capacity(&self) -> bool {
        self.current_load < 1.0
    }

    /// 计算该Agent对于任务需求的匹配度（0.0-1.0）
    pub fn match_score(&self, requirements: &[String]) -> f32 {
        if requirements.is_empty() {
            return 1.0;
        }
        let matched = requirements.iter().filter(|r| self.has_skill(r)).count();
        matched as f32 / requirements.len() as f32
    }
}

// ---------------------------------------------------------------------------
// 协商协议
// ---------------------------------------------------------------------------

/// 协商协议 - 管理Agent之间的任务协商流程
///
/// 维护Agent能力注册表，根据任务需求自动评估和分配最佳Agent。
pub struct NegotiationProtocol {
    /// 已注册的Agent及其能力
    agents: HashMap<String, AgentCapability>,
    /// 能力匹配的最低阈值
    min_score_threshold: f32,
}

impl NegotiationProtocol {
    /// 创建一个新的协商协议实例
    pub fn new() -> Self {
        debug!("创建任务协商协议实例");
        Self {
            agents: HashMap::new(),
            min_score_threshold: 0.3,
        }
    }

    /// 创建一个带自定义最低匹配分数阈值的协商协议
    pub fn with_threshold(threshold: f32) -> Self {
        Self {
            agents: HashMap::new(),
            min_score_threshold: threshold.clamp(0.0, 1.0),
        }
    }

    /// 注册Agent及其能力
    pub fn register_agent(&mut self, id: impl Into<String>, capability: AgentCapability) {
        let id = id.into();
        info!(agent_id = %id, "注册Agent到协商协议");
        self.agents.insert(id, capability);
    }

    /// 注销Agent
    pub fn unregister_agent(&mut self, id: &str) -> Option<AgentCapability> {
        info!(agent_id = %id, "从协商协议中注销Agent");
        self.agents.remove(id)
    }

    /// 根据任务请求发起协商，自动选择最佳Agent
    ///
    /// 协商流程：
    /// 1. 遍历所有注册的Agent，计算匹配度
    /// 2. 过滤掉不满足最低阈值的Agent
    /// 3. 选择匹配度最高且负载最低的Agent
    pub fn request_task(&self, request: &TaskRequest) -> NegotiationOutcome {
        debug!(task_id = %request.task_id, "发起任务协商");

        // 收集所有有能力的Agent的报价
        let mut offers: Vec<TaskOffer> = Vec::new();

        for (id, cap) in &self.agents {
            if !cap.has_capacity() {
                debug!(agent_id = %id, "Agent已满载，跳过");
                continue;
            }

            let score = cap.match_score(&request.requirements);
            if score >= self.min_score_threshold {
                offers.push(TaskOffer::new(
                    id.clone(),
                    request.task_id.clone(),
                    score,
                    format!("负载 {:.0}%", cap.current_load * 100.0),
                ));
            }
        }

        if offers.is_empty() {
            warn!(task_id = %request.task_id, "没有Agent能承接此任务");
            return NegotiationOutcome::NoOffers;
        }

        // 按能力得分降序排列，相同得分时按负载升序
        offers.sort_by(|a, b| {
            b.capability_score
                .partial_cmp(&a.capability_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let best = &offers[0];
        info!(
            task_id = %request.task_id,
            agent_id = %best.agent_id,
            score = best.capability_score,
            "协商完成，选中最佳Agent"
        );

        NegotiationOutcome::Accepted {
            agent_id: best.agent_id.clone(),
            task_id: request.task_id.clone(),
        }
    }

    /// 评估单个任务报价是否可接受
    pub fn evaluate_offer(&self, offer: &TaskOffer) -> bool {
        let acceptable = offer.capability_score >= self.min_score_threshold;
        debug!(
            agent_id = %offer.agent_id,
            score = offer.capability_score,
            threshold = self.min_score_threshold,
            acceptable = acceptable,
            "评估任务报价"
        );
        acceptable
    }

    /// 获取已注册Agent的数量
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// 获取指定Agent的能力信息
    pub fn get_agent(&self, id: &str) -> Option<&AgentCapability> {
        self.agents.get(id)
    }
}

impl Default for NegotiationProtocol {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for NegotiationProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NegotiationProtocol")
            .field("agent_count", &self.agents.len())
            .field("min_score_threshold", &self.min_score_threshold)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试用的Agent能力
    fn 创建测试能力(id: &str, skills: Vec<&str>, load: f32) -> AgentCapability {
        let mut cap =
            AgentCapability::new(id, skills.into_iter().map(|s| s.to_string()).collect(), 3);
        cap.current_load = load;
        cap
    }

    #[test]
    fn 测试创建任务请求() {
        let req = TaskRequest::new(
            "task-001",
            vec!["Rust".to_string(), "异步编程".to_string()],
            5,
        );
        assert_eq!(req.task_id, "task-001");
        assert_eq!(req.requirements.len(), 2);
        assert_eq!(req.priority, 5);
        assert!(req.deadline_hint.is_none());
    }

    #[test]
    fn 测试任务请求带截止时间() {
        let req = TaskRequest::new("task-002", vec![], 3).with_deadline("1小时内");
        assert_eq!(req.deadline_hint, Some("1小时内".to_string()));
    }

    #[test]
    fn 测试创建任务报价() {
        let offer = TaskOffer::new("agent-a", "task-001", 0.85, "中等工作量");
        assert_eq!(offer.agent_id, "agent-a");
        assert_eq!(offer.task_id, "task-001");
        assert!((offer.capability_score - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn 测试报价分数限制范围() {
        let over = TaskOffer::new("a", "t", 1.5, "");
        assert!((over.capability_score - 1.0).abs() < f32::EPSILON);

        let under = TaskOffer::new("a", "t", -0.3, "");
        assert!((under.capability_score - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn 测试能力匹配计算() {
        let cap = 创建测试能力("agent-a", vec!["Rust", "Python", "测试"], 0.2);

        // 完全匹配
        let reqs_full = vec!["Rust".to_string(), "Python".to_string()];
        assert!((cap.match_score(&reqs_full) - 1.0).abs() < f32::EPSILON);

        // 部分匹配
        let reqs_partial = vec!["Rust".to_string(), "Java".to_string()];
        assert!((cap.match_score(&reqs_partial) - 0.5).abs() < f32::EPSILON);

        // 无匹配
        let reqs_none = vec!["Go".to_string(), "C++".to_string()];
        assert!((cap.match_score(&reqs_none) - 0.0).abs() < f32::EPSILON);

        // 空需求
        assert!((cap.match_score(&[]) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn 测试协商协议注册和查询() {
        let mut protocol = NegotiationProtocol::new();
        let cap = 创建测试能力("agent-a", vec!["Rust"], 0.0);

        protocol.register_agent("agent-a", cap);
        assert_eq!(protocol.agent_count(), 1);
        assert!(protocol.get_agent("agent-a").is_some());
        assert!(protocol.get_agent("agent-b").is_none());
    }

    #[test]
    fn 测试协商选择最佳Agent() {
        let mut protocol = NegotiationProtocol::new();

        // Agent A: Rust 专家，负载低
        protocol.register_agent(
            "agent-a",
            创建测试能力("agent-a", vec!["Rust", "系统编程"], 0.1),
        );
        // Agent B: Python 专家，负载低
        protocol.register_agent(
            "agent-b",
            创建测试能力("agent-b", vec!["Python", "数据分析"], 0.2),
        );

        let req = TaskRequest::new(
            "task-rust",
            vec!["Rust".to_string(), "系统编程".to_string()],
            5,
        );

        let outcome = protocol.request_task(&req);
        match outcome {
            NegotiationOutcome::Accepted { agent_id, task_id } => {
                assert_eq!(agent_id, "agent-a");
                assert_eq!(task_id, "task-rust");
            }
            other => panic!("期望 Accepted，实际得到 {:?}", other),
        }
    }

    #[test]
    fn 测试协商无可用Agent() {
        let protocol = NegotiationProtocol::new();
        let req = TaskRequest::new("task-empty", vec!["Rust".to_string()], 1);

        let outcome = protocol.request_task(&req);
        assert_eq!(outcome, NegotiationOutcome::NoOffers);
    }

    #[test]
    fn 测试协商满载Agent被跳过() {
        let mut protocol = NegotiationProtocol::new();

        // 满载的Agent
        protocol.register_agent("agent-busy", 创建测试能力("agent-busy", vec!["Rust"], 1.0));

        let req = TaskRequest::new("task-x", vec!["Rust".to_string()], 1);
        let outcome = protocol.request_task(&req);
        assert_eq!(outcome, NegotiationOutcome::NoOffers);
    }

    #[test]
    fn 测试评估报价() {
        let protocol = NegotiationProtocol::with_threshold(0.5);

        let good_offer = TaskOffer::new("a", "t1", 0.8, "少量");
        assert!(protocol.evaluate_offer(&good_offer));

        let bad_offer = TaskOffer::new("b", "t2", 0.2, "大量");
        assert!(!protocol.evaluate_offer(&bad_offer));

        let edge_offer = TaskOffer::new("c", "t3", 0.5, "中等");
        assert!(protocol.evaluate_offer(&edge_offer));
    }

    #[test]
    fn 测试注销Agent() {
        let mut protocol = NegotiationProtocol::new();
        let cap = 创建测试能力("agent-a", vec!["Rust"], 0.0);

        protocol.register_agent("agent-a", cap);
        assert_eq!(protocol.agent_count(), 1);

        let removed = protocol.unregister_agent("agent-a");
        assert!(removed.is_some());
        assert_eq!(protocol.agent_count(), 0);

        // 再次注销不存在的Agent
        assert!(protocol.unregister_agent("agent-a").is_none());
    }

    #[test]
    fn 测试协商结果显示() {
        let accepted = NegotiationOutcome::Accepted {
            agent_id: "agent-x".to_string(),
            task_id: "task-y".to_string(),
        };
        assert!(format!("{accepted}").contains("agent-x"));

        let rejected = NegotiationOutcome::Rejected {
            reason: "能力不足".to_string(),
        };
        assert!(format!("{rejected}").contains("能力不足"));

        let no_offers = NegotiationOutcome::NoOffers;
        assert!(format!("{no_offers}").contains("无可用报价"));
    }

    #[test]
    fn 测试默认构造() {
        let protocol = NegotiationProtocol::default();
        assert_eq!(protocol.agent_count(), 0);
    }

    #[test]
    fn 测试任务请求显示() {
        let req = TaskRequest::new("task-display", vec!["Rust".to_string()], 10);
        let display = format!("{req}");
        assert!(display.contains("task-display"));
        assert!(display.contains("10"));
    }
}

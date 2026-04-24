//! # Boulder 会话连续性系统
//!
//! 管理跨会话的状态持久化，使中断的工作流能够无缝恢复。
//!
//! ## 存储位置
//!
//! 状态文件存储于 `.sisyphus/boulder.json`。
//!
//! ## 恢复逻辑
//!
//! 1. 读取 boulder.json
//! 2. 计算进度
//! 3. 注入继续提示

use serde::{Deserialize, Serialize};

/// Boulder 状态文件路径
pub const BOULDER_FILE_PATH: &str = ".sisyphus/boulder.json";

// ============================================================
// Boulder 状态
// ============================================================

/// 进度追踪
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    /// 已完成的任务数
    pub checked_count: usize,
    /// 总任务数
    pub total_count: usize,
}

impl Progress {
    /// 创建新的进度实例
    pub fn new(total: usize) -> Self {
        Self {
            checked_count: 0,
            total_count: total,
        }
    }

    /// 计算完成百分比
    pub fn percentage(&self) -> f64 {
        if self.total_count == 0 {
            return 0.0;
        }
        (self.checked_count as f64 / self.total_count as f64) * 100.0
    }

    /// 是否全部完成
    pub fn is_complete(&self) -> bool {
        self.total_count > 0 && self.checked_count >= self.total_count
    }
}

/// Harness 快照
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessSnapshot {
    /// 任务契约
    pub mission_contract: crate::harness::MissionContract,
    /// 最近已接受检查点
    pub last_accepted_checkpoint: Option<String>,
}

/// Boulder 持久化状态（序列化到 boulder.json）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoulderState {
    /// 当前活跃的计划文件路径
    pub active_plan: String,
    /// 关联的会话 ID 列表
    pub session_ids: Vec<String>,
    /// 开始时间（ISO 8601 格式）
    pub started_at: String,
    /// 计划名称
    pub plan_name: String,
    /// 执行进度
    pub progress: Progress,
    /// Harness 快照
    pub harness: Option<HarnessSnapshot>,
}

impl BoulderState {
    /// 创建新的 Boulder 状态
    pub fn new(
        active_plan: impl Into<String>,
        plan_name: impl Into<String>,
        total_tasks: usize,
    ) -> Self {
        Self {
            active_plan: active_plan.into(),
            session_ids: Vec::new(),
            started_at: chrono::Utc::now().to_rfc3339(),
            plan_name: plan_name.into(),
            progress: Progress::new(total_tasks),
            harness: None,
        }
    }
}

// ============================================================
// Boulder 管理器
// ============================================================

/// Boulder 会话连续性管理器
///
/// 负责加载、保存、创建和恢复会话状态。
#[derive(Debug, Clone)]
pub struct BoulderManager {
    /// 当前内存中的状态
    state: Option<BoulderState>,
}

impl BoulderManager {
    /// 创建新的管理器（无状态）
    pub fn new() -> Self {
        Self { state: None }
    }

    /// 从 JSON 字符串加载状态
    pub fn load(&mut self, json: &str) -> Result<(), String> {
        let state: BoulderState =
            serde_json::from_str(json).map_err(|e| format!("解析 boulder.json 失败: {e}"))?;
        self.state = Some(state);
        Ok(())
    }

    /// 序列化当前状态为 JSON 字符串
    pub fn save(&self) -> Result<String, String> {
        let state = self
            .state
            .as_ref()
            .ok_or_else(|| "无活跃状态可保存".to_string())?;
        serde_json::to_string_pretty(state).map_err(|e| format!("序列化 boulder 状态失败: {e}"))
    }

    /// 创建新的会话状态
    pub fn create_new(
        &mut self,
        active_plan: impl Into<String>,
        plan_name: impl Into<String>,
        total_tasks: usize,
        session_id: impl Into<String>,
    ) -> &BoulderState {
        let mut state = BoulderState::new(active_plan, plan_name, total_tasks);
        state.session_ids.push(session_id.into());
        self.state = Some(state);
        self.state.as_ref().unwrap()
    }

    /// 恢复会话——加载状态后注入新的会话 ID
    pub fn resume(
        &mut self,
        json: &str,
        new_session_id: impl Into<String>,
    ) -> Result<String, String> {
        self.load(json)?;

        let state = self.state.as_mut().unwrap();
        state.session_ids.push(new_session_id.into());

        // 生成继续提示
        let harness_note = state
            .harness
            .as_ref()
            .and_then(|snapshot| snapshot.last_accepted_checkpoint.as_ref())
            .map(|checkpoint| format!("\n最近已接受检查点: {checkpoint}"))
            .unwrap_or_default();

        let prompt = format!(
            "继续执行计划「{}」。\n\
             当前进度: {}/{}（{:.1}%）\n\
             已使用 {} 个会话。{}",
            state.plan_name,
            state.progress.checked_count,
            state.progress.total_count,
            state.progress.percentage(),
            state.session_ids.len(),
            harness_note,
        );

        Ok(prompt)
    }

    /// 附加 Harness 快照
    pub fn attach_harness_snapshot(
        &mut self,
        mission_contract: crate::harness::MissionContract,
        last_accepted_checkpoint: Option<String>,
    ) -> bool {
        match self.state.as_mut() {
            Some(state) => {
                state.harness = Some(HarnessSnapshot {
                    mission_contract,
                    last_accepted_checkpoint,
                });
                true
            }
            None => false,
        }
    }

    /// 更新执行进度
    pub fn update_progress(&mut self, checked: usize) -> bool {
        match self.state.as_mut() {
            Some(state) => {
                state.progress.checked_count = checked;
                true
            }
            None => false,
        }
    }

    /// 是否有活跃状态
    pub fn is_active(&self) -> bool {
        self.state.is_some()
    }

    /// 获取当前状态的引用
    pub fn state(&self) -> Option<&BoulderState> {
        self.state.as_ref()
    }

    /// 获取 Boulder 文件路径
    pub fn file_path() -> &'static str {
        BOULDER_FILE_PATH
    }
}

impl Default for BoulderManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助函数：创建测试用 BoulderState JSON
    fn sample_boulder_json() -> String {
        serde_json::to_string_pretty(&BoulderState {
            active_plan: ".sisyphus/plans/plan-001.json".to_string(),
            session_ids: vec!["session-abc".to_string()],
            started_at: "2024-01-15T10:30:00+00:00".to_string(),
            plan_name: "重构认证模块".to_string(),
            progress: Progress {
                checked_count: 3,
                total_count: 10,
            },
            harness: None,
        })
        .unwrap()
    }

    #[test]
    fn 新建管理器无活跃状态() {
        let mgr = BoulderManager::new();
        assert!(!mgr.is_active());
        assert!(mgr.state().is_none());
    }

    #[test]
    fn 创建新会话状态() {
        let mut mgr = BoulderManager::new();
        let state = mgr.create_new(".sisyphus/plans/p1.json", "测试计划", 5, "sess-001");
        assert_eq!(state.plan_name, "测试计划");
        assert_eq!(state.progress.total_count, 5);
        assert_eq!(state.session_ids.len(), 1);
        assert!(mgr.is_active());
    }

    #[test]
    fn 加载有效的json() {
        let mut mgr = BoulderManager::new();
        assert!(mgr.load(&sample_boulder_json()).is_ok());
        assert!(mgr.is_active());
        let state = mgr.state().unwrap();
        assert_eq!(state.plan_name, "重构认证模块");
    }

    #[test]
    fn 加载无效json返回错误() {
        let mut mgr = BoulderManager::new();
        let result = mgr.load("{ invalid json }");
        assert!(result.is_err());
        assert!(!mgr.is_active());
    }

    #[test]
    fn 保存和加载往返一致() {
        let mut mgr = BoulderManager::new();
        mgr.create_new(".sisyphus/plans/p1.json", "往返测试", 8, "s1");
        let json = mgr.save().unwrap();

        let mut mgr2 = BoulderManager::new();
        mgr2.load(&json).unwrap();

        assert_eq!(
            mgr.state().unwrap().plan_name,
            mgr2.state().unwrap().plan_name,
        );
    }

    #[test]
    fn 恢复会话生成继续提示() {
        let mut mgr = BoulderManager::new();
        let prompt = mgr.resume(&sample_boulder_json(), "session-def").unwrap();
        assert!(prompt.contains("重构认证模块"));
        assert!(prompt.contains("3/10"));
        assert!(prompt.contains("2 个会话")); // 原有1个 + 新增1个
    }

    #[test]
    fn 恢复会话包含最近已接受检查点() {
        let mut mgr = BoulderManager::new();
        let mut state = BoulderState::new(".sisyphus/plans/plan-001.json", "重构认证模块", 10);
        state.session_ids.push("session-abc".to_string());
        state.harness = Some(HarnessSnapshot {
            mission_contract: crate::harness::MissionContract::new(
                "实现 Harness 对齐层".into(),
                vec!["出现检查点".into()],
                crate::harness::ReviewGatePolicy::MajorPlanChange,
                crate::harness::HarnessConfig::default(),
            ),
            last_accepted_checkpoint: Some("完成了第一段执行".into()),
        });

        let json = serde_json::to_string_pretty(&state).unwrap();
        let prompt = mgr.resume(&json, "session-def").unwrap();

        assert!(prompt.contains("最近已接受检查点"));
        assert!(prompt.contains("完成了第一段执行"));
    }

    #[test]
    fn 更新进度() {
        let mut mgr = BoulderManager::new();
        mgr.create_new(".sisyphus/plans/p1.json", "进度测试", 10, "s1");
        assert!(mgr.update_progress(7));
        assert_eq!(mgr.state().unwrap().progress.checked_count, 7);
    }

    #[test]
    fn 无状态时更新进度返回失败() {
        let mut mgr = BoulderManager::new();
        assert!(!mgr.update_progress(5));
    }

    #[test]
    fn 进度百分比计算() {
        let p = Progress {
            checked_count: 3,
            total_count: 10,
        };
        assert!((p.percentage() - 30.0).abs() < f64::EPSILON);
        assert!(!p.is_complete());
    }

    #[test]
    fn 进度完成判定() {
        let p = Progress {
            checked_count: 5,
            total_count: 5,
        };
        assert!(p.is_complete());
        assert!((p.percentage() - 100.0).abs() < f64::EPSILON);

        // 零总数不算完成
        let empty = Progress::new(0);
        assert!(!empty.is_complete());
    }

    #[test]
    fn 文件路径常量正确() {
        assert_eq!(BoulderManager::file_path(), ".sisyphus/boulder.json");
    }

    #[test]
    fn boulder_state_保存_harness_snapshot() {
        let mut mgr = BoulderManager::new();
        let contract = crate::harness::MissionContract::new(
            "实现 Harness".into(),
            vec!["保持对齐".into()],
            crate::harness::ReviewGatePolicy::MajorPlanChange,
            crate::harness::HarnessConfig::default(),
        );

        let state = mgr.create_new(".sisyphus/plans/p1.json", "Harness 计划", 4, "sess-1");
        assert!(state.harness.is_none());

        mgr.attach_harness_snapshot(contract.clone(), Some("完成了第一段执行".into()));
        let json = mgr.save().unwrap();

        assert!(json.contains("实现 Harness"));
        assert!(json.contains("完成了第一段执行"));
    }
}

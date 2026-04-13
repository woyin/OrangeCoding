//! # 代理管道模块
//!
//! 编排消息处理流程：输入 → 钩子 → AI → TTSR → 工具执行 → 钩子 → 输出。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// 数据结构
// ---------------------------------------------------------------------------

/// 代理管道 — 编排消息处理流程
pub struct AgentPipeline {
    /// 管道配置
    config: super::agent_config::AgentConfig,
    /// 管道步骤序列
    steps: Vec<PipelineStep>,
}

/// 管道步骤
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PipelineStep {
    /// 预处理钩子
    PreHook,
    /// 规则注入
    InjectRules,
    /// AI 调用
    AiCall,
    /// TTSR 处理
    TtsrProcess,
    /// 工具执行
    ToolExecution,
    /// 后处理钩子
    PostHook,
    /// 记忆提取
    MemoryExtraction,
    /// 上下文压缩检查
    CompactionCheck,
}

/// 管道上下文 — 贯穿管道各步骤的状态
#[derive(Clone, Debug)]
pub struct PipelineContext {
    /// 用户输入
    pub input: String,
    /// AI 输出
    pub output: Option<String>,
    /// 工具调用记录
    pub tool_calls: Vec<PipelineToolCall>,
    /// 元数据（键值对）
    pub metadata: HashMap<String, String>,
    /// 是否继续执行管道
    pub should_continue: bool,
    /// 当前步骤索引
    pub current_step: usize,
}

/// 管道工具调用
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PipelineToolCall {
    /// 工具名称
    pub name: String,
    /// 调用参数
    pub arguments: serde_json::Value,
    /// 执行结果
    pub result: Option<String>,
    /// 是否执行出错
    pub is_error: bool,
}

// ---------------------------------------------------------------------------
// 管道实现
// ---------------------------------------------------------------------------

impl AgentPipeline {
    /// 创建新管道
    pub fn new(config: super::agent_config::AgentConfig) -> Self {
        Self {
            config,
            steps: Self::default_steps(),
        }
    }

    /// 获取默认管道步骤顺序
    pub fn default_steps() -> Vec<PipelineStep> {
        vec![
            PipelineStep::PreHook,
            PipelineStep::InjectRules,
            PipelineStep::AiCall,
            PipelineStep::TtsrProcess,
            PipelineStep::ToolExecution,
            PipelineStep::PostHook,
            PipelineStep::MemoryExtraction,
            PipelineStep::CompactionCheck,
        ]
    }

    /// 创建管道上下文
    pub fn create_context(input: &str) -> PipelineContext {
        PipelineContext {
            input: input.to_string(),
            output: None,
            tool_calls: Vec::new(),
            metadata: HashMap::new(),
            should_continue: true,
            current_step: 0,
        }
    }

    /// 获取当前步骤
    pub fn current_step(&self, ctx: &PipelineContext) -> Option<&PipelineStep> {
        self.steps.get(ctx.current_step)
    }

    /// 推进到下一步，返回是否还有更多步骤
    pub fn advance(&self, ctx: &mut PipelineContext) -> bool {
        if ctx.current_step + 1 < self.steps.len() {
            ctx.current_step += 1;
            true
        } else {
            ctx.should_continue = false;
            false
        }
    }

    /// 检查管道是否完成
    pub fn is_complete(&self, ctx: &PipelineContext) -> bool {
        !ctx.should_continue || ctx.current_step >= self.steps.len()
    }

    /// 获取步骤数量
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }
}

// ===========================================================================
// 测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_config::AgentConfig;

    /// 测试默认步骤顺序
    #[test]
    fn test_default_steps_order() {
        let steps = AgentPipeline::default_steps();
        assert_eq!(steps.len(), 8);
        assert_eq!(steps[0], PipelineStep::PreHook);
        assert_eq!(steps[1], PipelineStep::InjectRules);
        assert_eq!(steps[2], PipelineStep::AiCall);
        assert_eq!(steps[3], PipelineStep::TtsrProcess);
        assert_eq!(steps[4], PipelineStep::ToolExecution);
        assert_eq!(steps[5], PipelineStep::PostHook);
        assert_eq!(steps[6], PipelineStep::MemoryExtraction);
        assert_eq!(steps[7], PipelineStep::CompactionCheck);
    }

    /// 测试创建上下文
    #[test]
    fn test_create_context() {
        let ctx = AgentPipeline::create_context("你好");
        assert_eq!(ctx.input, "你好");
        assert!(ctx.output.is_none());
        assert!(ctx.tool_calls.is_empty());
        assert!(ctx.metadata.is_empty());
        assert!(ctx.should_continue);
        assert_eq!(ctx.current_step, 0);
    }

    /// 测试逐步推进管道
    #[test]
    fn test_advance_through_pipeline() {
        let pipeline = AgentPipeline::new(AgentConfig::default());
        let mut ctx = AgentPipeline::create_context("测试输入");

        // 从第 0 步推进到最后一步
        let total = pipeline.step_count();
        for i in 0..total - 1 {
            assert_eq!(ctx.current_step, i);
            assert!(pipeline.advance(&mut ctx));
        }
        // 最后一步推进应返回 false
        assert!(!pipeline.advance(&mut ctx));
        assert!(!ctx.should_continue);
    }

    /// 测试管道完成检测
    #[test]
    fn test_is_complete() {
        let pipeline = AgentPipeline::new(AgentConfig::default());
        let mut ctx = AgentPipeline::create_context("测试");

        // 刚创建时未完成
        assert!(!pipeline.is_complete(&ctx));

        // 走完所有步骤后完成
        while pipeline.advance(&mut ctx) {}
        assert!(pipeline.is_complete(&ctx));
    }

    /// 测试获取当前步骤
    #[test]
    fn test_current_step() {
        let pipeline = AgentPipeline::new(AgentConfig::default());
        let mut ctx = AgentPipeline::create_context("测试");

        assert_eq!(pipeline.current_step(&ctx), Some(&PipelineStep::PreHook));

        pipeline.advance(&mut ctx);
        assert_eq!(
            pipeline.current_step(&ctx),
            Some(&PipelineStep::InjectRules)
        );

        pipeline.advance(&mut ctx);
        assert_eq!(pipeline.current_step(&ctx), Some(&PipelineStep::AiCall));
    }

    /// 测试步骤数量
    #[test]
    fn test_step_count() {
        let pipeline = AgentPipeline::new(AgentConfig::default());
        assert_eq!(pipeline.step_count(), 8);
    }

    /// 测试管道上下文元数据
    #[test]
    fn test_pipeline_context_metadata() {
        let mut ctx = AgentPipeline::create_context("带元数据的输入");
        ctx.metadata
            .insert("session_id".to_string(), "abc-123".to_string());
        ctx.metadata
            .insert("user".to_string(), "test_user".to_string());

        assert_eq!(ctx.metadata.get("session_id").unwrap(), "abc-123");
        assert_eq!(ctx.metadata.get("user").unwrap(), "test_user");
        assert_eq!(ctx.metadata.len(), 2);
    }

    /// 测试工具调用追踪
    #[test]
    fn test_tool_call_tracking() {
        let mut ctx = AgentPipeline::create_context("执行工具");
        ctx.tool_calls.push(PipelineToolCall {
            name: "bash".to_string(),
            arguments: serde_json::json!({"command": "ls"}),
            result: Some("file1.rs\nfile2.rs".to_string()),
            is_error: false,
        });
        ctx.tool_calls.push(PipelineToolCall {
            name: "read".to_string(),
            arguments: serde_json::json!({"path": "src/main.rs"}),
            result: None,
            is_error: true,
        });

        assert_eq!(ctx.tool_calls.len(), 2);
        assert_eq!(ctx.tool_calls[0].name, "bash");
        assert!(!ctx.tool_calls[0].is_error);
        assert!(ctx.tool_calls[0].result.is_some());
        assert_eq!(ctx.tool_calls[1].name, "read");
        assert!(ctx.tool_calls[1].is_error);
        assert!(ctx.tool_calls[1].result.is_none());
    }
}

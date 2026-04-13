//! # 钩子系统
//!
//! 提供生命周期钩子机制，在代理执行的关键节点拦截并处理事件。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;

/// 钩子事件类型
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEvent {
    /// 会话开始前
    PreSession,
    /// 会话结束后
    PostSession,
    /// 消息发送前
    PreMessage,
    /// 消息接收后
    PostMessage,
    /// 工具调用前
    PreToolCall,
    /// 工具调用后
    PostToolCall,
    /// 上下文压缩前
    PreCompaction,
    /// 上下文压缩后
    PostCompaction,
}

/// 钩子动作
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HookAction {
    /// 继续执行
    Continue,
    /// 修改内容后继续
    Modify(String),
    /// 阻止执行
    Block(String),
    /// 跳过后续钩子
    Skip,
}

/// 钩子定义
#[derive(Clone, Debug)]
pub struct HookDef {
    /// 钩子名称
    pub name: String,
    /// 触发事件
    pub event: HookEvent,
    /// 优先级（数值越小优先级越高）
    pub priority: i32,
    /// 处理器
    pub handler: HookHandler,
}

/// 钩子处理器
#[derive(Clone, Debug)]
pub enum HookHandler {
    /// 内联动作（用于内置钩子）
    Inline(String),
    /// 外部脚本路径
    Script(std::path::PathBuf),
}

/// 钩子上下文
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HookContext {
    /// 触发事件
    pub event: HookEvent,
    /// 上下文数据
    pub data: HashMap<String, serde_json::Value>,
}

/// 钩子注册表
pub struct HookRegistry {
    hooks: Vec<HookDef>,
}

impl HookRegistry {
    /// 创建空的钩子注册表
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// 注册钩子
    pub fn register(&mut self, hook: HookDef) {
        self.hooks.push(hook);
    }

    /// 注销指定名称的钩子，返回是否成功移除
    pub fn unregister(&mut self, name: &str) -> bool {
        let before = self.hooks.len();
        self.hooks.retain(|h| h.name != name);
        self.hooks.len() < before
    }

    /// 获取指定事件的所有钩子（按优先级升序排列）
    pub fn get_hooks_for(&self, event: &HookEvent) -> Vec<&HookDef> {
        let mut matched: Vec<&HookDef> = self.hooks.iter().filter(|h| &h.event == event).collect();
        matched.sort_by_key(|h| h.priority);
        matched
    }

    /// 执行钩子链并返回最终动作
    ///
    /// 内联处理器格式：
    /// - `"continue"` → Continue
    /// - `"block:<原因>"` → Block
    /// - `"modify:<内容>"` → Modify
    /// - `"skip"` → Skip
    pub fn execute_hooks(&self, ctx: &HookContext) -> HookAction {
        let hooks = self.get_hooks_for(&ctx.event);
        if hooks.is_empty() {
            return HookAction::Continue;
        }

        for hook in &hooks {
            let action = match &hook.handler {
                HookHandler::Inline(cmd) => parse_inline_action(cmd),
                HookHandler::Script(_) => HookAction::Continue,
            };
            match &action {
                HookAction::Block(_) | HookAction::Skip | HookAction::Modify(_) => return action,
                HookAction::Continue => continue,
            }
        }

        HookAction::Continue
    }

    /// 返回已注册钩子数量
    pub fn count(&self) -> usize {
        self.hooks.len()
    }

    /// 清空所有钩子
    pub fn clear(&mut self) {
        self.hooks.clear();
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 解析内联动作字符串为 `HookAction`
fn parse_inline_action(cmd: &str) -> HookAction {
    if cmd == "continue" {
        HookAction::Continue
    } else if cmd == "skip" {
        HookAction::Skip
    } else if let Some(reason) = cmd.strip_prefix("block:") {
        HookAction::Block(reason.to_string())
    } else if let Some(content) = cmd.strip_prefix("modify:") {
        HookAction::Modify(content.to_string())
    } else {
        HookAction::Continue
    }
}

/// 扩展钩子事件类型（用于内置钩子绑定）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEventType {
    /// 工具调用前
    PreToolUse,
    /// 工具调用后
    PostToolUse,
    /// 消息事件
    Message,
    /// 通用事件
    Event,
    /// 变换事件
    Transform,
    /// 参数注入事件
    Params,
}

/// 钩子优先级（数值越小优先级越高）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum HookPriority {
    /// 关键（最高优先级）
    Critical,
    /// 高
    High,
    /// 普通
    Normal,
    /// 低
    Low,
}

impl HookPriority {
    /// 返回优先级对应的数值
    pub fn value(&self) -> u8 {
        match self {
            HookPriority::Critical => 0,
            HookPriority::High => 1,
            HookPriority::Normal => 2,
            HookPriority::Low => 3,
        }
    }
}

/// 钩子执行结果
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HookResult {
    /// 继续执行
    Continue,
    /// 阻止执行并附带原因
    Block(String),
    /// 修改后的数据
    Modified(serde_json::Value),
}

/// 内置钩子类型枚举（26 个内置钩子）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuiltinHook {
    /// 关键词检测器
    KeywordDetector,
    /// 思考模式
    ThinkMode,
    /// 注释检查器
    CommentChecker,
    /// 编辑错误恢复
    EditErrorRecovery,
    /// 写入已有文件保护
    WriteExistingFileGuard,
    /// 会话恢复
    SessionRecovery,
    /// TODO 延续执行器
    TodoContinuationEnforcer,
    /// 压缩时 TODO 保留器
    CompactionTodoPreserver,
    /// 后台通知
    BackgroundNotification,
    /// 工具输出截断器
    ToolOutputTruncator,
    /// 循环检测
    RalphLoop,
    /// 启动工作
    StartWork,
    /// 停止延续保护
    StopContinuationGuard,
    /// 仅限 Prometheus MD
    PrometheusMdOnly,
    /// 哈希行读取增强器
    HashlineReadEnhancer,
    /// 哈希行编辑差异增强器
    HashlineEditDiffEnhancer,
    /// 目录代理注入器
    DirectoryAgentsInjector,
    /// 规则注入器
    RulesInjector,
    /// 压缩上下文注入器
    CompactionContextInjector,
    /// 自动更新检查器
    AutoUpdateChecker,
    /// 运行时回退
    RuntimeFallback,
    /// 模型回退
    ModelFallback,
    /// Anthropic 投入度
    AnthropicEffort,
    /// 代理使用提醒
    AgentUsageReminder,
    /// 委托任务重试
    DelegateTaskRetry,
    /// 不稳定代理监护
    UnstableAgentBabysitter,
}

impl BuiltinHook {
    /// 返回该钩子监听的事件类型列表
    pub fn event_types(&self) -> Vec<HookEventType> {
        match self {
            BuiltinHook::KeywordDetector => vec![HookEventType::Message],
            BuiltinHook::ThinkMode => vec![HookEventType::Params],
            BuiltinHook::CommentChecker => vec![HookEventType::PostToolUse],
            BuiltinHook::EditErrorRecovery => vec![HookEventType::PostToolUse],
            BuiltinHook::WriteExistingFileGuard => vec![HookEventType::PreToolUse],
            BuiltinHook::SessionRecovery => vec![HookEventType::Event],
            BuiltinHook::TodoContinuationEnforcer => {
                vec![HookEventType::Message, HookEventType::Event]
            }
            BuiltinHook::CompactionTodoPreserver => vec![HookEventType::Transform],
            BuiltinHook::BackgroundNotification => vec![HookEventType::Event],
            BuiltinHook::ToolOutputTruncator => vec![HookEventType::PostToolUse],
            BuiltinHook::RalphLoop => vec![HookEventType::Message],
            BuiltinHook::StartWork => vec![HookEventType::Event],
            BuiltinHook::StopContinuationGuard => vec![HookEventType::Message],
            BuiltinHook::PrometheusMdOnly => vec![HookEventType::PreToolUse],
            BuiltinHook::HashlineReadEnhancer => vec![HookEventType::PostToolUse],
            BuiltinHook::HashlineEditDiffEnhancer => vec![HookEventType::PreToolUse],
            BuiltinHook::DirectoryAgentsInjector => vec![HookEventType::Params],
            BuiltinHook::RulesInjector => vec![HookEventType::Params],
            BuiltinHook::CompactionContextInjector => vec![HookEventType::Transform],
            BuiltinHook::AutoUpdateChecker => vec![HookEventType::Event],
            BuiltinHook::RuntimeFallback => vec![HookEventType::Event],
            BuiltinHook::ModelFallback => vec![HookEventType::Event],
            BuiltinHook::AnthropicEffort => vec![HookEventType::Params],
            BuiltinHook::AgentUsageReminder => vec![HookEventType::PostToolUse],
            BuiltinHook::DelegateTaskRetry => vec![HookEventType::PostToolUse],
            BuiltinHook::UnstableAgentBabysitter => vec![HookEventType::PostToolUse],
        }
    }

    /// 返回钩子的蛇形命名名称
    pub fn name(&self) -> &str {
        match self {
            BuiltinHook::KeywordDetector => "keyword_detector",
            BuiltinHook::ThinkMode => "think_mode",
            BuiltinHook::CommentChecker => "comment_checker",
            BuiltinHook::EditErrorRecovery => "edit_error_recovery",
            BuiltinHook::WriteExistingFileGuard => "write_existing_file_guard",
            BuiltinHook::SessionRecovery => "session_recovery",
            BuiltinHook::TodoContinuationEnforcer => "todo_continuation_enforcer",
            BuiltinHook::CompactionTodoPreserver => "compaction_todo_preserver",
            BuiltinHook::BackgroundNotification => "background_notification",
            BuiltinHook::ToolOutputTruncator => "tool_output_truncator",
            BuiltinHook::RalphLoop => "ralph_loop",
            BuiltinHook::StartWork => "start_work",
            BuiltinHook::StopContinuationGuard => "stop_continuation_guard",
            BuiltinHook::PrometheusMdOnly => "prometheus_md_only",
            BuiltinHook::HashlineReadEnhancer => "hashline_read_enhancer",
            BuiltinHook::HashlineEditDiffEnhancer => "hashline_edit_diff_enhancer",
            BuiltinHook::DirectoryAgentsInjector => "directory_agents_injector",
            BuiltinHook::RulesInjector => "rules_injector",
            BuiltinHook::CompactionContextInjector => "compaction_context_injector",
            BuiltinHook::AutoUpdateChecker => "auto_update_checker",
            BuiltinHook::RuntimeFallback => "runtime_fallback",
            BuiltinHook::ModelFallback => "model_fallback",
            BuiltinHook::AnthropicEffort => "anthropic_effort",
            BuiltinHook::AgentUsageReminder => "agent_usage_reminder",
            BuiltinHook::DelegateTaskRetry => "delegate_task_retry",
            BuiltinHook::UnstableAgentBabysitter => "unstable_agent_babysitter",
        }
    }

    /// 返回所有 26 个内置钩子变体
    pub fn all() -> Vec<BuiltinHook> {
        vec![
            BuiltinHook::KeywordDetector,
            BuiltinHook::ThinkMode,
            BuiltinHook::CommentChecker,
            BuiltinHook::EditErrorRecovery,
            BuiltinHook::WriteExistingFileGuard,
            BuiltinHook::SessionRecovery,
            BuiltinHook::TodoContinuationEnforcer,
            BuiltinHook::CompactionTodoPreserver,
            BuiltinHook::BackgroundNotification,
            BuiltinHook::ToolOutputTruncator,
            BuiltinHook::RalphLoop,
            BuiltinHook::StartWork,
            BuiltinHook::StopContinuationGuard,
            BuiltinHook::PrometheusMdOnly,
            BuiltinHook::HashlineReadEnhancer,
            BuiltinHook::HashlineEditDiffEnhancer,
            BuiltinHook::DirectoryAgentsInjector,
            BuiltinHook::RulesInjector,
            BuiltinHook::CompactionContextInjector,
            BuiltinHook::AutoUpdateChecker,
            BuiltinHook::RuntimeFallback,
            BuiltinHook::ModelFallback,
            BuiltinHook::AnthropicEffort,
            BuiltinHook::AgentUsageReminder,
            BuiltinHook::DelegateTaskRetry,
            BuiltinHook::UnstableAgentBabysitter,
        ]
    }
}

/// 扩展钩子注册表（按优先级排序）
pub struct ExtendedHookRegistry {
    /// 注册的钩子列表：(优先级, 名称, 事件类型列表)
    hooks: Vec<(HookPriority, String, Vec<HookEventType>)>,
}

impl ExtendedHookRegistry {
    /// 创建空的扩展注册表
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// 注册钩子并按优先级排序
    pub fn register(
        &mut self,
        name: impl Into<String>,
        priority: HookPriority,
        events: Vec<HookEventType>,
    ) {
        self.hooks.push((priority, name.into(), events));
        self.hooks.sort_by_key(|(p, _, _)| *p);
    }

    /// 注销指定名称的钩子，返回是否存在并移除
    pub fn unregister(&mut self, name: &str) -> bool {
        let before = self.hooks.len();
        self.hooks.retain(|(_, n, _)| n != name);
        self.hooks.len() < before
    }

    /// 触发事件，返回监听该事件的钩子名称（按优先级顺序）
    pub fn fire(&self, event: &HookEventType) -> Vec<&str> {
        self.hooks
            .iter()
            .filter(|(_, _, events)| events.contains(event))
            .map(|(_, name, _)| name.as_str())
            .collect()
    }

    /// 返回已注册钩子数量
    pub fn count(&self) -> usize {
        self.hooks.len()
    }
}

impl Default for ExtendedHookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 已禁用钩子集合
pub struct DisabledHooks {
    /// 被禁用的钩子名称集合
    disabled: HashSet<String>,
}

impl DisabledHooks {
    /// 创建空的禁用集合
    pub fn new() -> Self {
        Self {
            disabled: HashSet::new(),
        }
    }

    /// 禁用指定钩子
    pub fn disable(&mut self, name: impl Into<String>) {
        self.disabled.insert(name.into());
    }

    /// 启用指定钩子（从禁用集合移除），返回是否之前处于禁用状态
    pub fn enable(&mut self, name: &str) -> bool {
        self.disabled.remove(name)
    }

    /// 检查指定钩子是否被禁用
    pub fn is_disabled(&self, name: &str) -> bool {
        self.disabled.contains(name)
    }

    /// 返回已禁用钩子数量
    pub fn count(&self) -> usize {
        self.disabled.len()
    }
}

impl Default for DisabledHooks {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// 测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助函数：构造内联钩子
    fn make_hook(name: &str, event: HookEvent, priority: i32, action: &str) -> HookDef {
        HookDef {
            name: name.to_string(),
            event,
            priority,
            handler: HookHandler::Inline(action.to_string()),
        }
    }

    #[test]
    fn test_register_hook() {
        let mut reg = HookRegistry::new();
        reg.register(make_hook("h1", HookEvent::PreSession, 0, "continue"));
        assert_eq!(reg.count(), 1);
    }

    #[test]
    fn test_unregister_hook() {
        let mut reg = HookRegistry::new();
        reg.register(make_hook("h1", HookEvent::PreSession, 0, "continue"));
        assert!(reg.unregister("h1"));
        assert_eq!(reg.count(), 0);
        // 注销不存在的钩子应返回 false
        assert!(!reg.unregister("nonexistent"));
    }

    #[test]
    fn test_get_hooks_for_event() {
        let mut reg = HookRegistry::new();
        reg.register(make_hook("h1", HookEvent::PreSession, 0, "continue"));
        reg.register(make_hook("h2", HookEvent::PostSession, 0, "continue"));
        reg.register(make_hook("h3", HookEvent::PreSession, 1, "continue"));

        let hooks = reg.get_hooks_for(&HookEvent::PreSession);
        assert_eq!(hooks.len(), 2);
        assert!(hooks.iter().all(|h| h.event == HookEvent::PreSession));
    }

    #[test]
    fn test_hooks_sorted_by_priority() {
        let mut reg = HookRegistry::new();
        reg.register(make_hook("low", HookEvent::PreMessage, 10, "continue"));
        reg.register(make_hook("high", HookEvent::PreMessage, 1, "continue"));
        reg.register(make_hook("mid", HookEvent::PreMessage, 5, "continue"));

        let hooks = reg.get_hooks_for(&HookEvent::PreMessage);
        assert_eq!(hooks[0].name, "high");
        assert_eq!(hooks[1].name, "mid");
        assert_eq!(hooks[2].name, "low");
    }

    #[test]
    fn test_execute_continue() {
        let mut reg = HookRegistry::new();
        reg.register(make_hook("h1", HookEvent::PreSession, 0, "continue"));

        let ctx = HookContext {
            event: HookEvent::PreSession,
            data: HashMap::new(),
        };
        assert_eq!(reg.execute_hooks(&ctx), HookAction::Continue);
    }

    #[test]
    fn test_execute_block() {
        let mut reg = HookRegistry::new();
        reg.register(make_hook(
            "blocker",
            HookEvent::PreToolCall,
            0,
            "block:denied",
        ));

        let ctx = HookContext {
            event: HookEvent::PreToolCall,
            data: HashMap::new(),
        };
        assert_eq!(
            reg.execute_hooks(&ctx),
            HookAction::Block("denied".to_string())
        );
    }

    #[test]
    fn test_no_hooks_returns_continue() {
        let reg = HookRegistry::new();
        let ctx = HookContext {
            event: HookEvent::PreSession,
            data: HashMap::new(),
        };
        assert_eq!(reg.execute_hooks(&ctx), HookAction::Continue);
    }

    #[test]
    fn test_clear() {
        let mut reg = HookRegistry::new();
        reg.register(make_hook("h1", HookEvent::PreSession, 0, "continue"));
        reg.register(make_hook("h2", HookEvent::PostSession, 0, "continue"));
        reg.clear();
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn test_count() {
        let mut reg = HookRegistry::new();
        assert_eq!(reg.count(), 0);
        reg.register(make_hook("h1", HookEvent::PreSession, 0, "continue"));
        assert_eq!(reg.count(), 1);
        reg.register(make_hook("h2", HookEvent::PostSession, 0, "continue"));
        assert_eq!(reg.count(), 2);
    }

    #[test]
    fn test_hook_context_serialization() {
        let mut data = HashMap::new();
        data.insert("key".to_string(), serde_json::json!("value"));
        let ctx = HookContext {
            event: HookEvent::PreMessage,
            data,
        };

        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: HookContext = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event, ctx.event);
        assert_eq!(deserialized.data["key"], serde_json::json!("value"));
    }

    // =======================================================================
    // 扩展类型测试
    // =======================================================================

    #[test]
    fn test_hook_event_type_variants() {
        // 验证所有 6 个 HookEventType 变体存在
        let variants = vec![
            HookEventType::PreToolUse,
            HookEventType::PostToolUse,
            HookEventType::Message,
            HookEventType::Event,
            HookEventType::Transform,
            HookEventType::Params,
        ];
        assert_eq!(variants.len(), 6);
        // 每个变体应与自身相等
        for v in &variants {
            assert_eq!(v, v);
        }
    }

    #[test]
    fn test_hook_priority_ordering() {
        // Critical < High < Normal < Low
        assert!(HookPriority::Critical < HookPriority::High);
        assert!(HookPriority::High < HookPriority::Normal);
        assert!(HookPriority::Normal < HookPriority::Low);
    }

    #[test]
    fn test_hook_priority_values() {
        assert_eq!(HookPriority::Critical.value(), 0);
        assert_eq!(HookPriority::High.value(), 1);
        assert_eq!(HookPriority::Normal.value(), 2);
        assert_eq!(HookPriority::Low.value(), 3);
    }

    #[test]
    fn test_hook_result_continue() {
        let result = HookResult::Continue;
        assert_eq!(result, HookResult::Continue);
    }

    #[test]
    fn test_hook_result_block() {
        let result = HookResult::Block("拒绝访问".to_string());
        if let HookResult::Block(reason) = &result {
            assert_eq!(reason, "拒绝访问");
        } else {
            panic!("期望 Block 变体");
        }
    }

    #[test]
    fn test_hook_result_modified() {
        let val = serde_json::json!({"key": "value"});
        let result = HookResult::Modified(val.clone());
        if let HookResult::Modified(v) = &result {
            assert_eq!(v, &val);
        } else {
            panic!("期望 Modified 变体");
        }
    }

    #[test]
    fn test_builtin_hook_all_count() {
        let all = BuiltinHook::all();
        assert_eq!(all.len(), 26);
    }

    #[test]
    fn test_builtin_hook_event_types() {
        // 验证部分钩子的事件类型映射
        assert_eq!(
            BuiltinHook::KeywordDetector.event_types(),
            vec![HookEventType::Message]
        );
        assert_eq!(
            BuiltinHook::WriteExistingFileGuard.event_types(),
            vec![HookEventType::PreToolUse]
        );
        assert_eq!(
            BuiltinHook::TodoContinuationEnforcer.event_types(),
            vec![HookEventType::Message, HookEventType::Event]
        );
        assert_eq!(
            BuiltinHook::CompactionTodoPreserver.event_types(),
            vec![HookEventType::Transform]
        );
    }

    #[test]
    fn test_builtin_hook_name() {
        assert_eq!(BuiltinHook::KeywordDetector.name(), "keyword_detector");
        assert_eq!(BuiltinHook::ThinkMode.name(), "think_mode");
        assert_eq!(BuiltinHook::RalphLoop.name(), "ralph_loop");
        assert_eq!(
            BuiltinHook::UnstableAgentBabysitter.name(),
            "unstable_agent_babysitter"
        );
        assert_eq!(BuiltinHook::AutoUpdateChecker.name(), "auto_update_checker");
    }

    #[test]
    fn test_extended_registry_register() {
        let mut reg = ExtendedHookRegistry::new();
        reg.register("hook_a", HookPriority::Normal, vec![HookEventType::Message]);
        reg.register("hook_b", HookPriority::High, vec![HookEventType::Event]);
        assert_eq!(reg.count(), 2);
    }

    #[test]
    fn test_extended_registry_unregister() {
        let mut reg = ExtendedHookRegistry::new();
        reg.register("hook_a", HookPriority::Normal, vec![HookEventType::Message]);
        assert!(reg.unregister("hook_a"));
        assert_eq!(reg.count(), 0);
        // 注销不存在的钩子应返回 false
        assert!(!reg.unregister("hook_a"));
    }

    #[test]
    fn test_extended_registry_fire() {
        let mut reg = ExtendedHookRegistry::new();
        reg.register(
            "msg_hook",
            HookPriority::Normal,
            vec![HookEventType::Message],
        );
        reg.register(
            "event_hook",
            HookPriority::Normal,
            vec![HookEventType::Event],
        );
        reg.register(
            "both_hook",
            HookPriority::High,
            vec![HookEventType::Message, HookEventType::Event],
        );

        let fired = reg.fire(&HookEventType::Message);
        assert_eq!(fired.len(), 2);
        assert!(fired.contains(&"msg_hook"));
        assert!(fired.contains(&"both_hook"));

        let fired_event = reg.fire(&HookEventType::Event);
        assert_eq!(fired_event.len(), 2);
    }

    #[test]
    fn test_extended_registry_priority_order() {
        let mut reg = ExtendedHookRegistry::new();
        reg.register("low_hook", HookPriority::Low, vec![HookEventType::Message]);
        reg.register(
            "critical_hook",
            HookPriority::Critical,
            vec![HookEventType::Message],
        );
        reg.register(
            "normal_hook",
            HookPriority::Normal,
            vec![HookEventType::Message],
        );

        let fired = reg.fire(&HookEventType::Message);
        // 应按 Critical, Normal, Low 顺序返回
        assert_eq!(fired, vec!["critical_hook", "normal_hook", "low_hook"]);
    }

    #[test]
    fn test_disabled_hooks_new() {
        let dh = DisabledHooks::new();
        assert_eq!(dh.count(), 0);
    }

    #[test]
    fn test_disabled_hooks_disable_enable() {
        let mut dh = DisabledHooks::new();
        dh.disable("hook_a");
        assert_eq!(dh.count(), 1);
        assert!(dh.is_disabled("hook_a"));
        // 启用后应移除
        assert!(dh.enable("hook_a"));
        assert_eq!(dh.count(), 0);
        // 再次启用应返回 false
        assert!(!dh.enable("hook_a"));
    }

    #[test]
    fn test_disabled_hooks_is_disabled() {
        let mut dh = DisabledHooks::new();
        assert!(!dh.is_disabled("hook_a"));
        dh.disable("hook_a");
        assert!(dh.is_disabled("hook_a"));
        assert!(!dh.is_disabled("hook_b"));
    }

    #[test]
    fn test_hook_result_serialization() {
        // 测试 Continue 序列化
        let cont = HookResult::Continue;
        let json = serde_json::to_string(&cont).unwrap();
        let de: HookResult = serde_json::from_str(&json).unwrap();
        assert_eq!(de, HookResult::Continue);

        // 测试 Block 序列化
        let block = HookResult::Block("原因".to_string());
        let json = serde_json::to_string(&block).unwrap();
        let de: HookResult = serde_json::from_str(&json).unwrap();
        assert_eq!(de, HookResult::Block("原因".to_string()));

        // 测试 Modified 序列化
        let modified = HookResult::Modified(serde_json::json!({"a": 1}));
        let json = serde_json::to_string(&modified).unwrap();
        let de: HookResult = serde_json::from_str(&json).unwrap();
        assert_eq!(de, HookResult::Modified(serde_json::json!({"a": 1})));
    }
}

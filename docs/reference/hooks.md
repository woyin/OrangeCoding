# Hook 系统参考手册

> OrangeCoding Hook 系统允许在 Agent 生命周期的关键节点插入自定义逻辑，实现行为扩展和监控。

## 目录

- [概述](#概述)
- [Hook 事件类型](#hook-事件类型)
  - [HookEvent（8 种内置事件）](#hookevent8-种内置事件)
  - [HookEventType（6 种分类）](#hookeventtype6-种分类)
- [Hook 优先级](#hook-优先级)
- [Hook 结果](#hook-结果)
- [Hook 定义](#hook-定义)
- [26 种内置 Hook](#26-种内置-hook)
- [Hook 注册表](#hook-注册表)
- [Hook 触发流程](#hook-触发流程)
- [自定义 Hook 编写](#自定义-hook-编写)
- [Hook 上下文](#hook-上下文)
- [最佳实践](#最佳实践)

---

## 概述

Hook 系统位于 `chengcoding-agent` crate 的 `hooks.rs` 模块中，提供了一套可扩展的事件拦截机制。通过注册 Hook，可以在 Agent 执行流程的关键点进行：

- **监控**：记录日志、收集指标
- **修改**：转换消息内容、调整参数
- **拦截**：阻止特定操作的执行
- **增强**：注入额外上下文或行为

```
┌─────────────────────────────────────────────────────┐
│                   Agent 执行流程                      │
│                                                     │
│  用户输入 → [PreMessage Hook] → 消息处理              │
│         → [PreToolCall Hook] → 工具执行               │
│         → [PostToolCall Hook] → 结果处理              │
│         → [PostMessage Hook] → 响应输出               │
│                                                     │
│  每个节点的 Hook 按优先级顺序执行                       │
│  任何 Hook 可以修改数据或阻止后续执行                    │
└─────────────────────────────────────────────────────┘
```

---

## Hook 事件类型

### HookEvent（8 种内置事件）

`HookEvent` 枚举定义了 Agent 生命周期中可以挂载 Hook 的 8 个关键节点：

```rust
pub enum HookEvent {
    /// 会话开始前触发
    PreSession,

    /// 会话结束后触发
    PostSession,

    /// 消息处理前触发（用户消息到达时）
    PreMessage,

    /// 消息处理后触发（AI 响应生成后）
    PostMessage,

    /// 工具调用执行前触发
    PreToolCall,

    /// 工具调用执行后触发
    PostToolCall,

    /// 上下文压缩前触发
    PreCompaction,

    /// 上下文压缩后触发
    PostCompaction,
}
```

#### 事件详情

| 事件 | 触发时机 | 典型用途 |
|------|----------|----------|
| `PreSession` | 会话创建后、首条消息前 | 初始化资源、设置系统提示 |
| `PostSession` | 会话结束、资源释放前 | 清理资源、保存状态、生成摘要 |
| `PreMessage` | 用户消息到达，送入 AI 模型前 | 内容过滤、消息转换、注入上下文 |
| `PostMessage` | AI 响应生成后，返回用户前 | 响应审查、格式调整、日志记录 |
| `PreToolCall` | 工具调用解析后、执行前 | 权限检查、参数验证、安全审计 |
| `PostToolCall` | 工具执行完成、结果返回前 | 结果过滤、脱敏处理、错误增强 |
| `PreCompaction` | 上下文压缩开始前 | 保存关键信息、标记不可压缩内容 |
| `PostCompaction` | 上下文压缩完成后 | 验证压缩质量、记录压缩统计 |

---

### HookEventType（6 种分类）

`HookEventType` 从更高层面将 Hook 事件分为 6 个类别：

```rust
pub enum HookEventType {
    /// 工具调用前拦截
    PreToolUse,

    /// 工具调用后拦截
    PostToolUse,

    /// 消息级别事件
    Message,

    /// 通用事件
    Event,

    /// 数据转换
    Transform,

    /// 参数修改
    Params,
}
```

#### 分类映射

| 分类 | 对应 HookEvent | 说明 |
|------|----------------|------|
| `PreToolUse` | `PreToolCall` | 工具执行前的拦截点，可阻止执行 |
| `PostToolUse` | `PostToolCall` | 工具执行后的后处理点 |
| `Message` | `PreMessage`, `PostMessage` | 消息处理相关的 Hook |
| `Event` | `PreSession`, `PostSession` | 生命周期事件 |
| `Transform` | `PreCompaction`, `PostCompaction` | 数据转换相关 |
| `Params` | 所有事件 | 参数级别的修改 |

---

## Hook 优先级

`HookPriority` 定义了 Hook 的执行顺序。数值越小，优先级越高：

```rust
pub enum HookPriority {
    /// 最高优先级（0）—— 安全相关 Hook
    Critical = 0,

    /// 高优先级（1）—— 核心功能 Hook
    High = 1,

    /// 普通优先级（2）—— 默认级别
    Normal = 2,

    /// 低优先级（3）—— 辅助功能 Hook
    Low = 3,
}
```

### 执行顺序

```
Critical (0) → High (1) → Normal (2) → Low (3)
     ↓            ↓            ↓            ↓
  安全检查      权限验证      业务逻辑      日志记录
```

**同优先级 Hook**：按注册顺序执行（先注册先执行）。

### 优先级使用建议

| 优先级 | 适用场景 | 示例 |
|--------|----------|------|
| `Critical` | 安全策略、路径验证、密钥检测 | 阻止访问 `/etc/passwd` |
| `High` | 权限检查、资源限制 | 验证 Bash 执行权限 |
| `Normal` | 业务逻辑、内容增强 | 添加代码审查注释 |
| `Low` | 日志记录、指标收集 | 记录工具调用统计 |

---

## Hook 结果

`HookAction` 枚举定义了 Hook 执行后的行为指令：

```rust
pub enum HookAction {
    /// 继续执行后续 Hook 和原操作
    Continue,

    /// 修改数据后继续执行
    Modify(String),

    /// 阻止后续执行，返回原因
    Block(String),

    /// 跳过后续 Hook，直接执行原操作
    Skip,
}
```

### 结果行为

| 结果 | 后续 Hook | 原操作 | 典型用途 |
|------|-----------|--------|----------|
| `Continue` | ✅ 继续 | ✅ 执行 | 监控类 Hook：观察但不干预 |
| `Modify(data)` | ✅ 继续 | ✅ 使用修改后数据执行 | 转换类 Hook：修改参数或内容 |
| `Block(reason)` | ❌ 停止 | ❌ 不执行 | 拦截类 Hook：阻止危险操作 |
| `Skip` | ❌ 跳过 | ✅ 执行 | 快速通过：跳过剩余低优先级 Hook |

### 结果流示意

```
Hook 1 (Critical) → Continue
  ↓
Hook 2 (High) → Modify("已修改数据")
  ↓（使用修改后数据）
Hook 3 (Normal) → Block("安全违规")
  ↓
❌ 执行被阻止，返回 "安全违规" 错误
```

---

## Hook 定义

### HookDef 结构体

```rust
pub struct HookDef {
    /// Hook 唯一标识符
    pub name: String,

    /// Hook 描述
    pub description: String,

    /// 绑定的事件类型
    pub event: HookEvent,

    /// 事件分类
    pub event_type: HookEventType,

    /// 执行优先级
    pub priority: HookPriority,

    /// 是否启用
    pub enabled: bool,

    /// Hook 处理器（二选一）
    pub handler: Option<HookHandler>,

    /// 外部脚本路径
    pub script: Option<PathBuf>,
}
```

### HookHandler 类型

```rust
/// 内联 Hook 处理函数
pub type HookHandler = Arc<dyn Fn(&HookContext) -> HookAction + Send + Sync>;
```

---

## 26 种内置 Hook

OrangeCoding 预注册了以下内置 Hook，覆盖安全、监控、转换等核心功能：

### 安全类 Hook（Critical 优先级）

| # | Hook 名称 | 事件 | 描述 |
|---|-----------|------|------|
| 1 | `security_path_check` | `PreToolCall` | 验证文件路径安全性 |
| 2 | `security_bash_guard` | `PreToolCall` | 阻止危险 Shell 命令 |
| 3 | `security_secret_scan` | `PostToolCall` | 扫描输出中的密钥泄露 |
| 4 | `security_sandbox_enforce` | `PreToolCall` | 强制沙箱路径限制 |
| 5 | `security_network_guard` | `PreToolCall` | 网络请求安全检查 |

### 权限类 Hook（High 优先级）

| # | Hook 名称 | 事件 | 描述 |
|---|-----------|------|------|
| 6 | `permission_edit_check` | `PreToolCall` | 检查文件编辑权限 |
| 7 | `permission_bash_check` | `PreToolCall` | 检查 Bash 执行权限 |
| 8 | `permission_web_check` | `PreToolCall` | 检查网络访问权限 |
| 9 | `permission_external_dir` | `PreToolCall` | 检查外部目录访问权限 |
| 10 | `permission_doom_loop` | `PreMessage` | 检测并阻止循环执行 |

### 审计类 Hook（Normal 优先级）

| # | Hook 名称 | 事件 | 描述 |
|---|-----------|------|------|
| 11 | `audit_tool_call` | `PostToolCall` | 记录工具调用到审计链 |
| 12 | `audit_message` | `PostMessage` | 记录消息到审计日志 |
| 13 | `audit_session_start` | `PreSession` | 记录会话开始 |
| 14 | `audit_session_end` | `PostSession` | 记录会话结束 |
| 15 | `audit_model_usage` | `PostMessage` | 记录模型 Token 使用量 |

### 转换类 Hook（Normal 优先级）

| # | Hook 名称 | 事件 | 描述 |
|---|-----------|------|------|
| 16 | `transform_sanitize_output` | `PostToolCall` | 脱敏工具输出中的敏感信息 |
| 17 | `transform_inject_context` | `PreMessage` | 注入上下文信息到消息 |
| 18 | `transform_compress_result` | `PostToolCall` | 压缩过长的工具输出 |
| 19 | `transform_format_response` | `PostMessage` | 格式化 AI 响应 |

### 监控类 Hook（Low 优先级）

| # | Hook 名称 | 事件 | 描述 |
|---|-----------|------|------|
| 20 | `monitor_performance` | `PostToolCall` | 记录工具执行耗时 |
| 21 | `monitor_token_usage` | `PostMessage` | 统计 Token 消耗 |
| 22 | `monitor_error_rate` | `PostToolCall` | 追踪工具错误率 |

### 生命周期 Hook（Normal 优先级）

| # | Hook 名称 | 事件 | 描述 |
|---|-----------|------|------|
| 23 | `lifecycle_init_tools` | `PreSession` | 初始化工具注册表 |
| 24 | `lifecycle_cleanup` | `PostSession` | 清理临时资源 |
| 25 | `lifecycle_compaction_guard` | `PreCompaction` | 保护关键上下文不被压缩 |
| 26 | `lifecycle_compaction_verify` | `PostCompaction` | 验证压缩后上下文完整性 |

---

## Hook 注册表

### HookRegistry 结构体

```rust
pub struct HookRegistry {
    /// 按事件类型分组的 Hook 集合
    hooks: HashMap<HookEvent, Vec<HookDef>>,
}
```

### 核心方法

```rust
impl HookRegistry {
    /// 创建空注册表
    pub fn new() -> Self;

    /// 注册新 Hook
    pub fn register(&mut self, hook: HookDef);

    /// 注销 Hook
    pub fn unregister(&mut self, name: &str) -> bool;

    /// 获取指定事件的所有 Hook（按优先级排序）
    pub fn get_hooks_for(&self, event: &HookEvent) -> Vec<&HookDef>;

    /// 执行指定事件的 Hook 链
    pub fn execute_hooks(&self, ctx: &HookContext) -> HookAction;

    /// 检查 Hook 是否已注册
    pub fn has_hook(&self, name: &str) -> bool;

    /// 启用/禁用 Hook
    pub fn set_enabled(&mut self, name: &str, enabled: bool);

    /// 列出所有已注册 Hook
    pub fn list_hooks(&self) -> Vec<&HookDef>;
}
```

### 注册示例

```rust
use chengcoding_agent::hooks::*;

let mut registry = HookRegistry::new();

// 注册内联 Hook
registry.register(HookDef {
    name: "my_custom_hook".to_string(),
    description: "自定义安全检查".to_string(),
    event: HookEvent::PreToolCall,
    event_type: HookEventType::PreToolUse,
    priority: HookPriority::High,
    enabled: true,
    handler: Some(Arc::new(|ctx| {
        if ctx.tool_name() == "bash" {
            let cmd = ctx.get_param("command").unwrap_or_default();
            if cmd.contains("rm -rf") {
                return HookAction::Block("禁止执行 rm -rf 命令".to_string());
            }
        }
        HookAction::Continue
    })),
    script: None,
});

// 注册外部脚本 Hook
registry.register(HookDef {
    name: "external_validator".to_string(),
    description: "外部验证脚本".to_string(),
    event: HookEvent::PreToolCall,
    event_type: HookEventType::PreToolUse,
    priority: HookPriority::Normal,
    enabled: true,
    handler: None,
    script: Some(PathBuf::from("hooks/validate.sh")),
});
```

---

## Hook 触发流程

### 完整执行流程

```
┌─────────────────────────────────────────────────────────┐
│                    Hook 触发流程                          │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  1. 事件发生（如 PreToolCall）                             │
│     ↓                                                   │
│  2. HookRegistry.get_hooks_for(event)                   │
│     按优先级排序：Critical → High → Normal → Low          │
│     ↓                                                   │
│  3. 构建 HookContext（包含事件数据）                        │
│     ↓                                                   │
│  4. 依次执行每个 Hook                                     │
│     ┌──────────────────────────────────────────┐        │
│     │ for hook in sorted_hooks:                │        │
│     │   if !hook.enabled: continue             │        │
│     │                                          │        │
│     │   result = hook.handler(ctx)             │        │
│     │                                          │        │
│     │   match result:                          │        │
│     │     Continue  → 继续下一个 Hook           │        │
│     │     Modify(d) → 更新 ctx，继续下一个      │        │
│     │     Block(r)  → 立即返回 Block            │        │
│     │     Skip      → 跳过剩余 Hook            │        │
│     └──────────────────────────────────────────┘        │
│     ↓                                                   │
│  5. 返回最终 HookAction                                  │
│     ↓                                                   │
│  6. 调用方根据结果决定后续行为                               │
│     Continue/Modify → 执行原操作                          │
│     Block           → 取消操作，返回错误                    │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### 时序图

```
Agent              HookRegistry          Hook_1(Critical)      Hook_2(High)       工具
  │                     │                      │                    │              │
  │──PreToolCall───────►│                      │                    │              │
  │                     │──execute(ctx)───────►│                    │              │
  │                     │◄──Continue───────────│                    │              │
  │                     │──execute(ctx)────────────────────────────►│              │
  │                     │◄──Modify(data)───────────────────────────│              │
  │                     │                      │                    │              │
  │◄──Modify(data)──────│                      │                    │              │
  │                     │                      │                    │              │
  │──execute(modified)─────────────────────────────────────────────────────────────►│
  │◄──result───────────────────────────────────────────────────────────────────────│
  │                     │                      │                    │              │
  │──PostToolCall──────►│                      │                    │              │
  │                     │──execute(ctx)───────►│                    │              │
  │                     │◄──Continue───────────│                    │              │
  │◄──Continue──────────│                      │                    │              │
  │                     │                      │                    │              │
```

---

## Hook 上下文

### HookContext 结构体

```rust
pub struct HookContext {
    /// 触发的事件类型
    pub event: HookEvent,

    /// 会话 ID
    pub session_id: SessionId,

    /// 当前消息（如适用）
    pub message: Option<Message>,

    /// 工具名称（PreToolCall/PostToolCall 时可用）
    pub tool_name: Option<String>,

    /// 工具参数（JSON）
    pub tool_params: Option<Value>,

    /// 工具执行结果（PostToolCall 时可用）
    pub tool_result: Option<String>,

    /// 元数据键值对
    pub metadata: HashMap<String, String>,
}
```

### 便捷方法

```rust
impl HookContext {
    /// 获取工具名称
    pub fn tool_name(&self) -> &str;

    /// 获取工具参数
    pub fn get_param(&self, key: &str) -> Option<String>;

    /// 获取工具执行结果
    pub fn tool_result(&self) -> Option<&str>;

    /// 设置元数据
    pub fn set_metadata(&mut self, key: &str, value: &str);

    /// 获取元数据
    pub fn get_metadata(&self, key: &str) -> Option<&str>;
}
```

---

## 自定义 Hook 编写

### 步骤一：定义 Hook

```rust
use chengcoding_agent::hooks::*;

fn create_my_hook() -> HookDef {
    HookDef {
        name: "my_logging_hook".to_string(),
        description: "记录所有工具调用的详细信息".to_string(),
        event: HookEvent::PostToolCall,
        event_type: HookEventType::PostToolUse,
        priority: HookPriority::Low,
        enabled: true,
        handler: Some(Arc::new(|ctx: &HookContext| {
            let tool = ctx.tool_name();
            let result = ctx.tool_result().unwrap_or("无结果");
            tracing::info!(
                tool = tool,
                result_len = result.len(),
                "工具调用完成"
            );
            HookAction::Continue
        })),
        script: None,
    }
}
```

### 步骤二：注册 Hook

```rust
let mut registry = HookRegistry::new();
registry.register(create_my_hook());
```

### 步骤三：外部脚本 Hook

可以使用外部脚本作为 Hook 处理器。脚本通过环境变量接收上下文：

**环境变量**:

| 变量名 | 描述 |
|--------|------|
| `OrangeCoding_HOOK_EVENT` | 事件类型（如 `PreToolCall`） |
| `OrangeCoding_HOOK_SESSION_ID` | 会话 ID |
| `OrangeCoding_HOOK_TOOL_NAME` | 工具名称 |
| `OrangeCoding_HOOK_TOOL_PARAMS` | 工具参数（JSON） |
| `OrangeCoding_HOOK_TOOL_RESULT` | 工具结果 |

**脚本返回值**:

| 退出码 | 含义 |
|--------|------|
| `0` | `Continue`（继续执行） |
| `1` | `Block`（阻止执行） |
| `2` | `Skip`（跳过后续 Hook） |

**stdout 输出**:
- 退出码为 `0` 且有 stdout 输出时，视为 `Modify(stdout_content)`

**示例脚本** (`hooks/validate.sh`):

```bash
#!/bin/bash

# 检查是否在敏感目录中执行
if echo "$OrangeCoding_HOOK_TOOL_PARAMS" | grep -q "/etc/"; then
    echo "禁止访问 /etc/ 目录" >&2
    exit 1  # Block
fi

# 继续执行
exit 0  # Continue
```

---

## 最佳实践

### 1. 优先级选择

```
安全相关    → Critical
权限验证    → High
业务逻辑    → Normal
日志/监控   → Low
```

### 2. 性能考虑

- **避免阻塞**：Hook 在 Agent 主循环中同步执行，长时间阻塞会影响响应速度
- **快速失败**：安全检查应尽早执行（Critical 优先级），避免不必要的后续处理
- **缓存结果**：对于重复检查（如路径验证），使用缓存避免重复计算

### 3. 错误处理

```rust
// ✅ 正确：在 Hook 内部处理错误
handler: Some(Arc::new(|ctx| {
    match validate_something(ctx) {
        Ok(_) => HookAction::Continue,
        Err(e) => {
            tracing::warn!(error = %e, "验证失败");
            HookAction::Block(format!("验证失败: {}", e))
        }
    }
})),

// ❌ 错误：在 Hook 中 panic
handler: Some(Arc::new(|ctx| {
    let data = ctx.tool_result().unwrap(); // 可能 panic
    HookAction::Continue
})),
```

### 4. Hook 链设计

- **单一职责**：每个 Hook 只负责一件事
- **松耦合**：Hook 之间不应有隐式依赖
- **可测试**：Hook 处理器应独立可测试
- **可配置**：通过 `enabled` 字段支持动态开关

### 5. 调试 Hook

```rust
// 在 Hook 中添加 tracing 日志
handler: Some(Arc::new(|ctx| {
    tracing::debug!(
        hook = "my_hook",
        event = ?ctx.event,
        tool = ctx.tool_name(),
        "Hook 执行中"
    );
    // ...
    HookAction::Continue
})),
```

---

## 相关文档

- [权限系统参考](./permissions.md) - Hook 与权限的交互
- [安全架构](../architecture/security.md) - 安全 Hook 的设计原理
- [Agent 系统架构](../architecture/agent-system.md) - Hook 在 Agent 生命周期中的位置

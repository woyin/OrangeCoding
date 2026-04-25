//! # 启动命令
//!
//! 实现 `orangecoding launch` 子命令，用于启动 AI 智能体。
//! 支持交互模式（TUI）和单次任务模式（提供 prompt 直接执行）。

use anyhow::{Context, Result};
use clap::Args;
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use tracing::{info, warn};

use orangecoding_agent::execution_prompt::{build_system_prompt, ExecutionMode};
use orangecoding_agent::instruction_anchor::InstructionAnchor;
use orangecoding_agent::model_router::{Difficulty, ModelRouter, OrangeRuntimeConfig, TaskType};
use orangecoding_agent::step_budget::{BudgetDecision, StepBudgetGuard};
use orangecoding_ai::provider::{FunctionDefinition, ToolParameter};
use orangecoding_ai::{
    AiProvider, ChatMessage, ChatOptions, MessageRole, ProviderConfig, ProviderFactory,
    ToolDefinition,
};
use orangecoding_config::{ModelsConfig, OrangeConfig};
use orangecoding_core::message::Role;
use orangecoding_tools::{create_default_registry, SecurityPolicy, ToolRegistry};
use orangecoding_tui::App;

// ============================================================
// 启动命令参数定义
// ============================================================

/// 启动命令的参数
///
/// 用于配置 AI 智能体的运行方式，包括：
/// - 直接提供 prompt 进行单次任务
/// - 选择 AI 模型和提供商
/// - 启用交互模式或禁用 TUI
/// - 启用 Autopilot 长任务全自动模式
#[derive(Args, Debug, Default)]
pub struct LaunchArgs {
    /// 要执行的任务描述（可选，不提供时进入交互模式）
    #[arg(short, long)]
    pub prompt: Option<String>,

    /// 指定 AI 模型名称（覆盖配置文件中的设置）
    #[arg(short, long)]
    pub model: Option<String>,

    /// 指定 AI 提供商名称（覆盖配置文件中的设置）
    #[arg(long)]
    pub provider: Option<String>,

    /// 启用交互模式（默认启用 TUI 界面）
    #[arg(short, long, default_value_t = false)]
    pub interactive: bool,

    /// 禁用 TUI 界面，使用纯文本输出
    #[arg(long, default_value_t = false)]
    pub no_tui: bool,

    /// 启用 Autopilot 长任务全自动模式
    ///
    /// 系统将自动执行 Plan → Execute → Verify → Replan 循环，
    /// 直到所有任务完成或达到最大循环轮次。
    #[arg(long)]
    pub autopilot: bool,

    /// Autopilot 模式的需求文件路径（替代 --autopilot 后直接跟 prompt）
    #[arg(long)]
    pub autopilot_file: Option<String>,

    /// Autopilot 最大循环轮次（覆盖配置文件设置）
    #[arg(long)]
    pub max_cycles: Option<u32>,

    /// Autopilot 严格验证模式：所有验收标准必须通过
    #[arg(long, default_value_t = false)]
    pub verify_strict: bool,

    /// Autopilot 每轮结束后暂停等待用户确认
    #[arg(long, default_value_t = false)]
    pub pause_between_cycles: bool,

    /// Autopilot 禁用自动测试运行
    #[arg(long, default_value_t = false)]
    pub no_auto_test: bool,
}

// ============================================================
// 命令执行入口
// ============================================================

/// 执行启动命令
///
/// 根据参数和配置决定运行模式：
/// 1. 如果提供了 prompt，以单次任务模式执行
/// 2. 如果启用了交互模式且未禁用 TUI，启动终端界面
/// 3. 否则进入交互式命令行循环
///
/// # 参数
/// - `args`: 命令行参数
/// - `config`: 当前生效的配置
pub async fn execute(args: LaunchArgs, config: OrangeConfig) -> Result<()> {
    info!("正在启动 AI 智能体...");

    // 创建 AI 提供商实例
    let provider = setup_provider(&args, &config).context("AI 提供商初始化失败")?;

    info!("AI 提供商已就绪: {}", provider.name());

    // 创建并注册默认工具集
    let registry = setup_tool_registry(&config);
    let tool_count = registry.len();
    info!("工具注册表已就绪，共 {} 个工具", tool_count);

    // 确定使用的模型名称
    let model_name = args.model.as_deref().unwrap_or(&config.ai.model);
    let explicit_model = args.model.is_some();

    // 根据运行模式分发
    if let Some(ref prompt) = args.prompt {
        // 单次任务模式：发送 prompt 并获取结果
        run_single_shot(
            provider.as_ref(),
            &registry,
            prompt,
            model_name,
            explicit_model,
            args.autopilot || args.autopilot_file.is_some(),
            &config,
        )
        .await
    } else if args.no_tui {
        // 用户明确禁用 TUI，使用纯文本交互模式
        run_interactive_mode(provider, registry, model_name, explicit_model, &config).await
    } else {
        // 默认启动 TUI 交互界面
        run_tui_mode(provider, registry, model_name, explicit_model, &config).await
    }
}

// ============================================================
// AI 提供商初始化
// ============================================================

/// 根据命令行参数和配置文件创建 AI 提供商实例
///
/// 优先级：命令行参数 > 配置文件 > 环境变量
///
/// # 参数
/// - `args`: 命令行参数（可能指定了 provider）
/// - `config`: 配置文件中的设置
fn setup_provider(args: &LaunchArgs, config: &OrangeConfig) -> Result<Box<dyn AiProvider>> {
    let requested_provider = args.provider.as_deref().unwrap_or(&config.ai.provider);
    let provider_name = ModelsConfig::canonical_provider_name(requested_provider);
    let builtin_provider = matches!(
        provider_name.as_str(),
        "openai"
            | "anthropic"
            | "claude"
            | "deepseek"
            | "qianwen"
            | "tongyi"
            | "dashscope"
            | "wenxin"
            | "ernie"
            | "baidu"
            | "zai"
            | "zen"
    );

    if !builtin_provider {
        return Err(anyhow::anyhow!(
            "自定义 provider '{}' 需要先在配置中声明模型清单，并映射到受支持的运行时 provider",
            requested_provider
        ));
    }

    let api_key = config.ai.api_key.clone().or_else(|| {
        let env_key = format!("{}_API_KEY", provider_name.to_uppercase().replace('.', "_"));
        std::env::var(&env_key)
            .ok()
            .or_else(|| std::env::var("ORANGECODING_API_KEY").ok())
    });

    let api_key = api_key.unwrap_or_default();

    if api_key.is_empty() {
        warn!("未配置 API 密钥，部分功能可能不可用");
    }

    let provider_config = ProviderConfig {
        api_key,
        api_secret: config.ai.api_secret.clone().or_else(|| {
            let env_secret = format!(
                "{}_API_SECRET",
                provider_name.to_uppercase().replace('.', "_")
            );
            std::env::var(&env_secret).ok()
        }),
        base_url: config.ai.base_url.clone(),
        default_model: Some(
            args.model
                .clone()
                .unwrap_or_else(|| config.ai.model.clone()),
        ),
        timeout_secs: config.agent.timeout_secs,
        extra: HashMap::new(),
    };

    ProviderFactory::create_provider(&provider_name, provider_config)
        .map_err(|e| anyhow::anyhow!("创建 AI 提供商 '{}' 失败: {}", provider_name, e))
}

// ============================================================
// 工具注册表初始化
// ============================================================

/// 创建并配置默认工具注册表
///
/// 根据配置中的 `tools.allowed_paths`、`tools.blocked_paths` 构建安全策略，
/// 并用 `FileOperationGuard` 包装文件操作工具。
/// 用户配置的 blocked_paths 会追加到默认敏感路径列表中，而非替换。
fn setup_tool_registry(config: &OrangeConfig) -> ToolRegistry {
    // 以默认安全策略为基础（包含 /etc, ~/.ssh, ~/.aws 等敏感路径）
    let mut policy = SecurityPolicy::default_policy();

    // 用户配置的允许路径覆盖默认值
    if !config.tools.allowed_paths.is_empty() {
        policy.allowed_dirs = config.tools.allowed_paths.clone();
    }

    // 用户配置的阻止路径追加到默认列表（不替换）
    for blocked in &config.tools.blocked_paths {
        if !policy.blocked_paths.contains(blocked) {
            policy.blocked_paths.push(blocked.clone());
        }
    }

    policy.allow_path_traversal = false;
    let registry = create_default_registry(policy);
    info!("已注册工具: {:?}", registry.list_tools());
    registry
}

/// 将工具注册表中的工具转换为 AI 可识别的工具定义列表
///
/// 遍历注册表中的所有工具，提取其名称、描述和参数模式，
/// 构造成符合 AI 函数调用规范的 ToolDefinition 数组。
fn build_tool_definitions(registry: &ToolRegistry) -> Vec<ToolDefinition> {
    registry
        .list_tools()
        .iter()
        .filter_map(|name| {
            registry.get(name).map(|tool| {
                // 将工具的 JSON Schema 参数转换为 ToolParameter
                let schema = tool.parameters_schema();
                let properties = schema
                    .get("properties")
                    .and_then(|p| p.as_object())
                    .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();
                let required = schema
                    .get("required")
                    .and_then(|r| r.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                ToolDefinition {
                    tool_type: "function".to_string(),
                    function: FunctionDefinition {
                        name: tool.name().to_string(),
                        description: tool.description().to_string(),
                        parameters: ToolParameter {
                            param_type: "object".to_string(),
                            properties,
                            required,
                        },
                    },
                }
            })
        })
        .collect()
}

/// 返回执行期配置文件路径。
fn orange_runtime_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".config/orangecoding/orange.json"))
}

/// 加载执行期配置，缺失或无效时使用默认值。
fn load_runtime_config() -> OrangeRuntimeConfig {
    orange_runtime_config_path()
        .map(|path| OrangeRuntimeConfig::load_or_default(&path))
        .unwrap_or_default()
}

/// 根据任务内容推断模型；显式选择模型时保持不变。
fn routed_model(
    router: &ModelRouter,
    requested_model: &str,
    prompt: &str,
    allow_routing: bool,
) -> String {
    if !allow_routing || !requested_model.trim().is_empty() {
        return requested_model.to_string();
    }

    let difficulty = Difficulty::infer(prompt);
    let task_type = TaskType::infer(prompt);
    router.route(difficulty, task_type).to_string()
}

/// 在当前提供商内路由模型，避免把模型名发送到不兼容的客户端。
fn routed_model_for_provider(
    router: &ModelRouter,
    fallback_model: &str,
    provider: &str,
    prompt: &str,
    allow_routing: bool,
) -> String {
    if !allow_routing {
        return fallback_model.to_string();
    }

    let routed = routed_model(router, "", prompt, true);
    compatible_model_for_provider(&routed, provider).unwrap_or_else(|| {
        warn!(
            routed_model = %routed,
            provider = %provider,
            fallback_model = %fallback_model,
            "路由模型与当前提供商不兼容，回退到当前模型"
        );
        fallback_model.to_string()
    })
}

fn compatible_model_for_provider(model: &str, provider: &str) -> Option<String> {
    let canonical_provider = ModelsConfig::canonical_provider_name(provider);
    let model = model.trim();
    if model.is_empty() {
        return None;
    }

    if let Some((model_provider, model_id)) = model.split_once('/') {
        let model_provider = ModelsConfig::canonical_provider_name(model_provider);
        return (providers_compatible(&model_provider, &canonical_provider)
            && !model_id.trim().is_empty())
        .then(|| model_id.trim().to_string());
    }

    if model_matches_provider(model, &canonical_provider) {
        Some(model.to_string())
    } else {
        None
    }
}

fn providers_compatible(model_provider: &str, active_provider: &str) -> bool {
    model_provider == active_provider
        || matches!(
            (model_provider, active_provider),
            ("anthropic", "claude") | ("claude", "anthropic")
        )
}

fn model_matches_provider(model: &str, canonical_provider: &str) -> bool {
    let normalized = model.trim().to_lowercase();
    match canonical_provider {
        "openai" => {
            normalized.starts_with("gpt-")
                || normalized.starts_with("o1")
                || normalized.starts_with("o3")
                || normalized.starts_with("o4")
        }
        "anthropic" | "claude" => normalized.starts_with("claude-"),
        "deepseek" => normalized.starts_with("deepseek-"),
        "qianwen" | "tongyi" | "dashscope" => {
            normalized.starts_with("qwen-")
                || normalized.starts_with("qwq-")
                || normalized.starts_with("qvq-")
        }
        "wenxin" | "ernie" | "baidu" => {
            normalized.starts_with("ernie") || normalized.starts_with("wenxin")
        }
        "zai" => normalized.starts_with("glm-"),
        "zen" => true,
        _ => true,
    }
}

/// 将 TUI 交互模式映射到代理执行模式。
fn mode_to_execution_mode(mode: orangecoding_tui::app::InteractionMode) -> ExecutionMode {
    match mode {
        orangecoding_tui::app::InteractionMode::Normal => ExecutionMode::Exec,
        orangecoding_tui::app::InteractionMode::Plan => ExecutionMode::Plan,
        orangecoding_tui::app::InteractionMode::Autopilot => ExecutionMode::Autopilot,
        orangecoding_tui::app::InteractionMode::UltraWork => ExecutionMode::UltraWork,
    }
}

/// 确保对话最前方只有一个模式系统提示词。
fn ensure_system_prompt(messages: &mut Vec<ChatMessage>, mode: ExecutionMode) {
    messages.retain(|message| !is_mode_system_prompt(message));
    messages.insert(0, ChatMessage::system(build_system_prompt(mode)));
}

fn is_mode_system_prompt(message: &ChatMessage) -> bool {
    if message.role != MessageRole::System {
        return false;
    }

    let Some(content) = message.content.as_deref() else {
        return false;
    };

    [
        ExecutionMode::Exec,
        ExecutionMode::Plan,
        ExecutionMode::Autopilot,
        ExecutionMode::UltraWork,
    ]
    .into_iter()
    .any(|mode| content == build_system_prompt(mode))
}

/// 判断消息是否为指令回锚系统消息。
fn is_instruction_anchor_message(message: &ChatMessage) -> bool {
    message.role == MessageRole::System
        && message
            .content
            .as_deref()
            .map(|content| content.trim_start().starts_with("[指令回锚]"))
            .unwrap_or(false)
}

/// 替换最新的指令回锚消息，避免历史中累积多条回锚。
fn replace_latest_anchor_message(messages: &mut Vec<ChatMessage>, anchor_message: String) {
    messages.retain(|message| !is_instruction_anchor_message(message));
    messages.push(ChatMessage::system(anchor_message));
}

/// 清理上一轮留下的回锚消息，避免跨用户请求污染上下文。
fn clear_instruction_anchor_messages(messages: &mut Vec<ChatMessage>) {
    messages.retain(|message| !is_instruction_anchor_message(message));
}

/// 将一批工具调用规范化为稳定签名，用于跨批次循环检测。
fn normalized_tool_batch_signature(tool_calls: &[orangecoding_ai::ToolCall]) -> String {
    let calls: BTreeSet<String> = tool_calls
        .iter()
        .map(|tool_call| {
            format!(
                "{}:{}",
                tool_call.function.name,
                normalized_arguments(&tool_call.function.arguments)
            )
        })
        .collect();

    if calls.is_empty() {
        "no-tool-calls".to_string()
    } else {
        calls.into_iter().collect::<Vec<_>>().join("|")
    }
}

fn normalized_arguments(arguments: &str) -> String {
    serde_json::from_str::<serde_json::Value>(arguments)
        .map(|value| canonical_json(&value))
        .unwrap_or_else(|_| arguments.trim().to_string())
}

fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {
            serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
        }
        serde_json::Value::Array(values) => {
            let items = values.iter().map(canonical_json).collect::<Vec<_>>();
            format!("[{}]", items.join(","))
        }
        serde_json::Value::Object(map) => {
            let fields = map
                .iter()
                .map(|(key, value)| {
                    let key = serde_json::to_string(key).unwrap_or_else(|_| format!("{key:?}"));
                    format!("{key}:{}", canonical_json(value))
                })
                .collect::<Vec<_>>();
            format!("{{{}}}", fields.join(","))
        }
    }
}

fn runtime_step_guard(runtime_config: &OrangeRuntimeConfig) -> StepBudgetGuard {
    StepBudgetGuard::new(
        runtime_config.execution.step_budget_initial,
        runtime_config.execution.loop_detection_threshold as usize,
    )
}

// ============================================================
// 单次任务模式
// ============================================================

/// 单次任务模式：发送一条 prompt，执行工具调用循环，输出结果
///
/// # 执行流程
/// 1. 构造系统提示词和用户消息
/// 2. 发送请求到 AI 模型
/// 3. 如果 AI 请求工具调用，执行工具并将结果返回
/// 4. 重复步骤 2-3 直到 AI 给出最终回答
/// 5. 打印最终结果
async fn run_single_shot(
    provider: &dyn AiProvider,
    registry: &ToolRegistry,
    prompt: &str,
    model: &str,
    explicit_model: bool,
    autopilot: bool,
    config: &OrangeConfig,
) -> Result<()> {
    println!("🚀 正在执行任务: {}", prompt);
    println!();

    let runtime_config = load_runtime_config();
    let execution_mode = if autopilot {
        ExecutionMode::Autopilot
    } else {
        ExecutionMode::Exec
    };
    let selected_model = if explicit_model {
        routed_model(runtime_config.model_router(), model, prompt, false)
    } else {
        routed_model_for_provider(
            runtime_config.model_router(),
            model,
            &config.ai.provider,
            prompt,
            true,
        )
    };
    let mut anchor = InstructionAnchor::new(prompt, runtime_config.execution.anchor_interval_steps);
    let mut budget_guard = runtime_step_guard(&runtime_config);

    // 构造消息列表
    let mut messages = vec![ChatMessage::user(prompt)];
    ensure_system_prompt(&mut messages, execution_mode);

    // 构建工具定义和请求选项
    let tools = build_tool_definitions(registry);
    let options = ChatOptions::with_model(selected_model)
        .temperature(config.ai.temperature)
        .max_tokens(config.ai.max_tokens);

    // 智能体循环：允许多轮工具调用
    let max_iterations = config.agent.max_iterations;
    for iteration in 0..max_iterations {
        info!("智能体迭代 #{}", iteration + 1);

        if let Some(anchor_message) = anchor.on_step() {
            replace_latest_anchor_message(&mut messages, anchor_message);
        }
        ensure_system_prompt(&mut messages, execution_mode);

        // 调用 AI 模型
        let response = provider
            .chat_completion(&messages, &tools, &options)
            .await
            .map_err(|e| anyhow::anyhow!("AI 请求失败: {}", e))?;

        // 如果 AI 返回了工具调用请求
        if !response.tool_calls.is_empty() {
            info!("AI 请求调用 {} 个工具", response.tool_calls.len());

            let batch_signature = normalized_tool_batch_signature(&response.tool_calls);
            if let BudgetDecision::HardStop { reason } = budget_guard.tick(&batch_signature) {
                anyhow::bail!(reason);
            }

            // 将 AI 的响应（包含工具调用请求）添加到消息历史
            messages.push(ChatMessage {
                role: orangecoding_ai::MessageRole::Assistant,
                content: if response.content.is_empty() {
                    None
                } else {
                    Some(response.content.clone())
                },
                tool_call_id: None,
                tool_calls: Some(response.tool_calls.clone()),
                name: None,
            });

            // 逐个执行工具调用
            for tool_call in &response.tool_calls {
                let tool_name = &tool_call.function.name;
                println!("🔧 调用工具: {}", tool_name);

                // 解析工具参数
                let params: serde_json::Value = serde_json::from_str(&tool_call.function.arguments)
                    .unwrap_or(serde_json::Value::Object(Default::default()));

                // 执行工具
                let tool_result = match registry.execute(tool_name, params).await {
                    Ok(output) => output,
                    Err(e) => format!("工具执行错误: {}", e),
                };

                // 将工具结果作为消息添加到历史
                messages.push(ChatMessage::tool_result(&tool_call.id, &tool_result));
            }

            continue;
        }

        // AI 返回了最终回答（无工具调用），输出结果
        println!("📝 AI 回复:");
        println!("{}", response.content);
        println!();
        println!(
            "📊 令牌使用: 输入 {} / 输出 {} / 总计 {}",
            response.usage.prompt_tokens,
            response.usage.completion_tokens,
            response.usage.total_tokens,
        );

        return Ok(());
    }

    // 超过最大迭代次数
    anyhow::bail!(
        "智能体已达到最大迭代次数 ({})，任务可能过于复杂",
        max_iterations,
    );
}

// ============================================================
// TUI 交互模式
// ============================================================

/// TUI 交互模式：启动终端用户界面
///
/// 创建真正的 ratatui TUI 应用，在终端中提供完整的图形化交互体验。
/// 支持侧边栏、交互模式切换、思考深度控制和斜杠命令。
///
/// # 设计原则（Harness Engineering）
///
/// - 查询循环是心跳：TUI 主循环以 50ms 节拍运行，持续响应用户输入
/// - 错误路径即主路径：AI 请求失败时在界面中显示错误并允许重试
/// - 模型是不稳定组件：通过交互模式控制 AI 的自主程度
async fn run_tui_mode(
    provider: Box<dyn AiProvider>,
    registry: ToolRegistry,
    model_name: &str,
    explicit_model: bool,
    config: &OrangeConfig,
) -> Result<()> {
    use crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use orangecoding_tui::components::MainLayout;
    use orangecoding_tui::AppAction;
    use ratatui::{backend::CrosstermBackend, Terminal};
    use std::io;
    use std::time::Duration;

    info!("正在启动 TUI 交互模式...");
    let runtime_config = load_runtime_config();
    let mut model_manually_selected = explicit_model;

    // 初始化终端
    enable_raw_mode().context("启用终端 raw 模式失败")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).context("进入备用终端屏幕失败")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("创建终端实例失败")?;

    // 创建 TUI 应用实例
    let mut app = App::new(model_name);

    // 从配置文件加载可用模型列表
    let models_config_path = dirs::home_dir().map(|h| h.join(".config/orangecoding/models.yml"));
    if let Some(path) = models_config_path {
        if path.exists() {
            if let Ok(mut models_cfg) = ModelsConfig::load_from_file(&path) {
                models_cfg.merge_with_predefined();
                let model_items: Vec<orangecoding_tui::app::CommandMenuItem> = models_cfg
                    .list_models()
                    .into_iter()
                    .map(|(provider, model)| orangecoding_tui::app::CommandMenuItem {
                        value: ModelsConfig::model_identity(&provider, &model.id),
                        description: format!(
                            "{} ({})",
                            model.name.as_deref().unwrap_or(&model.id),
                            ModelsConfig::provider_display_name(&provider),
                        ),
                    })
                    .collect();
                if !model_items.is_empty() {
                    app.set_available_models(model_items);
                }
            }
        }
    }

    // 添加欢迎消息
    app.add_message(
        Role::System,
        format!(
            "欢迎使用 OrangeCoding！\n模型: {} | 提供商: {}\n\n\
             快捷键: Shift+Tab 切换模式 | Ctrl+L 思考深度 | Ctrl+B 侧边栏 | ? 帮助\n\
             输入消息后按 Enter 发送，或使用 /命令 执行操作",
            model_name, config.ai.provider,
        ),
    );

    // 构建工具定义和请求选项
    let tools = build_tool_definitions(&registry);
    let mut options = ChatOptions::with_model(model_name)
        .temperature(config.ai.temperature)
        .max_tokens(config.ai.max_tokens);

    // 对话历史（用于多轮对话上下文）
    let mut messages: Vec<ChatMessage> = Vec::new();

    // TUI 主循环 — 查询循环心跳模式
    let result = 'main: loop {
        // 渲染界面
        if let Err(e) = terminal.draw(|frame| {
            MainLayout::render(frame, &app);
        }) {
            break Err(anyhow::anyhow!("界面渲染失败: {}", e));
        }

        // 检查是否应退出
        if !app.is_running {
            break Ok(());
        }

        // 等待事件（50ms 超时，保持界面响应性）
        if event::poll(Duration::from_millis(50)).unwrap_or(false) {
            if let Ok(event) = event::read() {
                let action = match event {
                    Event::Key(key_event) => app.handle_key_event(key_event),
                    Event::Mouse(mouse_event) => app.handle_mouse_event(mouse_event),
                    _ => AppAction::None,
                };

                match action {
                    AppAction::SendMessage(text) => {
                        // 显示用户消息
                        app.add_message(Role::User, &text);
                        app.status.status_text = "正在思考...".to_string();

                        // 重新渲染以显示用户消息和状态变化
                        let _ = terminal.draw(|frame| {
                            MainLayout::render(frame, &app);
                        });

                        // 构建 AI 请求
                        messages.push(ChatMessage::user(&text));
                        clear_instruction_anchor_messages(&mut messages);
                        let execution_mode = mode_to_execution_mode(app.interaction_mode.clone());
                        ensure_system_prompt(&mut messages, execution_mode);

                        if !model_manually_selected {
                            let selected_model = routed_model_for_provider(
                                runtime_config.model_router(),
                                model_name,
                                &config.ai.provider,
                                &text,
                                true,
                            );
                            options.model = selected_model.clone();
                            app.status.model_name = selected_model;
                        }

                        let mut anchor = InstructionAnchor::new(
                            &text,
                            runtime_config.execution.anchor_interval_steps,
                        );
                        let mut budget_guard = runtime_step_guard(&runtime_config);

                        // 智能体循环：支持多轮工具调用
                        let max_iterations = config.agent.max_iterations;
                        for iteration in 0..max_iterations {
                            if let Some(anchor_message) = anchor.on_step() {
                                replace_latest_anchor_message(&mut messages, anchor_message);
                            }
                            ensure_system_prompt(&mut messages, execution_mode);

                            let response =
                                match provider.chat_completion(&messages, &tools, &options).await {
                                    Ok(resp) => resp,
                                    Err(e) => {
                                        app.add_message(
                                            Role::System,
                                            format!("⚠️ AI 请求失败: {}。请重试。", e),
                                        );
                                        app.status.status_text = "错误 - 请重试".to_string();
                                        warn!("AI 请求失败: {:?}", e);
                                        break;
                                    }
                                };

                            // 处理工具调用
                            if !response.tool_calls.is_empty() {
                                let batch_signature =
                                    normalized_tool_batch_signature(&response.tool_calls);
                                if let BudgetDecision::HardStop { reason } =
                                    budget_guard.tick(&batch_signature)
                                {
                                    app.add_message(
                                        Role::System,
                                        format!("⛔ 步骤预算守卫停止执行: {}", reason),
                                    );
                                    break 'main Err(anyhow::anyhow!(reason));
                                }

                                // 记录 AI 的工具调用响应
                                messages.push(ChatMessage {
                                    role: orangecoding_ai::MessageRole::Assistant,
                                    content: if response.content.is_empty() {
                                        None
                                    } else {
                                        Some(response.content.clone())
                                    },
                                    tool_call_id: None,
                                    tool_calls: Some(response.tool_calls.clone()),
                                    name: None,
                                });

                                // 执行每个工具调用
                                for tool_call in &response.tool_calls {
                                    let tool_name = &tool_call.function.name;
                                    app.add_message(
                                        Role::System,
                                        format!("🔧 调用工具: {}", tool_name),
                                    );

                                    let params: serde_json::Value =
                                        serde_json::from_str(&tool_call.function.arguments)
                                            .unwrap_or(serde_json::Value::Object(
                                                Default::default(),
                                            ));

                                    let tool_result =
                                        match registry.execute(tool_name, params).await {
                                            Ok(output) => output,
                                            Err(e) => format!("工具执行错误: {}", e),
                                        };

                                    messages.push(ChatMessage::tool_result(
                                        &tool_call.id,
                                        &tool_result,
                                    ));
                                }

                                // 更新状态并继续循环
                                app.status.status_text = format!(
                                    "执行工具中... (迭代 {}/{})",
                                    iteration + 1,
                                    max_iterations
                                );
                                let _ = terminal.draw(|frame| {
                                    MainLayout::render(frame, &app);
                                });
                                continue;
                            }

                            // AI 返回最终回答（无工具调用）
                            let content = response.content.clone();
                            app.add_message(Role::Assistant, &content);
                            messages.push(ChatMessage::assistant(&content));

                            // 更新 token 使用量
                            app.status.token_count += response.usage.total_tokens as u64;
                            app.status.status_text = "就绪".to_string();
                            break;
                        }
                    }
                    AppAction::SlashCommand { name, args } => {
                        if name == "model" && !args.is_empty() {
                            model_manually_selected = true;
                        }
                        if handle_slash_command(&mut app, &mut options, &name, &args) {
                            messages.clear();
                        }
                    }
                    AppAction::SwitchInteractionMode(mode) => {
                        app.add_message(
                            Role::System,
                            format!("已切换到 {} 模式：{}", mode.label(), mode.description()),
                        );
                    }
                    AppAction::SwitchThinkingDepth(depth) => {
                        app.add_message(
                            Role::System,
                            format!("思考深度已调整为: {} {}", depth.icon(), depth.label()),
                        );
                    }
                    AppAction::Clear => {
                        messages.clear();
                    }
                    AppAction::Quit => {
                        break Ok(());
                    }
                    AppAction::ToggleSidebar | AppAction::None => {}
                }
            }
        }
    };

    // 清理终端
    disable_raw_mode().ok();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .ok();
    terminal.show_cursor().ok();

    result
}

/// 处理斜杠命令
///
/// 在 TUI 中执行用户输入的斜杠命令，更新应用状态并显示反馈。
fn handle_slash_command(app: &mut App, options: &mut ChatOptions, name: &str, args: &str) -> bool {
    use orangecoding_tui::app::{InteractionMode, ThinkingDepth};

    match name {
        "model" => {
            if args.is_empty() {
                // 无参数时打开交互式模型选择菜单
                app.open_model_menu("");
                app.mode = orangecoding_tui::app::AppMode::Command;
            } else {
                app.status.model_name = args.to_string();
                options.model = args.to_string();
                app.add_message(Role::System, format!("模型已切换为: {}", args));
            }
        }
        "mode" => {
            if args.is_empty() {
                app.open_mode_menu();
                app.mode = orangecoding_tui::app::AppMode::Command;
            } else if let Some(mode) = InteractionMode::from_str_name(args) {
                app.interaction_mode = mode.clone();
                app.add_message(
                    Role::System,
                    format!("已切换到 {} 模式：{}", mode.label(), mode.description()),
                );
            } else {
                app.add_message(
                    Role::System,
                    format!(
                        "未知模式: {}。可选: normal, plan, autopilot, ultrawork",
                        args
                    ),
                );
            }
        }
        "think" | "depth" => {
            if args.is_empty() {
                app.open_think_menu();
                app.mode = orangecoding_tui::app::AppMode::Command;
            } else if let Some(depth) = ThinkingDepth::from_str_name(args) {
                app.thinking_depth = depth.clone();
                app.add_message(
                    Role::System,
                    format!("思考深度已调整为: {} {}", depth.icon(), depth.label()),
                );
            } else {
                app.add_message(
                    Role::System,
                    format!(
                        "未知深度: {}。可选: off, light, medium, deep, maximum",
                        args
                    ),
                );
            }
        }
        "clear" | "cls" => {
            app.messages.clear();
            app.scroll_offset = 0;
            return true;
        }
        "help" | "h" => {
            app.add_message(
                Role::System,
                "可用命令:\n\
                 /model <名称>    - 切换 AI 模型\n\
                 /mode <模式>     - 切换交互模式 (normal/plan/autopilot/ultrawork)\n\
                 /think <深度>    - 切换思考深度 (off/light/medium/deep/maximum)\n\
                 /clear           - 清除对话\n\
                 /help            - 显示帮助\n\
                 /quit            - 退出\n\n\
                 快捷键: Shift+Tab 切换模式 | Ctrl+L 思考深度 | Ctrl+B 侧边栏"
                    .to_string(),
            );
        }
        "quit" | "exit" | "q" => {
            app.is_running = false;
        }
        _ => {
            app.add_message(
                Role::System,
                format!("未知命令: /{}。输入 /help 查看可用命令。", name),
            );
        }
    }
    false
}

// ============================================================
// 纯文本交互模式
// ============================================================

/// 纯文本交互模式：在命令行中进行对话
///
/// 循环读取用户输入，发送给 AI 模型，输出回复。
/// 支持工具调用和多轮对话。
async fn run_interactive_mode(
    provider: Box<dyn AiProvider>,
    registry: ToolRegistry,
    model_name: &str,
    explicit_model: bool,
    config: &OrangeConfig,
) -> Result<()> {
    info!("正在启动交互模式...");
    println!("🤖 OrangeCoding 交互模式");
    println!("   模型: {} | 提供商: {}", model_name, config.ai.provider);
    println!("   输入 'exit' 或 'quit' 退出");
    println!();

    // 创建 TUI 应用实例用于消息管理
    let mut app = App::new(model_name);

    // 构建工具定义和请求选项
    let tools = build_tool_definitions(&registry);
    let options = ChatOptions::with_model(model_name)
        .temperature(config.ai.temperature)
        .max_tokens(config.ai.max_tokens);

    run_text_loop(
        &mut app,
        provider.as_ref(),
        &registry,
        &tools,
        &options,
        explicit_model,
        config,
    )
    .await
}

/// 文本交互循环的核心实现
///
/// TUI 模式和纯文本模式共享此循环逻辑。
/// 读取用户输入、发送请求、处理工具调用、输出结果。
async fn run_text_loop(
    _app: &mut App,
    provider: &dyn AiProvider,
    registry: &ToolRegistry,
    tools: &[ToolDefinition],
    options: &ChatOptions,
    explicit_model: bool,
    config: &OrangeConfig,
) -> Result<()> {
    let runtime_config = load_runtime_config();

    // 维护对话消息历史
    let mut messages = Vec::new();
    ensure_system_prompt(&mut messages, ExecutionMode::Exec);

    // 读取输入的缓冲区
    let stdin = std::io::stdin();

    loop {
        // 显示输入提示符
        eprint!(">>> ");

        // 读取用户输入
        let mut input = String::new();
        stdin.read_line(&mut input).context("读取用户输入失败")?;

        let input = input.trim();

        // 检查退出命令
        if input.is_empty() {
            continue;
        }
        if input == "exit" || input == "quit" {
            println!("再见！👋");
            break;
        }

        // 将用户消息添加到历史
        messages.push(ChatMessage::user(input));
        clear_instruction_anchor_messages(&mut messages);
        ensure_system_prompt(&mut messages, ExecutionMode::Exec);

        let mut request_options = options.clone();
        if !explicit_model {
            request_options.model = routed_model_for_provider(
                runtime_config.model_router(),
                &options.model,
                &config.ai.provider,
                input,
                true,
            );
        }
        let mut anchor =
            InstructionAnchor::new(input, runtime_config.execution.anchor_interval_steps);
        let mut budget_guard = runtime_step_guard(&runtime_config);

        // 智能体循环处理（支持多轮工具调用）
        let max_iterations = config.agent.max_iterations;
        for iteration in 0..max_iterations {
            if let Some(anchor_message) = anchor.on_step() {
                replace_latest_anchor_message(&mut messages, anchor_message);
            }
            ensure_system_prompt(&mut messages, ExecutionMode::Exec);

            // 调用 AI 模型
            let response = match provider
                .chat_completion(&messages, tools, &request_options)
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    eprintln!("❌ AI 请求失败: {}", e);
                    break;
                }
            };

            // 处理工具调用
            if !response.tool_calls.is_empty() {
                info!(
                    "迭代 #{}: AI 请求调用 {} 个工具",
                    iteration + 1,
                    response.tool_calls.len()
                );

                let batch_signature = normalized_tool_batch_signature(&response.tool_calls);
                if let BudgetDecision::HardStop { reason } = budget_guard.tick(&batch_signature) {
                    anyhow::bail!(reason);
                }

                // 记录 AI 的工具调用响应
                messages.push(ChatMessage {
                    role: orangecoding_ai::MessageRole::Assistant,
                    content: if response.content.is_empty() {
                        None
                    } else {
                        Some(response.content.clone())
                    },
                    tool_call_id: None,
                    tool_calls: Some(response.tool_calls.clone()),
                    name: None,
                });

                // 执行每个工具调用
                for tool_call in &response.tool_calls {
                    let tool_name = &tool_call.function.name;
                    println!("  🔧 {}", tool_name);

                    let params: serde_json::Value =
                        serde_json::from_str(&tool_call.function.arguments)
                            .unwrap_or(serde_json::Value::Object(Default::default()));

                    let result = match registry.execute(tool_name, params).await {
                        Ok(output) => output,
                        Err(e) => format!("工具执行错误: {}", e),
                    };

                    messages.push(ChatMessage::tool_result(&tool_call.id, &result));
                }

                continue;
            }

            // 输出 AI 的最终回复
            println!();
            println!("{}", response.content);
            println!();

            // 记录助手回复到消息历史
            messages.push(ChatMessage::assistant(&response.content));

            break;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use orangecoding_agent::model_router::RoutingRule;
    use orangecoding_ai::provider::FunctionCall;
    use orangecoding_ai::{MessageRole, ToolCall};
    use orangecoding_tui::app::InteractionMode;

    #[test]
    fn 测试系统提示词只保留一个模式提示且不删除历史() {
        let mut messages = vec![
            ChatMessage::system(build_system_prompt(ExecutionMode::Exec)),
            ChatMessage::user("保留用户消息"),
            ChatMessage::assistant("保留助手消息"),
            ChatMessage::system(build_system_prompt(ExecutionMode::Plan)),
        ];

        ensure_system_prompt(&mut messages, ExecutionMode::Autopilot);

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(
            messages[0].content.as_deref(),
            Some(build_system_prompt(ExecutionMode::Autopilot).as_str())
        );
        assert_eq!(messages[1].content.as_deref(), Some("保留用户消息"));
        assert_eq!(messages[2].content.as_deref(), Some("保留助手消息"));
    }

    #[test]
    fn 测试回锚消息替换最新一条而不是累积() {
        let mut messages = vec![
            ChatMessage::system(build_system_prompt(ExecutionMode::Exec)),
            ChatMessage::user("任务"),
        ];

        replace_latest_anchor_message(&mut messages, "[指令回锚]\n第一次".to_string());
        replace_latest_anchor_message(&mut messages, "[指令回锚]\n第二次".to_string());

        let anchors: Vec<_> = messages
            .iter()
            .filter(|message| is_instruction_anchor_message(message))
            .collect();
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].content.as_deref(), Some("[指令回锚]\n第二次"));
        assert_eq!(messages[1].content.as_deref(), Some("任务"));
    }

    #[test]
    fn 测试清理回锚消息不会删除对话历史() {
        let mut messages = vec![
            ChatMessage::system(build_system_prompt(ExecutionMode::Exec)),
            ChatMessage::user("旧任务"),
            ChatMessage::system("[指令回锚]\n旧任务"),
            ChatMessage::assistant("旧回复"),
            ChatMessage::user("新任务"),
        ];

        clear_instruction_anchor_messages(&mut messages);

        assert!(!messages.iter().any(is_instruction_anchor_message));
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[1].content.as_deref(), Some("旧任务"));
        assert_eq!(messages[2].content.as_deref(), Some("旧回复"));
        assert_eq!(messages[3].content.as_deref(), Some("新任务"));
    }

    #[test]
    fn 测试工具批次签名排序去重并规范化参数() {
        let calls = vec![
            tool_call("2", "write_file", r#"{"b":2,"a":1}"#),
            tool_call("1", "read_file", r#"{"path":"Cargo.toml"}"#),
            tool_call("3", "write_file", r#"{"a":1,"b":2}"#),
        ];

        let signature = normalized_tool_batch_signature(&calls);

        assert_eq!(
            signature,
            r#"read_file:{"path":"Cargo.toml"}|write_file:{"a":1,"b":2}"#
        );
    }

    #[test]
    fn 测试模型路由尊重显式模型并在允许时推断路由() {
        let router = ModelRouter {
            rules: vec![RoutingRule::new(
                Some(Difficulty::Hard),
                Some(TaskType::Code),
                "hard-code-model",
            )],
            fallback: "fallback-model".to_string(),
        };

        assert_eq!(
            routed_model(&router, "manual-model", "修复复杂代码问题", false),
            "manual-model"
        );
        assert_eq!(
            routed_model(&router, "", "修复复杂代码问题并运行测试", true),
            "hard-code-model"
        );
    }

    #[test]
    fn 测试路由模型会回退不兼容的内置提供商模型() {
        let router = ModelRouter {
            rules: vec![RoutingRule::new(
                Some(Difficulty::Hard),
                Some(TaskType::Code),
                "claude-sonnet-4-5",
            )],
            fallback: "deepseek-chat".to_string(),
        };

        assert_eq!(
            routed_model_for_provider(
                &router,
                "deepseek-chat",
                "deepseek",
                "修复复杂代码问题并运行测试",
                true,
            ),
            "deepseek-chat"
        );
    }

    #[test]
    fn 测试路由模型接受当前提供商限定模型并去掉前缀() {
        let router = ModelRouter {
            rules: vec![RoutingRule::new(
                Some(Difficulty::Hard),
                Some(TaskType::Code),
                "anthropic/claude-sonnet-4-5",
            )],
            fallback: "claude-haiku".to_string(),
        };

        assert_eq!(
            routed_model_for_provider(
                &router,
                "claude-haiku",
                "claude",
                "修复复杂代码问题并运行测试",
                true,
            ),
            "claude-sonnet-4-5"
        );
    }

    #[test]
    fn 测试_tui_交互模式映射到执行模式() {
        assert_eq!(
            mode_to_execution_mode(InteractionMode::Normal),
            ExecutionMode::Exec
        );
        assert_eq!(
            mode_to_execution_mode(InteractionMode::Plan),
            ExecutionMode::Plan
        );
        assert_eq!(
            mode_to_execution_mode(InteractionMode::Autopilot),
            ExecutionMode::Autopilot
        );
        assert_eq!(
            mode_to_execution_mode(InteractionMode::UltraWork),
            ExecutionMode::UltraWork
        );
    }

    fn tool_call(id: &str, name: &str, arguments: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: name.to_string(),
                arguments: arguments.to_string(),
            },
        }
    }
}

//! # 启动命令
//!
//! 实现 `ceair launch` 子命令，用于启动 AI 智能体。
//! 支持交互模式（TUI）和单次任务模式（提供 prompt 直接执行）。

use anyhow::{Context, Result};
use clap::Args;
use std::collections::HashMap;
use tracing::{info, warn};

use chengcoding_ai::{
    AiProvider, ChatMessage, ChatOptions, ProviderConfig, ProviderFactory, ToolDefinition,
};
use chengcoding_ai::provider::{FunctionDefinition, ToolParameter};
use chengcoding_config::CeairConfig;
use chengcoding_core::message::Role;
use chengcoding_tools::{create_default_registry, SecurityPolicy, ToolRegistry};
use chengcoding_tui::App;

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
pub async fn execute(args: LaunchArgs, config: CeairConfig) -> Result<()> {
    info!("正在启动 AI 智能体...");

    // 创建 AI 提供商实例
    let provider = setup_provider(&args, &config)
        .context("AI 提供商初始化失败")?;

    info!("AI 提供商已就绪: {}", provider.name());

    // 创建并注册默认工具集
    let registry = setup_tool_registry(&config);
    let tool_count = registry.len();
    info!("工具注册表已就绪，共 {} 个工具", tool_count);

    // 确定使用的模型名称
    let model_name = args.model
        .as_deref()
        .unwrap_or(&config.ai.model);

    // 根据运行模式分发
    if let Some(ref prompt) = args.prompt {
        // 单次任务模式：发送 prompt 并获取结果
        run_single_shot(provider.as_ref(), &registry, prompt, model_name, &config)
            .await
    } else if args.no_tui {
        // 用户明确禁用 TUI，使用纯文本交互模式
        run_interactive_mode(provider, registry, model_name, &config).await
    } else {
        // 默认启动 TUI 交互界面
        run_tui_mode(provider, registry, model_name, &config).await
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
fn setup_provider(
    args: &LaunchArgs,
    config: &CeairConfig,
) -> Result<Box<dyn AiProvider>> {
    // 确定提供商名称（命令行优先）
    let provider_name = args.provider
        .as_deref()
        .unwrap_or(&config.ai.provider);

    // 获取 API 密钥（配置文件 > 环境变量）
    let api_key = config.ai.api_key.clone().or_else(|| {
        // 尝试从环境变量获取（格式：ChengCoding_API_KEY 或 <PROVIDER>_API_KEY）
        let env_key = format!("{}_API_KEY", provider_name.to_uppercase());
        std::env::var(&env_key).ok()
            .or_else(|| std::env::var("ChengCoding_API_KEY").ok())
    });

    let api_key = api_key.unwrap_or_default();

    if api_key.is_empty() {
        warn!("未配置 API 密钥，部分功能可能不可用");
    }

    // 构建提供商配置
    let provider_config = ProviderConfig {
        api_key,
        api_secret: config.ai.api_secret.clone().or_else(|| {
            let env_secret = format!("{}_API_SECRET", provider_name.to_uppercase());
            std::env::var(&env_secret).ok()
        }),
        base_url: config.ai.base_url.clone(),
        default_model: Some(
            args.model.clone().unwrap_or_else(|| config.ai.model.clone()),
        ),
        timeout_secs: config.agent.timeout_secs,
        extra: HashMap::new(),
    };

    // 通过工厂方法创建提供商
    ProviderFactory::create_provider(provider_name, provider_config)
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
fn setup_tool_registry(config: &CeairConfig) -> ToolRegistry {
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
    info!(
        "已注册工具: {:?}",
        registry.list_tools()
    );
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
    config: &CeairConfig,
) -> Result<()> {
    println!("🚀 正在执行任务: {}", prompt);
    println!();

    // 构造消息列表
    let mut messages = vec![
        ChatMessage::system(
            "你是一个专业的 AI 编程助手。你可以使用提供的工具来完成任务。\
             请仔细分析用户的需求，合理使用工具，并给出清晰的回答。",
        ),
        ChatMessage::user(prompt),
    ];

    // 构建工具定义和请求选项
    let tools = build_tool_definitions(registry);
    let options = ChatOptions::with_model(model)
        .temperature(config.ai.temperature)
        .max_tokens(config.ai.max_tokens);

    // 智能体循环：允许多轮工具调用
    let max_iterations = config.agent.max_iterations;
    for iteration in 0..max_iterations {
        info!("智能体迭代 #{}", iteration + 1);

        // 调用 AI 模型
        let response = provider
            .chat_completion(&messages, &tools, &options)
            .await
            .map_err(|e| anyhow::anyhow!("AI 请求失败: {}", e))?;

        // 如果 AI 返回了工具调用请求
        if !response.tool_calls.is_empty() {
            info!(
                "AI 请求调用 {} 个工具",
                response.tool_calls.len()
            );

            // 将 AI 的响应（包含工具调用请求）添加到消息历史
            messages.push(ChatMessage {
                role: chengcoding_ai::MessageRole::Assistant,
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
                let params: serde_json::Value =
                    serde_json::from_str(&tool_call.function.arguments)
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
    config: &CeairConfig,
) -> Result<()> {
    use crossterm::{
        event::{self, Event},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{backend::CrosstermBackend, Terminal};
    use std::io;
    use std::time::Duration;
    use chengcoding_tui::components::MainLayout;
    use chengcoding_tui::AppAction;

    info!("正在启动 TUI 交互模式...");

    // 初始化终端
    enable_raw_mode().context("启用终端 raw 模式失败")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("进入备用终端屏幕失败")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("创建终端实例失败")?;

    // 创建 TUI 应用实例
    let mut app = App::new(model_name);

    // 添加欢迎消息
    app.add_message(
        Role::System,
        format!(
            "欢迎使用 ChengCoding！\n模型: {} | 提供商: {}\n\n\
             快捷键: Shift+Tab 切换模式 | Ctrl+L 思考深度 | Ctrl+B 侧边栏 | ? 帮助\n\
             输入消息后按 Enter 发送，或使用 /命令 执行操作",
            model_name, config.ai.provider,
        ),
    );

    // 构建工具定义和请求选项
    let tools = build_tool_definitions(&registry);
    let options = ChatOptions::with_model(model_name)
        .temperature(config.ai.temperature)
        .max_tokens(config.ai.max_tokens);

    // 对话历史（用于多轮对话上下文）
    let mut messages: Vec<ChatMessage> = Vec::new();

    // TUI 主循环 — 查询循环心跳模式
    let result = loop {
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
            if let Ok(Event::Key(key_event)) = event::read() {
                let action = app.handle_key_event(key_event);

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

                        // 发送请求并处理响应
                        match provider
                            .chat_completion(&messages, &tools, &options)
                            .await
                        {
                            Ok(response) => {
                                let content = response.content.clone();
                                app.add_message(Role::Assistant, &content);
                                messages.push(ChatMessage::assistant(&content));

                                // 更新 token 使用量
                                app.status.token_count += response.usage.total_tokens as u64;
                                app.status.status_text = "就绪".to_string();
                            }
                            Err(e) => {
                                // 错误路径即主路径 — 显示错误但不崩溃
                                app.add_message(
                                    Role::System,
                                    format!("⚠️ AI 请求失败: {}。请重试。", e),
                                );
                                app.status.status_text = "错误 - 请重试".to_string();
                                warn!("AI 请求失败: {:?}", e);
                            }
                        }
                    }
                    AppAction::SlashCommand { name, args } => {
                        // 处理斜杠命令
                        handle_slash_command(&mut app, &name, &args);
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
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    result
}

/// 处理斜杠命令
///
/// 在 TUI 中执行用户输入的斜杠命令，更新应用状态并显示反馈。
fn handle_slash_command(app: &mut App, name: &str, args: &str) {
    use chengcoding_tui::app::{InteractionMode, ThinkingDepth};

    match name {
        "model" => {
            if args.is_empty() {
                app.add_message(
                    Role::System,
                    format!("当前模型: {}。用法: /model <模型名称>", app.status.model_name),
                );
            } else {
                app.status.model_name = args.to_string();
                app.add_message(
                    Role::System,
                    format!("模型已切换为: {}", args),
                );
            }
        }
        "mode" => {
            if args.is_empty() {
                app.add_message(
                    Role::System,
                    format!(
                        "当前模式: {}。可选: normal, plan, autopilot, ultrawork",
                        app.interaction_mode.label()
                    ),
                );
            } else if let Some(mode) = InteractionMode::from_str_name(args) {
                app.interaction_mode = mode.clone();
                app.add_message(
                    Role::System,
                    format!("已切换到 {} 模式：{}", mode.label(), mode.description()),
                );
            } else {
                app.add_message(
                    Role::System,
                    format!("未知模式: {}。可选: normal, plan, autopilot, ultrawork", args),
                );
            }
        }
        "think" | "depth" => {
            if args.is_empty() {
                app.add_message(
                    Role::System,
                    format!(
                        "当前思考深度: {} {}。可选: off, light, medium, deep, maximum",
                        app.thinking_depth.icon(),
                        app.thinking_depth.label()
                    ),
                );
            } else if let Some(depth) = ThinkingDepth::from_str_name(args) {
                app.thinking_depth = depth.clone();
                app.add_message(
                    Role::System,
                    format!("思考深度已调整为: {} {}", depth.icon(), depth.label()),
                );
            } else {
                app.add_message(
                    Role::System,
                    format!("未知深度: {}。可选: off, light, medium, deep, maximum", args),
                );
            }
        }
        "clear" | "cls" => {
            app.messages.clear();
            app.scroll_offset = 0;
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
    config: &CeairConfig,
) -> Result<()> {
    info!("正在启动交互模式...");
    println!("🤖 ChengCoding 交互模式");
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

    run_text_loop(&mut app, provider.as_ref(), &registry, &tools, &options, config)
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
    config: &CeairConfig,
) -> Result<()> {
    // 维护对话消息历史
    let mut messages = vec![ChatMessage::system(
        "你是一个专业的 AI 编程助手。你可以使用提供的工具来完成任务。\
         请仔细分析用户的需求，合理使用工具，并给出清晰的回答。",
    )];

    // 读取输入的缓冲区
    let stdin = std::io::stdin();

    loop {
        // 显示输入提示符
        eprint!(">>> ");

        // 读取用户输入
        let mut input = String::new();
        stdin.read_line(&mut input)
            .context("读取用户输入失败")?;

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

        // 智能体循环处理（支持多轮工具调用）
        let max_iterations = config.agent.max_iterations;
        for iteration in 0..max_iterations {
            // 调用 AI 模型
            let response = match provider.chat_completion(&messages, tools, options).await {
                Ok(resp) => resp,
                Err(e) => {
                    eprintln!("❌ AI 请求失败: {}", e);
                    break;
                }
            };

            // 处理工具调用
            if !response.tool_calls.is_empty() {
                info!("迭代 #{}: AI 请求调用 {} 个工具", iteration + 1, response.tool_calls.len());

                // 记录 AI 的工具调用响应
                messages.push(ChatMessage {
                    role: chengcoding_ai::MessageRole::Assistant,
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

//! # 启动命令
//!
//! 实现 `ceair launch` 子命令，用于启动 AI 智能体。
//! 支持交互模式（TUI）和单次任务模式（提供 prompt 直接执行）。

use anyhow::{Context, Result};
use clap::Args;
use std::collections::HashMap;
use tracing::{info, warn};

use ceair_ai::{
    AiProvider, ChatMessage, ChatOptions, ProviderConfig, ProviderFactory, ToolDefinition,
};
use ceair_ai::provider::{FunctionDefinition, ToolParameter};
use ceair_config::CeairConfig;
use ceair_core::message::Role;
use ceair_tools::{create_default_registry, ToolRegistry};
use ceair_tui::App;

// ============================================================
// 启动命令参数定义
// ============================================================

/// 启动命令的参数
///
/// 用于配置 AI 智能体的运行方式，包括：
/// - 直接提供 prompt 进行单次任务
/// - 选择 AI 模型和提供商
/// - 启用交互模式或禁用 TUI
#[derive(Args, Debug)]
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
    let registry = setup_tool_registry();
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
    } else if args.interactive && !args.no_tui {
        // 交互模式 + TUI 界面
        run_tui_mode(provider, registry, model_name, &config).await
    } else {
        // 交互式命令行模式（纯文本）
        run_interactive_mode(provider, registry, model_name, &config).await
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
        // 尝试从环境变量获取（格式：CEAIR_API_KEY 或 <PROVIDER>_API_KEY）
        let env_key = format!("{}_API_KEY", provider_name.to_uppercase());
        std::env::var(&env_key).ok()
            .or_else(|| std::env::var("CEAIR_API_KEY").ok())
    });

    let api_key = api_key.unwrap_or_default();

    if api_key.is_empty() {
        warn!("未配置 API 密钥，部分功能可能不可用");
    }

    // 构建提供商配置
    let provider_config = ProviderConfig {
        api_key,
        api_secret: None,
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
/// 注册所有内置的文件操作工具（读取、写入、编辑、搜索等）
fn setup_tool_registry() -> ToolRegistry {
    let registry = create_default_registry();
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
                role: ceair_ai::MessageRole::Assistant,
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
/// 创建 TUI 应用实例，在终端中提供图形化的交互体验。
/// 用户可以通过键盘输入消息，查看 AI 的流式回复。
async fn run_tui_mode(
    provider: Box<dyn AiProvider>,
    registry: ToolRegistry,
    model_name: &str,
    config: &CeairConfig,
) -> Result<()> {
    info!("正在启动 TUI 交互模式...");

    // 创建 TUI 应用实例
    let mut app = App::new(model_name);

    // 添加欢迎消息
    app.add_message(
        Role::System,
        format!(
            "欢迎使用 CEAIR！当前模型: {}，提供商: {}",
            model_name, config.ai.provider,
        ),
    );

    // 构建工具定义和请求选项
    let tools = build_tool_definitions(&registry);
    let options = ChatOptions::with_model(model_name)
        .temperature(config.ai.temperature)
        .max_tokens(config.ai.max_tokens);

    // TUI 主循环
    println!("🖥️  TUI 模式已启动（输入 'exit' 或按 Ctrl+C 退出）");
    println!("提示: 当前版本使用简化的文本交互界面");
    println!();

    // 使用简化的文本循环（完整 TUI 渲染待后续实现）
    run_text_loop(&mut app, provider.as_ref(), &registry, &tools, &options, config)
        .await
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
    println!("🤖 CEAIR 交互模式");
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
                    role: ceair_ai::MessageRole::Assistant,
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

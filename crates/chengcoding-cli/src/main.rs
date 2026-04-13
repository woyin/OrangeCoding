//! # ChengCoding CLI 主入口
//!
//! ChengCoding AI 编程助手的命令行入口模块。
//! 负责命令行参数解析、日志初始化、配置加载和子命令分发。

mod commands;
pub mod oauth;
pub mod rpc;
pub mod slash;
pub mod slash_builtins;
pub mod zellij;

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{error, info};

// ============================================================
// CLI 主结构体与子命令定义
// ============================================================

/// ChengCoding — 一个 AI 驱动的编码智能体 CLI 工具
#[derive(Parser, Debug)]
#[command(
    name = "chengcoding",
    version,
    about = "ChengCoding — AI 驱动的编码智能体命令行工具",
    long_about = "ChengCoding 是一个基于 AI 的编码助手 CLI 工具，\n\
                  支持多种 AI 提供商、工具调用和终端交互界面。\n\
                  直接运行 chengcoding 即等同于 chengcoding launch。"
)]
struct Cli {
    /// 子命令（不指定时默认执行 launch）
    #[command(subcommand)]
    command: Option<Commands>,

    /// 日志级别（trace / debug / info / warn / error）
    #[arg(long, global = true, default_value = "info")]
    log_level: String,

    /// 是否以 JSON 格式输出日志
    #[arg(long, global = true, default_value_t = false)]
    json_log: bool,
}

/// 支持的子命令枚举
#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// 启动 AI 智能体（交互模式或单次任务模式）
    Launch(commands::launch::LaunchArgs),

    /// 管理配置项（查看、设置、初始化）
    Config(commands::config::ConfigArgs),

    /// 显示系统状态（配置、提供商、工具等）
    Status(commands::status::StatusArgs),

    /// 启动本地 Web 控制服务器（HTTP + WebSocket）
    Serve(commands::serve::ServeArgs),

    /// 显示版本信息
    Version,
}

// ============================================================
// 日志初始化
// ============================================================

/// 初始化 tracing 日志系统
///
/// 根据传入的日志级别和格式选项配置全局日志订阅器。
/// 支持普通文本格式和 JSON 格式两种输出。
fn init_tracing(level: &str, json_format: bool) -> Result<()> {
    use tracing_subscriber::EnvFilter;

    // 环境变量优先，否则使用命令行参数指定的级别
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    if json_format {
        // JSON 格式日志，适合日志采集和分析
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .init();
    } else {
        // 人类可读的文本格式日志
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .init();
    }

    Ok(())
}

// ============================================================
// 主函数
// ============================================================

/// 程序主入口
///
/// 执行流程：
/// 1. 解析命令行参数
/// 2. 初始化日志系统
/// 3. 加载配置（配置文件 + 默认值）
/// 4. 注册 Ctrl+C 信号处理（优雅退出）
/// 5. 根据子命令分发到对应的处理函数
#[tokio::main]
async fn main() -> Result<()> {
    // 解析命令行参数
    let cli = Cli::parse();

    // 初始化日志系统
    init_tracing(&cli.log_level, cli.json_log).context("日志系统初始化失败")?;

    info!("ChengCoding CLI 启动中...");

    // 加载配置
    let config_manager = chengcoding_config::ConfigManager::new()
        .map_err(|e| anyhow::anyhow!("配置管理器初始化失败: {}", e))?;
    let config = config_manager
        .load()
        .await
        .map_err(|e| anyhow::anyhow!("配置加载失败: {}", e))?;

    info!("配置加载完成，AI 提供商: {}", config.ai.provider);

    // 注册 Ctrl+C 信号处理器，用于优雅退出
    let shutdown_signal = tokio::signal::ctrl_c();
    tokio::pin!(shutdown_signal);

    // 根据子命令分发执行（未指定子命令时默认为 launch）
    let command = cli
        .command
        .unwrap_or(Commands::Launch(commands::launch::LaunchArgs::default()));
    let result = tokio::select! {
        // 正常执行子命令
        result = dispatch_command(command, config, config_manager) => result,
        // 收到 Ctrl+C 信号，执行优雅退出
        _ = &mut shutdown_signal => {
            info!("收到中断信号，正在优雅退出...");
            println!("\n收到中断信号，正在退出...");
            Ok(())
        }
    };

    // 处理执行结果
    if let Err(ref e) = result {
        error!("命令执行失败: {:?}", e);
        eprintln!("错误: {:#}", e);
    }

    info!("ChengCoding CLI 已退出");
    result
}

/// 根据子命令分发到对应的处理函数
///
/// # 参数
/// - `command`: 解析后的子命令枚举
/// - `config`: 当前生效的配置
/// - `config_manager`: 配置管理器（部分命令需要修改配置）
async fn dispatch_command(
    command: Commands,
    config: chengcoding_config::CeairConfig,
    config_manager: chengcoding_config::ConfigManager,
) -> Result<()> {
    match command {
        Commands::Launch(args) => commands::launch::execute(args, config).await,
        Commands::Config(args) => commands::config::execute(args, config_manager).await,
        Commands::Status(args) => commands::status::execute(args, config).await,
        Commands::Serve(args) => commands::serve::execute(args, config).await,
        Commands::Version => {
            print_version();
            Ok(())
        }
    }
}

/// 打印详细的版本信息
fn print_version() {
    println!("chengcoding {}", env!("CARGO_PKG_VERSION"));
    println!("AI 驱动的编码智能体命令行工具");
    println!();
    println!("支持的 AI 提供商:");
    println!("  - OpenAI (GPT-5.4/GPT-4o)");
    println!("  - Anthropic (Claude Opus/Sonnet)");
    println!("  - DeepSeek（深度求索）");
    println!("  - Qianwen（通义千问）");
    println!("  - Wenxin（文心一言）");
}

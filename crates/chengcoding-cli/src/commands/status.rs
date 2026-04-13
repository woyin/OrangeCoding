//! # 状态查看命令
//!
//! 实现 `ceair status` 子命令，用于显示系统的运行状态信息。
//! 包括当前配置概要、AI 提供商状态、已注册工具列表和版本信息。

use anyhow::Result;
use clap::Args;

use chengcoding_config::CeairConfig;
use chengcoding_tools::{create_default_registry, SecurityPolicy};

// ============================================================
// 状态命令参数定义
// ============================================================

/// 状态命令的参数
#[derive(Args, Debug)]
pub struct StatusArgs {
    /// 显示详细信息（包括工具参数模式等）
    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,
}

// ============================================================
// 命令执行入口
// ============================================================

/// 执行状态查看命令
///
/// 输出以下信息：
/// - 版本信息
/// - AI 提供商配置状态
/// - API 密钥可用性
/// - 已注册的工具列表
/// - 配置文件位置
///
/// # 参数
/// - `args`: 状态命令参数
/// - `config`: 当前生效的配置
pub async fn execute(args: StatusArgs, config: CeairConfig) -> Result<()> {
    // 输出版本信息
    print_version_section();

    // 输出 AI 提供商状态
    print_provider_section(&config);

    // 输出 API 密钥状态
    print_api_key_section(&config);

    // 输出工具注册表状态
    print_tools_section(args.verbose);

    // 输出配置摘要
    print_config_section(&config);

    Ok(())
}

// ============================================================
// 各状态段的输出函数
// ============================================================

/// 输出版本信息段
fn print_version_section() {
    println!("📋 ChengCoding 系统状态");
    println!("{}", "─".repeat(50));
    println!("  版本:   {}", env!("CARGO_PKG_VERSION"));
    println!();
}

/// 输出 AI 提供商配置状态
///
/// 显示当前使用的提供商、模型以及相关参数。
fn print_provider_section(config: &CeairConfig) {
    println!("🤖 AI 提供商");
    println!("{}", "─".repeat(50));
    println!("  提供商:     {}", config.ai.provider);
    println!("  模型:       {}", config.ai.model);
    println!("  温度:       {}", config.ai.temperature);
    println!("  最大令牌:   {}", config.ai.max_tokens);

    // 显示自定义 API 地址（如果设置了）
    if let Some(ref url) = config.ai.base_url {
        println!("  API 地址:   {}", url);
    }

    // 列出所有支持的提供商
    println!();
    println!("  支持的提供商:");
    println!("    ✓ deepseek    — DeepSeek 深度求索");
    println!("    ✓ qianwen     — 通义千问（阿里云）");
    println!("    ✓ wenxin      — 文心一言（百度）");
    println!();
}

/// 输出 API 密钥的可用性状态
///
/// 检查配置文件和环境变量中的 API 密钥设置。
/// 密钥内容不会被显示，只报告是否可用。
fn print_api_key_section(config: &CeairConfig) {
    println!("🔑 API 密钥状态");
    println!("{}", "─".repeat(50));

    // 检查配置文件中的密钥
    let config_key_status = match &config.ai.api_key {
        Some(key) if !key.is_empty() => "✅ 已配置",
        _ => "❌ 未配置",
    };
    println!("  配置文件:   {}", config_key_status);

    // 检查环境变量中的密钥
    let env_keys = [
        ("ChengCoding_API_KEY", "通用密钥"),
        ("DEEPSEEK_API_KEY", "DeepSeek 密钥"),
        ("QIANWEN_API_KEY", "通义千问密钥"),
        ("WENXIN_API_KEY", "文心一言密钥"),
    ];

    for (env_var, desc) in &env_keys {
        let status = if std::env::var(env_var).is_ok() {
            "✅ 已设置"
        } else {
            "❌ 未设置"
        };
        println!("  {} ({}): {}", env_var, desc, status);
    }

    println!();
}

/// 输出工具注册表状态
///
/// 列出所有已注册的内置工具及其描述。
/// 在详细模式下还会显示每个工具的参数模式。
fn print_tools_section(verbose: bool) {
    println!("🔧 已注册工具");
    println!("{}", "─".repeat(50));

    // 创建默认工具注册表以获取工具列表
    let registry = create_default_registry(SecurityPolicy::default_policy());
    let tool_names = registry.list_tools();

    println!("  总计: {} 个工具", tool_names.len());
    println!();

    for name in &tool_names {
        if let Some(tool) = registry.get(name) {
            println!("  📌 {}", tool.name());
            println!("     {}", tool.description());

            // 详细模式下显示参数模式
            if verbose {
                let schema = tool.parameters_schema();
                if let Ok(formatted) = serde_json::to_string_pretty(&schema) {
                    // 缩进每行输出
                    for line in formatted.lines() {
                        println!("     {}", line);
                    }
                }
            }

            println!();
        }
    }
}

/// 输出配置摘要
///
/// 显示智能体、工具安全策略、TUI 和日志等配置的概要信息。
fn print_config_section(config: &CeairConfig) {
    println!("⚙️  配置摘要");
    println!("{}", "─".repeat(50));

    // 智能体配置
    println!("  智能体:");
    println!("    最大迭代次数:   {}", config.agent.max_iterations);
    println!("    超时时间:       {} 秒", config.agent.timeout_secs);
    println!(
        "    自动批准工具:   {}",
        if config.agent.auto_approve_tools {
            "是"
        } else {
            "否"
        },
    );

    // 工具安全配置
    println!("  工具安全策略:");
    println!("    允许路径数:     {}", config.tools.allowed_paths.len(),);
    println!("    禁止路径数:     {}", config.tools.blocked_paths.len(),);
    println!(
        "    最大文件大小:   {} MB",
        config.tools.max_file_size / (1024 * 1024),
    );

    // TUI 配置
    println!("  TUI 界面:");
    println!("    主题:           {}", config.tui.theme);
    println!(
        "    显示令牌用量:   {}",
        if config.tui.show_token_usage {
            "是"
        } else {
            "否"
        },
    );
    println!(
        "    显示时间戳:     {}",
        if config.tui.show_timestamps {
            "是"
        } else {
            "否"
        },
    );

    // 日志配置
    println!("  日志:");
    println!("    日志级别:       {}", config.logging.level);
    println!(
        "    JSON 格式:      {}",
        if config.logging.json_format {
            "是"
        } else {
            "否"
        },
    );

    // 配置文件位置提示
    println!();
    println!("  提示: 使用 'ceair config init' 初始化配置文件");

    println!();
}

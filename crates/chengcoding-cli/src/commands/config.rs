//! # 配置管理命令
//!
//! 实现 `ceair config` 子命令，用于查看和管理 ChengCoding 的配置。
//! 支持显示完整配置、获取/设置单个配置项、初始化默认配置文件。

use anyhow::Result;
use clap::{Args, Subcommand};
use tracing::info;

use chengcoding_config::ConfigManager;

// ============================================================
// 配置命令参数定义
// ============================================================

/// 配置管理命令的参数
#[derive(Args, Debug)]
pub struct ConfigArgs {
    /// 配置操作子命令
    #[command(subcommand)]
    pub action: ConfigAction,
}

/// 配置操作子命令枚举
#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// 显示当前完整配置
    Show {
        /// 以 JSON 格式输出（默认为 TOML）
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// 设置配置项的值（格式: key=value）
    Set {
        /// 配置键名（例如: ai.provider, ai.model, ai.api_key）
        key: String,

        /// 配置值
        value: String,
    },

    /// 获取指定配置项的值
    Get {
        /// 配置键名（例如: ai.provider, ai.model）
        key: String,
    },

    /// 初始化默认配置文件
    Init {
        /// 强制覆盖已有配置文件
        #[arg(long, default_value_t = false)]
        force: bool,
    },
}

// ============================================================
// 命令执行入口
// ============================================================

/// 执行配置管理命令
///
/// # 参数
/// - `args`: 配置命令参数
/// - `config_manager`: 配置管理器实例
pub async fn execute(args: ConfigArgs, config_manager: ConfigManager) -> Result<()> {
    match args.action {
        ConfigAction::Show { json } => show_config(&config_manager, json).await,
        ConfigAction::Set { key, value } => set_config(&config_manager, &key, &value).await,
        ConfigAction::Get { key } => get_config(&config_manager, &key).await,
        ConfigAction::Init { force } => init_config(&config_manager, force).await,
    }
}

// ============================================================
// 显示配置
// ============================================================

/// 显示当前完整配置
///
/// 从配置管理器获取当前配置，序列化为指定格式后输出。
/// 注意：API 密钥会被部分遮蔽以保护安全。
async fn show_config(config_manager: &ConfigManager, json: bool) -> Result<()> {
    let config = config_manager.get().await;

    if json {
        // JSON 格式输出
        let output = config
            .to_json()
            .map_err(|e| anyhow::anyhow!("配置序列化为 JSON 失败: {}", e))?;
        println!("{}", output);
    } else {
        // TOML 格式输出（默认）
        let output = config
            .to_toml()
            .map_err(|e| anyhow::anyhow!("配置序列化为 TOML 失败: {}", e))?;
        println!("{}", output);
    }

    Ok(())
}

// ============================================================
// 设置配置项
// ============================================================

/// 设置指定配置项的值
///
/// 支持使用点分隔的路径设置嵌套配置值。
/// 对于 API 密钥等敏感字段，值不会在日志中输出。
///
/// # 支持的配置键
/// - `ai.provider` - AI 提供商名称
/// - `ai.model` - AI 模型名称
/// - `ai.api_key` - API 密钥
/// - `ai.temperature` - 采样温度
/// - `ai.max_tokens` - 最大令牌数
/// - `ai.base_url` - 自定义 API 地址
/// - `agent.max_iterations` - 最大迭代次数
/// - `agent.timeout_secs` - 超时时间
/// - `agent.auto_approve_tools` - 自动批准工具调用
/// - `tui.theme` - TUI 主题
/// - `tui.show_token_usage` - 显示令牌使用量
/// - `tui.show_timestamps` - 显示时间戳
/// - `logging.level` - 日志级别
/// - `logging.json_format` - JSON 格式日志
async fn set_config(config_manager: &ConfigManager, key: &str, value: &str) -> Result<()> {
    // 判断是否为敏感字段（不在日志中输出值）
    let is_sensitive = key.contains("api_key") || key.contains("secret");

    if is_sensitive {
        info!("设置配置项: {} = [已隐藏]", key);
    } else {
        info!("设置配置项: {} = {}", key, value);
    }

    // 使用 update 闭包修改配置
    config_manager
        .update(|config| {
            apply_config_value(config, key, value);
        })
        .await
        .map_err(|e| anyhow::anyhow!("保存配置失败: {}", e))?;

    if is_sensitive {
        println!("✅ 配置项 '{}' 已安全更新", key);
    } else {
        println!("✅ 配置项 '{}' 已设置为 '{}'", key, value);
    }

    Ok(())
}

/// 将值应用到配置结构体的指定字段
///
/// 根据点分隔的键名路径，定位到对应的配置字段并更新值。
/// 不支持的键名会打印警告信息。
fn apply_config_value(config: &mut chengcoding_config::CeairConfig, key: &str, value: &str) {
    match key {
        // AI 相关配置
        "ai.provider" => config.ai.provider = value.to_string(),
        "ai.model" => config.ai.model = value.to_string(),
        "ai.api_key" => config.ai.api_key = Some(value.to_string()),
        "ai.temperature" => {
            if let Ok(temp) = value.parse::<f64>() {
                config.ai.temperature = temp;
            } else {
                eprintln!("⚠️  温度值必须是数字: {}", value);
            }
        }
        "ai.max_tokens" => {
            if let Ok(tokens) = value.parse::<u32>() {
                config.ai.max_tokens = tokens;
            } else {
                eprintln!("⚠️  最大令牌数必须是正整数: {}", value);
            }
        }
        "ai.base_url" => {
            if value.is_empty() || value == "none" {
                config.ai.base_url = None;
            } else {
                config.ai.base_url = Some(value.to_string());
            }
        }

        // 智能体相关配置
        "agent.max_iterations" => {
            if let Ok(n) = value.parse::<u32>() {
                config.agent.max_iterations = n;
            } else {
                eprintln!("⚠️  最大迭代次数必须是正整数: {}", value);
            }
        }
        "agent.timeout_secs" => {
            if let Ok(secs) = value.parse::<u64>() {
                config.agent.timeout_secs = secs;
            } else {
                eprintln!("⚠️  超时时间必须是正整数: {}", value);
            }
        }
        "agent.auto_approve_tools" => {
            config.agent.auto_approve_tools = parse_bool(value);
        }

        // TUI 相关配置
        "tui.theme" => config.tui.theme = value.to_string(),
        "tui.show_token_usage" => {
            config.tui.show_token_usage = parse_bool(value);
        }
        "tui.show_timestamps" => {
            config.tui.show_timestamps = parse_bool(value);
        }

        // 日志相关配置
        "logging.level" => config.logging.level = value.to_string(),
        "logging.json_format" => {
            config.logging.json_format = parse_bool(value);
        }

        // 未知配置键
        _ => {
            eprintln!("⚠️  未知的配置键: '{}'", key);
            eprintln!("   使用 'ceair config show' 查看所有可用的配置项");
        }
    }
}

/// 解析布尔值字符串
///
/// 支持多种布尔值表示方式：true/false, yes/no, 1/0, on/off
fn parse_bool(value: &str) -> bool {
    matches!(value.to_lowercase().as_str(), "true" | "yes" | "1" | "on")
}

// ============================================================
// 获取配置项
// ============================================================

/// 获取指定配置项的值并输出
///
/// 对于敏感字段（如 API 密钥），只显示部分内容。
async fn get_config(config_manager: &ConfigManager, key: &str) -> Result<()> {
    let config = config_manager.get().await;

    // 根据键名获取对应的值
    let value = match key {
        // AI 相关配置
        "ai.provider" => config.ai.provider.clone(),
        "ai.model" => config.ai.model.clone(),
        "ai.api_key" => {
            // 遮蔽 API 密钥，只显示前后各 4 个字符
            match &config.ai.api_key {
                Some(k) if k.len() > 8 => {
                    format!("{}...{}", &k[..4], &k[k.len() - 4..])
                }
                Some(_) => "[已设置]".to_string(),
                None => "[未设置]".to_string(),
            }
        }
        "ai.temperature" => config.ai.temperature.to_string(),
        "ai.max_tokens" => config.ai.max_tokens.to_string(),
        "ai.base_url" => config
            .ai
            .base_url
            .clone()
            .unwrap_or_else(|| "[未设置]".to_string()),

        // 智能体相关配置
        "agent.max_iterations" => config.agent.max_iterations.to_string(),
        "agent.timeout_secs" => config.agent.timeout_secs.to_string(),
        "agent.auto_approve_tools" => config.agent.auto_approve_tools.to_string(),

        // TUI 相关配置
        "tui.theme" => config.tui.theme.clone(),
        "tui.show_token_usage" => config.tui.show_token_usage.to_string(),
        "tui.show_timestamps" => config.tui.show_timestamps.to_string(),

        // 日志相关配置
        "logging.level" => config.logging.level.clone(),
        "logging.json_format" => config.logging.json_format.to_string(),

        // 未知键
        _ => {
            anyhow::bail!(
                "未知的配置键: '{}'\n使用 'ceair config show' 查看所有可用的配置项",
                key
            );
        }
    };

    println!("{} = {}", key, value);
    Ok(())
}

// ============================================================
// 初始化配置
// ============================================================

/// 初始化默认配置文件
///
/// 在 XDG 配置目录下创建默认的 config.toml 文件。
/// 如果文件已存在，需要 --force 参数才能覆盖。
async fn init_config(config_manager: &ConfigManager, force: bool) -> Result<()> {
    let config_dir = config_manager.config_dir();
    let config_path = config_dir.join("config.toml");

    // 检查配置文件是否已存在
    if config_path.exists() && !force {
        println!("⚠️  配置文件已存在: {}", config_path.display());
        println!("   使用 --force 选项覆盖现有配置");
        return Ok(());
    }

    // 保存默认配置
    config_manager
        .save()
        .await
        .map_err(|e| anyhow::anyhow!("保存默认配置失败: {}", e))?;

    println!("✅ 配置文件已创建: {}", config_path.display());
    println!();
    println!("接下来你可以:");
    println!("  1. 编辑配置文件: {}", config_path.display());
    println!("  2. 设置 API 密钥: ceair config set ai.api_key YOUR_KEY");
    println!("  3. 设置提供商:    ceair config set ai.provider deepseek");
    println!("  4. 查看配置:      ceair config show");

    Ok(())
}

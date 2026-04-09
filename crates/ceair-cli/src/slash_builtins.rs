//! # 内置斜杠命令实现
//!
//! 本模块提供所有内置斜杠命令的执行逻辑。
//! 内置命令无需 Markdown 模板，直接由代码处理。

use crate::slash::SlashCommandResult;

/// 执行内置斜杠命令
///
/// 根据命令名称分发到对应的处理函数。
/// 对于未识别的内置命令，返回 `NotFound`。
pub fn execute_builtin(name: &str, args: &str) -> SlashCommandResult {
    match name {
        "help" | "hotkeys" => SlashCommandResult::Prompt(format_help()),
        "model" | "models" => SlashCommandResult::Prompt(format_model_selector()),
        "plan" => SlashCommandResult::Executed,
        "compact" => {
            let focus = if args.is_empty() {
                None
            } else {
                Some(args)
            };
            SlashCommandResult::Prompt(format_compact(focus))
        }
        "new" => SlashCommandResult::Executed,
        "resume" => SlashCommandResult::Executed,
        "export" => SlashCommandResult::Executed,
        "session" => SlashCommandResult::Prompt(format_session_info()),
        "usage" => SlashCommandResult::Prompt(format_usage()),
        "exit" | "quit" => SlashCommandResult::Executed,
        "settings" => SlashCommandResult::Executed,
        "tree" => SlashCommandResult::Executed,
        "branch" => SlashCommandResult::Executed,
        "fork" => SlashCommandResult::Executed,
        "copy" => SlashCommandResult::Executed,
        "debug" => SlashCommandResult::Prompt(format_debug()),
        // 扩展命令：深度初始化、循环模式、重构、工作流控制
        "init-deep" => SlashCommandResult::Prompt(format_init_deep(args)),
        "ralph-loop" => SlashCommandResult::Prompt(format_ralph_loop(args)),
        "ulw-loop" => SlashCommandResult::Prompt(format_ulw_loop(args)),
        "refactor" => SlashCommandResult::Prompt(format_refactor(args)),
        "start-work" => SlashCommandResult::Prompt(format_start_work(args)),
        "stop-continuation" => SlashCommandResult::Executed,
        "handoff" => SlashCommandResult::Prompt(format_handoff(args)),
        _ => SlashCommandResult::NotFound(name.to_string()),
    }
}

/// 格式化帮助信息
fn format_help() -> String {
    let help = "\
CEAIR 斜杠命令帮助
==================

会话管理:
  /new            开始新会话
  /resume         打开会话选择器
  /session        显示会话信息
  /export [path]  导出会话为 HTML
  /tree           会话树导航
  /branch         分支选择器
  /fork           从消息分叉

模型与设置:
  /model          模型选择器
  /settings       设置菜单
  /plan           切换计划模式

上下文管理:
  /compact [focus] 手动压缩上下文

实用工具:
  /copy           复制最后一条消息
  /usage          显示用量
  /debug          调试工具
  /help           显示此帮助
  /hotkeys        显示快捷键

工作流:
  /init-deep      深度初始化项目
  /ralph-loop     Ralph 持续改进循环
  /ulw-loop       UltraWork 全自动模式
  /refactor       重构助手
  /start-work     开始新工作会话
  /stop-continuation 停止自动循环
  /handoff        任务交接

退出:
  /exit           退出
  /quit           退出";

    help.to_string()
}

/// 格式化模型选择器信息
fn format_model_selector() -> String {
    "可用模型:\n  1. DeepSeek Chat\n  2. Qianwen (通义千问)\n  3. Wenxin (文心一言)".to_string()
}

/// 格式化压缩上下文提示
fn format_compact(focus: Option<&str>) -> String {
    match focus {
        Some(f) => format!("正在压缩上下文，聚焦于: {}", f),
        None => "正在压缩上下文...".to_string(),
    }
}

/// 格式化会话信息
fn format_session_info() -> String {
    "当前会话信息（待实现）".to_string()
}

/// 格式化用量信息
fn format_usage() -> String {
    "用量统计（待实现）".to_string()
}

/// 格式化深度初始化提示
///
/// 扫描项目结构，创建 boulder.json，初始化 Agent 状态。
fn format_init_deep(args: &str) -> String {
    let target = if args.is_empty() { "." } else { args };
    format!(
        "深度初始化启动\n\
         目标路径: {}\n\
         步骤:\n\
         1. 扫描项目结构\n\
         2. 创建 boulder.json\n\
         3. 初始化 Agent 状态",
        target
    )
}

/// 格式化 Ralph 循环提示
///
/// 持续改进循环：plan → implement → review → refine。
fn format_ralph_loop(args: &str) -> String {
    let focus = if args.is_empty() {
        "全局".to_string()
    } else {
        args.to_string()
    };
    format!(
        "Ralph 循环已启动\n\
         聚焦: {}\n\
         循环阶段: plan → implement → review → refine",
        focus
    )
}

/// 格式化 UltraWork 循环提示
///
/// 全自动模式启动。
fn format_ulw_loop(args: &str) -> String {
    let config = if args.is_empty() {
        "默认配置".to_string()
    } else {
        args.to_string()
    };
    format!(
        "UltraWork 全自动循环已启动\n配置: {}",
        config
    )
}

/// 格式化重构助手提示
///
/// 分析代码并提出重构建议。
fn format_refactor(args: &str) -> String {
    if args.is_empty() {
        "重构助手已启动\n请指定目标文件或模块以开始分析。".to_string()
    } else {
        format!("重构助手已启动\n分析目标: {}", args)
    }
}

/// 格式化开始工作提示
///
/// 创建新的工作会话，初始化 Boulder。
fn format_start_work(args: &str) -> String {
    let task = if args.is_empty() {
        "未指定".to_string()
    } else {
        args.to_string()
    };
    format!(
        "工作会话已创建\n\
         任务: {}\n\
         Boulder 已初始化",
        task
    )
}

/// 格式化任务交接提示
///
/// 将当前任务交给另一个 Agent。
fn format_handoff(args: &str) -> String {
    if args.is_empty() {
        "任务交接\n请指定目标 Agent 名称。".to_string()
    } else {
        format!("任务交接\n目标 Agent: {}", args)
    }
}

/// 格式化调试信息
fn format_debug() -> String {
    format!(
        "CEAIR 调试信息\n版本: {}\n平台: {} {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_help() {
        let result = execute_builtin("help", "");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(text.contains("斜杠命令帮助"));
                assert!(text.contains("/new"));
                assert!(text.contains("/exit"));
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_execute_hotkeys_alias() {
        // hotkeys 应与 help 返回相同内容
        let result = execute_builtin("hotkeys", "");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(text.contains("斜杠命令帮助"));
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_execute_exit() {
        let result = execute_builtin("exit", "");
        assert!(matches!(result, SlashCommandResult::Executed));
    }

    #[test]
    fn test_execute_quit() {
        let result = execute_builtin("quit", "");
        assert!(matches!(result, SlashCommandResult::Executed));
    }

    #[test]
    fn test_execute_compact_no_focus() {
        let result = execute_builtin("compact", "");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(text.contains("压缩上下文"));
                assert!(!text.contains("聚焦于"));
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_execute_compact_with_focus() {
        let result = execute_builtin("compact", "API 性能");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(text.contains("聚焦于: API 性能"));
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_execute_model() {
        let result = execute_builtin("model", "");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(text.contains("可用模型"));
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_execute_debug() {
        let result = execute_builtin("debug", "");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(text.contains("调试信息"));
                assert!(text.contains("版本"));
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_execute_unknown() {
        let result = execute_builtin("nonexistent", "");
        assert!(matches!(result, SlashCommandResult::NotFound(_)));
    }

    #[test]
    fn test_execute_new() {
        let result = execute_builtin("new", "");
        assert!(matches!(result, SlashCommandResult::Executed));
    }

    #[test]
    fn test_execute_copy() {
        let result = execute_builtin("copy", "");
        assert!(matches!(result, SlashCommandResult::Executed));
    }

    #[test]
    fn test_execute_plan() {
        let result = execute_builtin("plan", "");
        assert!(matches!(result, SlashCommandResult::Executed));
    }

    // ---- 扩展命令测试 ----

    #[test]
    fn test_execute_init_deep() {
        // 验证深度初始化命令返回包含关键信息的提示
        let result = execute_builtin("init-deep", "");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(text.contains("深度初始化启动"));
                assert!(text.contains("boulder.json"));
                assert!(text.contains("Agent 状态"));
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_execute_init_deep_with_path() {
        // 验证深度初始化命令支持自定义路径参数
        let result = execute_builtin("init-deep", "src/core");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(text.contains("src/core"));
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_execute_ralph_loop() {
        // 验证 Ralph 循环命令返回循环阶段信息
        let result = execute_builtin("ralph-loop", "");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(text.contains("Ralph 循环已启动"));
                assert!(text.contains("plan → implement → review → refine"));
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_execute_ulw_loop() {
        // 验证 UltraWork 循环命令返回全自动模式信息
        let result = execute_builtin("ulw-loop", "");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(text.contains("UltraWork 全自动循环已启动"));
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_execute_refactor() {
        // 验证重构助手命令返回正确提示
        let result = execute_builtin("refactor", "");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(text.contains("重构助手已启动"));
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_execute_start_work() {
        // 验证开始工作命令创建会话并初始化 Boulder
        let result = execute_builtin("start-work", "实现用户认证");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(text.contains("工作会话已创建"));
                assert!(text.contains("实现用户认证"));
                assert!(text.contains("Boulder 已初始化"));
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_execute_stop_continuation() {
        // 验证停止继续命令返回 Executed 状态
        let result = execute_builtin("stop-continuation", "");
        assert!(matches!(result, SlashCommandResult::Executed));
    }

    #[test]
    fn test_execute_handoff() {
        // 验证任务交接命令包含目标 Agent 信息
        let result = execute_builtin("handoff", "reviewer-agent");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(text.contains("任务交接"));
                assert!(text.contains("reviewer-agent"));
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }
}

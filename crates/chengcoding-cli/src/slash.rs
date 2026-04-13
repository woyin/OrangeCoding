//! # 斜杠命令系统
//!
//! 本模块实现了聊天内的斜杠命令（以 `/` 为前缀的命令）的发现、注册与执行。
//! 支持内置命令和从 Markdown 文件中加载的自定义命令。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::slash_builtins;

/// 斜杠命令处理结果
#[derive(Clone, Debug)]
pub enum SlashCommandResult {
    /// 返回文本作为 LLM 提示
    Prompt(String),
    /// 无返回值（命令已执行）
    Executed,
    /// 命令未找到
    NotFound(String),
    /// 执行错误
    Error(String),
}

/// 命令来源
#[derive(Clone, Debug, PartialEq)]
pub enum CommandSource {
    /// 内置命令
    Builtin,
    /// 全局用户命令（~/.chengcoding/commands/）
    UserGlobal,
    /// 项目级命令（.chengcoding/commands/）
    Project,
}

/// 斜杠命令定义
#[derive(Clone, Debug)]
pub struct SlashCommandDef {
    /// 命令名称（不含 /）
    pub name: String,
    /// 命令描述
    pub description: String,
    /// 命令来源
    pub source: CommandSource,
    /// 命令内容（Markdown 模板）
    pub template: Option<String>,
    /// 模板文件路径（用于从磁盘加载的命令）
    pub file_path: Option<PathBuf>,
}

/// 斜杠命令注册表
pub struct SlashCommandRegistry {
    commands: HashMap<String, SlashCommandDef>,
}

impl SlashCommandRegistry {
    /// 创建空的命令注册表
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// 注册所有内置命令
    pub fn register_builtins(&mut self) {
        let builtins = [
            ("help", "显示帮助信息"),
            ("hotkeys", "显示快捷键"),
            ("model", "模型选择器"),
            ("models", "模型选择器"),
            ("plan", "切换计划模式"),
            ("compact", "手动压缩上下文"),
            ("new", "开始新会话"),
            ("resume", "打开会话选择器"),
            ("export", "导出会话为 HTML"),
            ("session", "显示会话信息"),
            ("usage", "显示用量"),
            ("exit", "退出"),
            ("quit", "退出"),
            ("settings", "设置菜单"),
            ("tree", "会话树导航"),
            ("branch", "分支选择器"),
            ("fork", "从消息分叉"),
            ("copy", "复制最后一条消息"),
            ("debug", "调试工具"),
            // 扩展命令
            (
                "init-deep",
                "深度初始化：扫描项目结构，创建 boulder.json，初始化 Agent 状态",
            ),
            (
                "ralph-loop",
                "Ralph 循环：持续改进循环（plan → implement → review → refine）",
            ),
            ("ulw-loop", "UltraWork 循环：全自动模式启动"),
            ("refactor", "重构助手：分析代码并提出重构建议"),
            ("start-work", "开始工作：创建新的工作会话，初始化 Boulder"),
            ("stop-continuation", "停止继续：终止当前的自动循环"),
            ("handoff", "任务交接：将当前任务交给另一个 Agent"),
        ];

        for (name, desc) in builtins {
            self.register(SlashCommandDef {
                name: name.to_string(),
                description: desc.to_string(),
                source: CommandSource::Builtin,
                template: None,
                file_path: None,
            });
        }
    }

    /// 从目录中发现 Markdown 命令文件
    ///
    /// 文件名（去掉 `.md` 后缀）即为命令名。
    /// 文件可以包含 YAML frontmatter 用于声明描述等元数据。
    ///
    /// 返回成功发现的命令数量。
    pub fn discover_from_dir(
        &mut self,
        dir: &Path,
        source: CommandSource,
    ) -> Result<usize, std::io::Error> {
        if !dir.is_dir() {
            return Ok(0);
        }

        let mut count = 0;
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            // 仅处理 .md 文件
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }

            let name = match path.file_stem().and_then(|s| s.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let content = std::fs::read_to_string(&path)?;
            let (description, template) = parse_frontmatter(&content);

            self.register(SlashCommandDef {
                name,
                description,
                source: source.clone(),
                template: Some(template),
                file_path: Some(path),
            });

            count += 1;
        }

        Ok(count)
    }

    /// 注册自定义命令（同名命令会被覆盖）
    pub fn register(&mut self, cmd: SlashCommandDef) {
        self.commands.insert(cmd.name.clone(), cmd);
    }

    /// 按名称查找命令
    pub fn get(&self, name: &str) -> Option<&SlashCommandDef> {
        self.commands.get(name)
    }

    /// 列出所有已注册命令
    pub fn list(&self) -> Vec<&SlashCommandDef> {
        let mut cmds: Vec<_> = self.commands.values().collect();
        cmds.sort_by(|a, b| a.name.cmp(&b.name));
        cmds
    }

    /// 执行斜杠命令
    ///
    /// 根据输入字符串解析命令名和参数，然后执行对应的命令。
    pub fn execute(&self, input: &str) -> SlashCommandResult {
        let (name, args) = match Self::parse_input(input) {
            Some(parsed) => parsed,
            None => return SlashCommandResult::NotFound(input.to_string()),
        };

        match self.commands.get(name) {
            Some(cmd) => match cmd.source {
                CommandSource::Builtin => slash_builtins::execute_builtin(name, args),
                _ => {
                    // Markdown 模板命令
                    match &cmd.template {
                        Some(tpl) => {
                            let processed = substitute_args(tpl, args);
                            SlashCommandResult::Prompt(processed)
                        }
                        None => SlashCommandResult::Error(format!("命令 '{}' 缺少模板内容", name)),
                    }
                }
            },
            None => SlashCommandResult::NotFound(name.to_string()),
        }
    }

    /// 解析用户输入，判断是否为斜杠命令
    ///
    /// 返回 `Some((命令名, 参数))` 或 `None`（非斜杠命令）。
    pub fn parse_input(input: &str) -> Option<(&str, &str)> {
        let trimmed = input.trim();
        if !trimmed.starts_with('/') {
            return None;
        }

        let without_slash = &trimmed[1..];
        if without_slash.is_empty() {
            return None;
        }

        // 按第一个空白字符分割命令名和参数
        match without_slash.find(char::is_whitespace) {
            Some(pos) => {
                let name = &without_slash[..pos];
                let args = without_slash[pos..].trim_start();
                Some((name, args))
            }
            None => Some((without_slash, "")),
        }
    }
}

/// 解析 Markdown 文件中的 YAML frontmatter
///
/// 返回 `(描述, 模板正文)`。
fn parse_frontmatter(content: &str) -> (String, String) {
    let trimmed = content.trim();

    if !trimmed.starts_with("---") {
        return (String::new(), content.to_string());
    }

    // 寻找结束标记 ---
    let after_start = &trimmed[3..];
    match after_start.find("---") {
        Some(end_pos) => {
            let frontmatter = &after_start[..end_pos];
            let body = after_start[end_pos + 3..].trim().to_string();

            // 从 frontmatter 中提取 description 字段
            let description = frontmatter
                .lines()
                .find_map(|line| {
                    let line = line.trim();
                    if let Some(rest) = line.strip_prefix("description:") {
                        Some(rest.trim().to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();

            (description, body)
        }
        None => (String::new(), content.to_string()),
    }
}

/// 替换模板中的参数占位符
///
/// 支持:
/// - `$1`, `$2`, ... — 位置参数
/// - `$@` 和 `$ARGUMENTS` — 所有参数（空格连接）
fn substitute_args(template: &str, args_str: &str) -> String {
    let args: Vec<&str> = if args_str.is_empty() {
        Vec::new()
    } else {
        args_str.split_whitespace().collect()
    };

    let mut result = template.to_string();

    // 先替换 $@ 和 $ARGUMENTS（全部参数）
    result = result.replace("$@", args_str);
    result = result.replace("$ARGUMENTS", args_str);

    // 替换位置参数 $1, $2, ...（从大到小避免 $1 先于 $10 匹配）
    for i in (0..args.len()).rev() {
        let placeholder = format!("${}", i + 1);
        let value = args.get(i).unwrap_or(&"");
        result = result.replace(&placeholder, value);
    }

    result
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ---- 输入解析测试 ----

    #[test]
    fn test_parse_input_slash_command() {
        let result = SlashCommandRegistry::parse_input("/help");
        assert_eq!(result, Some(("help", "")));
    }

    #[test]
    fn test_parse_input_slash_with_args() {
        let result = SlashCommandRegistry::parse_input("/compact focus on API");
        assert_eq!(result, Some(("compact", "focus on API")));
    }

    #[test]
    fn test_parse_input_not_slash() {
        let result = SlashCommandRegistry::parse_input("hello");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_input_empty() {
        let result = SlashCommandRegistry::parse_input("");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_input_slash_only() {
        // 仅 "/" 不构成有效命令
        let result = SlashCommandRegistry::parse_input("/");
        assert!(result.is_none());
    }

    // ---- 注册与查找测试 ----

    #[test]
    fn test_register_and_get() {
        let mut registry = SlashCommandRegistry::new();
        registry.register(SlashCommandDef {
            name: "test".to_string(),
            description: "测试命令".to_string(),
            source: CommandSource::Builtin,
            template: None,
            file_path: None,
        });

        let cmd = registry.get("test");
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().name, "test");
        assert_eq!(cmd.unwrap().description, "测试命令");
    }

    #[test]
    fn test_list_commands() {
        let mut registry = SlashCommandRegistry::new();
        registry.register(SlashCommandDef {
            name: "beta".to_string(),
            description: "B".to_string(),
            source: CommandSource::Builtin,
            template: None,
            file_path: None,
        });
        registry.register(SlashCommandDef {
            name: "alpha".to_string(),
            description: "A".to_string(),
            source: CommandSource::Builtin,
            template: None,
            file_path: None,
        });

        let list = registry.list();
        assert_eq!(list.len(), 2);
        // 按名称排序
        assert_eq!(list[0].name, "alpha");
        assert_eq!(list[1].name, "beta");
    }

    #[test]
    fn test_builtin_registration() {
        let mut registry = SlashCommandRegistry::new();
        registry.register_builtins();

        // 验证关键内置命令已注册
        assert!(registry.get("help").is_some());
        assert!(registry.get("exit").is_some());
        assert!(registry.get("quit").is_some());
        assert!(registry.get("model").is_some());
        assert!(registry.get("plan").is_some());
        assert!(registry.get("compact").is_some());
        assert!(registry.get("new").is_some());
        assert!(registry.get("copy").is_some());
        assert!(registry.get("debug").is_some());

        // 所有内置命令来源应为 Builtin
        for cmd in registry.list() {
            assert_eq!(cmd.source, CommandSource::Builtin);
            assert!(cmd.template.is_none());
        }
    }

    // ---- Markdown 命令发现测试 ----

    #[test]
    fn test_discover_markdown_commands() {
        let dir = tempfile::tempdir().unwrap();

        // 创建两个 .md 命令文件
        fs::write(
            dir.path().join("review.md"),
            "---\ndescription: 代码审查\n---\n审查以下代码变更。",
        )
        .unwrap();
        fs::write(dir.path().join("summarize.md"), "总结以下内容。").unwrap();

        // 创建一个非 .md 文件，应被忽略
        fs::write(dir.path().join("notes.txt"), "这不是命令").unwrap();

        let mut registry = SlashCommandRegistry::new();
        let count = registry
            .discover_from_dir(dir.path(), CommandSource::Project)
            .unwrap();

        assert_eq!(count, 2);
        assert!(registry.get("review").is_some());
        assert!(registry.get("summarize").is_some());
        assert!(registry.get("notes").is_none());

        // 验证来源
        assert_eq!(
            registry.get("review").unwrap().source,
            CommandSource::Project
        );
    }

    #[test]
    fn test_discover_nonexistent_dir() {
        let mut registry = SlashCommandRegistry::new();
        let count = registry
            .discover_from_dir(Path::new("/nonexistent/path"), CommandSource::UserGlobal)
            .unwrap();
        assert_eq!(count, 0);
    }

    // ---- Frontmatter 解析测试 ----

    #[test]
    fn test_frontmatter_parsing() {
        let content =
            "---\ndescription: Review staged git changes\n---\n\nReview the staged changes.";
        let (desc, body) = parse_frontmatter(content);

        assert_eq!(desc, "Review staged git changes");
        assert_eq!(body, "Review the staged changes.");
    }

    #[test]
    fn test_frontmatter_missing() {
        let content = "Just a plain template.";
        let (desc, body) = parse_frontmatter(content);

        assert!(desc.is_empty());
        assert_eq!(body, "Just a plain template.");
    }

    // ---- 模板参数替换测试 ----

    #[test]
    fn test_template_argument_substitution() {
        let template = "分析 $1 文件中的 $2 函数。";
        let result = substitute_args(template, "main.rs parse_input");

        assert_eq!(result, "分析 main.rs 文件中的 parse_input 函数。");
    }

    #[test]
    fn test_template_all_arguments() {
        let template = "请关注以下内容：$@";
        let result = substitute_args(template, "API 安全性 性能");

        assert_eq!(result, "请关注以下内容：API 安全性 性能");
    }

    #[test]
    fn test_template_arguments_alias() {
        let template = "关注 $ARGUMENTS";
        let result = substitute_args(template, "错误处理");

        assert_eq!(result, "关注 错误处理");
    }

    #[test]
    fn test_template_no_args() {
        let template = "无参数模板 $@";
        let result = substitute_args(template, "");

        assert_eq!(result, "无参数模板 ");
    }

    // ---- 命令执行测试 ----

    #[test]
    fn test_execute_builtin() {
        let mut registry = SlashCommandRegistry::new();
        registry.register_builtins();

        let result = registry.execute("/help");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert!(!text.is_empty());
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_execute_markdown_command() {
        let mut registry = SlashCommandRegistry::new();
        registry.register(SlashCommandDef {
            name: "greet".to_string(),
            description: "问候命令".to_string(),
            source: CommandSource::Project,
            template: Some("你好，$1！欢迎使用 ChengCoding。".to_string()),
            file_path: None,
        });

        let result = registry.execute("/greet 用户");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert_eq!(text, "你好，用户！欢迎使用 ChengCoding。");
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }

    #[test]
    fn test_execute_not_found() {
        let registry = SlashCommandRegistry::new();
        let result = registry.execute("/nonexistent");
        match result {
            SlashCommandResult::NotFound(name) => {
                assert_eq!(name, "nonexistent");
            }
            other => panic!("期望 NotFound，得到 {:?}", other),
        }
    }

    #[test]
    fn test_command_source_priority() {
        // 项目级命令应覆盖全局命令（后注册覆盖先注册）
        let mut registry = SlashCommandRegistry::new();

        // 先注册全局命令
        registry.register(SlashCommandDef {
            name: "review".to_string(),
            description: "全局审查".to_string(),
            source: CommandSource::UserGlobal,
            template: Some("全局模板".to_string()),
            file_path: None,
        });

        // 再注册项目级命令（同名，应覆盖）
        registry.register(SlashCommandDef {
            name: "review".to_string(),
            description: "项目审查".to_string(),
            source: CommandSource::Project,
            template: Some("项目模板".to_string()),
            file_path: None,
        });

        let cmd = registry.get("review").unwrap();
        assert_eq!(cmd.source, CommandSource::Project);
        assert_eq!(cmd.description, "项目审查");

        // 执行时应使用项目级模板
        let result = registry.execute("/review");
        match result {
            SlashCommandResult::Prompt(text) => {
                assert_eq!(text, "项目模板");
            }
            other => panic!("期望 Prompt，得到 {:?}", other),
        }
    }
}

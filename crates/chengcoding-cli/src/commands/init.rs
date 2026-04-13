//! # 项目初始化命令
//!
//! 实现 `ceair init` 子命令，用于初始化项目的 ChengCoding 配置。

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 数据结构
// ---------------------------------------------------------------------------

/// 项目初始化命令
pub struct InitCommand;

/// 初始化选项
#[derive(Clone, Debug)]
pub struct InitOptions {
    /// 项目根目录
    pub project_dir: PathBuf,
    /// 是否创建 config.toml
    pub create_config: bool,
    /// 是否创建 AGENTS.md
    pub create_agents_md: bool,
    /// 是否创建 .chengcoding/commands 目录
    pub create_commands_dir: bool,
    /// AI 提供商（可选）
    pub provider: Option<String>,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            project_dir: PathBuf::from("."),
            create_config: true,
            create_agents_md: true,
            create_commands_dir: true,
            provider: None,
        }
    }
}

// ---------------------------------------------------------------------------
// 命令实现
// ---------------------------------------------------------------------------

impl InitCommand {
    /// 生成默认 AGENTS.md 内容
    pub fn generate_agents_md(project_name: &str) -> String {
        format!(
            r#"# {} — ChengCoding 代理配置

## 项目概述

本项目使用 ChengCoding AI 编码代理进行辅助开发。

## 代理行为规则

- 所有注释必须使用中文
- 遵循项目的代码风格和约定
- 修改代码前先运行测试，确保不破坏现有功能

## 工具使用约定

- `bash`: 用于执行 shell 命令
- `read`: 用于读取文件内容
- `write`: 用于写入文件
- `search`: 用于搜索代码

## 测试要求

- 使用 TDD 方式开发：先写测试，再写实现
- 所有公共 API 必须有测试覆盖
"#,
            project_name
        )
    }

    /// 生成默认 config.toml 内容
    pub fn generate_config_toml(provider: &str) -> String {
        format!(
            r#"# ChengCoding 项目配置
# 由 ceair init 自动生成

[model]
provider = "{provider}"
model = "gpt-4o"

[tools]
bash_timeout = 30
allow_dangerous = false

[compaction]
enabled = true
max_tokens = 100000
keep_recent = 10

[memory]
enabled = false
max_entries = 1000
"#
        )
    }

    /// 获取需要创建的文件列表
    pub fn files_to_create(options: &InitOptions) -> Vec<(PathBuf, String)> {
        let mut files = Vec::new();
        let dir = &options.project_dir;
        let provider = options.provider.as_deref().unwrap_or("openai");

        if options.create_config {
            files.push((
                dir.join(".chengcoding").join("config.toml"),
                Self::generate_config_toml(provider),
            ));
        }

        if options.create_agents_md {
            // 从项目目录名推导项目名
            let project_name = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("my-project");
            files.push((
                dir.join("AGENTS.md"),
                Self::generate_agents_md(project_name),
            ));
        }

        if options.create_commands_dir {
            // 命令目录用 .gitkeep 占位
            files.push((
                dir.join(".chengcoding").join("commands").join(".gitkeep"),
                String::new(),
            ));
        }

        files
    }

    /// 检查项目是否已初始化
    pub fn is_initialized(project_dir: &Path) -> bool {
        project_dir
            .join(".chengcoding")
            .join("config.toml")
            .exists()
    }
}

// ===========================================================================
// 测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试生成 AGENTS.md
    #[test]
    fn test_generate_agents_md() {
        let content = InitCommand::generate_agents_md("test-project");
        assert!(content.contains("test-project"));
        assert!(content.contains("ChengCoding"));
        assert!(content.contains("代理"));
        assert!(content.contains("TDD"));
    }

    /// 测试生成 config.toml
    #[test]
    fn test_generate_config_toml() {
        let content = InitCommand::generate_config_toml("deepseek");
        assert!(content.contains("deepseek"));
        assert!(content.contains("[model]"));
        assert!(content.contains("[tools]"));
        assert!(content.contains("[compaction]"));
        assert!(content.contains("[memory]"));
    }

    /// 测试需要创建的文件列表
    #[test]
    fn test_files_to_create() {
        let options = InitOptions {
            project_dir: PathBuf::from("/fake/project"),
            create_config: true,
            create_agents_md: true,
            create_commands_dir: true,
            provider: Some("anthropic".to_string()),
        };

        let files = InitCommand::files_to_create(&options);
        assert_eq!(files.len(), 3);

        // 检查 config.toml
        let config_file = files.iter().find(|(p, _)| p.ends_with("config.toml"));
        assert!(config_file.is_some());
        assert!(config_file.unwrap().1.contains("anthropic"));

        // 检查 AGENTS.md
        let agents_file = files.iter().find(|(p, _)| p.ends_with("AGENTS.md"));
        assert!(agents_file.is_some());

        // 检查 commands 目录占位
        let gitkeep = files.iter().find(|(p, _)| p.ends_with(".gitkeep"));
        assert!(gitkeep.is_some());
    }

    /// 测试空目录未初始化
    #[test]
    fn test_is_initialized_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!InitCommand::is_initialized(dir.path()));
    }

    /// 测试已初始化的项目
    #[test]
    fn test_is_initialized_with_config() {
        let dir = tempfile::tempdir().unwrap();
        let chengcoding_dir = dir.path().join(".chengcoding");
        std::fs::create_dir_all(&chengcoding_dir).unwrap();
        std::fs::write(chengcoding_dir.join("config.toml"), "# config").unwrap();

        assert!(InitCommand::is_initialized(dir.path()));
    }

    /// 测试默认选项
    #[test]
    fn test_default_options() {
        let opts = InitOptions::default();
        assert_eq!(opts.project_dir, PathBuf::from("."));
        assert!(opts.create_config);
        assert!(opts.create_agents_md);
        assert!(opts.create_commands_dir);
        assert!(opts.provider.is_none());
    }
}

//! # 多工具配置发现模块
//!
//! 从多个 AI 编码工具的配置目录中发现和加载配置。
//! 支持 ChengCoding 原生配置、Claude Code、Codex、Gemini 等工具的配置发现。
//! 按优先级排序，高优先级的配置优先生效。

use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::debug;

// ---------------------------------------------------------------------------
// 配置提供者枚举
// ---------------------------------------------------------------------------

/// 配置提供者 — 标识配置来源
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConfigProvider {
    /// ChengCoding 原生配置 (.chengcoding/)
    Native,
    /// Claude Code 配置 (.claude/)
    Claude,
    /// Codex 配置 (.codex/)
    Codex,
    /// Gemini 配置 (.gemini/)
    Gemini,
}

impl ConfigProvider {
    /// 获取配置目录名
    pub fn dir_name(&self) -> &str {
        match self {
            ConfigProvider::Native => ".chengcoding",
            ConfigProvider::Claude => ".claude",
            ConfigProvider::Codex => ".codex",
            ConfigProvider::Gemini => ".gemini",
        }
    }

    /// 获取优先级（数值越高越优先）
    pub fn priority(&self) -> u32 {
        match self {
            ConfigProvider::Native => 100,
            ConfigProvider::Claude => 80,
            ConfigProvider::Codex => 70,
            ConfigProvider::Gemini => 60,
        }
    }

    /// 获取所有提供者（按优先级从高到低排序）
    pub fn all_sorted() -> Vec<ConfigProvider> {
        let mut providers = vec![
            ConfigProvider::Native,
            ConfigProvider::Claude,
            ConfigProvider::Codex,
            ConfigProvider::Gemini,
        ];
        providers.sort_by(|a, b| b.priority().cmp(&a.priority()));
        providers
    }
}

// ---------------------------------------------------------------------------
// 发现项类型
// ---------------------------------------------------------------------------

/// 配置项类型
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscoveryType {
    /// 规则文件（AGENTS.md, CLAUDE.md 等）
    Rule,
    /// MCP 服务器配置
    McpServer,
    /// 技能包
    Skill,
    /// 自定义工具
    Tool,
    /// 钩子
    Hook,
    /// 斜杠命令
    Command,
    /// 上下文文件
    ContextFile,
}

/// 发现到的配置项
#[derive(Clone, Debug)]
pub struct DiscoveredItem {
    /// 配置项类型
    pub item_type: DiscoveryType,
    /// 配置项名称（用于去重）
    pub name: String,
    /// 配置文件路径
    pub path: PathBuf,
    /// 配置来源提供者
    pub provider: ConfigProvider,
    /// 是否启用
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// 配置发现器
// ---------------------------------------------------------------------------

/// 配置发现器 — 从多个工具目录中发现配置
pub struct ConfigDiscovery {
    /// 项目根目录
    project_dir: PathBuf,
    /// 用户主目录
    home_dir: PathBuf,
    /// 已禁用的提供者
    disabled_providers: Vec<ConfigProvider>,
    /// 已禁用的扩展名
    disabled_extensions: Vec<String>,
}

impl ConfigDiscovery {
    /// 创建新的配置发现器
    pub fn new(project_dir: PathBuf, home_dir: PathBuf) -> Self {
        Self {
            project_dir,
            home_dir,
            disabled_providers: Vec::new(),
            disabled_extensions: Vec::new(),
        }
    }

    /// 发现所有配置目录（按优先级排序）
    ///
    /// 返回 `[(provider, path)]`，先用户级后项目级，按优先级从高到低。
    /// 项目级配置优先于用户级配置。
    pub fn get_config_dirs(&self) -> Vec<(ConfigProvider, PathBuf)> {
        let mut dirs = Vec::new();
        let providers = ConfigProvider::all_sorted();

        for provider in &providers {
            if self.is_provider_disabled(provider) {
                continue;
            }

            // 项目级配置（优先）
            let project_path = self.project_dir.join(provider.dir_name());
            if project_path.is_dir() {
                dirs.push((provider.clone(), project_path));
            }

            // 用户级配置
            let home_path = self.home_dir.join(provider.dir_name());
            if home_path.is_dir() {
                dirs.push((provider.clone(), home_path));
            }
        }

        dirs
    }

    /// 发现所有规则文件（AGENTS.md, CLAUDE.md 等）
    pub fn discover_rules(&self) -> Vec<DiscoveredItem> {
        let rule_files = ["AGENTS.md", "CLAUDE.md", "RULES.md"];
        let mut items = Vec::new();
        let mut seen_names: HashSet<String> = HashSet::new();

        for (provider, dir) in self.get_config_dirs() {
            for rule_file in &rule_files {
                let path = dir.join(rule_file);
                if path.is_file() {
                    let name = rule_file.to_string();
                    if seen_names.contains(&name) {
                        continue; // 去重：同名文件只保留高优先级的
                    }
                    seen_names.insert(name.clone());
                    items.push(DiscoveredItem {
                        item_type: DiscoveryType::Rule,
                        name,
                        path,
                        provider: provider.clone(),
                        enabled: true,
                    });
                }
            }
        }

        items
    }

    /// 发现所有 MCP 服务器配置
    pub fn discover_mcp_configs(&self) -> Vec<DiscoveredItem> {
        let mcp_files = ["mcp.json", "mcp.toml"];
        let mut items = Vec::new();
        let mut seen_names: HashSet<String> = HashSet::new();

        for (provider, dir) in self.get_config_dirs() {
            for mcp_file in &mcp_files {
                let path = dir.join(mcp_file);
                if path.is_file() {
                    let name = mcp_file.to_string();
                    if seen_names.contains(&name) {
                        continue;
                    }
                    seen_names.insert(name.clone());
                    items.push(DiscoveredItem {
                        item_type: DiscoveryType::McpServer,
                        name,
                        path,
                        provider: provider.clone(),
                        enabled: true,
                    });
                }
            }
        }

        items
    }

    /// 发现所有技能包
    pub fn discover_skills(&self) -> Vec<DiscoveredItem> {
        let mut items = Vec::new();
        let mut seen_names: HashSet<String> = HashSet::new();

        for (provider, dir) in self.get_config_dirs() {
            let skills_dir = dir.join("skills");
            if !skills_dir.is_dir() {
                continue;
            }

            // 遍历 skills/ 下的子目录
            if let Ok(entries) = std::fs::read_dir(&skills_dir) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.is_dir() {
                        let skill_file = entry_path.join("SKILL.md");
                        if skill_file.is_file() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if seen_names.contains(&name) {
                                continue;
                            }
                            seen_names.insert(name.clone());
                            items.push(DiscoveredItem {
                                item_type: DiscoveryType::Skill,
                                name,
                                path: skill_file,
                                provider: provider.clone(),
                                enabled: !self
                                    .is_extension_disabled(&entry.file_name().to_string_lossy()),
                            });
                        }
                    }
                }
            }
        }

        items
    }

    /// 发现所有自定义命令
    pub fn discover_commands(&self) -> Vec<DiscoveredItem> {
        let mut items = Vec::new();
        let mut seen_names: HashSet<String> = HashSet::new();

        for (provider, dir) in self.get_config_dirs() {
            let commands_dir = dir.join("commands");
            if !commands_dir.is_dir() {
                continue;
            }

            if let Ok(entries) = std::fs::read_dir(&commands_dir) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.is_file() {
                        if let Some(ext) = entry_path.extension() {
                            if ext == "md" {
                                let name = entry_path
                                    .file_stem()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string();
                                if seen_names.contains(&name) {
                                    continue;
                                }
                                seen_names.insert(name.clone());
                                items.push(DiscoveredItem {
                                    item_type: DiscoveryType::Command,
                                    name,
                                    path: entry_path,
                                    provider: provider.clone(),
                                    enabled: !self.is_extension_disabled(
                                        &entry.file_name().to_string_lossy(),
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }

        items
    }

    /// 发现所有扩展（汇总所有类型）
    pub fn discover_all(&self) -> Vec<DiscoveredItem> {
        let mut all = Vec::new();
        all.extend(self.discover_rules());
        all.extend(self.discover_mcp_configs());
        all.extend(self.discover_skills());
        all.extend(self.discover_commands());
        all
    }

    /// 查找第一个匹配的配置文件（按优先级）
    pub fn find_config_file(&self, subpath: &str) -> Option<(ConfigProvider, PathBuf)> {
        for (provider, dir) in self.get_config_dirs() {
            let full_path = dir.join(subpath);
            if full_path.is_file() {
                debug!(
                    "找到配置文件: {} (来源: {:?})",
                    full_path.display(),
                    provider
                );
                return Some((provider, full_path));
            }
        }
        None
    }

    /// 禁用某个提供者
    pub fn disable_provider(&mut self, provider: ConfigProvider) {
        if !self.disabled_providers.contains(&provider) {
            self.disabled_providers.push(provider);
        }
    }

    /// 禁用某个扩展
    pub fn disable_extension(&mut self, name: &str) {
        let name_str = name.to_string();
        if !self.disabled_extensions.contains(&name_str) {
            self.disabled_extensions.push(name_str);
        }
    }

    // -----------------------------------------------------------------------
    // 内部辅助方法
    // -----------------------------------------------------------------------

    /// 检查提供者是否被禁用
    fn is_provider_disabled(&self, provider: &ConfigProvider) -> bool {
        self.disabled_providers.contains(provider)
    }

    /// 检查扩展是否被禁用
    fn is_extension_disabled(&self, name: &str) -> bool {
        self.disabled_extensions.iter().any(|n| n == name)
    }
}

// ===========================================================================
// 单元测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    /// 辅助函数：创建目录结构
    fn create_dir(base: &Path, sub: &str) -> PathBuf {
        let p = base.join(sub);
        fs::create_dir_all(&p).unwrap();
        p
    }

    /// 辅助函数：创建文件
    fn create_file(base: &Path, sub: &str, content: &str) {
        let p = base.join(sub);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    // -----------------------------------------------------------------------
    // 1. test_provider_dir_names
    // -----------------------------------------------------------------------
    #[test]
    fn test_provider_dir_names() {
        assert_eq!(ConfigProvider::Native.dir_name(), ".chengcoding");
        assert_eq!(ConfigProvider::Claude.dir_name(), ".claude");
        assert_eq!(ConfigProvider::Codex.dir_name(), ".codex");
        assert_eq!(ConfigProvider::Gemini.dir_name(), ".gemini");
    }

    // -----------------------------------------------------------------------
    // 2. test_provider_priorities
    // -----------------------------------------------------------------------
    #[test]
    fn test_provider_priorities() {
        assert_eq!(ConfigProvider::Native.priority(), 100);
        assert_eq!(ConfigProvider::Claude.priority(), 80);
        assert_eq!(ConfigProvider::Codex.priority(), 70);
        assert_eq!(ConfigProvider::Gemini.priority(), 60);

        // 验证排序正确性
        assert!(ConfigProvider::Native.priority() > ConfigProvider::Claude.priority());
        assert!(ConfigProvider::Claude.priority() > ConfigProvider::Codex.priority());
        assert!(ConfigProvider::Codex.priority() > ConfigProvider::Gemini.priority());
    }

    // -----------------------------------------------------------------------
    // 3. test_config_dirs_ordering
    // -----------------------------------------------------------------------
    #[test]
    fn test_config_dirs_ordering() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("project");
        let home = tmp.path().join("home");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&home).unwrap();

        // 创建项目级和用户级配置目录
        create_dir(&project, ".chengcoding");
        create_dir(&project, ".claude");
        create_dir(&home, ".chengcoding");
        create_dir(&home, ".gemini");

        let discovery = ConfigDiscovery::new(project, home);
        let dirs = discovery.get_config_dirs();

        // Native 优先级最高，项目级优先于用户级
        assert_eq!(dirs[0].0, ConfigProvider::Native); // 项目级 .chengcoding
        assert_eq!(dirs[1].0, ConfigProvider::Native); // 用户级 .chengcoding
        assert_eq!(dirs[2].0, ConfigProvider::Claude); // 项目级 .claude
        assert_eq!(dirs[3].0, ConfigProvider::Gemini); // 用户级 .gemini
        assert_eq!(dirs.len(), 4);
    }

    // -----------------------------------------------------------------------
    // 4. test_discover_rules_native
    // -----------------------------------------------------------------------
    #[test]
    fn test_discover_rules_native() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("project");
        let home = tmp.path().join("home");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&home).unwrap();

        // 创建 .chengcoding/AGENTS.md
        create_file(&project, ".chengcoding/AGENTS.md", "# 规则文件");

        let discovery = ConfigDiscovery::new(project, home);
        let rules = discovery.discover_rules();

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].name, "AGENTS.md");
        assert_eq!(rules[0].provider, ConfigProvider::Native);
        assert_eq!(rules[0].item_type, DiscoveryType::Rule);
        assert!(rules[0].enabled);
    }

    // -----------------------------------------------------------------------
    // 5. test_discover_rules_claude
    // -----------------------------------------------------------------------
    #[test]
    fn test_discover_rules_claude() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("project");
        let home = tmp.path().join("home");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&home).unwrap();

        // 创建 .claude/CLAUDE.md
        create_file(&project, ".claude/CLAUDE.md", "# Claude 规则");

        let discovery = ConfigDiscovery::new(project, home);
        let rules = discovery.discover_rules();

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].name, "CLAUDE.md");
        assert_eq!(rules[0].provider, ConfigProvider::Claude);
    }

    // -----------------------------------------------------------------------
    // 6. test_discover_skills
    // -----------------------------------------------------------------------
    #[test]
    fn test_discover_skills() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("project");
        let home = tmp.path().join("home");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&home).unwrap();

        // 创建技能包目录结构
        create_file(
            &project,
            ".chengcoding/skills/code-review/SKILL.md",
            "# 代码审查技能",
        );
        create_file(
            &project,
            ".chengcoding/skills/testing/SKILL.md",
            "# 测试技能",
        );

        let discovery = ConfigDiscovery::new(project, home);
        let skills = discovery.discover_skills();

        assert_eq!(skills.len(), 2);
        assert!(skills.iter().all(|s| s.item_type == DiscoveryType::Skill));

        let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"code-review"));
        assert!(names.contains(&"testing"));
    }

    // -----------------------------------------------------------------------
    // 7. test_discover_commands
    // -----------------------------------------------------------------------
    #[test]
    fn test_discover_commands() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("project");
        let home = tmp.path().join("home");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&home).unwrap();

        // 创建命令目录
        create_file(&project, ".chengcoding/commands/deploy.md", "# 部署命令");
        create_file(&project, ".chengcoding/commands/lint.md", "# 代码检查命令");

        let discovery = ConfigDiscovery::new(project, home);
        let commands = discovery.discover_commands();

        assert_eq!(commands.len(), 2);
        assert!(commands
            .iter()
            .all(|c| c.item_type == DiscoveryType::Command));

        let names: Vec<&str> = commands.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"deploy"));
        assert!(names.contains(&"lint"));
    }

    // -----------------------------------------------------------------------
    // 8. test_dedup_by_name
    // -----------------------------------------------------------------------
    #[test]
    fn test_dedup_by_name() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("project");
        let home = tmp.path().join("home");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&home).unwrap();

        // Native（优先级100）和 Claude（优先级80）都有 AGENTS.md
        create_file(&project, ".chengcoding/AGENTS.md", "# Native 规则");
        create_file(&project, ".claude/AGENTS.md", "# Claude 规则");

        let discovery = ConfigDiscovery::new(project, home);
        let rules = discovery.discover_rules();

        // 去重后只保留高优先级的 Native 版本
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].provider, ConfigProvider::Native);
    }

    // -----------------------------------------------------------------------
    // 9. test_disabled_provider_skipped
    // -----------------------------------------------------------------------
    #[test]
    fn test_disabled_provider_skipped() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("project");
        let home = tmp.path().join("home");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&home).unwrap();

        create_file(&project, ".chengcoding/AGENTS.md", "# Native");
        create_file(&project, ".claude/CLAUDE.md", "# Claude");

        let mut discovery = ConfigDiscovery::new(project, home);
        discovery.disable_provider(ConfigProvider::Native);

        let rules = discovery.discover_rules();

        // Native 被禁用，只发现 Claude 的规则
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].provider, ConfigProvider::Claude);
    }

    // -----------------------------------------------------------------------
    // 10. test_disabled_extension_skipped
    // -----------------------------------------------------------------------
    #[test]
    fn test_disabled_extension_skipped() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("project");
        let home = tmp.path().join("home");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&home).unwrap();

        create_file(&project, ".chengcoding/skills/review/SKILL.md", "# 审查");
        create_file(&project, ".chengcoding/skills/deploy/SKILL.md", "# 部署");

        let mut discovery = ConfigDiscovery::new(project, home);
        discovery.disable_extension("review");

        let skills = discovery.discover_skills();

        // review 被禁用（enabled=false），deploy 正常
        let review = skills.iter().find(|s| s.name == "review");
        let deploy = skills.iter().find(|s| s.name == "deploy");

        assert!(review.is_some());
        assert!(!review.unwrap().enabled);
        assert!(deploy.is_some());
        assert!(deploy.unwrap().enabled);
    }

    // -----------------------------------------------------------------------
    // 11. test_find_config_file_priority
    // -----------------------------------------------------------------------
    #[test]
    fn test_find_config_file_priority() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("project");
        let home = tmp.path().join("home");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&home).unwrap();

        // 在项目级 Native 和 Claude 中都创建 mcp.json
        create_file(&project, ".chengcoding/mcp.json", r#"{"native": true}"#);
        create_file(&project, ".claude/mcp.json", r#"{"claude": true}"#);

        let discovery = ConfigDiscovery::new(project, home);
        let result = discovery.find_config_file("mcp.json");

        assert!(result.is_some());
        let (provider, _path) = result.unwrap();
        // Native 优先级更高，应该先匹配
        assert_eq!(provider, ConfigProvider::Native);
    }

    // -----------------------------------------------------------------------
    // 12. test_discover_mcp_configs
    // -----------------------------------------------------------------------
    #[test]
    fn test_discover_mcp_configs() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("project");
        let home = tmp.path().join("home");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&home).unwrap();

        create_file(&project, ".chengcoding/mcp.json", r#"{"servers": []}"#);
        create_file(&home, ".chengcoding/mcp.json", r#"{"servers": ["global"]}"#);

        let discovery = ConfigDiscovery::new(project, home);
        let mcp = discovery.discover_mcp_configs();

        // 去重后只保留项目级（优先）
        assert_eq!(mcp.len(), 1);
        assert_eq!(mcp[0].item_type, DiscoveryType::McpServer);
        assert_eq!(mcp[0].name, "mcp.json");
        assert_eq!(mcp[0].provider, ConfigProvider::Native);
    }

    // -----------------------------------------------------------------------
    // 13. test_no_config_dirs
    // -----------------------------------------------------------------------
    #[test]
    fn test_no_config_dirs() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("project");
        let home = tmp.path().join("home");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&home).unwrap();

        // 不创建任何配置目录
        let discovery = ConfigDiscovery::new(project, home);

        assert!(discovery.get_config_dirs().is_empty());
        assert!(discovery.discover_rules().is_empty());
        assert!(discovery.discover_mcp_configs().is_empty());
        assert!(discovery.discover_skills().is_empty());
        assert!(discovery.discover_commands().is_empty());
        assert!(discovery.discover_all().is_empty());
        assert!(discovery.find_config_file("mcp.json").is_none());
    }

    // -----------------------------------------------------------------------
    // 14. test_discover_all_aggregation
    // -----------------------------------------------------------------------
    #[test]
    fn test_discover_all_aggregation() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("project");
        let home = tmp.path().join("home");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&home).unwrap();

        // 创建多种类型的配置
        create_file(&project, ".chengcoding/AGENTS.md", "# 规则");
        create_file(&project, ".chengcoding/mcp.json", "{}");
        create_file(
            &project,
            ".chengcoding/skills/test-skill/SKILL.md",
            "# 技能",
        );
        create_file(&project, ".chengcoding/commands/build.md", "# 构建");

        let discovery = ConfigDiscovery::new(project, home);
        let all = discovery.discover_all();

        // 应包含：1 规则 + 1 MCP + 1 技能 + 1 命令 = 4
        assert_eq!(all.len(), 4);

        // 验证各类型都存在
        assert!(all.iter().any(|i| i.item_type == DiscoveryType::Rule));
        assert!(all.iter().any(|i| i.item_type == DiscoveryType::McpServer));
        assert!(all.iter().any(|i| i.item_type == DiscoveryType::Skill));
        assert!(all.iter().any(|i| i.item_type == DiscoveryType::Command));
    }
}

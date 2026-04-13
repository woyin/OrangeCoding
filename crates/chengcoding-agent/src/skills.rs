//! # 技能系统
//!
//! 技能包为代理提供领域知识、上下文规则和工具绑定。
//! 支持从目录中自动发现 `SKILL.md` 格式的技能定义。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// 技能包定义
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillPack {
    /// 技能名称
    pub name: String,
    /// 技能描述
    pub description: String,
    /// 版本号
    pub version: Option<String>,
    /// 规则列表
    pub rules: Vec<String>,
    /// 上下文文件路径
    pub context_files: Vec<PathBuf>,
    /// 关联工具列表
    pub tools: Vec<String>,
    /// 技能来源
    pub source: SkillSource,
    /// 是否启用
    pub enabled: bool,
}

/// 技能来源
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillSource {
    /// 内置技能
    Builtin,
    /// 用户全局技能
    UserGlobal,
    /// 项目级技能
    Project,
}

/// 技能注册表
pub struct SkillRegistry {
    skills: Vec<SkillPack>,
}

impl SkillRegistry {
    /// 创建空的技能注册表
    pub fn new() -> Self {
        Self { skills: Vec::new() }
    }

    /// 注册技能包（同名则覆盖）
    pub fn register(&mut self, skill: SkillPack) {
        self.skills.retain(|s| s.name != skill.name);
        self.skills.push(skill);
    }

    /// 按名称获取技能包
    pub fn get(&self, name: &str) -> Option<&SkillPack> {
        self.skills.iter().find(|s| s.name == name)
    }

    /// 列出所有已启用的技能包
    pub fn list_enabled(&self) -> Vec<&SkillPack> {
        self.skills.iter().filter(|s| s.enabled).collect()
    }

    /// 启用指定技能，返回是否找到
    pub fn enable(&mut self, name: &str) -> bool {
        if let Some(s) = self.skills.iter_mut().find(|s| s.name == name) {
            s.enabled = true;
            true
        } else {
            false
        }
    }

    /// 禁用指定技能，返回是否找到
    pub fn disable(&mut self, name: &str) -> bool {
        if let Some(s) = self.skills.iter_mut().find(|s| s.name == name) {
            s.enabled = false;
            true
        } else {
            false
        }
    }

    /// 从目录发现技能包
    ///
    /// 目录结构: `<dir>/<name>/SKILL.md`
    ///
    /// `SKILL.md` 使用 YAML frontmatter 定义元数据，正文中以 `- ` 开头的行作为规则。
    pub fn discover_from_dir(
        &mut self,
        dir: &Path,
        source: SkillSource,
    ) -> Result<usize, std::io::Error> {
        let mut count = 0;
        if !dir.exists() {
            return Ok(0);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let skill_file = path.join("SKILL.md");
                if skill_file.exists() {
                    let content = std::fs::read_to_string(&skill_file)?;
                    if let Some(mut skill) = parse_skill_md(&content) {
                        skill.source = source.clone();
                        skill.context_files = vec![skill_file];
                        self.register(skill);
                        count += 1;
                    }
                }
            }
        }

        Ok(count)
    }

    /// 收集所有已启用技能的规则
    pub fn collect_rules(&self) -> Vec<String> {
        self.skills
            .iter()
            .filter(|s| s.enabled)
            .flat_map(|s| s.rules.clone())
            .collect()
    }

    /// 返回已注册技能数量
    pub fn count(&self) -> usize {
        self.skills.len()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 解析 SKILL.md 文件内容
///
/// 格式示例：
/// ```text
/// ---
/// name: rust-expert
/// description: Rust programming expertise
/// version: 1.0
/// tools: [cargo, rustfmt]
/// ---
///
/// # Rules
///
/// - Always use Result for error handling
/// ```
fn parse_skill_md(content: &str) -> Option<SkillPack> {
    let trimmed = content.trim();
    if !trimmed.starts_with("---") {
        return None;
    }

    // 分割 frontmatter 和正文
    let after_first = &trimmed[3..];
    let end_idx = after_first.find("---")?;
    let frontmatter = after_first[..end_idx].trim();
    let body = after_first[end_idx + 3..].trim();

    // 解析 frontmatter 字段
    let mut name = String::new();
    let mut description = String::new();
    let mut version = None;
    let mut tools = Vec::new();

    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("name:") {
            name = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("description:") {
            description = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("version:") {
            version = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("tools:") {
            let val = val.trim();
            // 解析 [tool1, tool2] 格式
            if val.starts_with('[') && val.ends_with(']') {
                let inner = &val[1..val.len() - 1];
                tools = inner
                    .split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
            }
        }
    }

    if name.is_empty() {
        return None;
    }

    // 从正文提取规则（以 `- ` 开头的行）
    let rules: Vec<String> = body
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- ").map(|r| r.to_string()))
        .collect();

    Some(SkillPack {
        name,
        description,
        version,
        rules,
        context_files: Vec::new(),
        tools,
        source: SkillSource::Builtin,
        enabled: true,
    })
}

/// 内置技能类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuiltinSkill {
    GitMaster,
    Playwright,
    PlaywrightCli,
    AgentBrowser,
    DevBrowser,
    FrontendUiUx,
}

impl BuiltinSkill {
    /// 返回技能的 kebab-case 名称
    pub fn name(&self) -> &str {
        match self {
            Self::GitMaster => "git-master",
            Self::Playwright => "playwright",
            Self::PlaywrightCli => "playwright-cli",
            Self::AgentBrowser => "agent-browser",
            Self::DevBrowser => "dev-browser",
            Self::FrontendUiUx => "frontend-ui-ux",
        }
    }

    /// 返回技能的中文描述
    pub fn description(&self) -> &str {
        match self {
            Self::GitMaster => "Git 版本控制高级操作技能",
            Self::Playwright => "Playwright 浏览器自动化测试技能",
            Self::PlaywrightCli => "Playwright 命令行工具技能",
            Self::AgentBrowser => "代理浏览器控制技能",
            Self::DevBrowser => "开发者浏览器调试技能",
            Self::FrontendUiUx => "前端 UI/UX 设计与开发技能",
        }
    }

    /// 返回所有内置技能变体
    pub fn all() -> Vec<BuiltinSkill> {
        vec![
            Self::GitMaster,
            Self::Playwright,
            Self::PlaywrightCli,
            Self::AgentBrowser,
            Self::DevBrowser,
            Self::FrontendUiUx,
        ]
    }
}

/// 技能 frontmatter 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFrontmatter {
    /// 技能名称
    pub name: String,
    /// 技能描述
    pub description: String,
    /// MCP 服务器配置映射
    pub mcp: HashMap<String, McpConfig>,
}

/// MCP 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// 启动命令
    pub command: String,
    /// 命令参数列表
    pub args: Vec<String>,
    /// 环境变量
    pub env: HashMap<String, String>,
}

/// 已加载的技能实例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedSkill {
    /// 技能名称
    pub name: String,
    /// 技能描述
    pub description: String,
    /// 技能指令内容
    pub instructions: String,
    /// MCP 服务器配置映射
    pub mcps: HashMap<String, McpConfig>,
    /// 技能源文件路径
    pub source_path: PathBuf,
}

/// 技能加载器
pub struct SkillLoader;

impl SkillLoader {
    /// 技能加载路径（按优先级排列）
    pub const SKILL_LOAD_PATHS: [&str; 5] = [
        ".chengcoding/skills",
        ".claude/skills",
        ".config/ceair/skills",
        "~/.chengcoding/skills",
        "~/.config/ceair/skills",
    ];

    /// 创建新的技能加载器
    pub fn new() -> Self {
        Self
    }

    /// 从指定路径列表加载技能
    ///
    /// 遍历每个路径，查找 `SKILL.md` 文件，解析 frontmatter 和正文，
    /// 返回已加载的技能列表。
    pub fn load_from_paths(paths: &[PathBuf]) -> Vec<LoadedSkill> {
        let mut loaded = Vec::new();

        for path in paths {
            if path.is_dir() {
                // 在目录中查找 SKILL.md
                let skill_file = path.join("SKILL.md");
                if skill_file.exists() {
                    if let Ok(content) = std::fs::read_to_string(&skill_file) {
                        if let Some(skill) = Self::parse_skill_file(&content, &skill_file) {
                            loaded.push(skill);
                        }
                    }
                }
                // 同时扫描子目录
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let sub_path = entry.path();
                        if sub_path.is_dir() {
                            let skill_file = sub_path.join("SKILL.md");
                            if skill_file.exists() {
                                if let Ok(content) = std::fs::read_to_string(&skill_file) {
                                    if let Some(skill) =
                                        Self::parse_skill_file(&content, &skill_file)
                                    {
                                        loaded.push(skill);
                                    }
                                }
                            }
                        }
                    }
                }
            } else if path.is_file() && path.file_name().map_or(false, |f| f == "SKILL.md") {
                // 直接指向 SKILL.md 文件
                if let Ok(content) = std::fs::read_to_string(path) {
                    if let Some(skill) = Self::parse_skill_file(&content, path) {
                        loaded.push(skill);
                    }
                }
            }
        }

        loaded
    }

    /// 解析单个 SKILL.md 文件，提取 frontmatter 和正文
    fn parse_skill_file(content: &str, source_path: &Path) -> Option<LoadedSkill> {
        let trimmed = content.trim();
        if !trimmed.starts_with("---") {
            return None;
        }

        // 分割 frontmatter 和正文
        let after_first = &trimmed[3..];
        let end_idx = after_first.find("---")?;
        let frontmatter = after_first[..end_idx].trim();
        let body = after_first[end_idx + 3..].trim();

        // 解析 frontmatter 字段
        let mut name = String::new();
        let mut description = String::new();
        let mcps: HashMap<String, McpConfig> = HashMap::new();

        for line in frontmatter.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("name:") {
                name = val.trim().to_string();
            } else if let Some(val) = line.strip_prefix("description:") {
                description = val.trim().to_string();
            }
        }

        if name.is_empty() {
            return None;
        }

        Some(LoadedSkill {
            name,
            description,
            instructions: body.to_string(),
            mcps,
            source_path: source_path.to_path_buf(),
        })
    }
}

/// 已禁用技能集合
pub struct DisabledSkills {
    /// 被禁用的技能名称集合
    disabled: HashSet<String>,
}

impl DisabledSkills {
    /// 创建空的已禁用技能集合
    pub fn new() -> Self {
        Self {
            disabled: HashSet::new(),
        }
    }

    /// 禁用指定技能
    pub fn disable(&mut self, name: impl Into<String>) {
        self.disabled.insert(name.into());
    }

    /// 启用指定技能（从禁用列表中移除），返回是否存在
    pub fn enable(&mut self, name: &str) -> bool {
        self.disabled.remove(name)
    }

    /// 检查技能是否被禁用
    pub fn is_disabled(&self, name: &str) -> bool {
        self.disabled.contains(name)
    }

    /// 返回已禁用技能数量
    pub fn count(&self) -> usize {
        self.disabled.len()
    }
}

// ===========================================================================
// 测试
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 辅助函数：构造技能包
    fn make_skill(name: &str, enabled: bool) -> SkillPack {
        SkillPack {
            name: name.to_string(),
            description: format!("{name} description"),
            version: Some("1.0".to_string()),
            rules: vec![format!("rule from {name}")],
            context_files: Vec::new(),
            tools: Vec::new(),
            source: SkillSource::Builtin,
            enabled,
        }
    }

    #[test]
    fn test_register_skill() {
        let mut reg = SkillRegistry::new();
        reg.register(make_skill("s1", true));
        assert_eq!(reg.count(), 1);
    }

    #[test]
    fn test_get_skill() {
        let mut reg = SkillRegistry::new();
        reg.register(make_skill("alpha", true));
        assert!(reg.get("alpha").is_some());
        assert!(reg.get("beta").is_none());
    }

    #[test]
    fn test_list_enabled() {
        let mut reg = SkillRegistry::new();
        reg.register(make_skill("on", true));
        reg.register(make_skill("off", false));

        let enabled = reg.list_enabled();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "on");
    }

    #[test]
    fn test_enable_disable() {
        let mut reg = SkillRegistry::new();
        reg.register(make_skill("s1", true));

        assert!(reg.disable("s1"));
        assert!(reg.list_enabled().is_empty());

        assert!(reg.enable("s1"));
        assert_eq!(reg.list_enabled().len(), 1);

        // 操作不存在的技能应返回 false
        assert!(!reg.enable("nonexistent"));
        assert!(!reg.disable("nonexistent"));
    }

    #[test]
    fn test_collect_rules() {
        let mut reg = SkillRegistry::new();
        reg.register(make_skill("s1", true));
        reg.register(make_skill("s2", false));
        reg.register(make_skill("s3", true));

        let rules = reg.collect_rules();
        assert_eq!(rules.len(), 2);
        assert!(rules.contains(&"rule from s1".to_string()));
        assert!(rules.contains(&"rule from s3".to_string()));
    }

    #[test]
    fn test_discover_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("rust-expert");
        fs::create_dir_all(&skill_dir).unwrap();

        let skill_md = "\
---
name: rust-expert
description: Rust programming expertise
version: 1.0
tools: [cargo, rustfmt]
---

# Rust Expert Rules

- Always use Result for error handling
- Prefer &str over String for function parameters
";
        fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

        let mut reg = SkillRegistry::new();
        let count = reg
            .discover_from_dir(dir.path(), SkillSource::Project)
            .unwrap();
        assert_eq!(count, 1);

        let skill = reg.get("rust-expert").unwrap();
        assert_eq!(skill.description, "Rust programming expertise");
        assert_eq!(skill.version, Some("1.0".to_string()));
        assert_eq!(skill.tools, vec!["cargo", "rustfmt"]);
        assert_eq!(skill.rules.len(), 2);
        assert_eq!(skill.source, SkillSource::Project);
    }

    #[test]
    fn test_skill_md_parsing() {
        let content = "\
---
name: test-skill
description: A test skill
version: 2.0
tools: [tool1]
---

# Rules

- Rule one
- Rule two
- Rule three
";
        let skill = parse_skill_md(content).unwrap();
        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.description, "A test skill");
        assert_eq!(skill.version, Some("2.0".to_string()));
        assert_eq!(skill.tools, vec!["tool1"]);
        assert_eq!(skill.rules.len(), 3);
    }

    #[test]
    fn test_duplicate_name_override() {
        let mut reg = SkillRegistry::new();
        reg.register(make_skill("dup", true));
        reg.register(SkillPack {
            name: "dup".to_string(),
            description: "new description".to_string(),
            version: Some("2.0".to_string()),
            rules: vec![],
            context_files: Vec::new(),
            tools: Vec::new(),
            source: SkillSource::Project,
            enabled: true,
        });

        assert_eq!(reg.count(), 1);
        assert_eq!(reg.get("dup").unwrap().description, "new description");
    }

    #[test]
    fn test_count() {
        let mut reg = SkillRegistry::new();
        assert_eq!(reg.count(), 0);
        reg.register(make_skill("a", true));
        assert_eq!(reg.count(), 1);
        reg.register(make_skill("b", true));
        assert_eq!(reg.count(), 2);
    }

    #[test]
    fn test_empty_registry() {
        let reg = SkillRegistry::new();
        assert_eq!(reg.count(), 0);
        assert!(reg.list_enabled().is_empty());
        assert!(reg.get("x").is_none());
        assert!(reg.collect_rules().is_empty());
    }

    // -----------------------------------------------------------------------
    // 新增类型测试
    // -----------------------------------------------------------------------

    #[test]
    fn test_builtin_skill_variants() {
        // 验证所有 6 个内置技能变体存在
        let _git = BuiltinSkill::GitMaster;
        let _pw = BuiltinSkill::Playwright;
        let _pwc = BuiltinSkill::PlaywrightCli;
        let _ab = BuiltinSkill::AgentBrowser;
        let _db = BuiltinSkill::DevBrowser;
        let _fe = BuiltinSkill::FrontendUiUx;
        assert_eq!(BuiltinSkill::all().len(), 6);
    }

    #[test]
    fn test_builtin_skill_name() {
        // 验证 name() 返回正确的 kebab-case 名称
        assert_eq!(BuiltinSkill::GitMaster.name(), "git-master");
        assert_eq!(BuiltinSkill::Playwright.name(), "playwright");
        assert_eq!(BuiltinSkill::PlaywrightCli.name(), "playwright-cli");
        assert_eq!(BuiltinSkill::AgentBrowser.name(), "agent-browser");
        assert_eq!(BuiltinSkill::DevBrowser.name(), "dev-browser");
        assert_eq!(BuiltinSkill::FrontendUiUx.name(), "frontend-ui-ux");
    }

    #[test]
    fn test_builtin_skill_description() {
        // 验证 description() 返回非空字符串
        for skill in BuiltinSkill::all() {
            let desc = skill.description();
            assert!(!desc.is_empty(), "技能 {:?} 的描述不应为空", skill);
        }
    }

    #[test]
    fn test_builtin_skill_all() {
        // 验证 all() 返回 6 个变体且无重复
        let all = BuiltinSkill::all();
        assert_eq!(all.len(), 6);

        let unique: std::collections::HashSet<_> = all.iter().collect();
        assert_eq!(unique.len(), 6, "all() 不应包含重复变体");
    }

    #[test]
    fn test_skill_frontmatter_creation() {
        // 创建 SkillFrontmatter 并验证字段
        let fm = SkillFrontmatter {
            name: "test-fm".to_string(),
            description: "测试 frontmatter".to_string(),
            mcp: HashMap::new(),
        };
        assert_eq!(fm.name, "test-fm");
        assert_eq!(fm.description, "测试 frontmatter");
        assert!(fm.mcp.is_empty());
    }

    #[test]
    fn test_mcp_config_creation() {
        // 创建 McpConfig 并验证字段
        let mut env = HashMap::new();
        env.insert("API_KEY".to_string(), "secret".to_string());

        let cfg = McpConfig {
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "server".to_string()],
            env,
        };
        assert_eq!(cfg.command, "npx");
        assert_eq!(cfg.args.len(), 2);
        assert_eq!(cfg.env.get("API_KEY").unwrap(), "secret");
    }

    #[test]
    fn test_loaded_skill_creation() {
        // 创建 LoadedSkill 并验证字段
        let skill = LoadedSkill {
            name: "my-skill".to_string(),
            description: "我的技能".to_string(),
            instructions: "按照以下步骤操作".to_string(),
            mcps: HashMap::new(),
            source_path: PathBuf::from("/skills/my-skill/SKILL.md"),
        };
        assert_eq!(skill.name, "my-skill");
        assert_eq!(skill.instructions, "按照以下步骤操作");
        assert!(skill.mcps.is_empty());
        assert_eq!(
            skill.source_path,
            PathBuf::from("/skills/my-skill/SKILL.md")
        );
    }

    #[test]
    fn test_skill_loader_empty_paths() {
        // 传入空路径切片应返回空列表
        let result = SkillLoader::load_from_paths(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_skill_loader_nonexistent_path() {
        // 不存在的路径应返回空列表
        let result = SkillLoader::load_from_paths(&[PathBuf::from("/nonexistent/path/to/skills")]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_skill_loader_load_from_dir() {
        // 在临时目录中创建 SKILL.md 并验证加载结果
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("test-skill");
        fs::create_dir_all(&skill_dir).unwrap();

        let skill_md = "\
---
name: test-skill
description: 测试技能
---

这是技能说明。
";
        fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

        let result = SkillLoader::load_from_paths(&[dir.path().to_path_buf()]);
        assert_eq!(result.len(), 1);

        let skill = &result[0];
        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.description, "测试技能");
        assert_eq!(skill.instructions, "这是技能说明。");
        // 没有 mcp 配置时应为空 HashMap
        assert!(skill.mcps.is_empty());
    }

    #[test]
    fn test_disabled_skills_new() {
        // 新建的已禁用集合应为空
        let ds = DisabledSkills::new();
        assert_eq!(ds.count(), 0);
        assert!(!ds.is_disabled("any"));
    }

    #[test]
    fn test_disabled_skills_operations() {
        // 验证禁用、查询、启用和计数操作
        let mut ds = DisabledSkills::new();

        ds.disable("skill-a");
        ds.disable("skill-b");
        assert_eq!(ds.count(), 2);
        assert!(ds.is_disabled("skill-a"));
        assert!(ds.is_disabled("skill-b"));
        assert!(!ds.is_disabled("skill-c"));

        // 启用已禁用的技能
        assert!(ds.enable("skill-a"));
        assert_eq!(ds.count(), 1);
        assert!(!ds.is_disabled("skill-a"));

        // 启用不存在的技能应返回 false
        assert!(!ds.enable("nonexistent"));
        assert_eq!(ds.count(), 1);
    }
}

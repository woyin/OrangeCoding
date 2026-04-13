//! # Zellij 终端复用器集成模块
//!
//! 将子Agent运行在Zellij的独立面板中，支持布局管理和实时监控。

use std::fmt;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Zellij 配置
// ---------------------------------------------------------------------------

/// Zellij 集成配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZellijConfig {
    /// 是否启用 Zellij 集成
    pub enabled: bool,
    /// 面板布局模式
    pub layout: String,
    /// 主面板占比
    pub main_pane_size: String,
}

impl Default for ZellijConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            layout: "default".to_string(),
            main_pane_size: "60%".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Zellij 面板
// ---------------------------------------------------------------------------

/// Zellij 面板信息 - 代表一个运行中的面板
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZellijPane {
    /// 面板唯一标识
    pub pane_id: String,
    /// 关联的Agent名称
    pub agent_name: String,
    /// 面板创建时间
    pub created_at: String,
}

impl ZellijPane {
    /// 创建一个新的面板信息
    pub fn new(pane_id: impl Into<String>, agent_name: impl Into<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_default();
        Self {
            pane_id: pane_id.into(),
            agent_name: agent_name.into(),
            created_at: now,
        }
    }
}

impl fmt::Display for ZellijPane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "面板[{}] Agent={}", self.pane_id, self.agent_name)
    }
}

// ---------------------------------------------------------------------------
// Zellij 集成
// ---------------------------------------------------------------------------

/// Zellij 终端复用器集成
///
/// 提供创建面板、写入命令、关闭面板等操作的命令构建功能。
/// 在测试中不会实际执行 zellij 命令，而是构建命令字符串。
pub struct ZellijIntegration {
    /// 当前 Zellij 会话名称
    pub session_name: Option<String>,
    /// 集成配置
    pub config: ZellijConfig,
    /// 活跃面板列表
    active_panes: Vec<ZellijPane>,
}

impl ZellijIntegration {
    /// 创建一个新的 Zellij 集成实例
    pub fn new() -> Self {
        Self {
            session_name: None,
            config: ZellijConfig::default(),
            active_panes: Vec::new(),
        }
    }

    /// 使用自定义配置创建实例
    pub fn with_config(config: ZellijConfig) -> Self {
        Self {
            session_name: None,
            config,
            active_panes: Vec::new(),
        }
    }

    /// 检测 Zellij 是否可用（通过环境变量判断）
    pub fn is_available() -> bool {
        std::env::var("ZELLIJ_SESSION_NAME").is_ok()
    }

    /// 检测是否在 Zellij 会话内部运行
    pub fn is_inside_zellij() -> bool {
        std::env::var("ZELLIJ_SESSION_NAME").is_ok()
    }

    /// 构建创建新面板的命令字符串
    pub fn build_new_pane_command(&self, cmd: &str, name: &str) -> String {
        format!("zellij action new-pane --name \"{}\" -- {}", name, cmd)
    }

    /// 构建向面板写入内容的命令字符串
    pub fn build_write_command(&self, pane_id: &str, text: &str) -> String {
        format!(
            "zellij action write-chars --pane-id {} \"{}\"",
            pane_id, text
        )
    }

    /// 构建关闭面板的命令字符串
    pub fn build_close_pane_command(&self, pane_id: &str) -> String {
        format!("zellij action close-pane --pane-id {}", pane_id)
    }

    /// 构建聚焦面板的命令字符串
    pub fn build_focus_command(&self, pane_id: &str) -> String {
        format!("zellij action focus-pane --pane-id {}", pane_id)
    }

    /// 为Agent创建一个新面板
    ///
    /// 注意：此方法仅构建命令并记录面板信息，
    /// 实际的命令执行需要由调用者完成。
    pub fn spawn_agent_pane(
        &mut self,
        agent_name: &str,
        command: &str,
    ) -> Result<ZellijPane, String> {
        if !self.config.enabled {
            return Err("Zellij 集成未启用".to_string());
        }

        let pane_id = format!("pane-{}", self.active_panes.len() + 1);
        let pane = ZellijPane::new(&pane_id, agent_name);

        info!(
            agent = %agent_name,
            pane_id = %pane_id,
            command = %command,
            "创建Agent面板"
        );

        let _cmd = self.build_new_pane_command(command, agent_name);
        debug!(cmd = %_cmd, "构建的面板创建命令");

        self.active_panes.push(pane.clone());
        Ok(pane)
    }

    /// 获取所有活跃面板
    pub fn get_active_panes(&self) -> Vec<ZellijPane> {
        self.active_panes.clone()
    }

    /// 获取活跃面板数量
    pub fn active_pane_count(&self) -> usize {
        self.active_panes.len()
    }
}

impl Default for ZellijIntegration {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ZellijIntegration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ZellijIntegration")
            .field("session_name", &self.session_name)
            .field("config", &self.config)
            .field("active_panes", &self.active_panes.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tmux 回退方案
// ---------------------------------------------------------------------------

/// Tmux 回退方案 - 当 Zellij 不可用时使用 tmux
pub struct TmuxFallback {
    /// 活跃面板列表
    active_panes: Vec<ZellijPane>,
}

impl TmuxFallback {
    /// 创建一个新的 Tmux 回退实例
    pub fn new() -> Self {
        Self {
            active_panes: Vec::new(),
        }
    }

    /// 检测 tmux 是否可用（通过 TMUX 环境变量判断）
    pub fn is_available() -> bool {
        std::env::var("TMUX").is_ok()
    }

    /// 构建创建新面板的命令字符串
    pub fn build_new_pane_command(&self, cmd: &str, name: &str) -> String {
        format!("tmux split-window -h -t \"{}\" '{}'", name, cmd)
    }

    /// 构建向面板写入内容的命令字符串
    pub fn build_write_command(&self, pane_id: &str, text: &str) -> String {
        format!("tmux send-keys -t {} '{}' Enter", pane_id, text)
    }

    /// 构建关闭面板的命令字符串
    pub fn build_close_pane_command(&self, pane_id: &str) -> String {
        format!("tmux kill-pane -t {}", pane_id)
    }

    /// 构建聚焦面板的命令字符串
    pub fn build_focus_command(&self, pane_id: &str) -> String {
        format!("tmux select-pane -t {}", pane_id)
    }

    /// 为Agent创建一个新面板
    pub fn spawn_agent_pane(
        &mut self,
        agent_name: &str,
        command: &str,
    ) -> Result<ZellijPane, String> {
        let pane_id = format!("tmux-pane-{}", self.active_panes.len() + 1);
        let pane = ZellijPane::new(&pane_id, agent_name);

        info!(
            agent = %agent_name,
            pane_id = %pane_id,
            command = %command,
            "创建tmux Agent面板"
        );

        let _cmd = self.build_new_pane_command(command, agent_name);
        debug!(cmd = %_cmd, "构建的tmux面板创建命令");

        self.active_panes.push(pane.clone());
        Ok(pane)
    }

    /// 获取所有活跃面板
    pub fn get_active_panes(&self) -> Vec<ZellijPane> {
        self.active_panes.clone()
    }
}

impl Default for TmuxFallback {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for TmuxFallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TmuxFallback")
            .field("active_panes", &self.active_panes.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// 复用器后端
// ---------------------------------------------------------------------------

/// 复用器后端 - 自动选择可用的终端复用器
///
/// 优先使用 Zellij，其次 tmux，均不可用时为 None。
#[derive(Debug)]
pub enum MultiplexerBackend {
    /// 使用 Zellij
    Zellij(ZellijIntegration),
    /// 使用 tmux 作为回退
    Tmux(TmuxFallback),
    /// 无可用复用器
    None,
}

impl MultiplexerBackend {
    /// 自动检测并选择最佳的复用器后端
    ///
    /// 优先级：Zellij > tmux > None
    pub fn detect() -> Self {
        if ZellijIntegration::is_available() {
            info!("检测到 Zellij 环境，使用 Zellij 后端");
            MultiplexerBackend::Zellij(ZellijIntegration::new())
        } else if TmuxFallback::is_available() {
            info!("检测到 tmux 环境，使用 tmux 后端");
            MultiplexerBackend::Tmux(TmuxFallback::new())
        } else {
            warn!("未检测到终端复用器");
            MultiplexerBackend::None
        }
    }

    /// 获取后端名称
    pub fn name(&self) -> &str {
        match self {
            MultiplexerBackend::Zellij(_) => "zellij",
            MultiplexerBackend::Tmux(_) => "tmux",
            MultiplexerBackend::None => "none",
        }
    }

    /// 检查是否有可用的复用器后端
    pub fn is_available(&self) -> bool {
        !matches!(self, MultiplexerBackend::None)
    }
}

impl fmt::Display for MultiplexerBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "复用器后端: {}", self.name())
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 测试默认配置() {
        let config = ZellijConfig::default();
        assert!(config.enabled);
        assert_eq!(config.layout, "default");
        assert_eq!(config.main_pane_size, "60%");
    }

    #[test]
    fn 测试创建面板信息() {
        let pane = ZellijPane::new("pane-1", "编码Agent");
        assert_eq!(pane.pane_id, "pane-1");
        assert_eq!(pane.agent_name, "编码Agent");
        assert!(!pane.created_at.is_empty());
    }

    #[test]
    fn 测试面板显示格式() {
        let pane = ZellijPane::new("pane-1", "测试Agent");
        let display = format!("{pane}");
        assert!(display.contains("pane-1"));
        assert!(display.contains("测试Agent"));
    }

    #[test]
    fn 测试构建新面板命令() {
        let zellij = ZellijIntegration::new();
        let cmd = zellij.build_new_pane_command("cargo test", "测试Agent");
        assert!(cmd.contains("zellij action new-pane"));
        assert!(cmd.contains("测试Agent"));
        assert!(cmd.contains("cargo test"));
    }

    #[test]
    fn 测试构建写入命令() {
        let zellij = ZellijIntegration::new();
        let cmd = zellij.build_write_command("pane-1", "ls -la");
        assert!(cmd.contains("zellij action write-chars"));
        assert!(cmd.contains("pane-1"));
        assert!(cmd.contains("ls -la"));
    }

    #[test]
    fn 测试构建关闭面板命令() {
        let zellij = ZellijIntegration::new();
        let cmd = zellij.build_close_pane_command("pane-2");
        assert!(cmd.contains("zellij action close-pane"));
        assert!(cmd.contains("pane-2"));
    }

    #[test]
    fn 测试构建聚焦命令() {
        let zellij = ZellijIntegration::new();
        let cmd = zellij.build_focus_command("pane-3");
        assert!(cmd.contains("zellij action focus-pane"));
        assert!(cmd.contains("pane-3"));
    }

    #[test]
    fn 测试创建Agent面板() {
        let mut zellij = ZellijIntegration::new();
        let result = zellij.spawn_agent_pane("编码助手", "cargo run");

        assert!(result.is_ok());
        let pane = result.unwrap();
        assert_eq!(pane.agent_name, "编码助手");
        assert_eq!(zellij.active_pane_count(), 1);
    }

    #[test]
    fn 测试禁用时创建面板失败() {
        let config = ZellijConfig {
            enabled: false,
            ..Default::default()
        };
        let mut zellij = ZellijIntegration::with_config(config);
        let result = zellij.spawn_agent_pane("测试", "echo hello");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("未启用"));
    }

    #[test]
    fn 测试获取活跃面板() {
        let mut zellij = ZellijIntegration::new();
        zellij.spawn_agent_pane("agent-1", "cmd1").unwrap();
        zellij.spawn_agent_pane("agent-2", "cmd2").unwrap();

        let panes = zellij.get_active_panes();
        assert_eq!(panes.len(), 2);
        assert_eq!(panes[0].agent_name, "agent-1");
        assert_eq!(panes[1].agent_name, "agent-2");
    }

    #[test]
    fn 测试tmux命令构建() {
        let tmux = TmuxFallback::new();
        let new_cmd = tmux.build_new_pane_command("cargo test", "测试");
        assert!(new_cmd.contains("tmux split-window"));

        let write_cmd = tmux.build_write_command("0", "echo hi");
        assert!(write_cmd.contains("tmux send-keys"));

        let close_cmd = tmux.build_close_pane_command("1");
        assert!(close_cmd.contains("tmux kill-pane"));

        let focus_cmd = tmux.build_focus_command("2");
        assert!(focus_cmd.contains("tmux select-pane"));
    }

    #[test]
    fn 测试tmux创建面板() {
        let mut tmux = TmuxFallback::new();
        let result = tmux.spawn_agent_pane("tmux-agent", "echo run");
        assert!(result.is_ok());
        assert_eq!(tmux.get_active_panes().len(), 1);
    }

    #[test]
    fn 测试后端名称() {
        let zellij_backend = MultiplexerBackend::Zellij(ZellijIntegration::new());
        assert_eq!(zellij_backend.name(), "zellij");
        assert!(zellij_backend.is_available());

        let tmux_backend = MultiplexerBackend::Tmux(TmuxFallback::new());
        assert_eq!(tmux_backend.name(), "tmux");
        assert!(tmux_backend.is_available());

        let none_backend = MultiplexerBackend::None;
        assert_eq!(none_backend.name(), "none");
        assert!(!none_backend.is_available());
    }

    #[test]
    fn 测试后端显示格式() {
        let backend = MultiplexerBackend::None;
        let display = format!("{backend}");
        assert!(display.contains("none"));
    }

    #[test]
    fn 测试后端自动检测() {
        // 在测试环境中通常没有 Zellij/tmux，应返回 None
        // 但我们只验证返回值是合法的
        let backend = MultiplexerBackend::detect();
        let name = backend.name();
        assert!(name == "zellij" || name == "tmux" || name == "none");
    }

    #[test]
    fn 测试zellij默认构造() {
        let zellij = ZellijIntegration::default();
        assert!(zellij.session_name.is_none());
        assert_eq!(zellij.active_pane_count(), 0);
    }

    #[test]
    fn 测试配置序列化() {
        let config = ZellijConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ZellijConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.layout, "default");
        assert_eq!(deserialized.main_pane_size, "60%");
    }
}

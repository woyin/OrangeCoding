//! # 安全策略与路径验证
//!
//! 提供文件操作的安全防护机制：
//! - `SecurityPolicy` - 安全策略配置
//! - `PathValidator` - 路径安全性验证（阻止访问敏感系统路径）
//! - `FileOperationGuard` - 安全包装器（在工具执行前进行路径检查）

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, warn};

// ============================================================
// 安全策略
// ============================================================

/// 安全策略配置
///
/// 定义文件操作的安全边界，包括允许访问的目录列表和阻止列表。
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    /// 允许访问的目录列表（如果为空则不限制）
    pub allowed_dirs: Vec<PathBuf>,

    /// 显式阻止访问的路径列表
    pub blocked_paths: Vec<PathBuf>,

    /// 是否允许路径遍历（包含 .. 的路径）
    pub allow_path_traversal: bool,
}

impl SecurityPolicy {
    /// 创建默认安全策略
    ///
    /// 默认配置：
    /// - 不限制允许访问的目录
    /// - 阻止常见的敏感系统路径
    /// - 禁止路径遍历
    pub fn default_policy() -> Self {
        Self {
            allowed_dirs: Vec::new(),
            blocked_paths: Self::default_blocked_paths(),
            allow_path_traversal: false,
        }
    }

    /// 创建指定工作目录的安全策略
    ///
    /// 仅允许在指定目录及其子目录内操作。
    ///
    /// # 参数
    /// - `work_dir`: 工作目录路径
    pub fn with_work_dir(work_dir: PathBuf) -> Self {
        Self {
            allowed_dirs: vec![work_dir],
            blocked_paths: Self::default_blocked_paths(),
            allow_path_traversal: false,
        }
    }

    /// 获取默认的阻止路径列表
    ///
    /// 包含常见的敏感系统目录和用户配置目录
    fn default_blocked_paths() -> Vec<PathBuf> {
        vec![
            // Linux/macOS 系统关键目录
            PathBuf::from("/etc"),
            PathBuf::from("/sys"),
            PathBuf::from("/proc"),
            PathBuf::from("/boot"),
            PathBuf::from("/dev"),
            // 用户敏感配置目录
            PathBuf::from(shellexpand_home("~/.ssh")),
            PathBuf::from(shellexpand_home("~/.gnupg")),
            PathBuf::from(shellexpand_home("~/.gpg")),
            PathBuf::from(shellexpand_home("~/.aws")),
            PathBuf::from(shellexpand_home("~/.config/gcloud")),
            PathBuf::from(shellexpand_home("~/.kube")),
            PathBuf::from(shellexpand_home("~/.docker")),
            // 密钥和凭证相关目录
            PathBuf::from(shellexpand_home("~/.credentials")),
            PathBuf::from(shellexpand_home("~/.secrets")),
        ]
    }
}

/// 展开 ~ 为用户主目录
///
/// 简易实现：将路径中的 ~ 前缀替换为 HOME 环境变量的值
fn shellexpand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}/{}", home, rest);
        }
    }
    path.to_string()
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self::default_policy()
    }
}

// ============================================================
// 路径验证器
// ============================================================

/// 路径安全性验证器
///
/// 基于 `SecurityPolicy` 对文件路径进行安全检查，包括：
/// - 路径遍历攻击检测
/// - 敏感路径访问阻止
/// - 允许目录范围限制
#[derive(Debug, Clone)]
pub struct PathValidator {
    /// 关联的安全策略
    policy: SecurityPolicy,
}

impl PathValidator {
    /// 创建新的路径验证器
    ///
    /// # 参数
    /// - `policy`: 安全策略配置
    pub fn new(policy: SecurityPolicy) -> Self {
        Self { policy }
    }

    /// 使用默认安全策略创建验证器
    pub fn with_defaults() -> Self {
        Self::new(SecurityPolicy::default_policy())
    }

    /// 综合检查路径是否安全
    ///
    /// 依次执行以下检查：
    /// 1. 路径遍历攻击检测
    /// 2. 敏感路径阻止
    /// 3. 允许目录范围检查
    ///
    /// # 参数
    /// - `path`: 要检查的文件路径字符串
    ///
    /// # 返回值
    /// 路径安全则返回 `true`，不安全返回 `false`
    pub fn is_path_safe(&self, path: &str) -> bool {
        // 第一步：检测路径遍历攻击
        if !self.policy.allow_path_traversal && self.has_path_traversal(path) {
            warn!("检测到路径遍历攻击: {}", path);
            return false;
        }

        // 第二步：检查是否为阻止路径
        if self.is_blocked_path(path) {
            warn!("路径被安全策略阻止: {}", path);
            return false;
        }

        // 第三步：如果配置了允许目录，检查路径是否在范围内
        if !self.policy.allowed_dirs.is_empty() && !self.is_within_allowed_dirs(path) {
            warn!("路径不在允许的目录范围内: {}", path);
            return false;
        }

        true
    }

    /// 检查路径是否在允许的目录列表内
    ///
    /// 通过规范化路径后检查是否以某个允许目录为前缀。
    ///
    /// # 参数
    /// - `path`: 要检查的文件路径字符串
    ///
    /// # 返回值
    /// 路径在允许范围内返回 `true`
    pub fn is_within_allowed_dirs(&self, path: &str) -> bool {
        // 如果没有配置允许目录，视为无限制
        if self.policy.allowed_dirs.is_empty() {
            return true;
        }

        let check_path = Path::new(path);

        // 尝试规范化路径（解析符号链接和 . / ..）
        let canonical = match check_path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                // 文件可能不存在，使用原始路径进行检查
                check_path.to_path_buf()
            }
        };

        // 检查路径是否以任一允许目录为前缀
        for allowed_dir in &self.policy.allowed_dirs {
            let allowed_canonical = match allowed_dir.canonicalize() {
                Ok(p) => p,
                Err(_) => allowed_dir.clone(),
            };

            if canonical.starts_with(&allowed_canonical) {
                debug!("路径 {} 在允许目录 {} 范围内", path, allowed_dir.display());
                return true;
            }
        }

        false
    }

    /// 检查路径是否匹配任一阻止规则
    ///
    /// 通过检查路径是否以阻止路径为前缀来判断。
    ///
    /// # 参数
    /// - `path`: 要检查的文件路径字符串
    ///
    /// # 返回值
    /// 路径被阻止则返回 `true`
    pub fn is_blocked_path(&self, path: &str) -> bool {
        let check_path = Path::new(path);

        // 尝试规范化路径
        let canonical = match check_path.canonicalize() {
            Ok(p) => p,
            Err(_) => check_path.to_path_buf(),
        };

        for blocked in &self.policy.blocked_paths {
            // 检查原始路径或规范化路径是否匹配阻止规则
            if canonical.starts_with(blocked) || check_path.starts_with(blocked) {
                return true;
            }
        }

        false
    }

    /// 检测路径中是否包含路径遍历模式
    ///
    /// 检测以下危险模式：
    /// - `../` 或 `..\\`（相对路径向上遍历）
    /// - 路径组件中的纯 `..`
    ///
    /// # 参数
    /// - `path`: 要检查的路径字符串
    ///
    /// # 返回值
    /// 包含路径遍历模式返回 `true`
    fn has_path_traversal(&self, path: &str) -> bool {
        // 检查路径字符串中是否包含 ../ 或 ..\ 模式
        if path.contains("../") || path.contains("..\\") {
            return true;
        }

        // 检查路径组件中是否有纯粹的 .. 组件
        let path_obj = Path::new(path);
        for component in path_obj.components() {
            if let std::path::Component::ParentDir = component {
                return true;
            }
        }

        false
    }

    /// 验证路径并返回详细的错误信息
    ///
    /// 与 `is_path_safe` 类似，但在验证失败时返回具体的错误原因。
    ///
    /// # 参数
    /// - `path`: 要验证的路径字符串
    ///
    /// # 返回值
    /// 验证通过返回 `Ok(())`，失败返回包含原因的 `ToolError`
    pub fn validate_path(&self, path: &str) -> ToolResult<()> {
        // 检测路径遍历攻击
        if !self.policy.allow_path_traversal && self.has_path_traversal(path) {
            return Err(ToolError::SecurityViolation(format!(
                "检测到路径遍历攻击，路径包含 '..' 组件: {}",
                path
            )));
        }

        // 检查阻止路径
        if self.is_blocked_path(path) {
            return Err(ToolError::SecurityViolation(format!(
                "访问被拒绝，路径属于敏感系统目录: {}",
                path
            )));
        }

        // 检查允许目录范围
        if !self.policy.allowed_dirs.is_empty() && !self.is_within_allowed_dirs(path) {
            return Err(ToolError::SecurityViolation(format!(
                "路径超出允许的工作目录范围: {}",
                path
            )));
        }

        Ok(())
    }
}

// ============================================================
// 文件操作安全守卫
// ============================================================

/// 文件操作安全守卫
///
/// 包装一个内部工具，在执行前对路径参数进行安全检查。
/// 如果参数中包含 `path` 字段，会先通过 `PathValidator` 验证，
/// 不通过则拒绝执行并返回安全错误。
pub struct FileOperationGuard {
    /// 被包装的内部工具
    inner: Arc<dyn Tool>,

    /// 路径验证器
    validator: Arc<PathValidator>,
}

impl fmt::Debug for FileOperationGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileOperationGuard")
            .field("inner", &self.inner.name())
            .field("validator", &self.validator)
            .finish()
    }
}

impl FileOperationGuard {
    /// 创建新的文件操作安全守卫
    ///
    /// # 参数
    /// - `inner`: 要包装的内部工具
    /// - `validator`: 路径验证器
    pub fn new(inner: Arc<dyn Tool>, validator: Arc<PathValidator>) -> Self {
        Self { inner, validator }
    }

    /// 使用默认安全策略包装工具
    ///
    /// # 参数
    /// - `inner`: 要包装的内部工具
    pub fn with_defaults(inner: Arc<dyn Tool>) -> Self {
        Self {
            inner,
            validator: Arc::new(PathValidator::with_defaults()),
        }
    }

    /// 从 JSON 参数中提取所有路径字段并进行安全验证
    ///
    /// 检查 `path`、`source`、`destination` 等常见路径字段。
    ///
    /// # 参数
    /// - `params`: 工具的 JSON 参数
    ///
    /// # 返回值
    /// 所有路径验证通过返回 `Ok(())`，否则返回安全错误
    fn validate_params(&self, params: &Value) -> ToolResult<()> {
        // 需要检查的路径字段名列表
        let path_fields = ["path", "source", "destination", "target", "file"];

        for field in &path_fields {
            if let Some(path_value) = params.get(*field).and_then(|v| v.as_str()) {
                self.validator.validate_path(path_value)?;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Tool for FileOperationGuard {
    /// 返回被包装工具的名称
    fn name(&self) -> &str {
        self.inner.name()
    }

    /// 返回被包装工具的描述
    fn description(&self) -> &str {
        self.inner.description()
    }

    /// 返回被包装工具的参数 Schema
    fn parameters_schema(&self) -> Value {
        self.inner.parameters_schema()
    }

    /// 安全执行工具操作
    ///
    /// 先进行路径安全检查，通过后再委托给内部工具执行。
    async fn execute(&self, params: Value) -> ToolResult<String> {
        // 在执行前进行安全检查
        self.validate_params(&params)?;

        debug!("安全检查通过，执行工具: {}", self.inner.name());

        // 委托给内部工具执行
        self.inner.execute(params).await
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReadFileTool;
    use serde_json::json;
    use std::sync::Arc;

    /// 测试默认安全策略的初始化
    #[test]
    fn test_default_security_policy() {
        let policy = SecurityPolicy::default_policy();

        // 默认不限制工作目录
        assert!(policy.allowed_dirs.is_empty());

        // 默认有阻止路径
        assert!(!policy.blocked_paths.is_empty());

        // 默认禁止路径遍历
        assert!(!policy.allow_path_traversal);
    }

    /// 测试指定工作目录的安全策略
    #[test]
    fn test_security_policy_with_work_dir() {
        let policy = SecurityPolicy::with_work_dir(PathBuf::from("/home/user/project"));

        // 应该有一个允许目录
        assert_eq!(policy.allowed_dirs.len(), 1);
        assert_eq!(policy.allowed_dirs[0], PathBuf::from("/home/user/project"));
    }

    /// 测试路径遍历检测
    #[test]
    fn test_path_traversal_detection() {
        let validator = PathValidator::with_defaults();

        // 包含 ../ 的路径应被检测为不安全
        assert!(!validator.is_path_safe("../etc/passwd"));
        assert!(!validator.is_path_safe("/home/user/../etc/passwd"));
        assert!(!validator.is_path_safe("foo/../../bar"));

        // 正常路径应该是安全的（除非命中阻止列表）
        assert!(validator.is_path_safe("/home/user/project/file.txt"));
        assert!(validator.is_path_safe("src/main.rs"));
        assert!(validator.is_path_safe("./local/file.txt"));
    }

    /// 测试阻止路径检查
    #[test]
    fn test_blocked_paths() {
        let validator = PathValidator::with_defaults();

        // 系统敏感路径应被阻止
        assert!(validator.is_blocked_path("/etc/passwd"));
        assert!(validator.is_blocked_path("/etc/shadow"));
        assert!(validator.is_blocked_path("/proc/1/status"));
        assert!(validator.is_blocked_path("/sys/class/net"));

        // 用户敏感目录应被阻止
        if let Ok(home) = std::env::var("HOME") {
            assert!(validator.is_blocked_path(&format!("{}/.ssh/id_rsa", home)));
            assert!(validator.is_blocked_path(&format!("{}/.gnupg/private-keys", home)));
            assert!(validator.is_blocked_path(&format!("{}/.aws/credentials", home)));
        }

        // 普通路径不应被阻止
        assert!(!validator.is_blocked_path("/home/user/project/file.txt"));
        assert!(!validator.is_blocked_path("src/main.rs"));
    }

    /// 测试允许目录范围检查
    #[test]
    fn test_allowed_dirs_check() {
        let policy = SecurityPolicy {
            allowed_dirs: vec![PathBuf::from("/home/user/project")],
            blocked_paths: vec![],
            allow_path_traversal: false,
        };
        let validator = PathValidator::new(policy);

        // 在允许目录内的路径应通过
        assert!(validator.is_within_allowed_dirs("/home/user/project/src/main.rs"));
        assert!(validator.is_within_allowed_dirs("/home/user/project"));

        // 不在允许目录内的路径应被拒绝
        assert!(!validator.is_within_allowed_dirs("/home/user/other/file.txt"));
        assert!(!validator.is_within_allowed_dirs("/tmp/file.txt"));
    }

    /// 测试无限制目录的场景
    #[test]
    fn test_no_allowed_dirs_restriction() {
        let policy = SecurityPolicy {
            allowed_dirs: vec![], // 不限制目录
            blocked_paths: vec![],
            allow_path_traversal: true,
        };
        let validator = PathValidator::new(policy);

        // 所有路径都应被允许（因为没有限制条件）
        assert!(validator.is_path_safe("/any/path/file.txt"));
        assert!(validator.is_path_safe("relative/path.rs"));
    }

    /// 测试 validate_path 返回详细错误信息
    #[test]
    fn test_validate_path_error_messages() {
        let validator = PathValidator::with_defaults();

        // 路径遍历应返回明确的错误信息
        let result = validator.validate_path("../etc/passwd");
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("路径遍历"));

        // 阻止路径应返回明确的错误信息
        let result = validator.validate_path("/etc/passwd");
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("敏感系统目录"));
    }

    /// 测试 FileOperationGuard 阻止不安全路径
    #[tokio::test]
    async fn test_file_operation_guard_blocks_unsafe_path() {
        // 使用 ReadFileTool 作为内部工具
        let inner: Arc<dyn Tool> = Arc::new(ReadFileTool);
        let guard = FileOperationGuard::with_defaults(inner);

        // 尝试读取敏感路径应被阻止
        let result = guard.execute(json!({"path": "/etc/passwd"})).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::SecurityViolation(msg) => {
                assert!(msg.contains("敏感系统目录"));
            }
            other => panic!("期望 SecurityViolation 错误，得到: {:?}", other),
        }
    }

    /// 测试 FileOperationGuard 阻止路径遍历
    #[tokio::test]
    async fn test_file_operation_guard_blocks_traversal() {
        let inner: Arc<dyn Tool> = Arc::new(ReadFileTool);
        let guard = FileOperationGuard::with_defaults(inner);

        // 尝试路径遍历应被阻止
        let result = guard.execute(json!({"path": "../../etc/shadow"})).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::SecurityViolation(msg) => {
                assert!(msg.contains("路径遍历"));
            }
            other => panic!("期望 SecurityViolation 错误，得到: {:?}", other),
        }
    }

    /// 测试 FileOperationGuard 允许安全路径执行
    #[tokio::test]
    async fn test_file_operation_guard_allows_safe_path() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("safe_file.txt");
        tokio::fs::write(&file_path, "安全内容").await.unwrap();

        let inner: Arc<dyn Tool> = Arc::new(ReadFileTool);
        let guard = FileOperationGuard::with_defaults(inner);

        // 安全路径应正常执行
        let path_str = file_path.to_string_lossy().to_string();
        let result = guard.execute(json!({"path": path_str})).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "安全内容");
    }

    /// 测试 FileOperationGuard 的 Debug 和元数据方法
    #[test]
    fn test_file_operation_guard_metadata() {
        let inner: Arc<dyn Tool> = Arc::new(ReadFileTool);
        let guard = FileOperationGuard::with_defaults(inner);

        // 守卫应透传内部工具的元数据
        assert_eq!(guard.name(), "read_file");
        assert!(!guard.description().is_empty());
        assert!(guard.parameters_schema().is_object());
    }

    /// 测试 FileOperationGuard 检查多个路径字段
    #[tokio::test]
    async fn test_file_operation_guard_multiple_path_fields() {
        let inner: Arc<dyn Tool> = Arc::new(ReadFileTool);
        let guard = FileOperationGuard::with_defaults(inner);

        // source 字段也应被检查
        let result = guard
            .execute(json!({
                "path": "safe_path.txt",
                "source": "/etc/shadow"
            }))
            .await;
        assert!(result.is_err());
    }

    /// 测试 shellexpand_home 辅助函数
    #[test]
    fn test_shellexpand_home() {
        if let Ok(home) = std::env::var("HOME") {
            let expanded = shellexpand_home("~/.ssh");
            assert_eq!(expanded, format!("{}/.ssh", home));
        }

        // 不以 ~/ 开头的路径不应被修改
        let unchanged = shellexpand_home("/etc/passwd");
        assert_eq!(unchanged, "/etc/passwd");
    }
}

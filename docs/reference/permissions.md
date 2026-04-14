# 权限系统参考手册

> OrangeCoding 权限系统提供多层次的访问控制，确保 Agent 操作在安全边界内执行。

## 目录

- [概述](#概述)
- [权限种类 (PermissionKind)](#权限种类-permissionkind)
- [权限级别 (PermissionLevel)](#权限级别-permissionlevel)
- [权限策略 (PermissionPolicy)](#权限策略-permissionpolicy)
- [安全策略 (SecurityPolicy)](#安全策略-securitypolicy)
- [路径验证器 (PathValidator)](#路径验证器-pathvalidator)
- [文件操作守卫 (FileOperationGuard)](#文件操作守卫-fileoperationguard)
- [沙箱路径限制](#沙箱路径限制)
- [角色权限矩阵](#角色权限矩阵)
- [配置方法](#配置方法)
- [权限检查流程](#权限检查流程)

---

## 概述

OrangeCoding 的权限系统分为三层防护：

```
┌─────────────────────────────────────────────────────────┐
│                     第一层：权限策略                       │
│         PermissionPolicy + PermissionKind/Level          │
│         判断操作是否被允许/需要询问/被拒绝                   │
├─────────────────────────────────────────────────────────┤
│                     第二层：安全策略                       │
│            SecurityPolicy + PathValidator                │
│         验证路径安全性、阻止危险命令和路径                    │
├─────────────────────────────────────────────────────────┤
│                     第三层：操作守卫                       │
│              FileOperationGuard                         │
│         包装工具执行，自动应用安全检查                       │
└─────────────────────────────────────────────────────────┘
```

权限系统的实现跨越两个 crate：
- `chengcoding-tools`：`permissions.rs`（权限定义）和 `security.rs`（安全策略）
- `chengcoding-agent`：`hooks.rs`（权限 Hook 集成）

---

## 权限种类 (PermissionKind)

`PermissionKind` 枚举定义了系统中需要权限控制的操作类别：

```rust
pub enum PermissionKind {
    /// 文件编辑权限（read_file、write_file、edit）
    Edit,

    /// Shell 命令执行权限（bash）
    Bash,

    /// 网络请求权限（fetch、browser、web_search）
    WebFetch,

    /// 循环检测保护（防止 Agent 陷入无限循环）
    DoomLoop,

    /// 外部目录访问权限（工作目录之外的路径）
    ExternalDirectory,
}
```

### 各权限种类详解

#### Edit（文件编辑）

控制对文件系统的读写操作。

| 适用工具 | 检查内容 |
|----------|----------|
| `read_file` | 目标路径是否在允许范围内 |
| `write_file` | 目标路径是否可写、不在黑名单中 |
| `edit` | 同 `write_file` |

**典型配置**:
```toml
[permissions.edit]
level = "allow"  # 自动允许
paths = ["src/", "tests/", "docs/"]
```

#### Bash（Shell 执行）

控制 Shell 命令的执行权限。

| 检查项 | 描述 |
|--------|------|
| 命令白名单 | 仅允许预设的安全命令 |
| 命令黑名单 | 阻止危险命令（`rm -rf /`、`dd`、`mkfs` 等） |
| 超时限制 | 强制执行超时 |

**典型配置**:
```toml
[permissions.bash]
level = "ask"  # 每次询问用户确认
timeout = 300
```

#### WebFetch（网络请求）

控制对外网络通信。

| 检查项 | 描述 |
|--------|------|
| URL 白名单 | 仅允许预设域名 |
| 内网限制 | 阻止 localhost/内网地址 |
| HTTPS 要求 | 强制使用 HTTPS |

#### DoomLoop（循环保护）

检测并防止 Agent 陷入无效的重复操作循环。

| 检测指标 | 阈值 |
|----------|------|
| 相同工具连续调用次数 | > 5 次触发警告 |
| 相同参数重复调用 | > 3 次触发阻止 |
| 消息未进展检测 | 10 轮无新进展触发中止 |

#### ExternalDirectory（外部目录）

控制对工作目录之外路径的访问。

```
工作目录: /home/user/project/
├── src/        ← 允许（内部路径）
├── tests/      ← 允许（内部路径）
└── ...

/home/user/other-project/   ← 需要 ExternalDirectory 权限
/etc/                       ← 被 SecurityPolicy 永久阻止
```

---

## 权限级别 (PermissionLevel)

`PermissionLevel` 枚举定义了三种权限决策级别：

```rust
pub enum PermissionLevel {
    /// 每次操作前询问用户确认
    Ask,

    /// 自动允许，不需要确认
    Allow,

    /// 拒绝操作
    Deny,
}
```

### 权限级别决策矩阵

| 场景 | 推荐级别 | 说明 |
|------|----------|------|
| 开发环境本地文件编辑 | `Allow` | 信任本地操作 |
| 生产环境文件编辑 | `Ask` | 需要人工确认 |
| 任何环境的敏感路径 | `Deny` | 永不允许 |
| 安全的 Shell 命令 | `Allow` | 如 `git status`、`cargo build` |
| 未知的 Shell 命令 | `Ask` | 需要审查 |
| 网络请求（已知 API） | `Allow` | 信任的 API 端点 |
| 网络请求（未知地址） | `Ask` | 需要确认目标 |
| 外部目录访问 | `Ask` | 需要确认意图 |

---

## 权限策略 (PermissionPolicy)

`PermissionPolicy` 是权限配置的聚合结构：

```rust
pub struct PermissionPolicy {
    /// 各权限种类的默认级别
    pub defaults: HashMap<PermissionKind, PermissionLevel>,

    /// 工具专用权限覆盖
    pub tool_overrides: HashMap<String, PermissionLevel>,

    /// 路径专用权限覆盖
    pub path_overrides: HashMap<PathBuf, PermissionLevel>,

    /// 是否启用自动批准模式
    pub auto_approve: bool,
}
```

### 默认策略

```rust
impl Default for PermissionPolicy {
    fn default() -> Self {
        let mut defaults = HashMap::new();
        defaults.insert(PermissionKind::Edit, PermissionLevel::Allow);
        defaults.insert(PermissionKind::Bash, PermissionLevel::Ask);
        defaults.insert(PermissionKind::WebFetch, PermissionLevel::Ask);
        defaults.insert(PermissionKind::DoomLoop, PermissionLevel::Ask);
        defaults.insert(PermissionKind::ExternalDirectory, PermissionLevel::Ask);

        Self {
            defaults,
            tool_overrides: HashMap::new(),
            path_overrides: HashMap::new(),
            auto_approve: false,
        }
    }
}
```

### 权限解析顺序

```
1. 检查 tool_overrides（工具级覆盖）
   ↓ 未匹配
2. 检查 path_overrides（路径级覆盖）
   ↓ 未匹配
3. 检查 defaults（默认级别）
   ↓ 未匹配
4. 返回 Ask（最安全的默认值）
```

### 配置示例

```toml
[permissions]
auto_approve = false

[permissions.defaults]
edit = "allow"
bash = "ask"
web_fetch = "ask"
doom_loop = "ask"
external_directory = "ask"

[permissions.tool_overrides]
# 特定工具覆盖
grep = "allow"        # grep 始终允许
find = "allow"        # find 始终允许
ssh = "deny"          # SSH 禁止使用

[permissions.path_overrides]
# 特定路径覆盖
"src/" = "allow"
"tests/" = "allow"
"/etc/" = "deny"
"~/.config/" = "deny"
```

---

## 安全策略 (SecurityPolicy)

`SecurityPolicy` 提供路径和命令级别的安全约束：

```rust
pub struct SecurityPolicy {
    /// 允许访问的目录列表（白名单）
    pub allowed_dirs: Vec<PathBuf>,

    /// 阻止访问的路径列表（黑名单）
    pub blocked_paths: Vec<PathBuf>,

    /// 是否允许路径遍历（如 ../）
    pub allow_path_traversal: bool,
}
```

### 默认阻止路径列表

以下路径默认被阻止访问，无论权限策略如何配置：

| 路径 | 原因 |
|------|------|
| `/etc/shadow` | 系统密码文件 |
| `/etc/passwd` | 用户账户信息 |
| `/etc/sudoers` | sudo 配置 |
| `~/.ssh/` | SSH 密钥目录 |
| `~/.ssh/id_rsa` | SSH 私钥 |
| `~/.ssh/id_ed25519` | SSH 私钥 |
| `~/.ssh/authorized_keys` | SSH 授权密钥 |
| `~/.aws/` | AWS 凭证目录 |
| `~/.aws/credentials` | AWS 访问密钥 |
| `~/.aws/config` | AWS 配置 |
| `~/.gnupg/` | GPG 密钥目录 |
| `~/.config/gcloud/` | GCP 凭证 |
| `~/.kube/config` | Kubernetes 配置 |
| `~/.docker/config.json` | Docker 凭证 |
| `/sys/` | 系统文件系统 |
| `/proc/` | 进程文件系统 |
| `/dev/` | 设备文件系统 |
| `.env` | 环境变量文件（含敏感信息） |
| `.env.local` | 本地环境变量文件 |
| `*.pem` | 证书/密钥文件 |
| `*.key` | 密钥文件 |
| `*.p12` | PKCS12 证书 |

### 安全策略配置

```toml
[security]
allow_path_traversal = false

[security.allowed_dirs]
dirs = [
    ".",                    # 当前工作目录
    "src/",
    "tests/",
    "docs/",
]

[security.blocked_paths]
paths = [
    "/etc/",
    "~/.ssh/",
    "~/.aws/",
    ".env",
]
```

---

## 路径验证器 (PathValidator)

`PathValidator` 负责对文件路径进行全面的安全检查：

```rust
pub struct PathValidator {
    /// 安全策略引用
    policy: SecurityPolicy,
}
```

### 核心方法

```rust
impl PathValidator {
    /// 综合路径安全检查
    pub fn is_path_safe(&self, path: &Path) -> Result<(), ToolError>;

    /// 检查路径是否在允许目录中
    fn is_in_allowed_dir(&self, path: &Path) -> bool;

    /// 检查路径是否在阻止列表中
    fn is_blocked(&self, path: &Path) -> bool;

    /// 检查是否存在路径遍历攻击
    fn has_traversal(&self, path: &Path) -> bool;

    /// 规范化路径（解析符号链接、../ 等）
    fn canonicalize(&self, path: &Path) -> Result<PathBuf, ToolError>;
}
```

### 验证流程

```
输入路径: "../../../etc/passwd"
  ↓
1. 规范化路径
   → /etc/passwd
  ↓
2. 路径遍历检查
   → ❌ 包含 "../" 且 allow_path_traversal = false
   → 返回 SecurityViolation("路径遍历攻击")
  ↓
（如果通过遍历检查）
3. 阻止列表检查
   → ❌ /etc/passwd 在 blocked_paths 中
   → 返回 SecurityViolation("路径在阻止列表中")
  ↓
（如果通过阻止检查）
4. 允许目录检查
   → ❌ /etc/ 不在 allowed_dirs 中
   → 返回 SecurityViolation("路径不在允许目录中")
  ↓
✅ 通过所有检查 → Ok(())
```

### 路径遍历攻击防护

```
❌ 被阻止的路径模式:
  ../../../etc/passwd          # 显式遍历
  src/../../etc/passwd         # 嵌套遍历
  ./src/../../../etc/passwd    # 混合遍历
  src/symlink -> /etc/passwd   # 符号链接攻击

✅ 允许的路径模式:
  src/main.rs                  # 相对路径
  ./src/lib.rs                 # 当前目录相对路径
  /home/user/project/src/      # 在 allowed_dirs 中的绝对路径
```

---

## 文件操作守卫 (FileOperationGuard)

`FileOperationGuard` 是一个安全包装层，自动为文件操作工具添加安全检查：

```rust
pub struct FileOperationGuard {
    /// 安全策略
    policy: SecurityPolicy,

    /// 路径验证器
    validator: PathValidator,
}
```

### 核心方法

```rust
impl FileOperationGuard {
    /// 创建新的操作守卫
    pub fn new(policy: SecurityPolicy) -> Self;

    /// 检查读取操作是否安全
    pub fn check_read(&self, path: &Path) -> Result<(), ToolError>;

    /// 检查写入操作是否安全
    pub fn check_write(&self, path: &Path) -> Result<(), ToolError>;

    /// 检查删除操作是否安全
    pub fn check_delete(&self, path: &Path) -> Result<(), ToolError>;

    /// 包装工具执行，自动应用安全检查
    pub async fn guard_execute(
        &self,
        tool: &dyn Tool,
        params: Value,
    ) -> ToolResult<String>;
}
```

### 工作原理

```
工具调用请求
  ↓
FileOperationGuard.guard_execute()
  ↓
┌─────────────────────────────────────────┐
│ 1. 提取路径参数                           │
│    params["path"] → "/home/user/file"    │
│                                         │
│ 2. PathValidator.is_path_safe(path)      │
│    检查遍历 → 检查阻止列表 → 检查允许目录  │
│                                         │
│ 3. PermissionPolicy 权限检查             │
│    Edit / ExternalDirectory              │
│                                         │
│ 4. 如果是写操作：                         │
│    额外检查目标目录是否可写                 │
│    检查文件大小限制                        │
│                                         │
│ 5. 通过所有检查                           │
│    → tool.execute(params)               │
│                                         │
│ 6. 未通过检查                            │
│    → ToolError::SecurityViolation       │
└─────────────────────────────────────────┘
```

### 安全检查顺序

```
guard_execute()
  │
  ├── 1. 路径规范化
  │     └── 解析 ../ 、符号链接
  │
  ├── 2. 阻止列表匹配
  │     └── blocked_paths 包含检查
  │
  ├── 3. 路径遍历检测
  │     └── allow_path_traversal 判断
  │
  ├── 4. 允许目录验证
  │     └── allowed_dirs 包含检查
  │
  ├── 5. 权限级别检查
  │     └── PermissionLevel::Allow/Ask/Deny
  │
  ├── 6. 文件大小检查（写操作）
  │     └── max_file_size 限制
  │
  └── 7. 审计记录
        └── AuditLogger.log_tool_call()
```

---

## 沙箱路径限制

### 沙箱模型

OrangeCoding 采用基于白名单的沙箱模型：

```
┌─────────────────────────────────────────────────────┐
│                    文件系统                           │
│                                                     │
│  ┌─────────────────────────────────────────────┐    │
│  │              沙箱边界                         │    │
│  │                                             │    │
│  │  ┌─────────┐  ┌─────────┐  ┌──────────┐   │    │
│  │  │  src/    │  │ tests/  │  │  docs/   │   │    │
│  │  │ ✅ 允许  │  │ ✅ 允许  │  │ ✅ 允许   │   │    │
│  │  └─────────┘  └─────────┘  └──────────┘   │    │
│  │                                             │    │
│  │  工作目录: /home/user/project/              │    │
│  └─────────────────────────────────────────────┘    │
│                                                     │
│  ┌──────────┐  ┌──────────┐  ┌───────────┐        │
│  │ /etc/    │  │ ~/.ssh/  │  │  ~/.aws/  │        │
│  │ ❌ 阻止  │  │ ❌ 阻止  │  │  ❌ 阻止  │        │
│  └──────────┘  └──────────┘  └───────────┘        │
│                                                     │
│  ┌──────────────────────────────────────┐          │
│  │  /home/user/other-project/           │          │
│  │  ⚠️ 需要 ExternalDirectory 权限       │          │
│  └──────────────────────────────────────┘          │
└─────────────────────────────────────────────────────┘
```

### 沙箱规则

| 规则 | 描述 | 可覆盖 |
|------|------|--------|
| 工作目录内路径 | 默认允许 | ✅ 可通过 blocked_paths |
| 工作目录外路径 | 需要 ExternalDirectory 权限 | ✅ 可通过 allowed_dirs |
| 系统敏感路径 | 永久阻止 | ❌ 不可覆盖 |
| 用户凭证路径 | 永久阻止 | ❌ 不可覆盖 |
| 路径遍历尝试 | 默认阻止 | ✅ 可通过 allow_path_traversal |

### 永久阻止（不可覆盖）

以下路径无论任何配置都不可访问：

```
/etc/shadow
/etc/sudoers
~/.ssh/id_rsa
~/.ssh/id_ed25519
~/.aws/credentials
~/.gnupg/private-keys-v1.d/
```

---

## 角色权限矩阵

基于 `AgentRole` 的默认工具权限分配：

| 工具 | Coder | Reviewer | Planner | Executor | Observer |
|------|-------|----------|---------|----------|----------|
| `read_file` | ✅ | ✅ | ✅ | ✅ | ✅ |
| `write_file` | ✅ | ❌ | ❌ | ✅ | ❌ |
| `edit` | ✅ | ❌ | ❌ | ✅ | ❌ |
| `bash` | ✅ | ❌ | ❌ | ✅ | ❌ |
| `grep` | ✅ | ✅ | ✅ | ✅ | ✅ |
| `find` | ✅ | ✅ | ✅ | ✅ | ✅ |
| `browser` | ✅ | ✅ | ✅ | ✅ | ✅ |
| `web_search` | ✅ | ✅ | ✅ | ✅ | ✅ |
| `fetch` | ✅ | ✅ | ✅ | ✅ | ❌ |
| `lsp` | ✅ | ✅ | ❌ | ✅ | ❌ |
| `ast_grep` | ✅ | ✅ | ❌ | ✅ | ❌ |
| `python` | ✅ | ❌ | ❌ | ✅ | ❌ |
| `ssh` | ❌ | ❌ | ❌ | ✅ | ❌ |
| `todo` | ✅ | ✅ | ✅ | ✅ | ✅ |
| `task` | ✅ | ❌ | ✅ | ✅ | ❌ |
| `ask` | ✅ | ✅ | ✅ | ✅ | ✅ |

角色定义位于 `chengcoding-mesh` crate 的 `role_system.rs` 模块中。

---

## 配置方法

### 配置文件 (`~/.config/OrangeCoding/config.toml`)

```toml
[tools]
# 允许访问的路径
allowed_paths = [".", "src/", "tests/", "docs/"]

# 阻止访问的路径
blocked_paths = ["/etc/", "~/.ssh/", "~/.aws/", ".env"]

# 最大文件大小（字节）
max_file_size = 10485760  # 10MB

[agent]
# 是否自动批准工具调用
auto_approve_tools = false
```

### 环境变量

| 环境变量 | 描述 | 示例 |
|----------|------|------|
| `OrangeCoding_AUTO_APPROVE` | 自动批准所有操作 | `true` |
| `OrangeCoding_ALLOWED_DIRS` | 额外允许目录（逗号分隔） | `/opt/data,/var/log` |
| `OrangeCoding_BLOCKED_PATHS` | 额外阻止路径（逗号分隔） | `/tmp/secret` |
| `OrangeCoding_MAX_FILE_SIZE` | 最大文件大小 | `20971520` |

### 命令行参数

```bash
# 自动批准模式
OrangeCoding launch --auto-approve

# 指定允许路径
OrangeCoding launch --allowed-paths "src/,tests/"

# 指定日志级别
OrangeCoding launch --log-level debug
```

---

## 权限检查流程

### 完整流程图

```
Agent 发起工具调用
  ↓
┌─────────────────────────────────────────────────────────┐
│ 1. Hook 系统拦截（PreToolCall）                           │
│    ├── security_path_check (Critical)                    │
│    ├── security_bash_guard (Critical)                    │
│    ├── permission_edit_check (High)                      │
│    ├── permission_bash_check (High)                      │
│    └── permission_external_dir (High)                    │
│                                                         │
│    任何 Hook 返回 Block → 操作被拒绝                       │
└────────────────────────┬────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────┐
│ 2. FileOperationGuard 检查                              │
│    ├── 路径规范化                                        │
│    ├── PathValidator.is_path_safe()                     │
│    │   ├── 阻止列表匹配                                  │
│    │   ├── 路径遍历检测                                   │
│    │   └── 允许目录验证                                   │
│    └── 文件大小检查                                       │
│                                                         │
│    任何检查失败 → ToolError::SecurityViolation            │
└────────────────────────┬────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────┐
│ 3. PermissionPolicy 权限决策                             │
│    ├── 检查 tool_overrides                              │
│    ├── 检查 path_overrides                              │
│    ├── 检查 defaults                                    │
│    └── 决策：Allow / Ask / Deny                         │
│                                                         │
│    Deny → 操作被拒绝                                     │
│    Ask  → 请求用户确认                                    │
│    Allow → 继续执行                                      │
└────────────────────────┬────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────┐
│ 4. 工具执行                                              │
│    tool.execute(params) → ToolResult<String>             │
└────────────────────────┬────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────┐
│ 5. 审计记录                                              │
│    AuditLogger.log_tool_call(name, params, result)       │
│    包含脱敏处理（Sanitizer）                               │
└─────────────────────────────────────────────────────────┘
```

---

## 相关文档

- [Hook 系统参考](./hooks.md) - 权限 Hook 的详细说明
- [安全架构](../architecture/security.md) - 安全策略的设计原理
- [工具参考](./tools.md) - 各工具的权限要求

# 安全架构

## 安全层次

CEAIR 采用多层安全防护架构：

```
┌─────────────────────────────────────┐
│          权限系统 (Permissions)       │  ← 用户级控制
├─────────────────────────────────────┤
│      FileOperationGuard (沙箱)       │  ← 路径级控制
├─────────────────────────────────────┤
│       SecurityPolicy (策略)          │  ← 策略级控制
├─────────────────────────────────────┤
│        审计链 (AuditChain)           │  ← 审计级控制
├─────────────────────────────────────┤
│       密钥检测 (SecretDetector)       │  ← 数据级控制
└─────────────────────────────────────┘
```

## 权限系统

### PermissionKind — 5 种权限类型

| 类型 | 说明 |
|------|------|
| `Edit` | 文件编辑权限 |
| `Bash` | Shell 命令执行权限 |
| `WebFetch` | 网络请求权限 |
| `DoomLoop` | 自动循环执行权限 |
| `ExternalDirectory` | 外部目录访问权限 |

### PermissionLevel — 3 级控制

| 级别 | 行为 |
|------|------|
| `Ask` | 每次使用前询问用户确认 |
| `Allow` | 自动允许，不需确认 |
| `Deny` | 自动拒绝，禁止使用 |

### PermissionPolicy

```rust
pub struct PermissionPolicy {
    rules: HashMap<PermissionKind, PermissionLevel>,
}
```

- `check(kind)` — 检查权限级别
- `set(kind, level)` — 设置权限级别
- `default()` — 默认策略（所有权限为 Ask）

## FileOperationGuard — 沙箱守卫

### 工作原理

所有文件操作工具都被 `FileOperationGuard` 包装：

```
工具调用 → FileOperationGuard → 路径检查 → 允许/拒绝
```

### 保护的工具

| 工具 | 检查字段 |
|------|---------|
| read_file | path |
| write_file | path |
| edit | path |
| grep | path |
| find | path |
| notebook | path |
| ast_grep | path |

### 路径检查逻辑

```rust
fn is_path_allowed(&self, path: &str) -> bool {
    // 1. 检查是否在阻止路径列表中
    if self.blocked_paths.iter().any(|bp| path.starts_with(bp)) {
        return false;
    }
    // 2. 检查是否在允许路径列表中
    if self.allowed_paths.is_empty() {
        return true;  // 未配置允许路径则允许所有
    }
    self.allowed_paths.iter().any(|ap| path.starts_with(ap))
}
```

## SecurityPolicy — 安全策略

### 默认阻止路径

以下路径默认被阻止访问：

```
~/.ssh/          — SSH 密钥和配置
~/.aws/          — AWS 凭证
~/.gnupg/        — GPG 密钥
~/.config/       — 应用配置（部分）
/etc/passwd      — 系统用户信息
/etc/shadow      — 密码哈希
/etc/sudoers     — sudo 配置
~/.bash_history  — 命令历史
~/.zsh_history   — Zsh 命令历史
~/.env           — 环境变量文件
~/.netrc         — 网络凭证
~/.git-credentials — Git 凭证
~/.docker/       — Docker 配置
~/.kube/         — Kubernetes 配置
```

### 配置合并

用户配置的 `blocked_paths` 与默认列表**合并**（不替换）：

```rust
// 正确行为：合并默认和用户配置
let policy = SecurityPolicy::default_policy();
policy.blocked_paths.extend(user_blocked_paths);
```

## 审计链 (AuditChain)

### 链式哈希

每条审计记录包含前一条记录的哈希，形成不可篡改的链：

```
Entry1 → SHA-256(Entry1) → Entry2.prev_hash
Entry2 → SHA-256(Entry2) → Entry3.prev_hash
```

### 审计内容

- 工具调用记录
- 文件修改记录
- 权限检查结果
- Agent 切换事件

## 密钥检测 (SecretDetector)

### 检测模式

支持多种 API 密钥格式的自动检测：

- `sk-` 前缀 — OpenAI 密钥
- `sk-ant-` 前缀 — Anthropic 密钥
- AWS Access Key 格式
- 通用 Base64 长字符串

### 脱敏模式

| 模式 | 效果 |
|------|------|
| Placeholder | 替换为 `[REDACTED]` 占位符 |
| Redact | 保留前4字符，其余替换为 `***` |

## OAuth 2.1 安全

### PKCE (Proof Key for Code Exchange)

使用 S256 方法：

```
1. 生成随机 code_verifier
2. 计算 code_challenge = BASE64URL(SHA256(code_verifier))
3. 授权请求携带 code_challenge
4. 令牌交换携带 code_verifier
5. 服务器验证 SHA256(code_verifier) == code_challenge
```

### Token 安全存储

- 存储路径: `~/.config/opencode/mcp-oauth.json`
- Token 自动刷新
- 过期检测和清理

## MCP 认证安全

### 发现流程 (RFC 9728)

```
1. GET /.well-known/oauth-protected-resource
   → 获取授权服务器地址
2. GET /.well-known/oauth-authorization-server
   → 获取授权端点和令牌端点
3. 执行 PKCE 授权流程
```

### 安全要求

- 所有通信使用 HTTPS
- PKCE 强制使用 S256（不允许 plain）
- Token 刷新使用独立端点
- 客户端动态注册 (RFC 7591)

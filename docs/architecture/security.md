# 安全架构

> OrangeCoding 采用纵深防御策略，通过多层安全机制保护系统免受未授权访问和数据泄露。

## 目录

- [概述](#概述)
- [安全策略层次](#安全策略层次)
- [默认阻止路径列表](#默认阻止路径列表)
- [FileOperationGuard 工作原理](#fileoperationguard-工作原理)
- [沙箱路径验证](#沙箱路径验证)
- [审计链 (AuditChain)](#审计链-auditchain)
- [密钥检测和脱敏](#密钥检测和脱敏)
- [MCP 认证安全](#mcp-认证安全)
- [加密存储](#加密存储)
- [安全配置最佳实践](#安全配置最佳实践)

---

## 概述

OrangeCoding 的安全架构遵循"纵深防御"（Defense in Depth）原则，在多个层次设置安全控制点：

```
┌──────────────────────────────────────────────────────────────┐
│                   OrangeCoding 安全架构全景                           │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌── 第 1 层: 请求拦截 ─────────────────────────────────┐    │
│  │  Hook 系统 (PreToolCall)                              │    │
│  │  • security_path_check (Critical)                     │    │
│  │  • security_bash_guard (Critical)                     │    │
│  │  • security_network_guard (Critical)                  │    │
│  │  • security_sandbox_enforce (Critical)                │    │
│  └───────────────────────────────────────────────────────┘    │
│                          ↓                                    │
│  ┌── 第 2 层: 路径安全 ─────────────────────────────────┐    │
│  │  FileOperationGuard + PathValidator                   │    │
│  │  • 路径规范化（解析符号链接）                           │    │
│  │  • 路径遍历攻击检测                                    │    │
│  │  • 阻止列表匹配                                       │    │
│  │  • 允许目录验证                                       │    │
│  └───────────────────────────────────────────────────────┘    │
│                          ↓                                    │
│  ┌── 第 3 层: 权限控制 ─────────────────────────────────┐    │
│  │  PermissionPolicy                                     │    │
│  │  • PermissionKind 分类检查                             │    │
│  │  • PermissionLevel 决策 (Allow/Ask/Deny)              │    │
│  │  • 角色权限矩阵                                       │    │
│  └───────────────────────────────────────────────────────┘    │
│                          ↓                                    │
│  ┌── 第 4 层: 执行隔离 ─────────────────────────────────┐    │
│  │  工具沙箱                                             │    │
│  │  • Bash 命令超时和资源限制                              │    │
│  │  • 网络请求 HTTPS 强制                                 │    │
│  │  • 文件大小限制                                       │    │
│  └───────────────────────────────────────────────────────┘    │
│                          ↓                                    │
│  ┌── 第 5 层: 输出审查 ─────────────────────────────────┐    │
│  │  PostToolCall Hooks                                   │    │
│  │  • security_secret_scan → 密钥检测                    │    │
│  │  • transform_sanitize_output → 脱敏处理               │    │
│  └───────────────────────────────────────────────────────┘    │
│                          ↓                                    │
│  ┌── 第 6 层: 审计追溯 ─────────────────────────────────┐    │
│  │  AuditChain (不可篡改)                                │    │
│  │  • SHA-256 哈希链                                     │    │
│  │  • 完整操作日志                                       │    │
│  │  • 链完整性验证                                       │    │
│  └───────────────────────────────────────────────────────┘    │
│                                                              │
│  ┌── 横切关注: 加密存储 ────────────────────────────────┐    │
│  │  CryptoStore (AES-256-GCM)                            │    │
│  │  • API 密钥加密                                       │    │
│  │  • OAuth Token 加密                                   │    │
│  │  • 配置文件敏感字段加密                                │    │
│  └───────────────────────────────────────────────────────┘    │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

---

## 安全策略层次

### 第 1 层：Hook 拦截

通过 `HookRegistry` 的 `Critical` 优先级 Hook 实现第一道防线。

```rust
HookPriority::Critical = 0  // 最高优先级，不可跳过
```

#### 安全 Hook 清单

| Hook | 事件 | 检查内容 | 阻止条件 |
|------|------|----------|----------|
| `security_path_check` | PreToolCall | 文件路径安全性 | 路径在阻止列表中 |
| `security_bash_guard` | PreToolCall | Shell 命令安全性 | 包含危险命令 |
| `security_secret_scan` | PostToolCall | 输出中的密钥泄露 | 检测到 API Key 等 |
| `security_sandbox_enforce` | PreToolCall | 沙箱边界 | 操作超出沙箱范围 |
| `security_network_guard` | PreToolCall | 网络请求目标 | 目标为内网地址 |

#### Bash 命令守卫

```
阻止的命令模式:
  rm -rf /                    # 递归删除根目录
  rm -rf ~                    # 递归删除用户目录
  dd if=/dev/zero of=...      # 磁盘覆写
  mkfs.*                      # 格式化文件系统
  chmod -R 777 /              # 全局权限修改
  curl ... | bash             # 管道执行远程脚本
  wget ... | sh               # 管道执行远程脚本
  :(){ :|:& };:              # Fork 炸弹
  > /dev/sda                  # 直接写入磁盘设备
```

### 第 2 层：路径安全

`FileOperationGuard` 和 `PathValidator` 确保所有文件操作在安全边界内。

### 第 3 层：权限控制

`PermissionPolicy` 提供细粒度的操作权限控制。

### 第 4 层：执行隔离

工具执行时的资源限制和隔离措施。

### 第 5 层：输出审查

在工具执行后对输出进行安全检查和脱敏。

### 第 6 层：审计追溯

不可篡改的审计链记录所有操作。

---

## 默认阻止路径列表

以下路径默认被阻止访问，分为**永久阻止**（不可覆盖）和**默认阻止**（可通过配置覆盖）两类：

### 永久阻止路径（不可覆盖）

| 路径 | 分类 | 风险说明 |
|------|------|----------|
| `/etc/shadow` | 系统凭证 | 包含加密的用户密码哈希 |
| `/etc/sudoers` | 系统权限 | sudo 权限配置，泄露可导致提权 |
| `~/.ssh/id_rsa` | SSH 密钥 | RSA 私钥，泄露可导致远程登录 |
| `~/.ssh/id_ed25519` | SSH 密钥 | Ed25519 私钥 |
| `~/.ssh/id_ecdsa` | SSH 密钥 | ECDSA 私钥 |
| `~/.aws/credentials` | 云凭证 | AWS 访问密钥和秘密密钥 |
| `~/.gnupg/private-keys-v1.d/` | GPG 密钥 | GPG 私钥目录 |

### 默认阻止路径（可通过配置覆盖）

| 路径 | 分类 | 风险说明 |
|------|------|----------|
| `/etc/passwd` | 系统信息 | 用户账户列表 |
| `~/.ssh/` | SSH 配置 | SSH 配置目录 |
| `~/.ssh/authorized_keys` | SSH 授权 | 授权访问密钥 |
| `~/.ssh/known_hosts` | SSH 主机 | 已知主机记录 |
| `~/.aws/config` | 云配置 | AWS 配置（含区域、角色等） |
| `~/.config/gcloud/` | 云凭证 | GCP 凭证目录 |
| `~/.kube/config` | K8s 配置 | Kubernetes 集群凭证 |
| `~/.docker/config.json` | 容器凭证 | Docker 注册表凭证 |
| `/sys/` | 系统 | 系统文件系统 |
| `/proc/` | 进程 | 进程信息（可能含敏感数据） |
| `/dev/` | 设备 | 设备文件（写入可导致数据丢失） |
| `.env` | 环境变量 | 环境变量文件（常含密钥） |
| `.env.local` | 环境变量 | 本地环境变量文件 |
| `.env.production` | 环境变量 | 生产环境变量文件 |
| `*.pem` | 证书 | 证书/密钥文件 |
| `*.key` | 密钥 | 密钥文件 |
| `*.p12` | 证书 | PKCS#12 证书 |
| `*.pfx` | 证书 | PFX 证书文件 |
| `*.keystore` | 密钥库 | Java 密钥库 |

### 路径匹配规则

```
路径匹配采用以下顺序:
  1. 精确匹配: /etc/shadow → 完全匹配
  2. 目录前缀: ~/.ssh/ → 匹配 ~/.ssh/ 下所有文件
  3. Glob 模式: *.pem → 匹配所有 .pem 文件
  4. 路径规范化后匹配: 先解析符号链接和 ../ 再匹配
```

---

## FileOperationGuard 工作原理

`FileOperationGuard` 是文件操作的安全包装层，所有文件工具的操作都经过它的检查。

### 检查流程

```
输入: Tool + Params
  ↓
步骤 1: 提取路径参数
  params["path"] → 原始路径字符串
  ↓
步骤 2: 路径规范化
  • 展开 ~ 为用户主目录
  • 解析 . 和 .. 组件
  • 解析符号链接到真实路径
  • 转换为绝对路径
  "../../../etc/passwd" → /etc/passwd
  ↓
步骤 3: 路径遍历检测
  如果 allow_path_traversal = false:
    检查原始路径中是否包含 "../"
    ❌ → SecurityViolation
  ↓
步骤 4: 阻止列表检查
  遍历 blocked_paths:
    精确匹配 / 目录前缀 / Glob 模式
  ❌ → SecurityViolation
  ↓
步骤 5: 允许目录检查
  路径是否在 allowed_dirs 中
  ❌ → 需要 ExternalDirectory 权限
  ↓
步骤 6: 文件大小检查（写操作）
  content.len() <= max_file_size
  ↓
步骤 7: 权限策略检查
  Allow → 执行 | Ask → 确认 | Deny → 拒绝
  ↓
✅ 通过 → tool.execute(params)
```

### 符号链接攻击防护

```
攻击: src/safe_link → /etc/shadow
防护: 规范化后 src/safe_link → /etc/shadow → 阻止列表命中 → ❌ 拒绝
```

---

## 沙箱路径验证

```
文件系统视图:

  /
  ├── etc/                    ❌ 系统永久阻止
  ├── sys/                    ❌ 系统永久阻止
  ├── proc/                   ❌ 系统永久阻止
  ├── home/user/
  │   ├── .ssh/               ❌ 用户凭证阻止
  │   ├── .aws/               ❌ 用户凭证阻止
  │   ├── project/            ← 工作目录（沙箱根）
  │   │   ├── src/            ✅ 沙箱内，自由访问
  │   │   ├── tests/          ✅ 沙箱内，自由访问
  │   │   ├── docs/           ✅ 沙箱内，自由访问
  │   │   ├── .env            ⚠️ 默认阻止，可配置
  │   │   └── target/         ✅ 沙箱内，自由访问
  │   └── other-project/      ⚠️ 需要 ExternalDirectory 权限
  └── opt/data/               ⚠️ 需要添加到 allowed_dirs
```

---

## 审计链 (AuditChain)

### AuditEntry 结构

```rust
pub struct AuditEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub action: String,           // "tool_call", "ai_request"
    pub actor: String,            // 用户或系统组件
    pub target: Option<String>,   // 文件路径、API 端点
    pub details: Value,           // JSON 详情
    pub hash: String,             // SHA-256 哈希
    pub previous_hash: String,    // 前一条目哈希
}
```

### 哈希链机制

```
创世条目:
  previous_hash = "0000...0000"
  hash = SHA256("0000...0000" + data) = "a1b2..."

条目 1:
  previous_hash = "a1b2..."
  hash = SHA256("a1b2..." + data) = "e5f6..."

条目 2:
  previous_hash = "e5f6..."
  hash = SHA256("e5f6..." + data) = "i9j0..."

篡改检测:
  如果修改条目 1 → hash 变化 → 条目 2 的 previous_hash 不匹配
  → 整条链断裂 → verify() 返回 false
```

### HashChain API

```rust
impl HashChain {
    pub fn new() -> Self;
    pub fn append(&mut self, action, actor, target, details) -> AuditEntry;
    pub fn verify(&self) -> bool;  // 验证整链完整性
    pub fn len(&self) -> usize;
}
```

### 存储格式

JSON Lines (`.jsonl`)，每行一个审计条目：

```jsonl
{"id":"550e8400-...","timestamp":"2024-01-15T10:30:00Z","action":"tool_call","actor":"agent:junior","target":"src/main.rs","details":{"tool":"edit"},"hash":"a1b2...","previous_hash":"0000..."}
```

### AuditLogger 配置

```rust
pub struct AuditLoggerConfig {
    pub log_path: PathBuf,      // 默认: ~/.config/OrangeCoding/audit/
    pub max_file_size: u64,     // 默认: 10MB
    pub batch_size: usize,      // 默认: 100 条
    pub sanitize: bool,         // 默认: true
}
```

---

## 密钥检测和脱敏

### 检测模式

| 模式 | 正则表达式 | 匹配示例 |
|------|-----------|----------|
| OpenAI Key | `sk-[a-zA-Z0-9]{48}` | `sk-abc123...` |
| Anthropic Key | `sk-ant-[a-zA-Z0-9-]{95}` | `sk-ant-api03-...` |
| AWS Access Key | `AKIA[0-9A-Z]{16}` | `AKIAIOSFODNN7...` |
| GitHub Token | `gh[ps]_[A-Za-z0-9_]{36}` | `ghp_xxxx...` |
| Bearer Token | `Bearer\s+[a-zA-Z0-9._-]{20,}` | `Bearer eyJhbG...` |
| 私钥标记 | `-----BEGIN.*PRIVATE KEY-----` | PEM 私钥 |
| 密码字段 | `password\s*[:=]\s*"[^"]{8,}"` | `password = "..."` |
| 连接字符串 | `[a-z]+://[^:]+:[^@]+@` | `mysql://user:pass@` |

### 脱敏模式

```rust
pub enum ObfuscationMode {
    Mask,    // sk-abc123 → sk-***...***
    Hash,    // sk-abc123 → [SECRET:SHA256:7f83b1...]
    Remove,  // sk-abc123 → [REDACTED]
}
```

### SecretSource 分类

```rust
pub enum SecretSource {
    ApiKey,        // API 密钥
    Token,         // 访问令牌
    Credential,    // 用户凭证
    Password,      // 密码
    EncryptionKey, // 加密密钥
    Custom(String),// 自定义类型
}
```

### 脱敏流程

```
工具输出 → 正则扫描 → 密钥匹配 → ObfuscationMode 处理 → 脱敏输出
                                                         ↓
                                                  审计记录检测事件
```

---

## MCP 认证安全

详见 [OAuth 2.1 参考](../reference/oauth.md)。

| 安全措施 | 实现方式 |
|----------|----------|
| PKCE (S256) | 防止授权码拦截，仅支持 S256 方法 |
| State 参数 | 随机生成，防止 CSRF 攻击 |
| 仅本地回调 | 回调服务器绑定 `127.0.0.1` |
| TLS 强制 | 所有 OAuth 通信使用 HTTPS (rustls) |
| Token 加密 | AES-256-GCM 加密存储 |
| 自动过期 | 提前 60 秒刷新，尊重 expires_in |
| 端口随机化 | 回调使用随机端口避免预测 |

### MCP 传输安全

```
stdio 模式（本地子进程）:
  • 进程间通信，不经过网络
  • Token 通过环境变量安全传递
  • 子进程继承父进程权限限制

HTTP 模式（远程服务器）:
  • TLS 加密传输 (rustls，严格证书验证)
  • OAuth 2.1 Bearer Token 认证
  • Token 自动刷新
```

---

## 加密存储

### CryptoStore

| 参数 | 值 |
|------|------|
| 算法 | AES-256-GCM |
| 密钥长度 | 256 位 |
| Nonce | 96 位随机 |
| 认证标签 | 128 位 |
| 实现库 | `ring` crate |

### 加密流程

```
明文数据 → JSON 序列化 → 生成随机 Nonce
  → AES-256-GCM 加密 → nonce + ciphertext + tag → 写入文件 (chmod 600)
```

### 密钥来源（优先级）

1. 系统密钥环（macOS Keychain / Linux Secret Service）
2. 用户密码 + PBKDF2 派生
3. 文件密钥 (`~/.config/OrangeCoding/keyring`)

---

## 安全配置最佳实践

### 生产环境

```toml
[permissions]
auto_approve = false
[permissions.defaults]
edit = "ask"
bash = "ask"
web_fetch = "ask"
external_directory = "deny"

[security]
allow_path_traversal = false

[audit]
sanitize = true
```

### 开发环境

```toml
[permissions.defaults]
edit = "allow"
bash = "ask"
web_fetch = "allow"
[permissions.tool_overrides]
grep = "allow"
find = "allow"
lsp = "allow"
```

### 安全检查清单

```
□ API 密钥通过 CryptoStore 加密存储
□ .env 文件在阻止列表中
□ allow_path_traversal = false
□ 审计日志已启用
□ 输出脱敏已开启
□ OAuth 使用 PKCE (S256)
□ MCP 通信使用 HTTPS
□ 文件权限正确设置 (600)
□ 定期验证审计链完整性
□ 权限级别配置合理
```

---

## 相关文档

- [权限系统参考](../reference/permissions.md) - 权限配置详解
- [OAuth 2.1 参考](../reference/oauth.md) - OAuth 认证流程
- [Hook 系统参考](../reference/hooks.md) - 安全 Hook 详解
- [架构概览](./overview.md) - 系统安全边界图

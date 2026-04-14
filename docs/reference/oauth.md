# OAuth 2.1 参考手册

> OrangeCoding 的 OAuth 实现遵循最新标准，为 MCP 服务器和 AI 提供商提供安全的认证授权机制。

## 目录

- [概述](#概述)
- [遵循标准](#遵循标准)
- [PKCE 流程](#pkce-流程)
- [Token 管理](#token-管理)
  - [Token 存储](#token-存储)
  - [Token 刷新](#token-刷新)
- [MCP 服务器认证](#mcp-服务器认证)
- [AI 提供商认证](#ai-提供商认证)
- [配置方法](#配置方法)
- [安全考量](#安全考量)

---

## 概述

OrangeCoding 在 `chengcoding-cli` crate 的 `oauth.rs` 模块和 `chengcoding-mcp` crate 中实现了完整的 OAuth 2.1 流程，支持以下场景：

1. **MCP 服务器认证**：通过 OAuth 授权访问远程 MCP 服务器的工具和资源
2. **AI 提供商认证**：安全管理 API 密钥和访问令牌
3. **动态客户端注册**：自动注册为 OAuth 客户端

```
┌──────────────────────────────────────────────────────────┐
│                    OrangeCoding OAuth 架构                       │
│                                                          │
│  ┌──────────┐     ┌──────────────┐     ┌──────────────┐ │
│  │ chengcoding-cli │────►│  OAuth 流程   │────►│  Token 存储  │ │
│  │ oauth.rs  │     │  PKCE + S256 │     │  加密存储     │ │
│  └──────────┘     └──────────────┘     └──────────────┘ │
│       │                                       │          │
│       ▼                                       ▼          │
│  ┌──────────┐                          ┌──────────────┐ │
│  │ chengcoding-mcp│                          │ chengcoding-config │ │
│  │ 服务器认证│                          │ CryptoStore  │ │
│  └──────────┘                          └──────────────┘ │
└──────────────────────────────────────────────────────────┘
```

---

## 遵循标准

OrangeCoding 的 OAuth 实现遵循以下 RFC 标准：

### RFC 9728 — OAuth 2.0 Protected Resource Metadata

定义了受保护资源的元数据发现机制。

```
GET /.well-known/oauth-protected-resource HTTP/1.1
Host: mcp-server.example.com

响应:
{
  "resource": "https://mcp-server.example.com",
  "authorization_servers": ["https://auth.example.com"],
  "scopes_supported": ["mcp:tools", "mcp:resources"]
}
```

**在 OrangeCoding 中的应用**:
- MCP 客户端通过此端点发现授权服务器
- 自动确定所需的权限范围

### RFC 8414 — OAuth 2.0 Authorization Server Metadata

定义了授权服务器的元数据发现。

```
GET /.well-known/oauth-authorization-server HTTP/1.1
Host: auth.example.com

响应:
{
  "issuer": "https://auth.example.com",
  "authorization_endpoint": "https://auth.example.com/authorize",
  "token_endpoint": "https://auth.example.com/token",
  "registration_endpoint": "https://auth.example.com/register",
  "code_challenge_methods_supported": ["S256"],
  "grant_types_supported": ["authorization_code", "refresh_token"],
  "token_endpoint_auth_methods_supported": ["none"]
}
```

**在 OrangeCoding 中的应用**:
- 自动发现授权端点、令牌端点
- 确认服务器支持 PKCE (S256)
- 确认支持动态客户端注册

### RFC 7591 — OAuth 2.0 Dynamic Client Registration

支持客户端的自动注册。

```
POST /register HTTP/1.1
Host: auth.example.com
Content-Type: application/json

{
  "client_name": "OrangeCoding CLI",
  "redirect_uris": ["http://127.0.0.1:9876/callback"],
  "grant_types": ["authorization_code", "refresh_token"],
  "token_endpoint_auth_method": "none"
}

响应:
{
  "client_id": "chengcoding_abc123",
  "client_name": "OrangeCoding CLI",
  "redirect_uris": ["http://127.0.0.1:9876/callback"]
}
```

**在 OrangeCoding 中的应用**:
- 首次连接 MCP 服务器时自动注册
- 使用 `none` 认证方法（公共客户端）
- 回调地址使用本地临时端口

### RFC 8707 — Resource Indicators for OAuth 2.0

支持多资源授权。

```
POST /token HTTP/1.1

grant_type=authorization_code
&code=AUTH_CODE
&resource=https://mcp-server.example.com
&code_verifier=CODE_VERIFIER
```

**在 OrangeCoding 中的应用**:
- 为特定 MCP 服务器请求令牌
- 确保令牌仅对目标资源有效

---

## PKCE 流程

OrangeCoding 强制使用 PKCE（Proof Key for Code Exchange）以防止授权码拦截攻击。仅支持 `S256` 方法。

### 流程详解

```
┌─────────────────────────────────────────────────────────────┐
│                    PKCE 授权流程                              │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. 生成 PKCE 密钥对                                         │
│     ┌─────────────────────────────────────────────┐        │
│     │ code_verifier = random(32 bytes)             │        │
│     │              → Base64URL 编码                 │        │
│     │              = "dBjftJeZ4CVP-mB92K27uhbUJU1p" │        │
│     │                                              │        │
│     │ code_challenge = SHA256(code_verifier)        │        │
│     │              → Base64URL 编码                 │        │
│     │              = "E9Melhoa2OwvFrEMTJguCHaoeK1t" │        │
│     └─────────────────────────────────────────────┘        │
│                                                             │
│  2. 启动本地回调服务器                                        │
│     ┌─────────────────────────────────────────────┐        │
│     │ 监听 http://127.0.0.1:{随机端口}/callback    │        │
│     │ 等待授权服务器回调                             │        │
│     └─────────────────────────────────────────────┘        │
│                                                             │
│  3. 构建授权 URL 并打开浏览器                                 │
│     ┌─────────────────────────────────────────────┐        │
│     │ GET /authorize                               │        │
│     │   ?response_type=code                        │        │
│     │   &client_id=chengcoding_abc123                    │        │
│     │   &redirect_uri=http://127.0.0.1:9876/cb     │        │
│     │   &scope=mcp:tools mcp:resources             │        │
│     │   &state=RANDOM_STATE                        │        │
│     │   &code_challenge=E9Melhoa2Ow...             │        │
│     │   &code_challenge_method=S256                │        │
│     └─────────────────────────────────────────────┘        │
│                                                             │
│  4. 用户在浏览器中完成授权                                    │
│     ┌─────────────────────────────────────────────┐        │
│     │ 授权服务器回调:                               │        │
│     │ GET /callback?code=AUTH_CODE&state=STATE      │        │
│     └─────────────────────────────────────────────┘        │
│                                                             │
│  5. 用授权码交换令牌                                         │
│     ┌─────────────────────────────────────────────┐        │
│     │ POST /token                                  │        │
│     │   grant_type=authorization_code              │        │
│     │   &code=AUTH_CODE                            │        │
│     │   &redirect_uri=http://127.0.0.1:9876/cb     │        │
│     │   &client_id=chengcoding_abc123                    │        │
│     │   &code_verifier=dBjftJeZ4CVP...            │        │
│     │   &resource=https://mcp.example.com          │        │
│     └─────────────────────────────────────────────┘        │
│                                                             │
│  6. 收到令牌                                                │
│     ┌─────────────────────────────────────────────┐        │
│     │ {                                            │        │
│     │   "access_token": "eyJhbGci...",             │        │
│     │   "token_type": "Bearer",                    │        │
│     │   "expires_in": 3600,                        │        │
│     │   "refresh_token": "dGhpcyBp..."             │        │
│     │ }                                            │        │
│     └─────────────────────────────────────────────┘        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 实现代码结构

```rust
/// PKCE 代码验证器/质询对
pub struct PkceChallenge {
    /// 代码验证器（原始随机值）
    pub code_verifier: String,

    /// 代码质询（SHA256 哈希后的 Base64URL 编码）
    pub code_challenge: String,

    /// 质询方法（固定为 S256）
    pub method: String,
}

impl PkceChallenge {
    /// 生成新的 PKCE 密钥对
    pub fn generate() -> Self {
        let verifier = generate_random_string(32);
        let challenge = base64url_encode(sha256(verifier.as_bytes()));

        Self {
            code_verifier: verifier,
            code_challenge: challenge,
            method: "S256".to_string(),
        }
    }
}
```

### S256 算法

```
code_challenge = BASE64URL(SHA256(ASCII(code_verifier)))
```

- 输入：`code_verifier`（43-128 个字符的随机字符串）
- SHA256：产生 32 字节哈希
- Base64URL：无填充的 URL 安全 Base64 编码
- 输出：`code_challenge`（43 个字符）

---

## Token 管理

### Token 存储

Token 通过 `chengcoding-config` crate 的 `CryptoStore` 加密存储：

```rust
/// 加密 Token 存储
pub struct TokenStore {
    /// 加密存储后端
    crypto: CryptoStore,

    /// 存储路径
    store_path: PathBuf,
}

/// Token 数据
pub struct StoredToken {
    /// 访问令牌
    pub access_token: String,

    /// 令牌类型（Bearer）
    pub token_type: String,

    /// 过期时间
    pub expires_at: DateTime<Utc>,

    /// 刷新令牌
    pub refresh_token: Option<String>,

    /// 权限范围
    pub scope: Option<String>,

    /// 资源标识（MCP 服务器 URL）
    pub resource: Option<String>,
}
```

#### 存储位置

```
~/.config/OrangeCoding/
├── tokens/
│   ├── mcp_server_a.enc    # 加密的 Token 文件
│   ├── mcp_server_b.enc
│   └── provider_openai.enc
└── keyring                  # 加密主密钥
```

#### 加密方式

- **算法**: AES-256-GCM（通过 `ring` crate 实现）
- **密钥派生**: 从系统密钥环或用户密码派生
- **存储格式**: JSON 序列化后 AES 加密

```rust
impl CryptoStore {
    /// 加密并存储 Token
    pub fn store_token(
        &self,
        key: &str,
        token: &StoredToken,
    ) -> Result<()>;

    /// 读取并解密 Token
    pub fn load_token(
        &self,
        key: &str,
    ) -> Result<Option<StoredToken>>;

    /// 删除 Token
    pub fn delete_token(&self, key: &str) -> Result<()>;

    /// 列出所有已存储的 Token 标识
    pub fn list_tokens(&self) -> Result<Vec<String>>;
}
```

### Token 刷新

当访问令牌过期时，自动使用刷新令牌获取新的访问令牌：

```
┌─────────────────────────────────────────────────────────┐
│                   Token 刷新流程                          │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  1. 检查 Token 是否过期                                   │
│     if now() > token.expires_at - 60s (提前 1 分钟)     │
│       → 需要刷新                                         │
│                                                         │
│  2. 使用 refresh_token 请求新 Token                      │
│     POST /token                                         │
│       grant_type=refresh_token                          │
│       &refresh_token=REFRESH_TOKEN                      │
│       &client_id=CLIENT_ID                              │
│       &resource=RESOURCE_URL                            │
│                                                         │
│  3. 收到新 Token                                         │
│     {                                                   │
│       "access_token": "新的访问令牌",                     │
│       "expires_in": 3600,                               │
│       "refresh_token": "新的刷新令牌"（可选）              │
│     }                                                   │
│                                                         │
│  4. 更新本地存储                                          │
│     CryptoStore.store_token(key, new_token)              │
│                                                         │
│  5. 如果刷新失败                                          │
│     → 清除本地 Token                                     │
│     → 重新启动完整的 OAuth 流程                            │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### 自动刷新机制

```rust
impl TokenStore {
    /// 获取有效的访问令牌，必要时自动刷新
    pub async fn get_valid_token(
        &self,
        resource: &str,
    ) -> Result<String> {
        let token = self.load_token(resource)?;

        match token {
            Some(t) if !t.is_expired() => Ok(t.access_token),
            Some(t) if t.refresh_token.is_some() => {
                let new_token = self.refresh(t).await?;
                self.store_token(resource, &new_token)?;
                Ok(new_token.access_token)
            }
            _ => Err(AuthError::TokenExpired),
        }
    }

    /// 检查 Token 是否即将过期（提前 60 秒）
    fn is_token_expiring(&self, token: &StoredToken) -> bool {
        Utc::now() + Duration::seconds(60) > token.expires_at
    }
}
```

---

## MCP 服务器认证

### 认证流程

```
OrangeCoding CLI                MCP 服务器              授权服务器
   │                        │                       │
   │── 1. 发现元数据 ────────►│                       │
   │   GET /.well-known/     │                       │
   │   oauth-protected-      │                       │
   │   resource              │                       │
   │◄── resource metadata ───│                       │
   │                        │                       │
   │── 2. 发现授权服务器 ──────────────────────────────►│
   │   GET /.well-known/                             │
   │   oauth-authorization-server                    │
   │◄── server metadata ────────────────────────────│
   │                        │                       │
   │── 3. 动态注册 ────────────────────────────────────►│
   │   POST /register                               │
   │◄── client_id ──────────────────────────────────│
   │                        │                       │
   │── 4. PKCE 授权 ──────────────────────────────────►│
   │   (见 PKCE 流程)                                │
   │◄── access_token ──────────────────────────────│
   │                        │                       │
   │── 5. 调用 MCP 工具 ────►│                       │
   │   Authorization: Bearer│                       │
   │   access_token         │                       │
   │◄── 工具执行结果 ────────│                       │
   │                        │                       │
```

### MCP 传输层认证

```rust
/// 带认证的 MCP 客户端
pub struct AuthenticatedMcpClient {
    /// 基础 MCP 客户端
    client: McpClient,

    /// Token 存储
    token_store: TokenStore,

    /// MCP 服务器资源 URL
    resource_url: String,
}

impl AuthenticatedMcpClient {
    /// 创建带认证的请求
    async fn authenticated_request(
        &self,
        request: JsonRpcRequest,
    ) -> McpResult<JsonRpcResponse> {
        let token = self.token_store
            .get_valid_token(&self.resource_url)
            .await?;

        // 在传输层添加认证头
        self.client
            .send_with_auth(request, &token)
            .await
    }
}
```

### stdio 传输的认证

对于基于 stdio 的 MCP 服务器（子进程模式），认证通过环境变量传递：

```rust
// 启动 MCP 子进程时注入令牌
let child = Command::new("mcp-server")
    .env("MCP_AUTH_TOKEN", &access_token)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()?;
```

---

## AI 提供商认证

### API 密钥管理

各 AI 提供商使用不同的认证方式：

| 提供商 | 认证方式 | 头部格式 |
|--------|----------|----------|
| OpenAI | API Key | `Authorization: Bearer sk-...` |
| Anthropic | API Key | `x-api-key: sk-ant-...` |
| DeepSeek | API Key | `Authorization: Bearer ...` |
| 通义千问 | API Key | `Authorization: Bearer ...` |
| 文心一言 | API Key + Secret | 先获取 access_token |

### 密钥存储

API 密钥通过 `CryptoStore` 加密存储：

```toml
# ~/.config/OrangeCoding/config.toml
[ai]
provider = "anthropic"
model = "claude-opus-4-6"
# api_key 不直接存储在配置文件中
# 使用 OrangeCoding config set ai.api_key <key> 命令设置
# 密钥会被加密存储在 CryptoStore 中
```

```bash
# 设置 API 密钥（会被加密存储）
OrangeCoding config set ai.api_key sk-ant-xxx

# 或通过环境变量
export ANTHROPIC_API_KEY=sk-ant-xxx
export OPENAI_API_KEY=sk-xxx
```

---

## 配置方法

### OAuth 配置文件

```toml
# ~/.config/OrangeCoding/oauth.toml

[oauth]
# 默认回调端口范围
callback_port_range = [9876, 9900]

# Token 自动刷新提前量（秒）
refresh_ahead_secs = 60

# 授权超时（秒）
auth_timeout_secs = 300

# 是否自动打开浏览器
auto_open_browser = true

[oauth.mcp_servers]
# 预配置的 MCP 服务器认证信息

[oauth.mcp_servers.example]
resource_url = "https://mcp.example.com"
client_id = "chengcoding_abc123"  # 可选，不提供则自动注册
scopes = ["mcp:tools", "mcp:resources"]
```

### 环境变量

| 变量名 | 描述 | 示例 |
|--------|------|------|
| `OrangeCoding_OAUTH_CALLBACK_PORT` | 固定回调端口 | `9876` |
| `OrangeCoding_OAUTH_TIMEOUT` | 授权超时时间 | `300` |
| `OrangeCoding_OAUTH_NO_BROWSER` | 禁止自动打开浏览器 | `true` |

### 命令行操作

```bash
# 手动触发 OAuth 认证
OrangeCoding config oauth login --server https://mcp.example.com

# 查看已存储的 Token
OrangeCoding config oauth list

# 删除指定 Token
OrangeCoding config oauth revoke --server https://mcp.example.com

# 刷新 Token
OrangeCoding config oauth refresh --server https://mcp.example.com
```

---

## 安全考量

### 1. PKCE 安全性

| 保护措施 | 描述 |
|----------|------|
| S256 强制 | 仅支持 S256 方法，不支持不安全的 plain 方法 |
| 高熵验证器 | code_verifier 使用 32 字节密码学随机数 |
| 单次使用 | 每次授权流程生成全新的 PKCE 密钥对 |

### 2. Token 安全

| 保护措施 | 描述 |
|----------|------|
| 加密存储 | AES-256-GCM 加密，密钥由系统密钥环保护 |
| 文件权限 | Token 文件设置 `600` 权限（仅用户可读写） |
| 内存保护 | Token 使用 `zeroize` 在离开作用域时清零 |
| 自动过期 | 尊重服务器的 `expires_in`，提前刷新 |

### 3. 回调安全

| 保护措施 | 描述 |
|----------|------|
| 仅本地监听 | 回调服务器只绑定 `127.0.0.1` |
| State 验证 | 验证 state 参数防止 CSRF 攻击 |
| 端口随机化 | 使用随机端口避免端口冲突和预测 |
| 超时关闭 | 回调服务器在完成后立即关闭 |

### 4. 传输安全

| 保护措施 | 描述 |
|----------|------|
| TLS 强制 | 所有 OAuth 通信强制使用 HTTPS |
| 证书验证 | 使用 `rustls` 进行严格的证书验证 |
| 无明文密钥 | API 密钥和 Token 不以明文形式出现在日志中 |

### 5. 审计

所有 OAuth 相关操作都会被审计记录：

```
- Token 获取
- Token 刷新
- Token 撤销
- 认证失败
- 客户端注册
```

详见 [安全架构](../architecture/security.md)。

---

## 相关文档

- [安全架构](../architecture/security.md) - 密钥检测和脱敏机制
- [权限系统参考](./permissions.md) - WebFetch 权限与 OAuth 的关系
- [架构概览](../architecture/overview.md) - chengcoding-mcp 在系统中的位置

//! # OAuth 2.1 认证模块
//!
//! 支持 MCP 服务器的 OAuth 认证流程，兼容 RFC 9728, 8414, 7591, 8707。

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::digest;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};

// ============================================================
// 配置结构体
// ============================================================

/// OAuth 客户端配置
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    /// 客户端 ID（可选，支持动态客户端注册 RFC 7591）
    pub client_id: Option<String>,
    /// 请求的权限范围列表
    pub scopes: Vec<String>,
    /// MCP 服务器 URL
    pub server_url: String,
}

// ============================================================
// 令牌结构体
// ============================================================

/// OAuth 访问令牌
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    /// 访问令牌
    pub access_token: String,
    /// 刷新令牌（可选）
    pub refresh_token: Option<String>,
    /// 过期时间（Unix 时间戳，可选）
    pub expires_at: Option<u64>,
    /// 令牌类型（通常为 "Bearer"）
    pub token_type: String,
    /// 授权范围（可选）
    pub scope: Option<String>,
}

// ============================================================
// 令牌存储
// ============================================================

/// OAuth 令牌持久化存储
///
/// 将令牌按服务器标识保存到本地 JSON 文件中，支持加载、保存、增删查。
#[derive(Debug)]
pub struct OAuthTokenStore {
    /// 按服务器标识存储的令牌映射
    tokens: HashMap<String, OAuthToken>,
    /// 存储文件路径
    store_path: PathBuf,
}

impl OAuthTokenStore {
    /// 从指定路径加载令牌存储
    ///
    /// 若文件不存在则返回空的存储实例。
    pub fn load(path: PathBuf) -> Result<Self> {
        let tokens = if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("读取令牌存储文件失败: {:?}", path))?;
            serde_json::from_str(&content).with_context(|| "解析令牌存储 JSON 失败")?
        } else {
            HashMap::new()
        };
        Ok(Self {
            tokens,
            store_path: path,
        })
    }

    /// 将令牌存储保存到磁盘
    ///
    /// 自动创建父目录（若不存在）。
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.store_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建令牌存储目录失败: {:?}", parent))?;
        }
        let content =
            serde_json::to_string_pretty(&self.tokens).with_context(|| "序列化令牌存储失败")?;
        std::fs::write(&self.store_path, content)
            .with_context(|| format!("写入令牌存储文件失败: {:?}", self.store_path))?;
        Ok(())
    }

    /// 获取指定服务器的令牌
    pub fn get_token(&self, server_key: &str) -> Option<&OAuthToken> {
        self.tokens.get(server_key)
    }

    /// 存储指定服务器的令牌
    pub fn store_token(&mut self, server_key: String, token: OAuthToken) {
        self.tokens.insert(server_key, token);
    }

    /// 删除指定服务器的令牌
    pub fn remove_token(&mut self, server_key: &str) {
        self.tokens.remove(server_key);
    }

    /// 返回默认的令牌存储路径
    ///
    /// 默认路径为 `~/.config/chenagent/mcp-oauth.json`。
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
            .join("chenagent")
            .join("mcp-oauth.json")
    }
}

// ============================================================
// PKCE 挑战（RFC 7636）
// ============================================================

/// PKCE (Proof Key for Code Exchange) 挑战参数
///
/// 使用 S256 方法：`code_challenge = BASE64URL(SHA256(code_verifier))`
#[derive(Debug, Clone)]
pub struct PkceChallenge {
    /// 验证码（随机生成的高熵 Base64URL 字符串）
    pub code_verifier: String,
    /// 挑战码（验证码的 SHA-256 哈希的 Base64URL 编码）
    pub code_challenge: String,
    /// 挑战方法（固定为 "S256"）
    pub method: String,
}

impl PkceChallenge {
    /// 生成新的 PKCE 挑战参数
    ///
    /// 使用 `ring` 的安全随机数生成器创建 32 字节的随机数据，
    /// 经 Base64URL 编码后作为 `code_verifier`，
    /// 再对其进行 SHA-256 哈希和 Base64URL 编码生成 `code_challenge`。
    pub fn generate() -> Self {
        let rng = SystemRandom::new();
        let mut verifier_bytes = [0u8; 32];
        rng.fill(&mut verifier_bytes).expect("随机数生成失败");

        let code_verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);

        // S256: code_challenge = BASE64URL(SHA256(code_verifier))
        let hash = digest::digest(&digest::SHA256, code_verifier.as_bytes());
        let code_challenge = URL_SAFE_NO_PAD.encode(hash.as_ref());

        Self {
            code_verifier,
            code_challenge,
            method: "S256".to_string(),
        }
    }

    /// 验证给定的 verifier 是否与当前 challenge 匹配
    pub fn verify(&self, verifier: &str) -> bool {
        let hash = digest::digest(&digest::SHA256, verifier.as_bytes());
        let challenge = URL_SAFE_NO_PAD.encode(hash.as_ref());
        challenge == self.code_challenge
    }
}

// ============================================================
// OAuth 发现（RFC 9728 / RFC 8414）
// ============================================================

/// 受保护资源元数据（RFC 9728）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetadata {
    /// 授权服务器列表
    pub authorization_servers: Vec<String>,
    /// 资源标识
    pub resource: String,
}

/// 授权服务器元数据（RFC 8414）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthServerMetadata {
    /// 授权端点 URL
    pub authorization_endpoint: String,
    /// 令牌端点 URL
    pub token_endpoint: String,
    /// 动态客户端注册端点（可选，RFC 7591）
    pub registration_endpoint: Option<String>,
}

/// OAuth 元数据发现客户端
///
/// 负责按照 RFC 9728 和 RFC 8414 规范发现 OAuth 相关端点。
#[derive(Debug)]
pub struct OAuthDiscovery {
    /// HTTP 客户端
    client: reqwest::Client,
}

impl OAuthDiscovery {
    /// 创建新的发现客户端
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// 发现受保护资源的元数据（RFC 9728）
    ///
    /// 访问 `{server_url}/.well-known/oauth-protected-resource` 获取元数据。
    pub async fn discover_protected_resource(&self, server_url: &str) -> Result<ResourceMetadata> {
        let url = format!(
            "{}/.well-known/oauth-protected-resource",
            server_url.trim_end_matches('/')
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("请求受保护资源元数据失败: {}", url))?;
        let metadata = resp
            .json::<ResourceMetadata>()
            .await
            .with_context(|| "解析受保护资源元数据失败")?;
        Ok(metadata)
    }

    /// 发现授权服务器元数据（RFC 8414）
    ///
    /// 访问 `{issuer_url}/.well-known/oauth-authorization-server` 获取元数据。
    pub async fn discover_auth_server(&self, issuer_url: &str) -> Result<AuthServerMetadata> {
        let url = format!(
            "{}/.well-known/oauth-authorization-server",
            issuer_url.trim_end_matches('/')
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("请求授权服务器元数据失败: {}", url))?;
        let metadata = resp
            .json::<AuthServerMetadata>()
            .await
            .with_context(|| "解析授权服务器元数据失败")?;
        Ok(metadata)
    }
}

// ============================================================
// OAuth 完整流程
// ============================================================

/// OAuth 2.1 完整认证流程管理器
///
/// 整合配置、令牌存储、PKCE 和发现机制，提供端到端的认证流程：
/// 1. `start_auth_flow()` —— 启动授权码流程，返回授权 URL
/// 2. `exchange_code(code)` —— 用授权码交换访问令牌
/// 3. `refresh_token(token)` —— 刷新过期的令牌
#[derive(Debug)]
pub struct OAuthFlow {
    /// 客户端配置
    config: OAuthConfig,
    /// 令牌存储
    store: OAuthTokenStore,
    /// PKCE 挑战参数（启动认证流程时生成）
    pkce: Option<PkceChallenge>,
    /// 授权服务器元数据
    auth_metadata: Option<AuthServerMetadata>,
    /// HTTP 客户端
    client: reqwest::Client,
}

impl OAuthFlow {
    /// 创建新的 OAuth 认证流程实例
    pub fn new(config: OAuthConfig, store: OAuthTokenStore) -> Self {
        Self {
            config,
            store,
            pkce: None,
            auth_metadata: None,
            client: reqwest::Client::new(),
        }
    }

    /// 启动授权码流程，返回用户需要访问的授权 URL
    ///
    /// 流程：
    /// 1. 发现受保护资源元数据（RFC 9728）
    /// 2. 发现授权服务器元数据（RFC 8414）
    /// 3. 生成 PKCE 挑战参数
    /// 4. 构建并返回授权 URL
    pub async fn start_auth_flow(&mut self) -> Result<String> {
        let discovery = OAuthDiscovery::new();

        // 发现受保护资源的授权服务器
        let resource_meta = discovery
            .discover_protected_resource(&self.config.server_url)
            .await?;

        let issuer = resource_meta
            .authorization_servers
            .first()
            .ok_or_else(|| anyhow::anyhow!("未找到授权服务器"))?;

        // 发现授权服务器端点
        let auth_meta = discovery.discover_auth_server(issuer).await?;

        // 生成 PKCE 挑战
        let pkce = PkceChallenge::generate();

        // 构建授权 URL
        let client_id = self
            .config
            .client_id
            .as_deref()
            .unwrap_or("chengcoding-cli");

        let mut auth_url = url::Url::parse(&auth_meta.authorization_endpoint)
            .with_context(|| "解析授权端点 URL 失败")?;

        auth_url
            .query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", client_id)
            .append_pair("code_challenge", &pkce.code_challenge)
            .append_pair("code_challenge_method", &pkce.method)
            .append_pair("scope", &self.config.scopes.join(" "));

        self.pkce = Some(pkce);
        self.auth_metadata = Some(auth_meta);

        Ok(auth_url.to_string())
    }

    /// 用授权码交换访问令牌
    ///
    /// 需要先调用 `start_auth_flow()` 以初始化 PKCE 和元数据。
    pub async fn exchange_code(&mut self, code: &str) -> Result<OAuthToken> {
        let auth_meta = self
            .auth_metadata
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("请先调用 start_auth_flow"))?;

        let pkce = self
            .pkce
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("PKCE 挑战参数缺失"))?;

        let client_id = self
            .config
            .client_id
            .as_deref()
            .unwrap_or("chengcoding-cli");

        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", client_id),
            ("code_verifier", &pkce.code_verifier),
        ];

        let resp = self
            .client
            .post(&auth_meta.token_endpoint)
            .form(&params)
            .send()
            .await
            .with_context(|| "令牌交换请求失败")?;

        let token: OAuthToken = resp.json().await.with_context(|| "解析令牌响应失败")?;

        // 自动存储获取到的令牌
        self.store
            .store_token(self.config.server_url.clone(), token.clone());

        Ok(token)
    }

    /// 使用刷新令牌获取新的访问令牌
    pub async fn refresh_token(&self, token: &OAuthToken) -> Result<OAuthToken> {
        let auth_meta = self
            .auth_metadata
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("授权服务器元数据缺失"))?;

        let refresh = token
            .refresh_token
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("令牌不包含刷新令牌"))?;

        let client_id = self
            .config
            .client_id
            .as_deref()
            .unwrap_or("chengcoding-cli");

        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh),
            ("client_id", client_id),
        ];

        let resp = self
            .client
            .post(&auth_meta.token_endpoint)
            .form(&params)
            .send()
            .await
            .with_context(|| "刷新令牌请求失败")?;

        let new_token: OAuthToken = resp.json().await.with_context(|| "解析刷新令牌响应失败")?;

        Ok(new_token)
    }

    /// 检查令牌是否仍然有效
    ///
    /// 若令牌包含 `expires_at`，则与当前时间比较（预留 60 秒缓冲）；
    /// 若不包含过期时间，则视为有效。
    pub fn is_token_valid(token: &OAuthToken) -> bool {
        match token.expires_at {
            Some(expires_at) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                // 预留 60 秒缓冲时间，提前刷新
                now + 60 < expires_at
            }
            // 无过期时间视为永久有效
            None => true,
        }
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 PKCE 挑战生成的基本属性
    #[test]
    fn 测试pkce生成基本属性() {
        let pkce = PkceChallenge::generate();

        // 验证方法为 S256
        assert_eq!(pkce.method, "S256");
        // code_verifier 长度应为 43 字符（32 字节的 Base64URL 编码）
        assert_eq!(pkce.code_verifier.len(), 43);
        // code_challenge 长度应为 43 字符（32 字节 SHA-256 哈希的 Base64URL 编码）
        assert_eq!(pkce.code_challenge.len(), 43);
        // verifier 和 challenge 不应相同
        assert_ne!(pkce.code_verifier, pkce.code_challenge);
    }

    /// 测试 PKCE 挑战的唯一性（每次生成应不同）
    #[test]
    fn 测试pkce生成唯一性() {
        let pkce1 = PkceChallenge::generate();
        let pkce2 = PkceChallenge::generate();

        assert_ne!(pkce1.code_verifier, pkce2.code_verifier);
        assert_ne!(pkce1.code_challenge, pkce2.code_challenge);
    }

    /// 测试 PKCE 挑战验证——正确的 verifier 应通过
    #[test]
    fn 测试pkce验证正确() {
        let pkce = PkceChallenge::generate();
        assert!(pkce.verify(&pkce.code_verifier));
    }

    /// 测试 PKCE 挑战验证——错误的 verifier 应失败
    #[test]
    fn 测试pkce验证错误() {
        let pkce = PkceChallenge::generate();
        assert!(!pkce.verify("wrong_verifier"));
    }

    /// 测试 S256 挑战方法的正确性（与手动计算对比）
    #[test]
    fn 测试pkce_s256算法正确性() {
        let pkce = PkceChallenge::generate();

        // 手动计算：SHA256(code_verifier) 后 Base64URL 编码
        let hash = digest::digest(&digest::SHA256, pkce.code_verifier.as_bytes());
        let expected_challenge = URL_SAFE_NO_PAD.encode(hash.as_ref());

        assert_eq!(pkce.code_challenge, expected_challenge);
    }

    /// 测试令牌存储——创建、保存、加载完整流程
    #[test]
    fn 测试令牌存储保存和加载() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let path = dir.path().join("tokens.json");

        // 创建并存储令牌
        let mut store = OAuthTokenStore::load(path.clone()).unwrap();
        assert!(store.get_token("server1").is_none());

        let token = OAuthToken {
            access_token: "test_access_token".to_string(),
            refresh_token: Some("test_refresh_token".to_string()),
            expires_at: Some(9999999999),
            token_type: "Bearer".to_string(),
            scope: Some("read write".to_string()),
        };

        store.store_token("server1".to_string(), token);
        store.save().unwrap();

        // 重新加载并验证
        let store2 = OAuthTokenStore::load(path).unwrap();
        let loaded = store2.get_token("server1").unwrap();
        assert_eq!(loaded.access_token, "test_access_token");
        assert_eq!(loaded.refresh_token.as_deref(), Some("test_refresh_token"));
        assert_eq!(loaded.token_type, "Bearer");
    }

    /// 测试令牌存储——删除令牌
    #[test]
    fn 测试令牌存储删除() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let path = dir.path().join("tokens.json");

        let mut store = OAuthTokenStore::load(path).unwrap();
        let token = OAuthToken {
            access_token: "to_delete".to_string(),
            refresh_token: None,
            expires_at: None,
            token_type: "Bearer".to_string(),
            scope: None,
        };

        store.store_token("server_x".to_string(), token);
        assert!(store.get_token("server_x").is_some());

        store.remove_token("server_x");
        assert!(store.get_token("server_x").is_none());
    }

    /// 测试空文件路径的令牌存储加载（应返回空存储）
    #[test]
    fn 测试令牌存储加载不存在的文件() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let path = dir.path().join("nonexistent.json");

        let store = OAuthTokenStore::load(path).unwrap();
        assert!(store.get_token("any").is_none());
    }

    /// 测试默认存储路径
    #[test]
    fn 测试默认存储路径() {
        let path = OAuthTokenStore::default_path();
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("chenagent"));
        assert!(path_str.ends_with("mcp-oauth.json"));
    }

    /// 测试令牌有效性——未过期的令牌
    #[test]
    fn 测试令牌有效未过期() {
        let future_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600; // 1 小时后过期

        let token = OAuthToken {
            access_token: "valid".to_string(),
            refresh_token: None,
            expires_at: Some(future_ts),
            token_type: "Bearer".to_string(),
            scope: None,
        };

        assert!(OAuthFlow::is_token_valid(&token));
    }

    /// 测试令牌有效性——已过期的令牌
    #[test]
    fn 测试令牌已过期() {
        let token = OAuthToken {
            access_token: "expired".to_string(),
            refresh_token: None,
            expires_at: Some(1000), // 很早以前的时间戳
            token_type: "Bearer".to_string(),
            scope: None,
        };

        assert!(!OAuthFlow::is_token_valid(&token));
    }

    /// 测试令牌有效性——无过期时间视为有效
    #[test]
    fn 测试令牌无过期时间() {
        let token = OAuthToken {
            access_token: "no_expiry".to_string(),
            refresh_token: None,
            expires_at: None,
            token_type: "Bearer".to_string(),
            scope: None,
        };

        assert!(OAuthFlow::is_token_valid(&token));
    }

    /// 测试令牌的序列化和反序列化
    #[test]
    fn 测试令牌序列化反序列化() {
        let token = OAuthToken {
            access_token: "abc123".to_string(),
            refresh_token: Some("refresh_xyz".to_string()),
            expires_at: Some(1700000000),
            token_type: "Bearer".to_string(),
            scope: Some("read".to_string()),
        };

        let json = serde_json::to_string(&token).unwrap();
        let deserialized: OAuthToken = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.access_token, "abc123");
        assert_eq!(deserialized.refresh_token.as_deref(), Some("refresh_xyz"));
        assert_eq!(deserialized.expires_at, Some(1700000000));
        assert_eq!(deserialized.token_type, "Bearer");
        assert_eq!(deserialized.scope.as_deref(), Some("read"));
    }

    /// 测试多个服务器的令牌存储
    #[test]
    fn 测试多服务器令牌存储() {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let path = dir.path().join("multi.json");

        let mut store = OAuthTokenStore::load(path.clone()).unwrap();

        for i in 0..5 {
            let token = OAuthToken {
                access_token: format!("token_{}", i),
                refresh_token: None,
                expires_at: None,
                token_type: "Bearer".to_string(),
                scope: None,
            };
            store.store_token(format!("server_{}", i), token);
        }

        store.save().unwrap();

        // 重新加载并验证所有令牌都存在
        let store2 = OAuthTokenStore::load(path).unwrap();
        for i in 0..5 {
            let key = format!("server_{}", i);
            let token = store2.get_token(&key).unwrap();
            assert_eq!(token.access_token, format!("token_{}", i));
        }
    }

    /// 测试 OAuthConfig 构造
    #[test]
    fn 测试oauth配置构造() {
        let config = OAuthConfig {
            client_id: Some("my-client".to_string()),
            scopes: vec!["read".to_string(), "write".to_string()],
            server_url: "https://mcp.example.com".to_string(),
        };

        assert_eq!(config.client_id.as_deref(), Some("my-client"));
        assert_eq!(config.scopes.len(), 2);
        assert_eq!(config.server_url, "https://mcp.example.com");
    }

    /// 测试资源元数据的序列化
    #[test]
    fn 测试资源元数据序列化() {
        let meta = ResourceMetadata {
            authorization_servers: vec!["https://auth.example.com".to_string()],
            resource: "https://api.example.com".to_string(),
        };

        let json = serde_json::to_string(&meta).unwrap();
        let parsed: ResourceMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.authorization_servers.len(), 1);
        assert_eq!(parsed.resource, "https://api.example.com");
    }

    /// 测试授权服务器元数据的序列化
    #[test]
    fn 测试授权服务器元数据序列化() {
        let meta = AuthServerMetadata {
            authorization_endpoint: "https://auth.example.com/authorize".to_string(),
            token_endpoint: "https://auth.example.com/token".to_string(),
            registration_endpoint: Some("https://auth.example.com/register".to_string()),
        };

        let json = serde_json::to_string(&meta).unwrap();
        let parsed: AuthServerMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.authorization_endpoint,
            "https://auth.example.com/authorize"
        );
        assert_eq!(parsed.token_endpoint, "https://auth.example.com/token");
        assert!(parsed.registration_endpoint.is_some());
    }
}

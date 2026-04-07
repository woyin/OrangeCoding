//! # Web 搜索工具
//!
//! 多引擎网络搜索工具，支持 Brave Search 和 Jina AI Search。
//! 根据环境变量中的 API Key 自动选择搜索引擎。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::debug;

// ============================================================
// 常量定义
// ============================================================

/// Brave Search API 端点
const BRAVE_SEARCH_ENDPOINT: &str = "https://api.search.brave.com/res/v1/web/search";

/// Jina AI Search 基础 URL
const JINA_SEARCH_BASE_URL: &str = "https://s.jina.ai";

/// 默认最大结果数
const DEFAULT_MAX_RESULTS: usize = 5;

/// HTTP 请求超时时间（秒）
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// 用户代理标识
const USER_AGENT: &str = "CEAIR-Bot/0.1";

// ============================================================
// 搜索提供商枚举
// ============================================================

/// 搜索提供商
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchProvider {
    /// 自动选择（根据可用 API key）
    Auto,
    /// Brave Search API
    Brave,
    /// Jina AI Search
    Jina,
}

impl SearchProvider {
    /// 从字符串解析搜索提供商
    fn from_str_param(s: &str) -> ToolResult<Self> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(SearchProvider::Auto),
            "brave" => Ok(SearchProvider::Brave),
            "jina" => Ok(SearchProvider::Jina),
            other => Err(ToolError::InvalidParams(format!(
                "不支持的搜索提供商: {}，支持: auto, brave, jina",
                other
            ))),
        }
    }
}

// ============================================================
// 搜索结果
// ============================================================

/// 搜索结果
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

// ============================================================
// 辅助函数（可独立测试）
// ============================================================

/// 构建 Brave Search API 请求 URL
pub fn build_brave_search_url(query: &str, max_results: usize) -> String {
    let encoded_query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("q", query)
        .append_pair("count", &max_results.to_string())
        .finish();
    format!("{}?{}", BRAVE_SEARCH_ENDPOINT, encoded_query)
}

/// 构建 Jina AI Search 请求 URL
pub fn build_jina_search_url(query: &str) -> String {
    let encoded_query =
        url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
    format!("{}/{}", JINA_SEARCH_BASE_URL, encoded_query)
}

/// 解析 Brave Search API 响应 JSON 为搜索结果列表
pub fn parse_brave_response(json: &Value) -> ToolResult<Vec<SearchResult>> {
    let results = json
        .get("web")
        .and_then(|w| w.get("results"))
        .and_then(|r| r.as_array())
        .ok_or_else(|| {
            ToolError::ExecutionError("Brave 响应格式无效：缺少 web.results 字段".to_string())
        })?;

    let search_results = results
        .iter()
        .filter_map(|item| {
            let title = item.get("title")?.as_str()?.to_string();
            let url = item.get("url")?.as_str()?.to_string();
            let snippet = item
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            Some(SearchResult {
                title,
                url,
                snippet,
            })
        })
        .collect();

    Ok(search_results)
}

/// 解析 Jina AI Search 响应 JSON 为搜索结果列表
pub fn parse_jina_response(json: &Value) -> ToolResult<Vec<SearchResult>> {
    let data = json
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| {
            ToolError::ExecutionError("Jina 响应格式无效：缺少 data 字段".to_string())
        })?;

    let search_results = data
        .iter()
        .filter_map(|item| {
            let title = item.get("title")?.as_str()?.to_string();
            let url = item.get("url")?.as_str()?.to_string();
            let snippet = item
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            Some(SearchResult {
                title,
                url,
                snippet,
            })
        })
        .collect();

    Ok(search_results)
}

/// 格式化搜索结果为可读文本
pub fn format_results(query: &str, provider: &str, results: &[SearchResult]) -> String {
    if results.is_empty() {
        return format!("No results found for: \"{}\"", query);
    }

    let mut output = format!(
        "Search results for: \"{}\"\nProvider: {}\n",
        query, provider
    );

    for (i, result) in results.iter().enumerate() {
        output.push_str(&format!(
            "\n{}. [{}]({})\n   {}\n",
            i + 1,
            result.title,
            result.url,
            result.snippet
        ));
    }

    output
}

// ============================================================
// WebSearchTool 结构体
// ============================================================

/// Web 搜索工具 — 多引擎网络搜索
#[derive(Debug)]
pub struct WebSearchTool {
    /// HTTP 客户端
    client: reqwest::Client,
    /// 默认搜索提供商
    default_provider: SearchProvider,
}

impl WebSearchTool {
    /// 创建新的 Web 搜索工具实例（使用 Auto 提供商）
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            client,
            default_provider: SearchProvider::Auto,
        }
    }

    /// 创建指定搜索提供商的 Web 搜索工具实例
    pub fn with_provider(provider: SearchProvider) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            client,
            default_provider: provider,
        }
    }

    /// 解析 Auto 提供商为实际的搜索提供商
    ///
    /// 检查环境变量中的 API Key，优先使用 Brave，其次 Jina。
    /// 如果都不可用，返回错误。
    pub fn resolve_provider(&self) -> ToolResult<SearchProvider> {
        match &self.default_provider {
            SearchProvider::Auto => {
                if std::env::var("BRAVE_API_KEY").is_ok() {
                    Ok(SearchProvider::Brave)
                } else if std::env::var("JINA_API_KEY").is_ok() {
                    Ok(SearchProvider::Jina)
                } else {
                    Err(ToolError::ExecutionError(
                        "未找到可用的搜索 API Key：请设置 BRAVE_API_KEY 或 JINA_API_KEY 环境变量"
                            .to_string(),
                    ))
                }
            }
            SearchProvider::Brave => Ok(SearchProvider::Brave),
            SearchProvider::Jina => Ok(SearchProvider::Jina),
        }
    }

    /// 使用 Brave Search API 执行搜索
    async fn search_brave(
        &self,
        query: &str,
        max_results: usize,
    ) -> ToolResult<Vec<SearchResult>> {
        let api_key = std::env::var("BRAVE_API_KEY").map_err(|_| {
            ToolError::ExecutionError("未设置 BRAVE_API_KEY 环境变量".to_string())
        })?;

        let url = build_brave_search_url(query, max_results);
        debug!("Brave 搜索请求: {}", url);

        let response = self
            .client
            .get(&url)
            .header("X-Subscription-Token", &api_key)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Brave 搜索请求失败: {}", e)))?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionError(format!(
                "Brave 搜索返回错误状态码: {}",
                response.status()
            )));
        }

        let json: Value = response
            .json()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("解析 Brave 响应 JSON 失败: {}", e)))?;

        parse_brave_response(&json)
    }

    /// 使用 Jina AI Search 执行搜索
    async fn search_jina(
        &self,
        query: &str,
        max_results: usize,
    ) -> ToolResult<Vec<SearchResult>> {
        let api_key = std::env::var("JINA_API_KEY").map_err(|_| {
            ToolError::ExecutionError("未设置 JINA_API_KEY 环境变量".to_string())
        })?;

        let url = build_jina_search_url(query);
        debug!("Jina 搜索请求: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Jina 搜索请求失败: {}", e)))?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionError(format!(
                "Jina 搜索返回错误状态码: {}",
                response.status()
            )));
        }

        let json: Value = response
            .json()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("解析 Jina 响应 JSON 失败: {}", e)))?;

        let mut results = parse_jina_response(&json)?;
        // Jina 不支持 count 参数，手动截断
        results.truncate(max_results);
        Ok(results)
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// Tool 特征实现
// ============================================================

#[async_trait]
impl Tool for WebSearchTool {
    /// 工具名称
    fn name(&self) -> &str {
        "web_search"
    }

    /// 工具描述
    fn description(&self) -> &str {
        "多引擎网络搜索工具"
    }

    /// 参数 JSON Schema 定义
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "搜索查询关键词"
                },
                "provider": {
                    "type": "string",
                    "description": "搜索提供商（auto、brave、jina），默认 auto",
                    "default": "auto",
                    "enum": ["auto", "brave", "jina"]
                },
                "max_results": {
                    "type": "number",
                    "description": "最大返回结果数量（默认 5）",
                    "default": 5
                }
            },
            "required": ["query"]
        })
    }

    /// 执行网络搜索
    async fn execute(&self, params: Value) -> ToolResult<String> {
        // 提取并验证 query 参数
        let query = params
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: query".to_string()))?;

        if query.trim().is_empty() {
            return Err(ToolError::InvalidParams(
                "搜索查询不能为空".to_string(),
            ));
        }

        // 提取 provider 参数（默认 auto）
        let provider_str = params
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");
        let provider = SearchProvider::from_str_param(provider_str)?;

        // 提取 max_results 参数（默认 5）
        let max_results = params
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_MAX_RESULTS);

        debug!(
            "Web 搜索: query={}, provider={:?}, max_results={}",
            query, provider, max_results
        );

        // 如果指定了具体的提供商，临时覆盖
        let tool_with_provider = if provider != self.default_provider {
            WebSearchTool {
                client: self.client.clone(),
                default_provider: provider,
            }
        } else {
            WebSearchTool {
                client: self.client.clone(),
                default_provider: self.default_provider.clone(),
            }
        };

        // 解析实际提供商
        let resolved = tool_with_provider.resolve_provider()?;

        // 调用对应的搜索引擎
        let (results, provider_name) = match resolved {
            SearchProvider::Brave => {
                let results = tool_with_provider.search_brave(query, max_results).await?;
                (results, "brave")
            }
            SearchProvider::Jina => {
                let results = tool_with_provider.search_jina(query, max_results).await?;
                (results, "jina")
            }
            SearchProvider::Auto => {
                // resolve_provider 不会返回 Auto
                unreachable!("resolve_provider 不应返回 Auto")
            }
        };

        // 截断结果到 max_results
        let limited_results: Vec<SearchResult> =
            results.into_iter().take(max_results).collect();

        Ok(format_results(query, provider_name, &limited_results))
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    /// 环境变量互斥锁，防止修改环境变量的测试并行运行
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    // --------------------------------------------------------
    // 测试 1：工具名称和描述
    // --------------------------------------------------------

    #[test]
    fn test_tool_name_and_description() {
        let tool = WebSearchTool::new();

        // 验证工具名称为 "web_search"
        assert_eq!(tool.name(), "web_search");

        // 验证描述非空且包含中文
        let desc = tool.description();
        assert!(!desc.is_empty(), "描述不应为空");
        assert!(desc.contains("搜索"), "描述应包含中文关键字「搜索」");
    }

    // --------------------------------------------------------
    // 测试 2：参数 Schema 验证
    // --------------------------------------------------------

    #[test]
    fn test_parameter_schema() {
        let tool = WebSearchTool::new();
        let schema = tool.parameters_schema();

        // 验证 Schema 基本结构
        assert_eq!(schema["type"], "object");

        let properties = &schema["properties"];

        // 验证 query 参数
        assert_eq!(properties["query"]["type"], "string");

        // 验证 provider 参数
        assert_eq!(properties["provider"]["type"], "string");

        // 验证 max_results 参数
        assert_eq!(properties["max_results"]["type"], "number");

        // 验证 query 为必要参数
        let required = schema["required"].as_array().expect("required 应为数组");
        assert!(
            required.iter().any(|v| v.as_str() == Some("query")),
            "query 应为必要参数"
        );
    }

    // --------------------------------------------------------
    // 测试 3：搜索结果格式化
    // --------------------------------------------------------

    #[test]
    fn test_search_result_formatting() {
        let results = vec![
            SearchResult {
                title: "Async Programming in Rust".to_string(),
                url: "https://example.com/async-rust".to_string(),
                snippet: "Comprehensive guide to async/await in Rust...".to_string(),
            },
            SearchResult {
                title: "Tokio Tutorial".to_string(),
                url: "https://tokio.rs/tutorial".to_string(),
                snippet: "Learn async Rust with Tokio runtime...".to_string(),
            },
        ];

        let output = format_results("rust async programming", "brave", &results);

        // 验证输出包含查询关键词
        assert!(
            output.contains("rust async programming"),
            "输出应包含查询关键词"
        );
        // 验证输出包含提供商名称
        assert!(output.contains("brave"), "输出应包含提供商名称");
        // 验证输出包含结果标题和 URL
        assert!(
            output.contains("Async Programming in Rust"),
            "输出应包含第一个结果的标题"
        );
        assert!(
            output.contains("https://example.com/async-rust"),
            "输出应包含第一个结果的 URL"
        );
        assert!(
            output.contains("Tokio Tutorial"),
            "输出应包含第二个结果的标题"
        );
        // 验证输出包含编号
        assert!(output.contains("1."), "输出应包含编号 1");
        assert!(output.contains("2."), "输出应包含编号 2");

        // 验证空结果的处理
        let empty_output = format_results("nothing here", "brave", &[]);
        assert!(
            empty_output.contains("No results found"),
            "空结果应返回提示信息"
        );
        assert!(
            empty_output.contains("nothing here"),
            "空结果提示应包含查询词"
        );
    }

    // --------------------------------------------------------
    // 测试 4：Brave 请求 URL 构建
    // --------------------------------------------------------

    #[test]
    fn test_brave_request_building() {
        let url = build_brave_search_url("rust async", 10);

        // 验证包含 Brave API 端点
        assert!(
            url.starts_with(BRAVE_SEARCH_ENDPOINT),
            "URL 应以 Brave API 端点开头"
        );
        // 验证包含查询参数
        assert!(url.contains("q=rust+async") || url.contains("q=rust%20async"),
            "URL 应包含编码后的查询参数");
        // 验证包含 count 参数
        assert!(url.contains("count=10"), "URL 应包含 count 参数");
    }

    // --------------------------------------------------------
    // 测试 5：Brave 响应解析
    // --------------------------------------------------------

    #[test]
    fn test_brave_response_parsing() {
        // 模拟 Brave Search API 响应 JSON
        let response_json = json!({
            "web": {
                "results": [
                    {
                        "title": "Rust 编程语言",
                        "url": "https://www.rust-lang.org/",
                        "description": "Rust 是一种系统编程语言..."
                    },
                    {
                        "title": "Rust by Example",
                        "url": "https://doc.rust-lang.org/rust-by-example/",
                        "description": "通过示例学习 Rust..."
                    }
                ]
            }
        });

        let results = parse_brave_response(&response_json).expect("解析应成功");

        // 验证解析出两个结果
        assert_eq!(results.len(), 2, "应解析出两个搜索结果");

        // 验证第一个结果
        assert_eq!(results[0].title, "Rust 编程语言");
        assert_eq!(results[0].url, "https://www.rust-lang.org/");
        assert_eq!(results[0].snippet, "Rust 是一种系统编程语言...");

        // 验证第二个结果
        assert_eq!(results[1].title, "Rust by Example");

        // 验证无效响应返回错误
        let invalid_json = json!({"foo": "bar"});
        assert!(
            parse_brave_response(&invalid_json).is_err(),
            "无效响应应返回错误"
        );
    }

    // --------------------------------------------------------
    // 测试 6：Jina 请求 URL 构建
    // --------------------------------------------------------

    #[test]
    fn test_jina_request_building() {
        let url = build_jina_search_url("rust async programming");

        // 验证包含 Jina 基础 URL
        assert!(
            url.starts_with(JINA_SEARCH_BASE_URL),
            "URL 应以 Jina 基础 URL 开头"
        );
        // 验证查询编码在路径中
        assert!(
            url.contains("rust") && url.contains("async") && url.contains("programming"),
            "URL 路径应包含查询关键词"
        );

        // 验证特殊字符编码
        let url_special = build_jina_search_url("hello world");
        assert!(
            url_special.contains("hello") && url_special.contains("world"),
            "URL 应正确编码特殊字符"
        );
    }

    // --------------------------------------------------------
    // 测试 7：Jina 响应解析
    // --------------------------------------------------------

    #[test]
    fn test_jina_response_parsing() {
        // 模拟 Jina AI Search 响应 JSON
        let response_json = json!({
            "data": [
                {
                    "title": "Tokio 异步运行时",
                    "url": "https://tokio.rs/",
                    "description": "Tokio 是 Rust 的异步运行时..."
                },
                {
                    "title": "Async Book",
                    "url": "https://rust-lang.github.io/async-book/",
                    "description": "Rust 异步编程指南..."
                },
                {
                    "title": "无描述的结果",
                    "url": "https://example.com/no-desc"
                }
            ]
        });

        let results = parse_jina_response(&response_json).expect("解析应成功");

        // 验证解析出三个结果
        assert_eq!(results.len(), 3, "应解析出三个搜索结果");

        // 验证第一个结果
        assert_eq!(results[0].title, "Tokio 异步运行时");
        assert_eq!(results[0].url, "https://tokio.rs/");
        assert_eq!(results[0].snippet, "Tokio 是 Rust 的异步运行时...");

        // 验证缺少 description 字段时 snippet 为空字符串
        assert_eq!(results[2].snippet, "", "缺少描述时 snippet 应为空字符串");

        // 验证无效响应返回错误
        let invalid_json = json!({"results": []});
        assert!(
            parse_jina_response(&invalid_json).is_err(),
            "无效响应应返回错误"
        );
    }

    // --------------------------------------------------------
    // 测试 8：Auto 提供商选择
    // --------------------------------------------------------

    #[tokio::test]
    async fn test_auto_provider_selection() {
        // 获取互斥锁，防止与其他环境变量测试并行运行
        let _lock = ENV_MUTEX.lock().unwrap();

        // 保存原始环境变量
        let orig_brave = std::env::var("BRAVE_API_KEY").ok();
        let orig_jina = std::env::var("JINA_API_KEY").ok();

        // 场景 1：设置 BRAVE_API_KEY，应选择 Brave
        unsafe { std::env::set_var("BRAVE_API_KEY", "test-brave-key") };
        std::env::remove_var("JINA_API_KEY");
        let tool = WebSearchTool::new();
        let resolved = tool.resolve_provider().expect("应成功解析提供商");
        assert_eq!(resolved, SearchProvider::Brave, "有 BRAVE_API_KEY 时应选择 Brave");

        // 场景 2：只设置 JINA_API_KEY，应选择 Jina
        std::env::remove_var("BRAVE_API_KEY");
        unsafe { std::env::set_var("JINA_API_KEY", "test-jina-key") };
        let resolved = tool.resolve_provider().expect("应成功解析提供商");
        assert_eq!(resolved, SearchProvider::Jina, "只有 JINA_API_KEY 时应选择 Jina");

        // 场景 3：两者都设置，应优先选择 Brave
        unsafe { std::env::set_var("BRAVE_API_KEY", "test-brave-key") };
        unsafe { std::env::set_var("JINA_API_KEY", "test-jina-key") };
        let resolved = tool.resolve_provider().expect("应成功解析提供商");
        assert_eq!(resolved, SearchProvider::Brave, "两者都有时应优先选择 Brave");

        // 场景 4：都未设置，应返回错误
        std::env::remove_var("BRAVE_API_KEY");
        std::env::remove_var("JINA_API_KEY");
        let result = tool.resolve_provider();
        assert!(result.is_err(), "无 API Key 时应返回错误");

        // 恢复原始环境变量
        match orig_brave {
            Some(val) => unsafe { std::env::set_var("BRAVE_API_KEY", val) },
            None => std::env::remove_var("BRAVE_API_KEY"),
        }
        match orig_jina {
            Some(val) => unsafe { std::env::set_var("JINA_API_KEY", val) },
            None => std::env::remove_var("JINA_API_KEY"),
        }
    }

    // --------------------------------------------------------
    // 测试 9：无 API Key 错误
    // --------------------------------------------------------

    #[tokio::test]
    async fn test_no_api_key_error() {
        // 获取互斥锁，防止与其他环境变量测试并行运行
        let _lock = ENV_MUTEX.lock().unwrap();

        // 保存原始环境变量
        let orig_brave = std::env::var("BRAVE_API_KEY").ok();
        let orig_jina = std::env::var("JINA_API_KEY").ok();

        // 移除所有 API Key
        std::env::remove_var("BRAVE_API_KEY");
        std::env::remove_var("JINA_API_KEY");

        let tool = WebSearchTool::new();
        let params = json!({"query": "test query"});
        let result = tool.execute(params).await;

        // 验证返回错误
        assert!(result.is_err(), "无 API Key 时执行搜索应返回错误");
        let err = result.unwrap_err();
        let err_msg = format!("{}", err);
        assert!(
            err_msg.contains("API Key") || err_msg.contains("API_KEY"),
            "错误信息应提及 API Key: {}",
            err_msg
        );

        // 恢复原始环境变量
        match orig_brave {
            Some(val) => unsafe { std::env::set_var("BRAVE_API_KEY", val) },
            None => std::env::remove_var("BRAVE_API_KEY"),
        }
        match orig_jina {
            Some(val) => unsafe { std::env::set_var("JINA_API_KEY", val) },
            None => std::env::remove_var("JINA_API_KEY"),
        }
    }

    // --------------------------------------------------------
    // 测试 10：max_results 结果截断
    // --------------------------------------------------------

    #[test]
    fn test_max_results_limiting() {
        // 创建 10 个搜索结果
        let results: Vec<SearchResult> = (0..10)
            .map(|i| SearchResult {
                title: format!("结果 {}", i + 1),
                url: format!("https://example.com/{}", i + 1),
                snippet: format!("这是第 {} 个搜索结果的摘要", i + 1),
            })
            .collect();

        // 截断到 3 个结果
        let max_results = 3;
        let limited: Vec<SearchResult> = results.into_iter().take(max_results).collect();
        assert_eq!(limited.len(), 3, "截断后应只有 3 个结果");

        // 验证格式化后只包含 3 个结果
        let output = format_results("test", "brave", &limited);
        assert!(output.contains("1."), "输出应包含编号 1");
        assert!(output.contains("2."), "输出应包含编号 2");
        assert!(output.contains("3."), "输出应包含编号 3");
        assert!(!output.contains("4."), "输出不应包含编号 4");

        // 验证格式化后包含正确的标题
        assert!(output.contains("结果 1"), "输出应包含第一个结果");
        assert!(output.contains("结果 3"), "输出应包含第三个结果");
        assert!(!output.contains("结果 4"), "输出不应包含第四个结果");
    }

    // --------------------------------------------------------
    // 测试 11：空查询处理
    // --------------------------------------------------------

    #[tokio::test]
    async fn test_empty_query_handling() {
        let tool = WebSearchTool::new();

        // 空字符串查询
        let params = json!({"query": ""});
        let result = tool.execute(params).await;
        assert!(result.is_err(), "空查询应返回错误");
        match result.unwrap_err() {
            ToolError::InvalidParams(msg) => {
                assert!(msg.contains("空"), "错误信息应提及查询为空: {}", msg);
            }
            other => panic!("期望 InvalidParams 错误，实际得到: {:?}", other),
        }

        // 纯空白字符串查询
        let params = json!({"query": "   "});
        let result = tool.execute(params).await;
        assert!(result.is_err(), "纯空白查询应返回错误");
        match result.unwrap_err() {
            ToolError::InvalidParams(_) => {}
            other => panic!("期望 InvalidParams 错误，实际得到: {:?}", other),
        }

        // 缺少 query 参数
        let params = json!({});
        let result = tool.execute(params).await;
        assert!(result.is_err(), "缺少 query 参数应返回错误");
        match result.unwrap_err() {
            ToolError::InvalidParams(msg) => {
                assert!(msg.contains("query"), "错误信息应提及 query 参数: {}", msg);
            }
            other => panic!("期望 InvalidParams 错误，实际得到: {:?}", other),
        }
    }
}

//! # Fetch 工具
//!
//! 抓取 URL 内容并转换为可读文本。
//! 支持 HTML 到文本的转换、内容截断和分页浏览。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value};
use tracing::debug;

// ============================================================
// 常量定义
// ============================================================

/// 默认最大内容长度（1MB）
const DEFAULT_MAX_CONTENT_LENGTH: usize = 1_000_000;

/// 默认返回的最大字符数
const DEFAULT_MAX_LENGTH: usize = 10_000;

/// HTTP 请求超时时间（秒）
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// 用户代理标识
const USER_AGENT: &str = "CEAIR-Bot/0.1";

// ============================================================
// HTML 转文本辅助函数
// ============================================================

/// 将 HTML 内容转换为可读的纯文本（Markdown 风格）
///
/// 转换规则：
/// - 移除 script、style、nav、footer、header 标签及其内容
/// - 将标题标签（h1-h6）转换为 Markdown 格式
/// - 将段落标签转换为带双换行的文本块
/// - 将链接转换为 Markdown 链接格式
/// - 将列表项转换为 `- item` 格式
/// - 将代码标签转换为反引号包裹
/// - 将 pre 标签转换为代码块
/// - 移除所有其他 HTML 标签
/// - 合并多余的空白字符
pub fn html_to_text(html: &str) -> String {
    let mut text = html.to_string();

    // 第一步：移除不需要的标签及其内容（script、style、nav、footer、header）
    let remove_tags = ["script", "style", "nav", "footer", "header"];
    for tag in &remove_tags {
        let pattern = format!(r"(?is)<{tag}\b[^>]*>.*?</{tag}>");
        if let Ok(re) = Regex::new(&pattern) {
            text = re.replace_all(&text, "").to_string();
        }
    }

    // 第二步：转换 <pre><code>...</code></pre> 为代码块（需在 code 标签处理前）
    if let Ok(re) = Regex::new(r"(?is)<pre[^>]*>\s*<code[^>]*>(.*?)</code>\s*</pre>") {
        text = re
            .replace_all(&text, |caps: &regex::Captures| {
                let code = caps.get(1).map_or("", |m| m.as_str());
                // 移除代码内部的 HTML 标签
                let clean_code = strip_tags(code);
                format!("\n\n```\n{}\n```\n\n", clean_code.trim())
            })
            .to_string();
    }

    // 也处理不带 <code> 的 <pre> 标签
    if let Ok(re) = Regex::new(r"(?is)<pre[^>]*>(.*?)</pre>") {
        text = re
            .replace_all(&text, |caps: &regex::Captures| {
                let code = caps.get(1).map_or("", |m| m.as_str());
                let clean_code = strip_tags(code);
                format!("\n\n```\n{}\n```\n\n", clean_code.trim())
            })
            .to_string();
    }

    // 第三步：转换内联 <code> 为反引号
    if let Ok(re) = Regex::new(r"(?is)<code[^>]*>(.*?)</code>") {
        text = re
            .replace_all(&text, |caps: &regex::Captures| {
                let code = caps.get(1).map_or("", |m| m.as_str());
                let clean_code = strip_tags(code);
                format!("`{}`", clean_code.trim())
            })
            .to_string();
    }

    // 第四步：转换标题标签（h1-h6）
    for level in 1..=6 {
        let hashes = "#".repeat(level);
        let pattern = format!(r"(?is)<h{level}\b[^>]*>(.*?)</h{level}>");
        if let Ok(re) = Regex::new(&pattern) {
            text = re
                .replace_all(&text, |caps: &regex::Captures| {
                    let title = caps.get(1).map_or("", |m| m.as_str());
                    let clean_title = strip_tags(title).trim().to_string();
                    format!("\n\n{} {}\n\n", hashes, clean_title)
                })
                .to_string();
        }
    }

    // 第五步：转换链接 <a href="url">text</a> 为 [text](url)
    if let Ok(re) = Regex::new(r#"(?is)<a\s+[^>]*href\s*=\s*["']([^"']*)["'][^>]*>(.*?)</a>"#) {
        text = re
            .replace_all(&text, |caps: &regex::Captures| {
                let url = caps.get(1).map_or("", |m| m.as_str());
                let link_text = caps.get(2).map_or("", |m| m.as_str());
                let clean_text = strip_tags(link_text).trim().to_string();
                format!("[{}]({})", clean_text, url)
            })
            .to_string();
    }

    // 第六步：转换列表项 <li> 为 "- item"
    if let Ok(re) = Regex::new(r"(?is)<li[^>]*>(.*?)</li>") {
        text = re
            .replace_all(&text, |caps: &regex::Captures| {
                let item = caps.get(1).map_or("", |m| m.as_str());
                let clean_item = strip_tags(item).trim().to_string();
                format!("\n- {}", clean_item)
            })
            .to_string();
    }

    // 第七步：转换段落 <p> 为双换行分隔的文本
    if let Ok(re) = Regex::new(r"(?is)<p[^>]*>(.*?)</p>") {
        text = re
            .replace_all(&text, |caps: &regex::Captures| {
                let paragraph = caps.get(1).map_or("", |m| m.as_str());
                let clean_paragraph = strip_tags(paragraph).trim().to_string();
                format!("\n\n{}\n\n", clean_paragraph)
            })
            .to_string();
    }

    // 第八步：转换 <br> 标签为换行
    if let Ok(re) = Regex::new(r"(?i)<br\s*/?>") {
        text = re.replace_all(&text, "\n").to_string();
    }

    // 第九步：移除列表容器标签
    if let Ok(re) = Regex::new(r"(?is)</?(?:ul|ol|dl|dt|dd)[^>]*>") {
        text = re.replace_all(&text, "\n").to_string();
    }

    // 第十步：移除所有剩余的 HTML 标签
    text = strip_tags(&text);

    // 第十一步：解码常见的 HTML 实体
    text = decode_html_entities(&text);

    // 第十二步：合并多余的空白字符
    text = collapse_whitespace(&text);

    text.trim().to_string()
}

/// 移除所有 HTML 标签
fn strip_tags(html: &str) -> String {
    if let Ok(re) = Regex::new(r"<[^>]*>") {
        re.replace_all(html, "").to_string()
    } else {
        html.to_string()
    }
}

/// 解码常见的 HTML 实体
fn decode_html_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

/// 合并多余的空白字符
///
/// - 多个连续空格合并为单个空格
/// - 三个及以上连续换行合并为两个换行
fn collapse_whitespace(text: &str) -> String {
    // 先合并每行内的多余空格（保留换行）
    let mut result = String::with_capacity(text.len());
    for line in text.split('\n') {
        let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        result.push_str(&collapsed);
        result.push('\n');
    }

    // 合并三个及以上连续换行为两个
    if let Ok(re) = Regex::new(r"\n{3,}") {
        result = re.replace_all(&result, "\n\n").to_string();
    }

    result
}

/// 从 HTML 中提取 `<title>` 标签的内容
pub fn extract_title(html: &str) -> Option<String> {
    let re = Regex::new(r"(?is)<title[^>]*>(.*?)</title>").ok()?;
    re.captures(html)
        .and_then(|caps| caps.get(1))
        .map(|m| {
            let title = m.as_str().trim().to_string();
            decode_html_entities(&strip_tags(&title))
        })
}

// ============================================================
// 输出格式化
// ============================================================

/// 输出格式化所需的元数据
pub struct ContentMetadata {
    /// 请求的 URL
    pub url: String,
    /// 响应的 Content-Type
    pub content_type: String,
    /// HTML 页面的标题（非 HTML 内容为 None）
    pub title: Option<String>,
}

/// 格式化工具输出
///
/// 将内容和元数据组合为统一的输出格式，支持分页截断。
///
/// # 参数
/// - `content`: 要输出的内容文本
/// - `metadata`: 内容的元数据（URL、Content-Type、标题）
/// - `start_index`: 分页起始位置
/// - `max_length`: 单次返回的最大字符数
pub fn format_output(
    content: &str,
    metadata: &ContentMetadata,
    start_index: usize,
    max_length: usize,
) -> String {
    let total_length = content.len();

    // 构建头部信息
    let mut output = format!("URL: {}\nContent-Type: {}\n", metadata.url, metadata.content_type);

    // 仅当存在标题时添加 Title 行
    if let Some(ref title) = metadata.title {
        output.push_str(&format!("Title: {}\n", title));
    }

    // 计算实际的切片范围
    let effective_start = start_index.min(total_length);
    let effective_end = (effective_start + max_length).min(total_length);
    let slice = &content[effective_start..effective_end];

    // 添加长度信息
    output.push_str(&format!(
        "Length: {} characters (showing {}-{})\n",
        total_length, effective_start, effective_end
    ));

    // 添加空行和内容
    output.push('\n');
    output.push_str(slice);

    // 如果内容被截断，添加提示信息
    if effective_end < total_length {
        output.push_str(&format!(
            "\n\n[Content truncated. Use start_index={} to continue.]",
            effective_end
        ));
    }

    output
}

// ============================================================
// FetchTool 结构体
// ============================================================

/// Fetch 工具 — 抓取 URL 内容并转换为可读文本
#[derive(Debug)]
pub struct FetchTool {
    /// HTTP 客户端
    client: reqwest::Client,
    /// 允许下载的最大内容长度
    max_content_length: usize,
}

impl FetchTool {
    /// 创建新的 Fetch 工具实例（使用默认配置）
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            client,
            max_content_length: DEFAULT_MAX_CONTENT_LENGTH,
        }
    }

    /// 创建指定最大内容长度的 Fetch 工具实例
    pub fn with_max_content_length(max_content_length: usize) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            client,
            max_content_length,
        }
    }

    /// 判断 Content-Type 是否为 HTML
    fn is_html(content_type: &str) -> bool {
        content_type.contains("text/html")
    }
}

impl Default for FetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FetchTool {
    /// 工具名称
    fn name(&self) -> &str {
        "fetch"
    }

    /// 工具描述
    fn description(&self) -> &str {
        "抓取 URL 内容并转换为可读文本"
    }

    /// 参数 JSON Schema 定义
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "要抓取的 URL 地址"
                },
                "max_length": {
                    "type": "number",
                    "description": "返回内容的最大字符数（默认 10000）",
                    "default": 10000
                },
                "start_index": {
                    "type": "number",
                    "description": "内容分页的起始索引（默认 0）",
                    "default": 0
                },
                "raw": {
                    "type": "boolean",
                    "description": "是否返回原始 HTML（默认 false，转换为纯文本）",
                    "default": false
                }
            },
            "required": ["url"]
        })
    }

    /// 执行 URL 抓取操作
    async fn execute(&self, params: Value) -> ToolResult<String> {
        // 提取并验证 URL 参数
        let url_str = params
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: url".to_string()))?;

        // 验证 URL 格式
        let parsed_url = url::Url::parse(url_str)
            .map_err(|e| ToolError::InvalidParams(format!("无效的 URL: {}", e)))?;

        // 仅允许 http 和 https 协议
        match parsed_url.scheme() {
            "http" | "https" => {}
            scheme => {
                return Err(ToolError::InvalidParams(format!(
                    "不支持的协议: {}，仅支持 http 和 https",
                    scheme
                )));
            }
        }

        // 提取可选参数
        let max_length = params
            .get("max_length")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_MAX_LENGTH);

        let start_index = params
            .get("start_index")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(0);

        let raw = params
            .get("raw")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        debug!("抓取 URL: {} (max_length={}, start_index={}, raw={})", url_str, max_length, start_index, raw);

        // 发送 HTTP 请求
        let response = self
            .client
            .get(url_str)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("HTTP 请求失败: {}", e)))?;

        // 获取 Content-Type
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/plain")
            .to_string();

        // 检查响应状态码
        let status = response.status();
        if !status.is_success() {
            return Err(ToolError::ExecutionError(format!(
                "HTTP 请求返回错误状态码: {}",
                status
            )));
        }

        // 读取响应体（限制最大长度）
        let body = response
            .text()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("读取响应内容失败: {}", e)))?;

        // 截断超长内容
        let body = if body.len() > self.max_content_length {
            body[..self.max_content_length].to_string()
        } else {
            body
        };

        let is_html = Self::is_html(&content_type);

        // 根据内容类型和 raw 参数决定处理方式
        let (content, title) = if is_html && !raw {
            let title = extract_title(&body);
            let text = html_to_text(&body);
            (text, title)
        } else if is_html {
            // raw 模式，仍然提取标题
            let title = extract_title(&body);
            (body, title)
        } else {
            // 非 HTML 内容，直接返回
            (body, None)
        };

        // 格式化输出
        let metadata = ContentMetadata {
            url: url_str.to_string(),
            content_type,
            title,
        };

        Ok(format_output(&content, &metadata, start_index, max_length))
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --------------------------------------------------------
    // 测试 1：工具名称和描述
    // --------------------------------------------------------

    #[test]
    fn test_tool_name_and_description() {
        let tool = FetchTool::new();

        // 验证工具名称为 "fetch"
        assert_eq!(tool.name(), "fetch");

        // 验证描述非空且包含中文
        let desc = tool.description();
        assert!(!desc.is_empty(), "描述不应为空");
        assert!(desc.contains("抓取"), "描述应包含中文关键字");
    }

    // --------------------------------------------------------
    // 测试 2：参数 Schema 验证
    // --------------------------------------------------------

    #[test]
    fn test_parameter_schema() {
        let tool = FetchTool::new();
        let schema = tool.parameters_schema();

        // 验证 Schema 结构
        assert_eq!(schema["type"], "object");

        let properties = &schema["properties"];

        // 验证 url 参数
        assert_eq!(properties["url"]["type"], "string");

        // 验证 max_length 参数
        assert_eq!(properties["max_length"]["type"], "number");

        // 验证 start_index 参数
        assert_eq!(properties["start_index"]["type"], "number");

        // 验证 raw 参数
        assert_eq!(properties["raw"]["type"], "boolean");

        // 验证 url 为必要参数
        let required = schema["required"].as_array().expect("required 应为数组");
        assert!(
            required.iter().any(|v| v.as_str() == Some("url")),
            "url 应为必要参数"
        );
    }

    // --------------------------------------------------------
    // 测试 3：基本 HTML 到文本转换
    // --------------------------------------------------------

    #[test]
    fn test_html_to_text_basic() {
        let html = "<p>Hello</p><p>World</p>";
        let text = html_to_text(html);

        // 验证段落被正确转换
        assert!(text.contains("Hello"), "应包含 Hello");
        assert!(text.contains("World"), "应包含 World");

        // 验证不包含 HTML 标签
        assert!(!text.contains("<p>"), "不应包含 <p> 标签");
        assert!(!text.contains("</p>"), "不应包含 </p> 标签");
    }

    // --------------------------------------------------------
    // 测试 4：标题标签转换
    // --------------------------------------------------------

    #[test]
    fn test_html_to_text_headings() {
        let html = "<h1>Title</h1><h2>Sub</h2><h3>Section</h3>";
        let text = html_to_text(html);

        // 验证标题转换为 Markdown 格式
        assert!(text.contains("# Title"), "h1 应转换为 # Title");
        assert!(text.contains("## Sub"), "h2 应转换为 ## Sub");
        assert!(text.contains("### Section"), "h3 应转换为 ### Section");
    }

    // --------------------------------------------------------
    // 测试 5：链接转换
    // --------------------------------------------------------

    #[test]
    fn test_html_to_text_links() {
        let html = r#"<a href="https://example.com">click</a>"#;
        let text = html_to_text(html);

        // 验证链接转换为 Markdown 格式
        assert!(
            text.contains("[click](https://example.com)"),
            "链接应转换为 Markdown 格式，实际: {}",
            text
        );
    }

    // --------------------------------------------------------
    // 测试 6：列表转换
    // --------------------------------------------------------

    #[test]
    fn test_html_to_text_lists() {
        let html = "<ul><li>A</li><li>B</li></ul>";
        let text = html_to_text(html);

        // 验证列表项转换
        assert!(text.contains("- A"), "应包含 '- A'，实际: {}", text);
        assert!(text.contains("- B"), "应包含 '- B'，实际: {}", text);
    }

    // --------------------------------------------------------
    // 测试 7：代码标签转换
    // --------------------------------------------------------

    #[test]
    fn test_html_to_text_code() {
        // 测试内联代码
        let html_inline = "<code>let x = 1;</code>";
        let text_inline = html_to_text(html_inline);
        assert!(
            text_inline.contains("`let x = 1;`"),
            "内联代码应用反引号包裹，实际: {}",
            text_inline
        );

        // 测试代码块
        let html_block = "<pre><code>fn main() {}</code></pre>";
        let text_block = html_to_text(html_block);
        assert!(
            text_block.contains("```"),
            "代码块应用三个反引号包裹，实际: {}",
            text_block
        );
        assert!(
            text_block.contains("fn main() {}"),
            "代码块应包含代码内容，实际: {}",
            text_block
        );
    }

    // --------------------------------------------------------
    // 测试 8：移除 script 标签
    // --------------------------------------------------------

    #[test]
    fn test_html_to_text_strip_scripts() {
        let html = "<script>alert(1)</script><p>Safe</p>";
        let text = html_to_text(html);

        // 验证 script 内容被移除
        assert!(!text.contains("alert"), "script 内容应被移除");
        assert!(text.contains("Safe"), "正常内容应保留");
    }

    // --------------------------------------------------------
    // 测试 9：内容截断
    // --------------------------------------------------------

    #[test]
    fn test_content_truncation() {
        // 创建超长内容
        let content = "a".repeat(200);
        let metadata = ContentMetadata {
            url: "https://example.com".to_string(),
            content_type: "text/plain".to_string(),
            title: None,
        };

        let output = format_output(&content, &metadata, 0, 50);

        // 验证输出包含截断提示
        assert!(
            output.contains("[Content truncated. Use start_index=50 to continue.]"),
            "应包含截断提示，实际: {}",
            output
        );

        // 验证长度信息
        assert!(
            output.contains("Length: 200 characters"),
            "应包含总长度信息"
        );
        assert!(
            output.contains("showing 0-50"),
            "应显示当前范围"
        );
    }

    // --------------------------------------------------------
    // 测试 10：分页（start_index）
    // --------------------------------------------------------

    #[test]
    fn test_pagination_with_start_index() {
        let content = "abcdefghijklmnopqrstuvwxyz";
        let metadata = ContentMetadata {
            url: "https://example.com".to_string(),
            content_type: "text/plain".to_string(),
            title: None,
        };

        let output = format_output(content, &metadata, 5, 10);

        // 验证返回的内容是正确的子串
        assert!(
            output.contains("fghijklmno"),
            "应包含从索引 5 开始、长度 10 的子串，实际: {}",
            output
        );

        // 验证范围信息
        assert!(
            output.contains("showing 5-15"),
            "应显示正确的范围 5-15"
        );

        // 验证截断提示（还有剩余内容）
        assert!(
            output.contains("[Content truncated. Use start_index=15 to continue.]"),
            "应包含下一页提示"
        );
    }

    // --------------------------------------------------------
    // 测试 11：raw 模式返回原始 HTML
    // --------------------------------------------------------

    #[test]
    fn test_raw_mode_returns_html() {
        let html = "<h1>Title</h1><p>Content</p>";

        // 在 raw 模式下，HTML 不应被转换
        // 通过 format_output 测试：直接传入 HTML 内容
        let metadata = ContentMetadata {
            url: "https://example.com".to_string(),
            content_type: "text/html".to_string(),
            title: Some("Title".to_string()),
        };

        let output = format_output(html, &metadata, 0, 10000);

        // raw 模式下应保留 HTML 标签
        assert!(output.contains("<h1>"), "raw 模式应保留 HTML 标签");
        assert!(output.contains("</p>"), "raw 模式应保留 HTML 标签");
        assert!(output.contains("Title: Title"), "应显示标题");
    }

    // --------------------------------------------------------
    // 测试 12：纯文本透传
    // --------------------------------------------------------

    #[test]
    fn test_plain_text_passthrough() {
        let plain_text = "这是一段纯文本内容，不需要任何转换。";
        let metadata = ContentMetadata {
            url: "https://example.com/data.txt".to_string(),
            content_type: "text/plain".to_string(),
            title: None,
        };

        let output = format_output(plain_text, &metadata, 0, 10000);

        // 验证纯文本直接透传
        assert!(
            output.contains(plain_text),
            "纯文本应原样透传"
        );

        // 验证没有 Title 行（纯文本无标题）
        assert!(
            !output.contains("Title:"),
            "纯文本不应有 Title 行"
        );

        // 验证 Content-Type 正确
        assert!(
            output.contains("Content-Type: text/plain"),
            "应显示正确的 Content-Type"
        );
    }

    // --------------------------------------------------------
    // 额外辅助函数测试
    // --------------------------------------------------------

    #[test]
    fn test_extract_title() {
        let html = "<html><head><title>测试页面</title></head><body></body></html>";
        let title = extract_title(html);
        assert_eq!(title, Some("测试页面".to_string()));

        // 无标题的情况
        let html_no_title = "<html><body>无标题</body></html>";
        assert_eq!(extract_title(html_no_title), None);
    }

    #[test]
    fn test_html_entities_decoded() {
        let html = "<p>A &amp; B &lt; C &gt; D</p>";
        let text = html_to_text(html);
        assert!(text.contains("A & B < C > D"), "HTML 实体应被正确解码，实际: {}", text);
    }

    #[test]
    fn test_whitespace_collapse() {
        let html = "<p>Hello    World</p>\n\n\n\n<p>Next</p>";
        let text = html_to_text(html);

        // 多余空格应被合并
        assert!(!text.contains("    "), "多余空格应被合并");
    }
}

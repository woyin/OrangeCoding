//! SSE（Server-Sent Events）流式事件解析模块
//!
//! 本模块提供了对 SSE 协议的解析支持，以及将流式增量
//! 聚合为完整响应的功能。

use crate::provider::{AiResponse, FunctionCall, StreamEvent, TokenUsage, ToolCall};
use crate::{AiError, AiResult};

// ============================================================
// SSE 事件解析器
// ============================================================

/// SSE 事件结构体，表示一个完整的 SSE 事件
#[derive(Debug, Clone, Default)]
pub struct SseEvent {
    /// 事件类型（可选）
    pub event_type: Option<String>,
    /// 事件数据
    pub data: String,
    /// 事件 ID（可选）
    pub id: Option<String>,
}

/// SSE 流式事件解析器
///
/// 遵循 SSE 协议规范解析字节流中的事件数据：
/// - `data: {...}\n\n` — 普通数据事件
/// - `data: [DONE]\n\n` — 流结束标记
/// - `event: xxx\n` — 事件类型声明
/// - `: xxx\n` — 注释行（忽略）
pub struct SseParser {
    /// 缓冲区，用于存储不完整的行
    buffer: String,
    /// 当前正在构建的事件
    current_event: SseEvent,
    /// 标记流是否已结束
    done: bool,
}

impl SseParser {
    /// 创建新的 SSE 解析器
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            current_event: SseEvent::default(),
            done: false,
        }
    }

    /// 判断流是否已结束
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// 向解析器输入一块数据，返回解析出的事件列表
    ///
    /// # 参数
    /// - `chunk`: 从网络接收到的数据块
    ///
    /// # 返回
    /// 本次数据块中解析出的所有完整事件
    pub fn feed(&mut self, chunk: &str) -> Vec<SseEvent> {
        let mut events = Vec::new();

        // 将新数据追加到缓冲区
        self.buffer.push_str(chunk);

        // 按行分割处理
        while let Some(line_end) = self.buffer.find('\n') {
            let line = self.buffer[..line_end].trim_end_matches('\r').to_string();
            self.buffer = self.buffer[line_end + 1..].to_string();

            if let Some(event) = self.parse_line(&line) {
                events.push(event);
            }
        }

        events
    }

    /// 解析单行 SSE 数据
    ///
    /// 空行表示一个事件的结束，非空行按冒号分割为字段名和值。
    ///
    /// # 返回
    /// 如果遇到空行且当前事件有数据，则返回该事件
    fn parse_line(&mut self, line: &str) -> Option<SseEvent> {
        // 空行表示当前事件结束
        if line.is_empty() {
            return self.flush_event();
        }

        // 注释行（以冒号开头），直接忽略
        if line.starts_with(':') {
            return None;
        }

        // 解析字段名和字段值
        let (field, value) = if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            // 冒号后如果紧跟空格，跳过该空格
            let value = if line.len() > colon_pos + 1 && line.as_bytes()[colon_pos + 1] == b' ' {
                &line[colon_pos + 2..]
            } else {
                &line[colon_pos + 1..]
            };
            (field, value)
        } else {
            // 没有冒号的行，整行作为字段名，值为空
            (line.as_ref(), "")
        };

        // 根据字段名分发处理
        match field {
            "data" => {
                // 多个 data 字段用换行连接
                if !self.current_event.data.is_empty() {
                    self.current_event.data.push('\n');
                }
                self.current_event.data.push_str(value);
            }
            "event" => {
                self.current_event.event_type = Some(value.to_string());
            }
            "id" => {
                self.current_event.id = Some(value.to_string());
            }
            // 忽略未知字段（如 retry 等）
            _ => {}
        }

        None
    }

    /// 刷新当前事件：如果有数据则返回事件并重置状态
    fn flush_event(&mut self) -> Option<SseEvent> {
        // 如果当前事件没有任何数据，不生成事件
        if self.current_event.data.is_empty() && self.current_event.event_type.is_none() {
            return None;
        }

        // 检测流结束标记
        if self.current_event.data.trim() == "[DONE]" {
            self.done = true;
        }

        // 取出当前事件并重置
        let event = std::mem::take(&mut self.current_event);
        Some(event)
    }

    /// 解析 SSE 事件的 data 字段为 JSON 值
    ///
    /// # 参数
    /// - `data`: SSE data 字段的内容
    ///
    /// # 返回
    /// 解析后的 JSON 值，如果是 `[DONE]` 则返回 None
    pub fn parse_data(data: &str) -> AiResult<Option<serde_json::Value>> {
        let trimmed = data.trim();

        // 处理流结束标记
        if trimmed == "[DONE]" {
            return Ok(None);
        }

        // 尝试解析为 JSON
        let value: serde_json::Value = serde_json::from_str(trimmed).map_err(|e| {
            AiError::Parse(format!(
                "SSE 数据 JSON 解析失败: {} (原始数据: {})",
                e, trimmed
            ))
        })?;

        Ok(Some(value))
    }

    /// 从 SSE 事件中提取 OpenAI 兼容格式的流式事件
    ///
    /// 适用于 OpenAI 兼容 API（如 DeepSeek），解析 choices[0].delta 中的内容
    pub fn parse_openai_stream_event(data: &str) -> AiResult<StreamEvent> {
        // 先解析为 JSON
        let json = match Self::parse_data(data)? {
            Some(v) => v,
            None => return Ok(StreamEvent::Done),
        };

        // 提取 choices 数组
        let choices = json.get("choices").and_then(|c| c.as_array());

        if let Some(choices) = choices {
            if let Some(choice) = choices.first() {
                // 检查结束原因
                if let Some(finish_reason) = choice.get("finish_reason") {
                    if !finish_reason.is_null() {
                        // 在结束前检查是否有用量信息
                        if let Some(usage) = json.get("usage") {
                            let token_usage = TokenUsage {
                                prompt_tokens: usage
                                    .get("prompt_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0)
                                    as u32,
                                completion_tokens: usage
                                    .get("completion_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0)
                                    as u32,
                                total_tokens: usage
                                    .get("total_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0)
                                    as u32,
                            };
                            return Ok(StreamEvent::Usage(token_usage));
                        }
                    }
                }

                // 提取增量内容
                if let Some(delta) = choice.get("delta") {
                    // 文本内容增量
                    if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                        if !content.is_empty() {
                            return Ok(StreamEvent::ContentDelta(content.to_string()));
                        }
                    }

                    // 工具调用增量
                    if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                        if let Some(tc) = tool_calls.first() {
                            let id = tc
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let func = tc.get("function").cloned().unwrap_or_default();
                            let name = func
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let arguments = func
                                .get("arguments")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();

                            return Ok(StreamEvent::ToolCallDelta {
                                id,
                                name,
                                arguments,
                            });
                        }
                    }
                }
            }
        }

        // 仅包含用量信息的消息（部分提供者在最后发送）
        if let Some(usage) = json.get("usage") {
            if !usage.is_null() {
                let token_usage = TokenUsage {
                    prompt_tokens: usage
                        .get("prompt_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                    completion_tokens: usage
                        .get("completion_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                    total_tokens: usage
                        .get("total_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                };
                return Ok(StreamEvent::Usage(token_usage));
            }
        }

        // 无法识别的事件，返回空内容增量（兼容处理）
        Ok(StreamEvent::ContentDelta(String::new()))
    }
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// 流式增量聚合器
// ============================================================

/// 流式增量聚合器
///
/// 将流式传输中的多个增量事件聚合为一个完整的 AI 响应。
/// 处理文本拼接、工具调用参数拼接以及用量统计收集。
pub struct StreamAggregator {
    /// 聚合的文本内容
    content: String,
    /// 工具调用映射表（按 ID 索引）
    tool_calls: std::collections::HashMap<String, ToolCallBuilder>,
    /// 令牌用量统计
    usage: TokenUsage,
    /// 实际使用的模型名称
    model: String,
    /// 结束原因
    finish_reason: String,
}

/// 工具调用构建器，用于逐步聚合工具调用的增量数据
#[derive(Debug, Clone, Default)]
struct ToolCallBuilder {
    /// 工具调用 ID
    id: String,
    /// 函数名称
    name: String,
    /// 逐步拼接的函数参数 JSON
    arguments: String,
}

impl StreamAggregator {
    /// 创建新的聚合器
    pub fn new() -> Self {
        Self {
            content: String::new(),
            tool_calls: std::collections::HashMap::new(),
            usage: TokenUsage::default(),
            model: String::new(),
            finish_reason: String::new(),
        }
    }

    /// 设置模型名称
    pub fn set_model(&mut self, model: impl Into<String>) {
        self.model = model.into();
    }

    /// 设置结束原因
    pub fn set_finish_reason(&mut self, reason: impl Into<String>) {
        self.finish_reason = reason.into();
    }

    /// 处理一个流式事件，将其聚合到内部状态
    ///
    /// # 参数
    /// - `event`: 接收到的流式事件
    pub fn process_event(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::ContentDelta(text) => {
                // 拼接文本内容
                self.content.push_str(text);
            }
            StreamEvent::ToolCallDelta {
                id,
                name,
                arguments,
            } => {
                // 聚合工具调用增量
                let builder =
                    self.tool_calls
                        .entry(id.clone())
                        .or_insert_with(|| ToolCallBuilder {
                            id: id.clone(),
                            ..Default::default()
                        });
                // 如果有新的函数名称，更新之
                if !name.is_empty() {
                    builder.name = name.clone();
                }
                // 拼接参数片段
                builder.arguments.push_str(arguments);
            }
            StreamEvent::Usage(usage) => {
                // 更新用量统计
                self.usage = usage.clone();
            }
            StreamEvent::Done => {
                // 流结束，如果没有设置结束原因，默认设为 "stop"
                if self.finish_reason.is_empty() {
                    if self.tool_calls.is_empty() {
                        self.finish_reason = "stop".to_string();
                    } else {
                        self.finish_reason = "tool_calls".to_string();
                    }
                }
            }
        }
    }

    /// 将聚合的增量数据构建为完整的 AI 响应
    pub fn build(self) -> AiResponse {
        // 将工具调用构建器转换为正式的 ToolCall 结构
        let tool_calls: Vec<ToolCall> = self
            .tool_calls
            .into_values()
            .map(|builder| ToolCall {
                id: builder.id,
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: builder.name,
                    arguments: builder.arguments,
                },
            })
            .collect();

        // 确定结束原因
        let finish_reason = if self.finish_reason.is_empty() {
            if tool_calls.is_empty() {
                "stop".to_string()
            } else {
                "tool_calls".to_string()
            }
        } else {
            self.finish_reason
        };

        AiResponse {
            content: self.content,
            tool_calls,
            usage: self.usage,
            model: self.model,
            finish_reason,
        }
    }
}

impl Default for StreamAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_parser_basic() {
        // 测试基本的 SSE 数据解析
        let mut parser = SseParser::new();
        let events = parser.feed("data: {\"content\": \"你好\"}\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "{\"content\": \"你好\"}");
    }

    #[test]
    fn test_sse_parser_done_marker() {
        // 测试 [DONE] 流结束标记
        let mut parser = SseParser::new();
        assert!(!parser.is_done());

        let events = parser.feed("data: [DONE]\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "[DONE]");
        assert!(parser.is_done());
    }

    #[test]
    fn test_sse_parser_multi_line_data() {
        // 测试多个 data 字段合并为换行分隔的内容
        let mut parser = SseParser::new();
        let events = parser.feed("data: 第一行\ndata: 第二行\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "第一行\n第二行");
    }

    #[test]
    fn test_sse_parser_event_type() {
        // 测试带有事件类型的 SSE 事件
        let mut parser = SseParser::new();
        let events = parser.feed("event: message\ndata: 测试数据\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type.as_deref(), Some("message"));
        assert_eq!(events[0].data, "测试数据");
    }

    #[test]
    fn test_sse_parser_comment_ignored() {
        // 测试注释行被正确忽略
        let mut parser = SseParser::new();
        let events = parser.feed(": 这是注释\ndata: 真正的数据\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "真正的数据");
    }

    #[test]
    fn test_sse_parser_chunked_input() {
        // 测试分块输入（模拟网络分片到达）
        let mut parser = SseParser::new();

        // 第一块不完整，不应产生事件
        let events = parser.feed("data: {\"part\":");
        assert_eq!(events.len(), 0);

        // 第二块完成了事件
        let events = parser.feed(" \"完整\"}\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "{\"part\": \"完整\"}");
    }

    #[test]
    fn test_sse_parser_multiple_events() {
        // 测试一次输入中包含多个事件
        let mut parser = SseParser::new();
        let input = "data: 事件一\n\ndata: 事件二\n\ndata: [DONE]\n\n";
        let events = parser.feed(input);

        assert_eq!(events.len(), 3);
        assert_eq!(events[0].data, "事件一");
        assert_eq!(events[1].data, "事件二");
        assert_eq!(events[2].data, "[DONE]");
        assert!(parser.is_done());
    }

    #[test]
    fn test_parse_data_json() {
        // 测试 JSON 数据解析
        let result = SseParser::parse_data(r#"{"key": "值"}"#).unwrap();
        assert!(result.is_some());
        let val = result.unwrap();
        assert_eq!(val["key"], "值");
    }

    #[test]
    fn test_parse_data_done() {
        // 测试 [DONE] 标记解析
        let result = SseParser::parse_data("[DONE]").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_data_invalid_json() {
        // 测试无效 JSON 的错误处理
        let result = SseParser::parse_data("不是 JSON");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_openai_stream_content() {
        // 测试 OpenAI 兼容格式的文本内容增量
        let data = r#"{"choices":[{"delta":{"content":"你好"},"index":0}]}"#;
        let event = SseParser::parse_openai_stream_event(data).unwrap();
        match event {
            StreamEvent::ContentDelta(text) => assert_eq!(text, "你好"),
            _ => panic!("应该是 ContentDelta 事件"),
        }
    }

    #[test]
    fn test_parse_openai_stream_tool_call() {
        // 测试 OpenAI 兼容格式的工具调用增量
        let data = r#"{"choices":[{"delta":{"tool_calls":[{"id":"call_1","function":{"name":"test","arguments":"{\"a\":"}}]},"index":0}]}"#;
        let event = SseParser::parse_openai_stream_event(data).unwrap();
        match event {
            StreamEvent::ToolCallDelta {
                id,
                name,
                arguments,
            } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "test");
                assert_eq!(arguments, "{\"a\":");
            }
            _ => panic!("应该是 ToolCallDelta 事件"),
        }
    }

    #[test]
    fn test_parse_openai_stream_done() {
        // 测试 [DONE] 标记
        let event = SseParser::parse_openai_stream_event("[DONE]").unwrap();
        assert!(matches!(event, StreamEvent::Done));
    }

    #[test]
    fn test_stream_aggregator_content() {
        // 测试文本内容聚合
        let mut agg = StreamAggregator::new();
        agg.set_model("test-model");

        agg.process_event(&StreamEvent::ContentDelta("你".to_string()));
        agg.process_event(&StreamEvent::ContentDelta("好".to_string()));
        agg.process_event(&StreamEvent::ContentDelta("世界".to_string()));
        agg.process_event(&StreamEvent::Done);

        let response = agg.build();
        assert_eq!(response.content, "你好世界");
        assert_eq!(response.model, "test-model");
        assert_eq!(response.finish_reason, "stop");
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn test_stream_aggregator_tool_calls() {
        // 测试工具调用增量聚合
        let mut agg = StreamAggregator::new();

        // 模拟分多次接收工具调用参数
        agg.process_event(&StreamEvent::ToolCallDelta {
            id: "call_1".to_string(),
            name: "search".to_string(),
            arguments: "{\"query\":".to_string(),
        });
        agg.process_event(&StreamEvent::ToolCallDelta {
            id: "call_1".to_string(),
            name: String::new(),
            arguments: " \"Rust 编程\"}".to_string(),
        });
        agg.process_event(&StreamEvent::Done);

        let response = agg.build();
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].function.name, "search");
        assert_eq!(
            response.tool_calls[0].function.arguments,
            "{\"query\": \"Rust 编程\"}"
        );
        assert_eq!(response.finish_reason, "tool_calls");
    }

    #[test]
    fn test_stream_aggregator_usage() {
        // 测试用量统计收集
        let mut agg = StreamAggregator::new();

        agg.process_event(&StreamEvent::ContentDelta("测试".to_string()));
        agg.process_event(&StreamEvent::Usage(TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        }));
        agg.process_event(&StreamEvent::Done);

        let response = agg.build();
        assert_eq!(response.usage.prompt_tokens, 10);
        assert_eq!(response.usage.completion_tokens, 5);
        assert_eq!(response.usage.total_tokens, 15);
    }
}

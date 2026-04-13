//! # RPC 协议模块
//!
//! 基于 JSONL（JSON Lines）的 RPC 协议实现，
//! 用于通过 stdio 提供编程式访问接口。
//!
//! 每条消息为单行 JSON，以 `\n` 结尾。

use serde::{Deserialize, Serialize};

// ============================================================
// RPC 消息类型定义
// ============================================================

/// RPC 消息类型
///
/// 使用 `type` 字段区分不同的消息类型，
/// 通过 serde 的 internally tagged enum 实现自动序列化/反序列化。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum RpcMessage {
    /// 用户输入
    #[serde(rename = "user_message")]
    UserMessage {
        content: String,
        #[serde(default)]
        images: Vec<String>,
    },

    /// 助手文本输出（流式）
    #[serde(rename = "assistant_text")]
    AssistantText {
        content: String,
        #[serde(default)]
        done: bool,
    },

    /// 工具调用
    #[serde(rename = "tool_call")]
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },

    /// 工具执行结果
    #[serde(rename = "tool_result")]
    ToolResult {
        id: String,
        output: String,
        #[serde(default)]
        is_error: bool,
    },

    /// 状态更新
    #[serde(rename = "status")]
    Status {
        state: String,
        #[serde(default)]
        message: Option<String>,
    },

    /// 用量统计
    #[serde(rename = "usage")]
    Usage {
        input_tokens: u64,
        output_tokens: u64,
        #[serde(default)]
        cost: Option<f64>,
    },

    /// 模型信息
    #[serde(rename = "model_info")]
    ModelInfo { provider: String, model: String },

    /// 会话信息
    #[serde(rename = "session_info")]
    SessionInfo {
        session_id: String,
        #[serde(default)]
        message_count: usize,
    },

    /// 错误
    #[serde(rename = "error")]
    Error { code: String, message: String },

    /// 心跳
    #[serde(rename = "ping")]
    Ping,

    /// 心跳回复
    #[serde(rename = "pong")]
    Pong,

    /// 退出请求
    #[serde(rename = "exit")]
    Exit,
}

// ============================================================
// RPC 协议编解码器
// ============================================================

/// RPC 协议编解码器
///
/// 负责将 `RpcMessage` 编码为 JSONL 格式的字符串，
/// 以及从 JSONL 字符串解码为 `RpcMessage`。
pub struct RpcCodec;

impl RpcCodec {
    /// 编码消息为 JSONL 行（末尾带换行符）
    pub fn encode(message: &RpcMessage) -> Result<String, serde_json::Error> {
        let mut line = serde_json::to_string(message)?;
        line.push('\n');
        Ok(line)
    }

    /// 解码 JSONL 行为消息（自动去除首尾空白）
    pub fn decode(line: &str) -> Result<RpcMessage, serde_json::Error> {
        serde_json::from_str(line.trim())
    }

    /// 批量编码多条消息为 JSONL 字符串
    pub fn encode_batch(messages: &[RpcMessage]) -> Result<String, serde_json::Error> {
        let mut output = String::new();
        for msg in messages {
            output.push_str(&serde_json::to_string(msg)?);
            output.push('\n');
        }
        Ok(output)
    }

    /// 批量解码 JSONL 字符串，逐行解析
    ///
    /// 每一行独立解码，返回各行的解码结果（可能成功也可能失败）。
    /// 空行会被跳过。
    pub fn decode_batch(input: &str) -> Vec<Result<RpcMessage, serde_json::Error>> {
        input
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line.trim()))
            .collect()
    }
}

// ============================================================
// RPC 会话管理器
// ============================================================

/// RPC 会话管理器
///
/// 维护消息历史并处理输入消息，生成对应的响应。
pub struct RpcSession {
    /// 消息历史
    messages: Vec<RpcMessage>,
    /// 是否已请求退出
    exited: bool,
}

impl RpcSession {
    /// 创建新的 RPC 会话
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            exited: false,
        }
    }

    /// 处理输入消息，返回响应列表
    ///
    /// 根据消息类型生成不同的响应：
    /// - `Ping` → 返回 `Pong`
    /// - `Exit` → 标记退出，无响应
    /// - `UserMessage` → 记录消息，返回状态确认
    /// - 其他 → 记录到历史
    pub fn handle_message(&mut self, msg: RpcMessage) -> Vec<RpcMessage> {
        let mut responses = Vec::new();

        match &msg {
            RpcMessage::Ping => {
                // 心跳直接回复，不记录到历史
                responses.push(RpcMessage::Pong);
            }
            RpcMessage::Exit => {
                // 标记退出
                self.exited = true;
            }
            RpcMessage::UserMessage { .. } => {
                // 记录用户消息并返回状态确认
                self.messages.push(msg);
                responses.push(RpcMessage::Status {
                    state: "thinking".to_string(),
                    message: None,
                });
            }
            _ => {
                // 其他消息仅记录到历史
                self.messages.push(msg);
            }
        }

        responses
    }

    /// 检查是否已请求退出
    pub fn is_exited(&self) -> bool {
        self.exited
    }

    /// 获取消息历史的只读引用
    pub fn history(&self) -> &[RpcMessage] {
        &self.messages
    }
}

impl Default for RpcSession {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// 测试模块
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --------------------------------------------------------
    // 编码测试
    // --------------------------------------------------------

    #[test]
    fn test_encode_user_message() {
        let msg = RpcMessage::UserMessage {
            content: "你好".to_string(),
            images: vec![],
        };
        let encoded = RpcCodec::encode(&msg).unwrap();
        assert!(encoded.ends_with('\n'), "JSONL 行必须以换行符结尾");
        assert!(encoded.contains(r#""type":"user_message""#));
        assert!(encoded.contains(r#""content":"你好""#));
    }

    #[test]
    fn test_encode_assistant_text() {
        let msg = RpcMessage::AssistantText {
            content: "我来帮你".to_string(),
            done: true,
        };
        let encoded = RpcCodec::encode(&msg).unwrap();
        assert!(encoded.contains(r#""type":"assistant_text""#));
        assert!(encoded.contains(r#""done":true"#));
    }

    #[test]
    fn test_encode_tool_call() {
        let msg = RpcMessage::ToolCall {
            id: "call_001".to_string(),
            name: "read_file".to_string(),
            arguments: json!({"path": "/tmp/test.txt"}),
        };
        let encoded = RpcCodec::encode(&msg).unwrap();
        assert!(encoded.contains(r#""type":"tool_call""#));
        assert!(encoded.contains(r#""name":"read_file""#));
        assert!(encoded.contains(r#""id":"call_001""#));
    }

    #[test]
    fn test_encode_tool_result() {
        let msg = RpcMessage::ToolResult {
            id: "call_001".to_string(),
            output: "文件内容".to_string(),
            is_error: false,
        };
        let encoded = RpcCodec::encode(&msg).unwrap();
        assert!(encoded.contains(r#""type":"tool_result""#));
        assert!(encoded.contains(r#""output":"文件内容""#));
    }

    #[test]
    fn test_encode_status() {
        let msg = RpcMessage::Status {
            state: "thinking".to_string(),
            message: Some("正在分析代码".to_string()),
        };
        let encoded = RpcCodec::encode(&msg).unwrap();
        assert!(encoded.contains(r#""type":"status""#));
        assert!(encoded.contains(r#""state":"thinking""#));
        assert!(encoded.contains(r#""message":"正在分析代码""#));
    }

    #[test]
    fn test_encode_usage() {
        let msg = RpcMessage::Usage {
            input_tokens: 100,
            output_tokens: 200,
            cost: Some(0.005),
        };
        let encoded = RpcCodec::encode(&msg).unwrap();
        assert!(encoded.contains(r#""type":"usage""#));
        assert!(encoded.contains(r#""input_tokens":100"#));
        assert!(encoded.contains(r#""output_tokens":200"#));
    }

    #[test]
    fn test_encode_error() {
        let msg = RpcMessage::Error {
            code: "RATE_LIMIT".to_string(),
            message: "请求过于频繁".to_string(),
        };
        let encoded = RpcCodec::encode(&msg).unwrap();
        assert!(encoded.contains(r#""type":"error""#));
        assert!(encoded.contains(r#""code":"RATE_LIMIT""#));
    }

    #[test]
    fn test_encode_ping_pong() {
        let ping = RpcCodec::encode(&RpcMessage::Ping).unwrap();
        assert!(ping.contains(r#""type":"ping""#));

        let pong = RpcCodec::encode(&RpcMessage::Pong).unwrap();
        assert!(pong.contains(r#""type":"pong""#));
    }

    // --------------------------------------------------------
    // 解码测试
    // --------------------------------------------------------

    #[test]
    fn test_decode_user_message() {
        let json = r#"{"type":"user_message","content":"Hello","images":[]}"#;
        let msg = RpcCodec::decode(json).unwrap();
        match msg {
            RpcMessage::UserMessage { content, images } => {
                assert_eq!(content, "Hello");
                assert!(images.is_empty());
            }
            _ => panic!("期望解码为 UserMessage"),
        }
    }

    #[test]
    fn test_decode_unknown_type() {
        // 未知的 type 值应优雅地返回错误
        let json = r#"{"type":"unknown_nonsense","data":"test"}"#;
        let result = RpcCodec::decode(json);
        assert!(result.is_err(), "未知消息类型应该返回错误");
    }

    // --------------------------------------------------------
    // 往返测试（编码→解码→验证一致性）
    // --------------------------------------------------------

    #[test]
    fn test_roundtrip_all_variants() {
        let messages = vec![
            RpcMessage::UserMessage {
                content: "测试".to_string(),
                images: vec!["img1.png".to_string()],
            },
            RpcMessage::AssistantText {
                content: "回复".to_string(),
                done: false,
            },
            RpcMessage::ToolCall {
                id: "tc1".to_string(),
                name: "bash".to_string(),
                arguments: json!({"cmd": "ls"}),
            },
            RpcMessage::ToolResult {
                id: "tc1".to_string(),
                output: "file.txt".to_string(),
                is_error: false,
            },
            RpcMessage::Status {
                state: "idle".to_string(),
                message: None,
            },
            RpcMessage::Usage {
                input_tokens: 10,
                output_tokens: 20,
                cost: None,
            },
            RpcMessage::ModelInfo {
                provider: "deepseek".to_string(),
                model: "deepseek-chat".to_string(),
            },
            RpcMessage::SessionInfo {
                session_id: "sess_123".to_string(),
                message_count: 5,
            },
            RpcMessage::Error {
                code: "INTERNAL".to_string(),
                message: "内部错误".to_string(),
            },
            RpcMessage::Ping,
            RpcMessage::Pong,
            RpcMessage::Exit,
        ];

        for original in &messages {
            let encoded = RpcCodec::encode(original).unwrap();
            let decoded = RpcCodec::decode(&encoded).unwrap();
            assert_eq!(original, &decoded, "往返测试失败: {:?}", original);
        }
    }

    // --------------------------------------------------------
    // 批量编解码测试
    // --------------------------------------------------------

    #[test]
    fn test_encode_batch() {
        let messages = vec![
            RpcMessage::Ping,
            RpcMessage::UserMessage {
                content: "你好".to_string(),
                images: vec![],
            },
            RpcMessage::Pong,
        ];
        let batch = RpcCodec::encode_batch(&messages).unwrap();
        let lines: Vec<&str> = batch.lines().collect();
        assert_eq!(lines.len(), 3, "批量编码应产生 3 行");
        assert!(lines[0].contains(r#""type":"ping""#));
        assert!(lines[1].contains(r#""type":"user_message""#));
        assert!(lines[2].contains(r#""type":"pong""#));
    }

    #[test]
    fn test_decode_batch() {
        let input = r#"{"type":"ping"}
{"type":"pong"}
{"type":"exit"}
"#;
        let results = RpcCodec::decode_batch(input);
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.is_ok()), "所有行都应该解码成功");
        assert_eq!(results[0].as_ref().unwrap(), &RpcMessage::Ping);
        assert_eq!(results[1].as_ref().unwrap(), &RpcMessage::Pong);
        assert_eq!(results[2].as_ref().unwrap(), &RpcMessage::Exit);
    }

    #[test]
    fn test_decode_batch_with_errors() {
        // 混合有效和无效的行
        let input = r#"{"type":"ping"}
这不是有效的JSON
{"type":"pong"}
{"type":"unknown_type"}
"#;
        let results = RpcCodec::decode_batch(input);
        assert_eq!(results.len(), 4);
        assert!(results[0].is_ok(), "第一行应解码成功");
        assert!(results[1].is_err(), "第二行应解码失败（无效 JSON）");
        assert!(results[2].is_ok(), "第三行应解码成功");
        assert!(results[3].is_err(), "第四行应解码失败（未知类型）");
    }

    // --------------------------------------------------------
    // 会话管理测试
    // --------------------------------------------------------

    #[test]
    fn test_session_ping_pong() {
        let mut session = RpcSession::new();
        let responses = session.handle_message(RpcMessage::Ping);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0], RpcMessage::Pong);
        // 心跳不应记录到历史
        assert!(session.history().is_empty(), "Ping 不应记录到历史");
    }

    #[test]
    fn test_session_exit() {
        let mut session = RpcSession::new();
        assert!(!session.is_exited(), "初始状态不应是退出");
        let responses = session.handle_message(RpcMessage::Exit);
        assert!(responses.is_empty(), "退出不应返回响应");
        assert!(session.is_exited(), "处理 Exit 后应标记为退出");
    }

    #[test]
    fn test_session_user_message() {
        let mut session = RpcSession::new();
        let responses = session.handle_message(RpcMessage::UserMessage {
            content: "帮我写代码".to_string(),
            images: vec![],
        });
        // 应返回"thinking"状态
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            RpcMessage::Status { state, .. } => {
                assert_eq!(state, "thinking");
            }
            other => panic!("期望 Status 消息，实际得到 {:?}", other),
        }
    }

    #[test]
    fn test_session_history() {
        let mut session = RpcSession::new();

        // 发送用户消息
        session.handle_message(RpcMessage::UserMessage {
            content: "第一条消息".to_string(),
            images: vec![],
        });

        // 发送工具结果
        session.handle_message(RpcMessage::ToolResult {
            id: "t1".to_string(),
            output: "执行成功".to_string(),
            is_error: false,
        });

        // Ping 不记录到历史
        session.handle_message(RpcMessage::Ping);

        // Exit 不记录到历史
        session.handle_message(RpcMessage::Exit);

        let history = session.history();
        assert_eq!(
            history.len(),
            2,
            "应只有 2 条历史记录（Ping 和 Exit 不记录）"
        );
    }
}

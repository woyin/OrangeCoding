//! # JSONC 解析模块
//!
//! 支持 JSON with Comments 格式——允许单行注释（//）、多行注释（/* */）和尾逗号。

use serde::de::DeserializeOwned;

// ============================================================
// 解析器状态
// ============================================================

/// 解析器内部状态，用于跟踪当前处于何种上下文中
#[derive(Debug, Clone, Copy, PartialEq)]
enum ParserState {
    /// 普通 JSON 内容
    Normal,
    /// 字符串内部（双引号包围）
    InString,
    /// 字符串中的转义字符之后（反斜杠后面的一个字符）
    InStringEscape,
    /// 单行注释内部（// 开头）
    InLineComment,
    /// 多行注释内部（/* 开头）
    InBlockComment,
    /// 多行注释中遇到 *，可能是结束标记
    InBlockCommentStar,
    /// 遇到第一个 /，可能是注释开头
    MaybeComment,
}

// ============================================================
// 核心解析函数
// ============================================================

/// 去除 JSONC 格式中的注释和尾逗号，返回合法的 JSON 字符串
///
/// 处理以下语法：
/// - `// 单行注释`
/// - `/* 多行注释 */`
/// - 对象尾逗号：`{ "a": 1, }` → `{ "a": 1 }`
/// - 数组尾逗号：`[1, 2, ]` → `[1, 2 ]`
/// - 字符串内的注释标记**不会**被去除
pub fn strip_jsonc(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut result = String::with_capacity(input.len());
    let mut state = ParserState::Normal;
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        match state {
            ParserState::Normal => match ch {
                '"' => {
                    result.push(ch);
                    state = ParserState::InString;
                }
                '/' => {
                    state = ParserState::MaybeComment;
                }
                _ => {
                    result.push(ch);
                }
            },

            ParserState::InString => {
                result.push(ch);
                match ch {
                    '\\' => {
                        state = ParserState::InStringEscape;
                    }
                    '"' => {
                        state = ParserState::Normal;
                    }
                    _ => {}
                }
            }

            ParserState::InStringEscape => {
                // 转义字符后的内容原样保留
                result.push(ch);
                state = ParserState::InString;
            }

            ParserState::MaybeComment => {
                match ch {
                    '/' => {
                        // 确认是单行注释
                        state = ParserState::InLineComment;
                    }
                    '*' => {
                        // 确认是多行注释
                        state = ParserState::InBlockComment;
                    }
                    _ => {
                        // 不是注释，把之前的 '/' 和当前字符都输出
                        result.push('/');
                        result.push(ch);
                        state = ParserState::Normal;
                    }
                }
            }

            ParserState::InLineComment => {
                // 单行注释直到换行符结束
                if ch == '\n' {
                    result.push('\n');
                    state = ParserState::Normal;
                }
            }

            ParserState::InBlockComment => {
                if ch == '*' {
                    state = ParserState::InBlockCommentStar;
                }
                // 多行注释内容被丢弃
            }

            ParserState::InBlockCommentStar => {
                if ch == '/' {
                    // 多行注释结束
                    state = ParserState::Normal;
                } else if ch == '*' {
                    // 连续的 *，继续等待 /
                } else {
                    state = ParserState::InBlockComment;
                }
            }
        }

        i += 1;
    }

    // 如果最后停留在 MaybeComment 状态，说明有一个落单的 '/'
    if state == ParserState::MaybeComment {
        result.push('/');
    }

    // 去除尾逗号
    strip_trailing_commas(&result)
}

/// 去除 JSON 中对象和数组的尾逗号
///
/// 匹配 `,` 后面紧跟 `}` 或 `]`（中间可以有空白字符）的模式。
fn strip_trailing_commas(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut result = String::with_capacity(input.len());
    let mut i = 0;
    let mut in_string = false;
    let mut escape_next = false;

    while i < chars.len() {
        let ch = chars[i];

        if escape_next {
            result.push(ch);
            escape_next = false;
            i += 1;
            continue;
        }

        if in_string {
            result.push(ch);
            if ch == '\\' {
                escape_next = true;
            } else if ch == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if ch == '"' {
            in_string = true;
            result.push(ch);
            i += 1;
            continue;
        }

        if ch == ',' {
            // 向前查找，跳过空白，看下一个非空白字符是否为 } 或 ]
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && (chars[j] == '}' || chars[j] == ']') {
                // 跳过此逗号（用空格替代以保持字符位置）
                result.push(' ');
            } else {
                result.push(ch);
            }
        } else {
            result.push(ch);
        }

        i += 1;
    }

    result
}

/// 去除 JSONC 注释和尾逗号后，将内容解析为指定类型
///
/// 先调用 `strip_jsonc` 去除注释和尾逗号，然后使用 `serde_json` 反序列化。
pub fn parse_jsonc<T: DeserializeOwned>(input: &str) -> Result<T, serde_json::Error> {
    let clean = strip_jsonc(input);
    serde_json::from_str(&clean)
}

/// 去除 JSONC 注释和尾逗号后，将内容解析为 `serde_json::Value`
///
/// 适用于不确定目标类型时的通用解析。
pub fn parse_jsonc_value(input: &str) -> Result<serde_json::Value, serde_json::Error> {
    let clean = strip_jsonc(input);
    serde_json::from_str(&clean)
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    /// 测试去除单行注释
    #[test]
    fn 测试去除单行注释() {
        let input = r#"{
            "name": "test" // 这是注释
        }"#;
        let result = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value["name"], "test");
    }

    /// 测试去除多行注释
    #[test]
    fn 测试去除多行注释() {
        let input = r#"{
            /* 这是
               多行注释 */
            "name": "test"
        }"#;
        let result = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value["name"], "test");
    }

    /// 测试去除尾逗号（对象）
    #[test]
    fn 测试去除对象尾逗号() {
        let input = r#"{ "a": 1, "b": 2, }"#;
        let result = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value["a"], 1);
        assert_eq!(value["b"], 2);
    }

    /// 测试去除尾逗号（数组）
    #[test]
    fn 测试去除数组尾逗号() {
        let input = r#"[1, 2, 3, ]"#;
        let result = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value, json!([1, 2, 3]));
    }

    /// 测试字符串内的注释标记不被去除
    #[test]
    fn 测试字符串内注释标记保留() {
        let input = r#"{ "url": "https://example.com // path", "note": "hello /* world */" }"#;
        let result = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value["url"], "https://example.com // path");
        assert_eq!(value["note"], "hello /* world */");
    }

    /// 测试字符串内的转义引号
    #[test]
    fn 测试字符串内转义引号() {
        let input = r#"{ "msg": "he said \"hello\" // still string" }"#;
        let result = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value["msg"], r#"he said "hello" // still string"#);
    }

    /// 测试注释和尾逗号组合
    #[test]
    fn 测试注释和尾逗号组合() {
        let input = r#"{
            // 数据库配置
            "host": "localhost",
            "port": 5432, // 默认端口
            /* 以下为可选项 */
            "ssl": true,
        }"#;
        let result = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value["host"], "localhost");
        assert_eq!(value["port"], 5432);
        assert_eq!(value["ssl"], true);
    }

    /// 测试空输入
    #[test]
    fn 测试空对象() {
        let input = "{}";
        let result = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(value.is_object());
    }

    /// 测试纯注释（只有注释，无有效 JSON）
    #[test]
    fn 测试仅含注释() {
        let input = "// 只有注释\n\"hello\"";
        let result = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value, "hello");
    }

    /// 测试嵌套结构
    #[test]
    fn 测试嵌套结构() {
        let input = r#"{
            "database": {
                "host": "localhost", // 主机
                "ports": [5432, 5433, ], // 端口列表
            },
        }"#;
        let result = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value["database"]["host"], "localhost");
        assert_eq!(value["database"]["ports"], json!([5432, 5433]));
    }

    /// 测试 parse_jsonc 泛型解析
    #[test]
    fn 测试泛型解析() {
        #[derive(Deserialize, Debug)]
        struct Config {
            name: String,
            version: u32,
        }

        let input = r#"{
            "name": "ceair", // 项目名
            "version": 1,
        }"#;

        let config: Config = parse_jsonc(input).unwrap();
        assert_eq!(config.name, "ceair");
        assert_eq!(config.version, 1);
    }

    /// 测试 parse_jsonc_value 通用解析
    #[test]
    fn 测试value解析() {
        let input = r#"[1, /* 中间注释 */ 2, 3]"#;
        let value = parse_jsonc_value(input).unwrap();
        assert_eq!(value, json!([1, 2, 3]));
    }

    /// 测试连续的多行注释
    #[test]
    fn 测试连续多行注释() {
        let input = r#"{ /* a */ /* b */ "key": "val" }"#;
        let result = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value["key"], "val");
    }

    /// 测试多行注释中包含星号
    #[test]
    fn 测试多行注释中含星号() {
        let input = r#"{ /* ** stars ** */ "key": 1 }"#;
        let result = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value["key"], 1);
    }

    /// 测试不含注释的普通 JSON 不受影响
    #[test]
    fn 测试普通json不变() {
        let input = r#"{"a": 1, "b": [2, 3], "c": {"d": true}}"#;
        let result = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value, json!({"a": 1, "b": [2, 3], "c": {"d": true}}));
    }

    /// 测试字符串中包含反斜杠和引号的复杂转义
    #[test]
    fn 测试复杂转义字符串() {
        let input = r#"{ "path": "C:\\Users\\test", "quote": "say \"hi\"" }"#;
        let result = strip_jsonc(input);
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value["path"], r"C:\Users\test");
        assert_eq!(value["quote"], r#"say "hi""#);
    }

    /// 测试行尾注释后的换行保留
    #[test]
    fn 测试行尾注释换行保留() {
        let input = "{\n\"a\": 1 // comment\n}";
        let result = strip_jsonc(input);
        // 换行符应当保留
        assert!(result.contains('\n'));
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value["a"], 1);
    }
}

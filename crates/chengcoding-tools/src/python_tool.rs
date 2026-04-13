//! # Python REPL 工具
//!
//! 通过 Jupyter 内核或直接调用 Python 解释器执行 Python 代码。
//! 支持脚本模式和 REPL 模式，内置语法校验和安全检查。

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::debug;

/// 危险模块/函数关键词列表
const DANGEROUS_IMPORTS: &[&str] = &["os", "subprocess", "shutil", "sys.exit", "ctypes", "signal"];

// ============================================================
// 数据类型定义
// ============================================================

/// Python 执行结果
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PythonResult {
    /// 标准输出
    pub stdout: String,
    /// 标准错误
    pub stderr: String,
    /// 返回值（如有）
    pub return_value: Option<String>,
    /// 退出码
    pub exit_code: i32,
    /// 执行耗时（毫秒）
    pub execution_time_ms: u64,
}

/// Python 执行模式
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PythonMode {
    /// 脚本模式（执行完整脚本）
    Script,
    /// REPL 模式（交互式）
    Repl,
}

// ============================================================
// PythonTool — Python REPL 工具
// ============================================================

/// Python 工具 — 通过 Jupyter 内核执行 Python 代码
#[derive(Debug)]
pub struct PythonTool {
    /// Python 解释器路径
    python_path: String,
    /// 超时时间（秒）
    timeout_secs: u64,
}

impl PythonTool {
    /// 创建默认的 Python 工具实例
    pub fn new() -> Self {
        let python_path = Self::detect_python().unwrap_or_else(|| "python3".to_string());
        Self {
            python_path,
            timeout_secs: 30,
        }
    }

    /// 使用指定 Python 路径创建工具实例
    pub fn with_python_path(path: &str) -> Self {
        Self {
            python_path: path.to_string(),
            timeout_secs: 30,
        }
    }

    /// 构建 Python 执行命令
    ///
    /// 根据执行模式生成对应的命令行参数列表。
    /// - Script 模式使用 `-c` 参数直接执行代码
    /// - REPL 模式使用 `-i` 参数进入交互模式
    pub fn build_command(&self, code: &str, mode: &PythonMode) -> Vec<String> {
        match mode {
            PythonMode::Script => {
                vec![self.python_path.clone(), "-c".to_string(), code.to_string()]
            }
            PythonMode::Repl => {
                vec![
                    self.python_path.clone(),
                    "-i".to_string(),
                    "-c".to_string(),
                    code.to_string(),
                ]
            }
        }
    }

    /// 验证 Python 代码（基本语法检查）
    ///
    /// 执行简单的括号/引号配对检查，不依赖 Python 解释器。
    pub fn validate_syntax(code: &str) -> Result<(), String> {
        // 检查括号配对
        let mut paren_depth = 0i32;
        let mut bracket_depth = 0i32;
        let mut brace_depth = 0i32;
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut in_triple_single = false;
        let mut in_triple_double = false;
        let mut prev_char = '\0';
        let chars: Vec<char> = code.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            let c = chars[i];

            // 处理三引号
            if i + 2 < len && !in_single_quote && !in_double_quote {
                let triple: String = chars[i..i + 3].iter().collect();
                if triple == "'''" && !in_triple_double {
                    in_triple_single = !in_triple_single;
                    i += 3;
                    continue;
                }
                if triple == "\"\"\"" && !in_triple_single {
                    in_triple_double = !in_triple_double;
                    i += 3;
                    continue;
                }
            }

            // 在三引号字符串内部跳过
            if in_triple_single || in_triple_double {
                i += 1;
                continue;
            }

            // 处理转义字符
            if prev_char == '\\' {
                prev_char = '\0';
                i += 1;
                continue;
            }

            // 处理单引号字符串
            if c == '\'' && !in_double_quote {
                in_single_quote = !in_single_quote;
            }
            // 处理双引号字符串
            if c == '"' && !in_single_quote {
                in_double_quote = !in_double_quote;
            }

            // 仅在字符串外检查括号
            if !in_single_quote && !in_double_quote {
                match c {
                    '(' => paren_depth += 1,
                    ')' => {
                        paren_depth -= 1;
                        if paren_depth < 0 {
                            return Err("语法错误: 多余的右括号 ')'".to_string());
                        }
                    }
                    '[' => bracket_depth += 1,
                    ']' => {
                        bracket_depth -= 1;
                        if bracket_depth < 0 {
                            return Err("语法错误: 多余的右方括号 ']'".to_string());
                        }
                    }
                    '{' => brace_depth += 1,
                    '}' => {
                        brace_depth -= 1;
                        if brace_depth < 0 {
                            return Err("语法错误: 多余的右花括号 '}'".to_string());
                        }
                    }
                    _ => {}
                }
            }

            prev_char = c;
            i += 1;
        }

        // 检查未闭合的字符串
        if in_single_quote {
            return Err("语法错误: 未闭合的单引号字符串".to_string());
        }
        if in_double_quote {
            return Err("语法错误: 未闭合的双引号字符串".to_string());
        }
        if in_triple_single {
            return Err("语法错误: 未闭合的三引号字符串 '''".to_string());
        }
        if in_triple_double {
            return Err("语法错误: 未闭合的三引号字符串 \"\"\"".to_string());
        }

        // 检查未闭合的括号
        if paren_depth != 0 {
            return Err("语法错误: 未闭合的括号 '('".to_string());
        }
        if bracket_depth != 0 {
            return Err("语法错误: 未闭合的方括号 '['".to_string());
        }
        if brace_depth != 0 {
            return Err("语法错误: 未闭合的花括号 '{'".to_string());
        }

        Ok(())
    }

    /// 检测代码中是否有危险操作
    ///
    /// 返回警告信息列表，不阻止执行。
    pub fn check_safety(code: &str) -> Vec<String> {
        let mut warnings = Vec::new();

        for &module in DANGEROUS_IMPORTS {
            // 检查 import 语句
            let import_pattern = format!("import {}", module);
            let from_pattern = format!("from {} import", module);
            let module_usage = format!("{}.", module);

            if code.contains(&import_pattern)
                || code.contains(&from_pattern)
                || code.contains(&module_usage)
            {
                warnings.push(format!("警告: 代码使用了危险模块 '{}'", module));
            }
        }

        // 检查 eval/exec 调用
        if code.contains("eval(") || code.contains("exec(") {
            warnings.push("警告: 代码使用了 eval/exec 动态执行".to_string());
        }

        // 检查 __import__ 调用
        if code.contains("__import__") {
            warnings.push("警告: 代码使用了 __import__ 动态导入".to_string());
        }

        warnings
    }

    /// 格式化执行结果
    pub fn format_result(result: &PythonResult) -> String {
        let mut output = String::new();

        if !result.stdout.is_empty() {
            output.push_str(&format!("stdout:\n{}\n", result.stdout));
        }

        if !result.stderr.is_empty() {
            output.push_str(&format!("stderr:\n{}\n", result.stderr));
        }

        if let Some(ref ret) = result.return_value {
            output.push_str(&format!("返回值: {}\n", ret));
        }

        if result.exit_code != 0 {
            output.push_str(&format!("退出码: {}\n", result.exit_code));
        }

        output.push_str(&format!("执行耗时: {}ms", result.execution_time_ms));

        output
    }

    /// 检测系统 Python 路径
    ///
    /// 按优先级依次尝试 python3、python。
    pub fn detect_python() -> Option<String> {
        let candidates = ["python3", "python"];
        for candidate in &candidates {
            if std::process::Command::new(candidate)
                .arg("--version")
                .output()
                .is_ok()
            {
                return Some(candidate.to_string());
            }
        }
        None
    }
}

impl Default for PythonTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for PythonTool {
    fn name(&self) -> &str {
        "python"
    }

    fn description(&self) -> &str {
        "执行 Python 代码，支持脚本模式和 REPL 模式"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "要执行的 Python 代码"
                },
                "timeout": {
                    "type": "number",
                    "description": "超时时间（秒），默认 30",
                    "default": 30
                }
            },
            "required": ["code"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<String> {
        let code = params
            .get("code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: code".to_string()))?;

        if code.trim().is_empty() {
            return Err(ToolError::InvalidParams("code 参数不能为空".to_string()));
        }

        // 语法检查
        if let Err(e) = Self::validate_syntax(code) {
            return Err(ToolError::InvalidParams(e));
        }

        // 安全检查（仅警告，不阻止）
        let warnings = Self::check_safety(code);
        for w in &warnings {
            debug!("{}", w);
        }

        let timeout = params
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.timeout_secs);

        let args = self.build_command(code, &PythonMode::Script);

        debug!("执行 Python 代码（超时: {}秒）", timeout);

        let start = std::time::Instant::now();

        let mut cmd = tokio::process::Command::new(&args[0]);
        for arg in &args[1..] {
            cmd.arg(arg);
        }
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let timeout_duration = std::time::Duration::from_secs(timeout);

        match tokio::time::timeout(timeout_duration, cmd.output()).await {
            Ok(Ok(output)) => {
                let elapsed = start.elapsed().as_millis() as u64;
                let result = PythonResult {
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    return_value: None,
                    exit_code: output.status.code().unwrap_or(-1),
                    execution_time_ms: elapsed,
                };

                let mut formatted = Self::format_result(&result);

                // 附加安全警告
                if !warnings.is_empty() {
                    formatted.push_str("\n\n--- 安全警告 ---\n");
                    for w in &warnings {
                        formatted.push_str(&format!("{}\n", w));
                    }
                }

                Ok(formatted)
            }
            Ok(Err(e)) => Err(ToolError::ExecutionError(format!(
                "启动 Python 进程失败: {}",
                e
            ))),
            Err(_) => Err(ToolError::ExecutionError(format!(
                "Python 执行超时（{}秒）",
                timeout
            ))),
        }
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 测试脚本模式下构建命令
    #[test]
    fn test_build_command_script() {
        let tool = PythonTool::with_python_path("/usr/bin/python3");
        let args = tool.build_command("print('hello')", &PythonMode::Script);

        assert_eq!(args[0], "/usr/bin/python3");
        assert_eq!(args[1], "-c");
        assert_eq!(args[2], "print('hello')");
        assert_eq!(args.len(), 3);
    }

    /// 测试 REPL 模式下构建命令
    #[test]
    fn test_build_command_repl() {
        let tool = PythonTool::with_python_path("/usr/bin/python3");
        let args = tool.build_command("x = 1", &PythonMode::Repl);

        assert_eq!(args[0], "/usr/bin/python3");
        assert_eq!(args[1], "-i");
        assert_eq!(args[2], "-c");
        assert_eq!(args[3], "x = 1");
        assert_eq!(args.len(), 4);
    }

    /// 测试有效语法校验通过
    #[test]
    fn test_validate_syntax_valid() {
        assert!(PythonTool::validate_syntax("print('hello')").is_ok());
        assert!(PythonTool::validate_syntax("x = [1, 2, 3]").is_ok());
        assert!(PythonTool::validate_syntax("d = {'a': 1}").is_ok());
        assert!(PythonTool::validate_syntax("s = \"hello\"").is_ok());
    }

    /// 测试无效语法（未闭合字符串）检测
    #[test]
    fn test_validate_syntax_invalid() {
        let result = PythonTool::validate_syntax("print('hello");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("未闭合"), "错误信息: {}", err);
    }

    /// 测试安全检查 — 检测 os 模块导入
    #[test]
    fn test_check_safety_import_os() {
        let warnings = PythonTool::check_safety("import os\nos.system('ls')");
        assert!(!warnings.is_empty());
        assert!(warnings.iter().any(|w| w.contains("os")));
    }

    /// 测试安全检查 — 检测 subprocess 模块
    #[test]
    fn test_check_safety_subprocess() {
        let warnings = PythonTool::check_safety("import subprocess\nsubprocess.run(['ls'])");
        assert!(!warnings.is_empty());
        assert!(warnings.iter().any(|w| w.contains("subprocess")));
    }

    /// 测试安全检查 — 安全代码无警告
    #[test]
    fn test_check_safety_safe_code() {
        let warnings = PythonTool::check_safety("print('hello world')\nx = 1 + 2");
        assert!(warnings.is_empty());
    }

    /// 测试格式化仅有 stdout 的结果
    #[test]
    fn test_format_result_stdout_only() {
        let result = PythonResult {
            stdout: "hello world".to_string(),
            stderr: String::new(),
            return_value: None,
            exit_code: 0,
            execution_time_ms: 42,
        };
        let formatted = PythonTool::format_result(&result);

        assert!(formatted.contains("hello world"));
        assert!(formatted.contains("42ms"));
        // 退出码为 0 时不应显示
        assert!(!formatted.contains("退出码"));
    }

    /// 测试格式化包含 stderr 的结果
    #[test]
    fn test_format_result_with_stderr() {
        let result = PythonResult {
            stdout: "output".to_string(),
            stderr: "some warning".to_string(),
            return_value: None,
            exit_code: 0,
            execution_time_ms: 10,
        };
        let formatted = PythonTool::format_result(&result);

        assert!(formatted.contains("output"));
        assert!(formatted.contains("some warning"));
        assert!(formatted.contains("stderr:"));
    }

    /// 测试格式化带有错误退出码的结果
    #[test]
    fn test_format_result_with_error() {
        let result = PythonResult {
            stdout: String::new(),
            stderr: "NameError: name 'x' is not defined".to_string(),
            return_value: None,
            exit_code: 1,
            execution_time_ms: 5,
        };
        let formatted = PythonTool::format_result(&result);

        assert!(formatted.contains("NameError"));
        assert!(formatted.contains("退出码: 1"));
    }

    /// 测试参数 Schema 结构
    #[test]
    fn test_parameter_schema() {
        let tool = PythonTool::new();
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["code"].is_object());
        assert!(schema["properties"]["timeout"].is_object());

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("code")));
    }

    /// 测试工具名称和描述
    #[test]
    fn test_tool_name() {
        let tool = PythonTool::new();
        assert_eq!(tool.name(), "python");
        assert!(!tool.description().is_empty());
    }
}

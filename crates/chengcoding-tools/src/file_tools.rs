//! # 文件操作工具集
//!
//! 提供六个文件操作工具，每个工具都实现了 `Tool` 特征：
//! - `ReadFileTool` - 读取文件内容（支持行范围）
//! - `WriteFileTool` - 写入文件内容（创建或覆盖）
//! - `EditFileTool` - 编辑文件（搜索替换）
//! - `ListDirectoryTool` - 递归列出目录内容
//! - `SearchFilesTool` - 使用 glob 模式搜索文件
//! - `DeleteFileTool` - 安全删除文件

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;
use tracing::{debug, warn};

// ============================================================
// ReadFileTool - 读取文件内容
// ============================================================

/// 文件读取工具
///
/// 读取指定文件的内容，支持通过行号范围限定读取区间。
/// 行号从 1 开始计数。
#[derive(Debug)]
pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    /// 工具名称
    fn name(&self) -> &str {
        "read_file"
    }

    /// 工具描述
    fn description(&self) -> &str {
        "读取文件内容，支持指定起始行和结束行来读取部分内容"
    }

    /// 参数 JSON Schema 定义
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要读取的文件路径"
                },
                "start_line": {
                    "type": "integer",
                    "description": "起始行号（从 1 开始，可选）"
                },
                "end_line": {
                    "type": "integer",
                    "description": "结束行号（包含该行，可选）"
                }
            },
            "required": ["path"]
        })
    }

    /// 执行文件读取操作
    async fn execute(&self, params: Value) -> ToolResult<String> {
        // 提取并验证文件路径参数
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: path".to_string()))?;

        debug!("读取文件: {}", path);

        // 检查文件是否存在
        let file_path = Path::new(path);
        if !file_path.exists() {
            return Err(ToolError::ExecutionError(format!("文件不存在: {}", path)));
        }

        // 检查目标是否为文件（而非目录）
        if !file_path.is_file() {
            return Err(ToolError::ExecutionError(format!("路径不是文件: {}", path)));
        }

        // 异步读取文件全部内容
        let content = fs::read_to_string(path).await?;
        let lines: Vec<&str> = content.lines().collect();

        // 提取可选的行范围参数
        let start_line = params
            .get("start_line")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let end_line = params
            .get("end_line")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        // 根据行范围截取内容
        match (start_line, end_line) {
            (Some(start), Some(end)) => {
                // 验证行号范围的有效性
                if start == 0 {
                    return Err(ToolError::InvalidParams(
                        "start_line 必须大于 0（行号从 1 开始）".to_string(),
                    ));
                }
                if end < start {
                    return Err(ToolError::InvalidParams(format!(
                        "end_line ({}) 不能小于 start_line ({})",
                        end, start
                    )));
                }

                // 转换为 0 基索引并截取指定范围
                let start_idx = start - 1;
                let end_idx = end.min(lines.len());
                let selected: Vec<&str> =
                    lines.get(start_idx..end_idx).unwrap_or_default().to_vec();
                Ok(selected.join("\n"))
            }
            (Some(start), None) => {
                // 只有起始行，读取到文件末尾
                if start == 0 {
                    return Err(ToolError::InvalidParams(
                        "start_line 必须大于 0（行号从 1 开始）".to_string(),
                    ));
                }
                let start_idx = start - 1;
                let selected: Vec<&str> = lines.get(start_idx..).unwrap_or_default().to_vec();
                Ok(selected.join("\n"))
            }
            (None, Some(end)) => {
                // 只有结束行，从文件开头读取到指定行
                let end_idx = end.min(lines.len());
                let selected: Vec<&str> = lines.get(..end_idx).unwrap_or_default().to_vec();
                Ok(selected.join("\n"))
            }
            (None, None) => {
                // 没有指定行范围，返回全部内容
                Ok(content)
            }
        }
    }
}

// ============================================================
// WriteFileTool - 写入文件内容
// ============================================================

/// 文件写入工具
///
/// 将内容写入指定文件。如果文件不存在则创建，如果已存在则覆盖。
/// 会自动创建所需的父级目录。
#[derive(Debug)]
pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    /// 工具名称
    fn name(&self) -> &str {
        "write_file"
    }

    /// 工具描述
    fn description(&self) -> &str {
        "将内容写入文件，如果文件不存在则创建，如果已存在则覆盖"
    }

    /// 参数 JSON Schema 定义
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要写入的文件路径"
                },
                "content": {
                    "type": "string",
                    "description": "要写入的文件内容"
                }
            },
            "required": ["path", "content"]
        })
    }

    /// 执行文件写入操作
    async fn execute(&self, params: Value) -> ToolResult<String> {
        // 提取并验证文件路径
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: path".to_string()))?;

        // 提取并验证写入内容
        let content = params
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: content".to_string()))?;

        debug!("写入文件: {}", path);

        // 确保父目录存在，不存在则递归创建
        let file_path = Path::new(path);
        if let Some(parent) = file_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await?;
                debug!("已创建父目录: {}", parent.display());
            }
        }

        // 异步写入文件内容
        fs::write(path, content).await?;

        Ok(format!(
            "已成功写入文件: {}（{} 字节）",
            path,
            content.len()
        ))
    }
}

// ============================================================
// EditFileTool - 编辑文件（搜索替换）
// ============================================================

/// 文件编辑工具
///
/// 在文件中查找指定的旧字符串并替换为新字符串。
/// 要求旧字符串在文件中精确匹配且恰好出现一次。
#[derive(Debug)]
pub struct EditFileTool;

#[async_trait]
impl Tool for EditFileTool {
    /// 工具名称
    fn name(&self) -> &str {
        "edit_file"
    }

    /// 工具描述
    fn description(&self) -> &str {
        "在文件中搜索指定文本并替换为新文本，要求匹配唯一"
    }

    /// 参数 JSON Schema 定义
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要编辑的文件路径"
                },
                "old_str": {
                    "type": "string",
                    "description": "要搜索的原始文本（必须精确匹配）"
                },
                "new_str": {
                    "type": "string",
                    "description": "替换后的新文本"
                }
            },
            "required": ["path", "old_str", "new_str"]
        })
    }

    /// 执行文件编辑操作
    async fn execute(&self, params: Value) -> ToolResult<String> {
        // 提取并验证所有必要参数
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: path".to_string()))?;

        let old_str = params
            .get("old_str")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: old_str".to_string()))?;

        let new_str = params
            .get("new_str")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: new_str".to_string()))?;

        debug!("编辑文件: {}", path);

        // 检查文件是否存在
        if !Path::new(path).exists() {
            return Err(ToolError::ExecutionError(format!("文件不存在: {}", path)));
        }

        // 读取文件当前内容
        let content = fs::read_to_string(path).await?;

        // 检查旧字符串是否存在以及出现次数
        let match_count = content.matches(old_str).count();
        if match_count == 0 {
            return Err(ToolError::ExecutionError(format!(
                "在文件 {} 中未找到要替换的文本",
                path
            )));
        }
        if match_count > 1 {
            return Err(ToolError::ExecutionError(format!(
                "在文件 {} 中找到 {} 处匹配，要求恰好匹配一处以避免歧义",
                path, match_count
            )));
        }

        // 执行替换操作（仅替换第一处匹配）
        let new_content = content.replacen(old_str, new_str, 1);

        // 将修改后的内容写回文件
        fs::write(path, &new_content).await?;

        Ok(format!("已成功编辑文件: {}（替换了 1 处匹配）", path))
    }
}

// ============================================================
// ListDirectoryTool - 列出目录内容
// ============================================================

/// 目录列表工具
///
/// 递归列出指定目录的内容，支持设置最大递归深度。
/// 默认最大深度为 3 层，以避免列出过多文件。
#[derive(Debug)]
pub struct ListDirectoryTool;

/// 递归遍历目录的辅助函数
///
/// # 参数
/// - `dir_path`: 要遍历的目录路径
/// - `prefix`: 当前目录的显示前缀（用于树状缩进）
/// - `current_depth`: 当前递归深度
/// - `max_depth`: 最大允许递归深度
/// - `entries`: 收集遍历结果的可变向量
fn list_dir_recursive<'a>(
    dir_path: &'a Path,
    prefix: &'a str,
    current_depth: usize,
    max_depth: usize,
    entries: &'a mut Vec<String>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ToolResult<()>> + Send + 'a>> {
    Box::pin(async move {
        // 达到最大深度时停止递归
        if current_depth > max_depth {
            return Ok(());
        }

        // 读取目录内容
        let mut read_dir = fs::read_dir(dir_path).await?;

        // 收集并排序目录条目（使文件列表有确定性的顺序）
        let mut items: Vec<(String, bool)> = Vec::new();
        while let Some(entry) = read_dir.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            // 跳过隐藏文件和目录（以 . 开头的条目）
            if name.starts_with('.') {
                continue;
            }
            let is_dir = entry.file_type().await?.is_dir();
            items.push((name, is_dir));
        }
        items.sort_by(|a, b| a.0.cmp(&b.0));

        // 遍历排序后的条目
        for (name, is_dir) in &items {
            if *is_dir {
                // 目录条目以 / 结尾标识
                entries.push(format!("{}{}/", prefix, name));
                // 递归遍历子目录
                let sub_path = dir_path.join(name);
                let sub_prefix = format!("{}  ", prefix);
                list_dir_recursive(
                    &sub_path,
                    &sub_prefix,
                    current_depth + 1,
                    max_depth,
                    entries,
                )
                .await?;
            } else {
                // 文件条目
                entries.push(format!("{}{}", prefix, name));
            }
        }

        Ok(())
    })
}

#[async_trait]
impl Tool for ListDirectoryTool {
    /// 工具名称
    fn name(&self) -> &str {
        "list_directory"
    }

    /// 工具描述
    fn description(&self) -> &str {
        "递归列出目录内容，支持设置最大递归深度"
    }

    /// 参数 JSON Schema 定义
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要列出内容的目录路径"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "最大递归深度（默认为 3）"
                }
            },
            "required": ["path"]
        })
    }

    /// 执行目录列表操作
    async fn execute(&self, params: Value) -> ToolResult<String> {
        // 提取并验证目录路径
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: path".to_string()))?;

        // 提取可选的最大深度参数（默认为 3）
        let max_depth = params
            .get("max_depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(3) as usize;

        debug!("列出目录: {}（最大深度: {}）", path, max_depth);

        // 检查目录是否存在
        let dir_path = Path::new(path);
        if !dir_path.exists() {
            return Err(ToolError::ExecutionError(format!("目录不存在: {}", path)));
        }

        // 检查目标是否为目录
        if !dir_path.is_dir() {
            return Err(ToolError::ExecutionError(format!("路径不是目录: {}", path)));
        }

        // 递归遍历目录并收集结果
        let mut entries = Vec::new();
        list_dir_recursive(dir_path, "", 0, max_depth, &mut entries).await?;

        if entries.is_empty() {
            Ok(format!("目录 {} 为空", path))
        } else {
            Ok(entries.join("\n"))
        }
    }
}

// ============================================================
// SearchFilesTool - 使用 glob 模式搜索文件
// ============================================================

/// 文件搜索工具
///
/// 使用 glob 模式在指定目录下搜索匹配的文件。
/// 支持的通配符：
/// - `*` 匹配文件名中的任意字符（不包括路径分隔符）
/// - `**` 匹配任意深度的路径
/// - `?` 匹配单个字符
#[derive(Debug)]
pub struct SearchFilesTool;

/// 将 glob 模式转换为正则表达式
///
/// # 参数
/// - `pattern`: glob 模式字符串
///
/// # 返回值
/// 转换后的正则表达式字符串
fn glob_to_regex(pattern: &str) -> String {
    let mut regex = String::from("^");
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '*' => {
                // 检查是否为 ** 通配符（匹配任意路径深度）
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    regex.push_str(".*");
                    i += 2;
                    // 跳过 ** 后面的路径分隔符
                    if i < chars.len() && chars[i] == '/' {
                        i += 1;
                    }
                    continue;
                } else {
                    // 单个 * 匹配文件名中的任意字符（不含路径分隔符）
                    regex.push_str("[^/]*");
                }
            }
            '?' => {
                // ? 匹配单个非分隔符字符
                regex.push_str("[^/]");
            }
            '.' => {
                // 转义正则表达式中的特殊字符
                regex.push_str("\\.");
            }
            '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                // 转义其他正则表达式特殊字符
                regex.push('\\');
                regex.push(chars[i]);
            }
            c => {
                // 普通字符直接添加
                regex.push(c);
            }
        }
        i += 1;
    }

    regex.push('$');
    regex
}

/// 递归搜索匹配 glob 模式的文件
///
/// # 参数
/// - `dir_path`: 搜索的根目录
/// - `regex`: 编译后的正则表达式
/// - `base_path`: 用于计算相对路径的基础目录
/// - `results`: 收集匹配结果的向量
/// - `max_results`: 最大结果数量限制
fn search_files_recursive<'a>(
    dir_path: &'a Path,
    regex: &'a Regex,
    base_path: &'a Path,
    results: &'a mut Vec<String>,
    max_results: usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ToolResult<()>> + Send + 'a>> {
    Box::pin(async move {
        // 达到最大结果数量时停止搜索
        if results.len() >= max_results {
            return Ok(());
        }

        // 尝试读取目录，忽略权限不足的目录
        let mut read_dir = match fs::read_dir(dir_path).await {
            Ok(rd) => rd,
            Err(e) => {
                warn!("无法读取目录 {}: {}", dir_path.display(), e);
                return Ok(());
            }
        };

        while let Some(entry) = read_dir.next_entry().await? {
            // 达到上限时提前退出
            if results.len() >= max_results {
                break;
            }

            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // 跳过隐藏文件和目录
            if name.starts_with('.') {
                continue;
            }

            // 计算相对路径用于模式匹配
            let relative_path = entry_path
                .strip_prefix(base_path)
                .unwrap_or(&entry_path)
                .to_string_lossy()
                .to_string();

            if entry_path.is_dir() {
                // 递归搜索子目录
                search_files_recursive(&entry_path, regex, base_path, results, max_results).await?;
            } else if regex.is_match(&relative_path) {
                // 文件路径匹配 glob 模式，添加到结果集
                results.push(relative_path);
            }
        }

        Ok(())
    })
}

#[async_trait]
impl Tool for SearchFilesTool {
    /// 工具名称
    fn name(&self) -> &str {
        "search_files"
    }

    /// 工具描述
    fn description(&self) -> &str {
        "使用 glob 模式搜索文件，支持 * 和 ** 通配符"
    }

    /// 参数 JSON Schema 定义
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "glob 搜索模式（如 *.rs、src/**/*.toml）"
                },
                "path": {
                    "type": "string",
                    "description": "搜索的根目录路径（默认为当前目录）"
                }
            },
            "required": ["pattern"]
        })
    }

    /// 执行文件搜索操作
    async fn execute(&self, params: Value) -> ToolResult<String> {
        // 提取并验证 glob 模式
        let pattern = params
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: pattern".to_string()))?;

        // 搜索根目录（默认为当前目录）
        let search_path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        debug!("搜索文件: 模式={}, 路径={}", pattern, search_path);

        // 检查搜索目录是否存在
        let dir_path = Path::new(search_path);
        if !dir_path.exists() {
            return Err(ToolError::ExecutionError(format!(
                "搜索目录不存在: {}",
                search_path
            )));
        }

        // 将 glob 模式转换为正则表达式
        let regex_str = glob_to_regex(pattern);
        let regex = Regex::new(&regex_str).map_err(|e| {
            ToolError::InvalidParams(format!("无效的搜索模式 '{}': {}", pattern, e))
        })?;

        // 递归搜索匹配的文件（最多返回 1000 个结果）
        let max_results = 1000;
        let mut results = Vec::new();
        search_files_recursive(dir_path, &regex, dir_path, &mut results, max_results).await?;

        // 对结果排序，确保输出顺序确定
        results.sort();

        if results.is_empty() {
            Ok(format!("未找到匹配模式 '{}' 的文件", pattern))
        } else {
            let count = results.len();
            let suffix = if count >= max_results {
                format!("\n\n（结果已截断，最多显示 {} 个）", max_results)
            } else {
                String::new()
            };
            Ok(format!(
                "找到 {} 个匹配文件:\n{}{}",
                count,
                results.join("\n"),
                suffix
            ))
        }
    }
}

// ============================================================
// DeleteFileTool - 安全删除文件
// ============================================================

/// 文件删除工具
///
/// 删除指定文件，附带安全检查：
/// - 不允许删除目录（防止误删整个目录树）
/// - 验证文件确实存在再执行删除
#[derive(Debug)]
pub struct DeleteFileTool;

#[async_trait]
impl Tool for DeleteFileTool {
    /// 工具名称
    fn name(&self) -> &str {
        "delete_file"
    }

    /// 工具描述
    fn description(&self) -> &str {
        "删除指定文件（不允许删除目录）"
    }

    /// 参数 JSON Schema 定义
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要删除的文件路径"
                }
            },
            "required": ["path"]
        })
    }

    /// 执行文件删除操作
    async fn execute(&self, params: Value) -> ToolResult<String> {
        // 提取并验证文件路径
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("缺少必要参数: path".to_string()))?;

        debug!("删除文件: {}", path);

        let file_path = Path::new(path);

        // 检查文件是否存在
        if !file_path.exists() {
            return Err(ToolError::ExecutionError(format!("文件不存在: {}", path)));
        }

        // 安全检查：不允许删除目录
        if file_path.is_dir() {
            return Err(ToolError::ExecutionError(format!(
                "不允许删除目录，请使用专用的目录删除命令: {}",
                path
            )));
        }

        // 执行异步文件删除
        fs::remove_file(path).await?;

        Ok(format!("已成功删除文件: {}", path))
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    /// 辅助函数：创建临时测试目录
    fn create_temp_dir() -> TempDir {
        tempfile::tempdir().expect("创建临时目录失败")
    }

    /// 辅助函数：在临时目录中创建文件
    async fn create_test_file(dir: &Path, name: &str, content: &str) -> String {
        let file_path = dir.join(name);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await.unwrap();
        }
        fs::write(&file_path, content).await.unwrap();
        file_path.to_string_lossy().to_string()
    }

    // ==================== ReadFileTool 测试 ====================

    /// 测试读取文件全部内容
    #[tokio::test]
    async fn test_read_file_full_content() {
        let dir = create_temp_dir();
        let content = "第一行\n第二行\n第三行";
        let path = create_test_file(dir.path(), "test.txt", content).await;

        let tool = ReadFileTool;
        let result = tool.execute(json!({"path": path})).await.unwrap();
        assert_eq!(result, content);
    }

    /// 测试读取文件的指定行范围
    #[tokio::test]
    async fn test_read_file_line_range() {
        let dir = create_temp_dir();
        let content = "第一行\n第二行\n第三行\n第四行\n第五行";
        let path = create_test_file(dir.path(), "test.txt", content).await;

        let tool = ReadFileTool;

        // 读取第 2 到第 4 行
        let result = tool
            .execute(json!({"path": path, "start_line": 2, "end_line": 4}))
            .await
            .unwrap();
        assert_eq!(result, "第二行\n第三行\n第四行");
    }

    /// 测试只指定起始行
    #[tokio::test]
    async fn test_read_file_start_line_only() {
        let dir = create_temp_dir();
        let content = "第一行\n第二行\n第三行";
        let path = create_test_file(dir.path(), "test.txt", content).await;

        let tool = ReadFileTool;
        let result = tool
            .execute(json!({"path": path, "start_line": 2}))
            .await
            .unwrap();
        assert_eq!(result, "第二行\n第三行");
    }

    /// 测试读取不存在的文件
    #[tokio::test]
    async fn test_read_file_not_found() {
        let tool = ReadFileTool;
        let result = tool.execute(json!({"path": "/nonexistent/file.txt"})).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ExecutionError(msg) => assert!(msg.contains("不存在")),
            other => panic!("期望 ExecutionError，得到: {:?}", other),
        }
    }

    /// 测试缺少必要参数
    #[tokio::test]
    async fn test_read_file_missing_params() {
        let tool = ReadFileTool;
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::InvalidParams(msg) => assert!(msg.contains("path")),
            other => panic!("期望 InvalidParams，得到: {:?}", other),
        }
    }

    // ==================== WriteFileTool 测试 ====================

    /// 测试写入新文件
    #[tokio::test]
    async fn test_write_file_create_new() {
        let dir = create_temp_dir();
        let file_path = dir.path().join("new_file.txt");
        let path_str = file_path.to_string_lossy().to_string();

        let tool = WriteFileTool;
        let result = tool
            .execute(json!({"path": path_str, "content": "你好世界"}))
            .await
            .unwrap();

        // 验证返回消息
        assert!(result.contains("成功"));

        // 验证文件内容
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "你好世界");
    }

    /// 测试覆盖已有文件
    #[tokio::test]
    async fn test_write_file_overwrite() {
        let dir = create_temp_dir();
        let path = create_test_file(dir.path(), "existing.txt", "旧内容").await;

        let tool = WriteFileTool;
        tool.execute(json!({"path": path.clone(), "content": "新内容"}))
            .await
            .unwrap();

        let content = fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "新内容");
    }

    /// 测试自动创建父目录
    #[tokio::test]
    async fn test_write_file_create_parent_dirs() {
        let dir = create_temp_dir();
        let file_path = dir.path().join("a").join("b").join("c").join("file.txt");
        let path_str = file_path.to_string_lossy().to_string();

        let tool = WriteFileTool;
        tool.execute(json!({"path": path_str, "content": "深层目录"}))
            .await
            .unwrap();

        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "深层目录");
    }

    // ==================== EditFileTool 测试 ====================

    /// 测试成功的文本替换
    #[tokio::test]
    async fn test_edit_file_success() {
        let dir = create_temp_dir();
        let content = "fn main() {\n    println!(\"hello\");\n}";
        let path = create_test_file(dir.path(), "test.rs", content).await;

        let tool = EditFileTool;
        let result = tool
            .execute(json!({
                "path": path.clone(),
                "old_str": "println!(\"hello\")",
                "new_str": "println!(\"你好世界\")"
            }))
            .await
            .unwrap();

        assert!(result.contains("成功"));

        // 验证文件内容已更新
        let new_content = fs::read_to_string(&path).await.unwrap();
        assert!(new_content.contains("println!(\"你好世界\")"));
        assert!(!new_content.contains("println!(\"hello\")"));
    }

    /// 测试未找到匹配文本
    #[tokio::test]
    async fn test_edit_file_no_match() {
        let dir = create_temp_dir();
        let path = create_test_file(dir.path(), "test.txt", "原始内容").await;

        let tool = EditFileTool;
        let result = tool
            .execute(json!({
                "path": path,
                "old_str": "不存在的文本",
                "new_str": "新文本"
            }))
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ExecutionError(msg) => assert!(msg.contains("未找到")),
            other => panic!("期望 ExecutionError，得到: {:?}", other),
        }
    }

    /// 测试多处匹配时应报错
    #[tokio::test]
    async fn test_edit_file_multiple_matches() {
        let dir = create_temp_dir();
        let content = "hello world\nhello rust\nhello ceair";
        let path = create_test_file(dir.path(), "test.txt", content).await;

        let tool = EditFileTool;
        let result = tool
            .execute(json!({
                "path": path,
                "old_str": "hello",
                "new_str": "你好"
            }))
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ExecutionError(msg) => assert!(msg.contains("3")),
            other => panic!("期望 ExecutionError（多处匹配），得到: {:?}", other),
        }
    }

    // ==================== ListDirectoryTool 测试 ====================

    /// 测试列出目录内容
    #[tokio::test]
    async fn test_list_directory() {
        let dir = create_temp_dir();
        // 创建测试文件结构
        create_test_file(dir.path(), "file1.txt", "内容1").await;
        create_test_file(dir.path(), "file2.rs", "内容2").await;
        create_test_file(dir.path(), "sub/file3.txt", "内容3").await;

        let tool = ListDirectoryTool;
        let path_str = dir.path().to_string_lossy().to_string();
        let result = tool.execute(json!({"path": path_str})).await.unwrap();

        // 验证结果包含预期的文件和目录
        assert!(result.contains("file1.txt"));
        assert!(result.contains("file2.rs"));
        assert!(result.contains("sub/"));
        assert!(result.contains("file3.txt"));
    }

    /// 测试空目录
    #[tokio::test]
    async fn test_list_empty_directory() {
        let dir = create_temp_dir();
        let tool = ListDirectoryTool;
        let path_str = dir.path().to_string_lossy().to_string();
        let result = tool.execute(json!({"path": path_str})).await.unwrap();
        assert!(result.contains("为空"));
    }

    /// 测试不存在的目录
    #[tokio::test]
    async fn test_list_nonexistent_directory() {
        let tool = ListDirectoryTool;
        let result = tool
            .execute(json!({"path": "/nonexistent/directory"}))
            .await;
        assert!(result.is_err());
    }

    // ==================== SearchFilesTool 测试 ====================

    /// 测试 glob 模式搜索
    #[tokio::test]
    async fn test_search_files_glob() {
        let dir = create_temp_dir();
        create_test_file(dir.path(), "file1.rs", "").await;
        create_test_file(dir.path(), "file2.rs", "").await;
        create_test_file(dir.path(), "file3.txt", "").await;
        create_test_file(dir.path(), "sub/file4.rs", "").await;

        let tool = SearchFilesTool;
        let path_str = dir.path().to_string_lossy().to_string();

        // 搜索所有 .rs 文件（仅当前目录）
        let result = tool
            .execute(json!({"pattern": "*.rs", "path": path_str}))
            .await
            .unwrap();

        assert!(result.contains("file1.rs"));
        assert!(result.contains("file2.rs"));
        // 子目录中的文件不应被 *.rs 匹配
        assert!(!result.contains("file4.rs"));
    }

    /// 测试 ** 通配符递归搜索
    #[tokio::test]
    async fn test_search_files_recursive_glob() {
        let dir = create_temp_dir();
        create_test_file(dir.path(), "file1.rs", "").await;
        create_test_file(dir.path(), "sub/file2.rs", "").await;
        create_test_file(dir.path(), "sub/deep/file3.rs", "").await;

        let tool = SearchFilesTool;
        let path_str = dir.path().to_string_lossy().to_string();

        // 使用 ** 递归搜索所有 .rs 文件
        let result = tool
            .execute(json!({"pattern": "**/*.rs", "path": path_str}))
            .await
            .unwrap();

        assert!(result.contains("file2.rs"));
        assert!(result.contains("file3.rs"));
    }

    /// 测试 glob 模式转正则表达式
    #[test]
    fn test_glob_to_regex_conversion() {
        // 星号匹配
        let regex = glob_to_regex("*.rs");
        assert_eq!(regex, "^[^/]*\\.rs$");

        // 双星号匹配
        let regex = glob_to_regex("**/*.rs");
        assert_eq!(regex, "^.*[^/]*\\.rs$");

        // 问号匹配
        let regex = glob_to_regex("file?.txt");
        assert_eq!(regex, "^file[^/]\\.txt$");
    }

    // ==================== DeleteFileTool 测试 ====================

    /// 测试成功删除文件
    #[tokio::test]
    async fn test_delete_file_success() {
        let dir = create_temp_dir();
        let path = create_test_file(dir.path(), "to_delete.txt", "临时内容").await;

        // 确认文件存在
        assert!(Path::new(&path).exists());

        let tool = DeleteFileTool;
        let result = tool.execute(json!({"path": path.clone()})).await.unwrap();

        // 验证删除成功
        assert!(result.contains("成功"));
        assert!(!Path::new(&path).exists());
    }

    /// 测试删除不存在的文件
    #[tokio::test]
    async fn test_delete_file_not_found() {
        let tool = DeleteFileTool;
        let result = tool.execute(json!({"path": "/nonexistent/file.txt"})).await;
        assert!(result.is_err());
    }

    /// 测试禁止删除目录
    #[tokio::test]
    async fn test_delete_directory_forbidden() {
        let dir = create_temp_dir();
        let path_str = dir.path().to_string_lossy().to_string();

        let tool = DeleteFileTool;
        let result = tool.execute(json!({"path": path_str})).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ExecutionError(msg) => assert!(msg.contains("不允许删除目录")),
            other => panic!("期望 ExecutionError，得到: {:?}", other),
        }
    }
}

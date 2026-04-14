# 工具参考手册

> OrangeCoding 工具系统完整参考文档。所有工具均实现 `Tool` trait，通过 `ToolRegistry` 统一管理。

## 目录

- [概述](#概述)
- [工具架构](#工具架构)
- [文件操作工具](#文件操作工具)
  - [read_file](#read_file)
  - [write_file](#write_file)
  - [edit](#edit)
- [Shell 执行工具](#shell-执行工具)
  - [bash](#bash)
- [搜索工具](#搜索工具)
  - [grep](#grep)
  - [find](#find)
- [网络工具](#网络工具)
  - [browser](#browser)
  - [web_search](#web_search)
  - [fetch](#fetch)
- [语言服务工具](#语言服务工具)
  - [lsp](#lsp)
- [代码分析工具](#代码分析工具)
  - [ast_grep](#ast_grep)
- [执行工具](#执行工具)
  - [python](#python)
  - [notebook](#notebook)
- [交互工具](#交互工具)
  - [ask](#ask)
  - [calc](#calc)
- [远程执行工具](#远程执行工具)
  - [ssh](#ssh)
- [任务管理工具](#任务管理工具)
  - [todo](#todo)
  - [task](#task)
- [会话管理工具](#会话管理工具)
  - [session_list](#session_list)
  - [session_read](#session_read)
  - [session_search](#session_search)
  - [session_info](#session_info)
- [任务生命周期工具](#任务生命周期工具)
  - [task_create](#task_create)
  - [task_get](#task_get)
  - [task_list](#task_list)
  - [task_update](#task_update)
- [工具注册表](#工具注册表)
- [错误处理](#错误处理)
- [安全约束](#安全约束)

---

## 概述

OrangeCoding 的工具系统基于 `chengcoding-tools` crate 实现，提供了一套统一的工具接口。所有工具均实现以下核心 trait：

```rust
#[async_trait]
pub trait Tool: Send + Sync + Debug {
    /// 工具唯一标识符
    fn name(&self) -> &str;

    /// 工具功能描述（供 AI 模型理解用途）
    fn description(&self) -> &str;

    /// 工具参数的 JSON Schema 定义
    fn parameters_schema(&self) -> Value;

    /// 执行工具并返回结果
    async fn execute(&self, params: Value) -> ToolResult<String>;
}
```

### 核心类型

```rust
/// 工具执行错误
pub enum ToolError {
    /// 参数无效
    InvalidParams(String),
    /// 执行错误
    ExecutionError(String),
    /// IO 错误
    Io(std::io::Error),
    /// 安全违规
    SecurityViolation(String),
    /// 工具未找到
    NotFound(String),
}

/// 工具执行结果
pub type ToolResult<T> = Result<T, ToolError>;
```

---

## 工具架构

```
┌──────────────────────────────────────────────────┐
│                  ToolRegistry                     │
│           (DashMap<String, Arc<dyn Tool>>)        │
├──────────────────────────────────────────────────┤
│  register()  │  get()  │  list_tools()           │
│  unregister()│         │  get_schemas()           │
└───────┬──────┴─────────┴────────────┬────────────┘
        │                             │
        ▼                             ▼
┌──────────────┐             ┌──────────────────┐
│ FileOperation│             │  SecurityPolicy  │
│    Guard     │◄────────────│  PathValidator   │
└──────────────┘             └──────────────────┘
        │
        ▼
┌──────────────────────────────────────────────────┐
│              具体工具实现                          │
│  BashTool │ EditTool │ GrepTool │ FetchTool │ …  │
└──────────────────────────────────────────────────┘
```

---

## 文件操作工具

### read_file

读取文件内容。支持文本文件和二进制文件（Base64 编码返回）。

**源文件**: `crates/chengcoding-tools/src/edit_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `path` | `string` | ✅ | 文件绝对路径或相对路径 |
| `encoding` | `string` | ❌ | 编码格式，默认 `utf-8` |
| `line_range` | `[u32, u32]` | ❌ | 行范围 `[起始行, 结束行]`，从 1 开始 |

**返回值**: 文件内容字符串。若指定 `line_range`，仅返回对应行范围内容。

**安全约束**:
- 路径必须通过 `PathValidator` 验证
- 不得读取 `~/.ssh`、`~/.aws`、`/etc/shadow` 等敏感路径
- 文件大小不得超过 `max_file_size`（默认 10MB）

**示例**:

```json
{
  "path": "src/main.rs"
}
```

```json
{
  "path": "src/lib.rs",
  "line_range": [1, 50]
}
```

---

### write_file

创建或覆盖写入文件。

**源文件**: `crates/chengcoding-tools/src/edit_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `path` | `string` | ✅ | 目标文件路径 |
| `content` | `string` | ✅ | 写入内容 |
| `create_dirs` | `bool` | ❌ | 是否自动创建父目录，默认 `false` |

**返回值**: 写入确认信息，包含写入字节数。

**安全约束**:
- 需要 `PermissionKind::Edit` 权限
- 路径必须在 `allowed_paths` 范围内
- 不得写入 `blocked_paths` 中的路径

**示例**:

```json
{
  "path": "src/utils.rs",
  "content": "pub fn hello() {\n    println!(\"Hello, World!\");\n}\n",
  "create_dirs": true
}
```

---

### edit

对现有文件进行精确编辑。支持基于旧文本/新文本的替换操作。

**源文件**: `crates/chengcoding-tools/src/edit_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `path` | `string` | ✅ | 目标文件路径 |
| `old_text` | `string` | ✅ | 要替换的原文本（精确匹配） |
| `new_text` | `string` | ✅ | 替换后的新文本 |

**返回值**: 编辑确认信息，包含修改行数。

**注意事项**:
- `old_text` 必须与文件中的内容精确匹配（包括空白字符）
- 如果 `old_text` 在文件中出现多次，需提供足够的上下文以保证唯一性
- 支持多行文本替换

**示例**:

```json
{
  "path": "src/main.rs",
  "old_text": "fn main() {\n    println!(\"Hello\");\n}",
  "new_text": "fn main() {\n    println!(\"Hello, OrangeCoding!\");\n}"
}
```

---

## Shell 执行工具

### bash

在系统 Shell 中执行命令。

**源文件**: `crates/chengcoding-tools/src/bash_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `command` | `string` | ✅ | 要执行的 Shell 命令 |
| `cwd` | `string` | ❌ | 工作目录，默认为当前会话工作目录 |
| `timeout` | `u64` | ❌ | 超时时间（秒），默认 300 |
| `env` | `object` | ❌ | 额外环境变量 |

**返回值**: 命令输出（stdout + stderr 合并），包含退出码。

**安全约束**:
- 需要 `PermissionKind::Bash` 权限
- 默认阻止危险命令（如 `rm -rf /`、`dd`）
- 命令执行受 `SecurityPolicy` 控制

**示例**:

```json
{
  "command": "cargo build --release",
  "cwd": "/home/user/project",
  "timeout": 600
}
```

```json
{
  "command": "git log --oneline -10",
  "env": {
    "GIT_PAGER": ""
  }
}
```

---

## 搜索工具

### grep

在文件中搜索正则表达式模式。基于 `regex` crate 实现高性能搜索。

**源文件**: `crates/chengcoding-tools/src/grep_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `pattern` | `string` | ✅ | 正则表达式搜索模式 |
| `path` | `string` | ❌ | 搜索根路径，默认当前目录 |
| `include` | `string` | ❌ | 文件名 glob 过滤（如 `*.rs`） |
| `exclude` | `string` | ❌ | 排除文件名 glob |
| `max_results` | `u32` | ❌ | 最大结果数量，默认 100 |
| `case_sensitive` | `bool` | ❌ | 是否区分大小写，默认 `true` |
| `context_lines` | `u32` | ❌ | 上下文行数，默认 0 |

**返回值**: 匹配结果列表，每项包含文件路径、行号、匹配行内容。

**示例**:

```json
{
  "pattern": "pub\\s+fn\\s+\\w+",
  "path": "src/",
  "include": "*.rs",
  "max_results": 50
}
```

```json
{
  "pattern": "TODO|FIXME|HACK",
  "path": ".",
  "case_sensitive": false,
  "context_lines": 2
}
```

---

### find

按文件名和属性搜索文件。基于 `walkdir` 和 `glob` crate 实现。

**源文件**: `crates/chengcoding-tools/src/find_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `path` | `string` | ❌ | 搜索根路径，默认当前目录 |
| `pattern` | `string` | ❌ | 文件名 glob 模式（如 `*.rs`） |
| `type` | `string` | ❌ | 类型过滤：`file`、`dir`、`symlink` |
| `max_depth` | `u32` | ❌ | 最大递归深度 |
| `max_results` | `u32` | ❌ | 最大结果数量，默认 200 |

**返回值**: 匹配的文件/目录路径列表。

**示例**:

```json
{
  "pattern": "*.toml",
  "path": ".",
  "type": "file"
}
```

```json
{
  "path": "src/",
  "type": "dir",
  "max_depth": 2
}
```

---

## 网络工具

### browser

模拟浏览器访问网页并提取内容。

**源文件**: `crates/chengcoding-tools/src/browser_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `url` | `string` | ✅ | 目标网页 URL |
| `selector` | `string` | ❌ | CSS 选择器，提取特定元素 |
| `wait_for` | `string` | ❌ | 等待特定元素出现 |
| `screenshot` | `bool` | ❌ | 是否截图，默认 `false` |

**返回值**: 网页文本内容或截图数据。

**安全约束**:
- 需要 `PermissionKind::WebFetch` 权限
- 遵循 robots.txt
- 内部网络地址（127.0.0.1、localhost 等）默认受限

**示例**:

```json
{
  "url": "https://docs.rs/tokio/latest/tokio/",
  "selector": "main"
}
```

---

### web_search

执行网络搜索并返回结果摘要。

**源文件**: `crates/chengcoding-tools/src/web_search_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `query` | `string` | ✅ | 搜索查询字符串 |
| `max_results` | `u32` | ❌ | 最大结果数，默认 10 |
| `language` | `string` | ❌ | 结果语言偏好 |

**返回值**: 搜索结果列表，每项包含标题、URL、摘要。

**示例**:

```json
{
  "query": "Rust async trait 最佳实践",
  "max_results": 5,
  "language": "zh"
}
```

---

### fetch

发送 HTTP 请求并获取响应。基于 `reqwest` crate 实现。

**源文件**: `crates/chengcoding-tools/src/fetch_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `url` | `string` | ✅ | 请求 URL |
| `method` | `string` | ❌ | HTTP 方法，默认 `GET` |
| `headers` | `object` | ❌ | 请求头键值对 |
| `body` | `string` | ❌ | 请求体 |
| `timeout` | `u64` | ❌ | 超时时间（秒），默认 30 |

**返回值**: HTTP 响应（状态码 + 响应体）。

**安全约束**:
- 需要 `PermissionKind::WebFetch` 权限
- 使用 `rustls-tls` 作为 TLS 后端
- 支持流式响应处理

**示例**:

```json
{
  "url": "https://api.example.com/data",
  "method": "POST",
  "headers": {
    "Content-Type": "application/json"
  },
  "body": "{\"key\": \"value\"}"
}
```

---

## 语言服务工具

### lsp

与语言服务器协议（Language Server Protocol）交互。

**源文件**: `crates/chengcoding-tools/src/lsp_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `action` | `string` | ✅ | LSP 操作类型（见下表） |
| `file_path` | `string` | ✅ | 目标文件路径 |
| `line` | `u32` | ❌ | 行号（从 0 开始） |
| `character` | `u32` | ❌ | 列号（从 0 开始） |
| `query` | `string` | ❌ | 搜索查询（用于 `workspace_symbol`） |

**支持的 LSP 操作**:

| 操作 | 描述 | 必需参数 |
|------|------|----------|
| `hover` | 获取悬停信息 | `file_path`, `line`, `character` |
| `definition` | 跳转到定义 | `file_path`, `line`, `character` |
| `references` | 查找所有引用 | `file_path`, `line`, `character` |
| `completion` | 代码补全 | `file_path`, `line`, `character` |
| `diagnostics` | 获取诊断信息 | `file_path` |
| `workspace_symbol` | 工作空间符号搜索 | `query` |
| `document_symbol` | 文档符号列表 | `file_path` |
| `rename` | 重命名符号 | `file_path`, `line`, `character`, `new_name` |

**返回值**: 根据操作类型返回对应的 LSP 响应数据。

**示例**:

```json
{
  "action": "definition",
  "file_path": "src/main.rs",
  "line": 15,
  "character": 10
}
```

```json
{
  "action": "diagnostics",
  "file_path": "src/lib.rs"
}
```

---

## 代码分析工具

### ast_grep

基于 AST（抽象语法树）的代码搜索和转换。

**源文件**: `crates/chengcoding-tools/src/ast_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `pattern` | `string` | ✅ | AST 搜索模式 |
| `path` | `string` | ❌ | 搜索路径，默认当前目录 |
| `language` | `string` | ❌ | 目标语言（`rust`、`python`、`javascript` 等） |
| `rewrite` | `string` | ❌ | 替换模式（若提供则执行转换） |

**返回值**: 匹配的代码节点列表，包含文件路径、行号、匹配内容。

**模式语法**: 使用 `$VAR` 作为通配符匹配任意表达式。

**示例**:

```json
{
  "pattern": "fn $NAME($$$ARGS) -> Result<$RET, $ERR>",
  "path": "src/",
  "language": "rust"
}
```

```json
{
  "pattern": "unwrap()",
  "path": "src/",
  "language": "rust"
}
```

---

## 执行工具

### python

执行 Python 代码片段。

**源文件**: `crates/chengcoding-tools/src/python_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `code` | `string` | ✅ | Python 代码 |
| `timeout` | `u64` | ❌ | 超时时间（秒），默认 60 |

**返回值**: Python 标准输出和标准错误。

**示例**:

```json
{
  "code": "import json\ndata = {'name': 'OrangeCoding', 'version': '0.1.0'}\nprint(json.dumps(data, indent=2))"
}
```

---

### notebook

操作 Jupyter Notebook（`.ipynb` 文件）。

**源文件**: `crates/chengcoding-tools/src/notebook_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `action` | `string` | ✅ | 操作类型：`read`、`write`、`execute`、`add_cell` |
| `path` | `string` | ✅ | Notebook 文件路径 |
| `cell_index` | `u32` | ❌ | 单元格索引 |
| `content` | `string` | ❌ | 单元格内容 |
| `cell_type` | `string` | ❌ | 单元格类型：`code`、`markdown` |

**返回值**: 根据操作类型返回 Notebook 内容或执行结果。

**示例**:

```json
{
  "action": "read",
  "path": "analysis.ipynb"
}
```

```json
{
  "action": "add_cell",
  "path": "analysis.ipynb",
  "content": "import pandas as pd\ndf = pd.read_csv('data.csv')\ndf.head()",
  "cell_type": "code"
}
```

---

## 交互工具

### ask

向用户提出问题并等待回答。用于需要用户确认或输入的场景。

**源文件**: `crates/chengcoding-tools/src/ask_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `question` | `string` | ✅ | 要向用户提出的问题 |
| `options` | `[string]` | ❌ | 预设选项列表 |
| `default` | `string` | ❌ | 默认值 |

**返回值**: 用户的回答字符串。

**示例**:

```json
{
  "question": "是否继续执行此操作？",
  "options": ["是", "否"],
  "default": "是"
}
```

```json
{
  "question": "请输入数据库连接字符串："
}
```

---

### calc

执行数学计算表达式。

**源文件**: `crates/chengcoding-tools/src/calc_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `expression` | `string` | ✅ | 数学表达式 |

**返回值**: 计算结果（数值字符串）。

**支持的运算**:
- 基本运算：`+`、`-`、`*`、`/`、`%`
- 幂运算：`**`
- 括号分组：`()`
- 数学函数：`sin`、`cos`、`tan`、`sqrt`、`log`、`ln`、`abs`
- 常量：`pi`、`e`

**示例**:

```json
{
  "expression": "sqrt(144) + 2 * pi"
}
```

---

## 远程执行工具

### ssh

通过 SSH 在远程服务器上执行命令。

**源文件**: `crates/chengcoding-tools/src/ssh_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `host` | `string` | ✅ | SSH 主机地址 |
| `command` | `string` | ✅ | 要执行的命令 |
| `user` | `string` | ❌ | SSH 用户名 |
| `port` | `u16` | ❌ | SSH 端口，默认 22 |
| `key_path` | `string` | ❌ | 私钥文件路径 |
| `timeout` | `u64` | ❌ | 超时时间（秒），默认 60 |

**返回值**: 远程命令输出。

**安全约束**:
- 不得读取 `~/.ssh/id_rsa` 等私钥内容
- 连接信息会被审计记录

**示例**:

```json
{
  "host": "server.example.com",
  "command": "uname -a && df -h",
  "user": "deploy",
  "port": 22
}
```

---

## 任务管理工具

### todo

管理待办事项列表。

**源文件**: `crates/chengcoding-tools/src/todo_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `action` | `string` | ✅ | 操作：`add`、`list`、`complete`、`remove`、`update` |
| `title` | `string` | ❌ | 待办标题（`add` 时必需） |
| `description` | `string` | ❌ | 待办描述 |
| `id` | `string` | ❌ | 待办 ID（`complete`、`remove`、`update` 时必需） |
| `status` | `string` | ❌ | 状态过滤/更新：`pending`、`in_progress`、`done` |

**返回值**: 操作结果或待办列表。

**示例**:

```json
{
  "action": "add",
  "title": "实现用户认证模块",
  "description": "使用 JWT 实现登录/登出功能"
}
```

```json
{
  "action": "list",
  "status": "pending"
}
```

```json
{
  "action": "complete",
  "id": "todo-001"
}
```

---

### task

创建并管理子 Agent 任务。允许当前 Agent 委派任务给其他 Agent。

**源文件**: `crates/chengcoding-tools/src/task_tool.rs`

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `action` | `string` | ✅ | 操作：`create`、`status`、`cancel`、`list` |
| `prompt` | `string` | ❌ | 任务描述（`create` 时必需） |
| `agent_type` | `string` | ❌ | 目标 Agent 类型 |
| `task_id` | `string` | ❌ | 任务 ID（`status`、`cancel` 时必需） |
| `timeout` | `u64` | ❌ | 任务超时（秒） |

**返回值**: 任务 ID（创建时）或任务状态信息。

**示例**:

```json
{
  "action": "create",
  "prompt": "分析 src/ 目录下所有文件的代码质量",
  "agent_type": "explore"
}
```

```json
{
  "action": "status",
  "task_id": "task-abc123"
}
```

---

## 会话管理工具

### session_list

列出所有可用的会话。

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `cwd` | `string` | ❌ | 按工作目录过滤 |
| `limit` | `u32` | ❌ | 最大返回数量，默认 20 |

**返回值**: 会话信息列表，包含 ID、标题、创建时间、条目数。

**示例**:

```json
{
  "cwd": "/home/user/project",
  "limit": 10
}
```

---

### session_read

读取指定会话的内容。

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `session_id` | `string` | ✅ | 会话 ID |
| `entry_types` | `[string]` | ❌ | 过滤条目类型：`Message`、`ToolCall`、`Compaction` 等 |
| `limit` | `u32` | ❌ | 最大条目数 |

**返回值**: 会话条目列表。

**示例**:

```json
{
  "session_id": "sess-abc123",
  "entry_types": ["Message"],
  "limit": 50
}
```

---

### session_search

在会话历史中搜索内容。

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `query` | `string` | ✅ | 搜索关键词 |
| `session_id` | `string` | ❌ | 限定搜索的会话 ID |
| `max_results` | `u32` | ❌ | 最大结果数 |

**返回值**: 匹配的会话条目列表。

**示例**:

```json
{
  "query": "数据库迁移",
  "max_results": 20
}
```

---

### session_info

获取当前会话的详细信息。

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `session_id` | `string` | ❌ | 会话 ID，默认当前会话 |

**返回值**: 会话元数据（ID、工作目录、创建时间、更新时间、条目计数）。

**示例**:

```json
{
  "session_id": "sess-abc123"
}
```

---

## 任务生命周期工具

### task_create

创建新的编排任务（用于 `TaskOrchestrator` 的 DAG 任务调度）。

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `title` | `string` | ✅ | 任务标题 |
| `description` | `string` | ❌ | 任务详细描述 |
| `dependencies` | `[string]` | ❌ | 依赖的任务 ID 列表 |
| `assigned_to` | `string` | ❌ | 分配的 Agent ID |

**返回值**: 任务 ID 和初始状态。

**示例**:

```json
{
  "title": "构建前端组件",
  "description": "实现 React 仪表盘组件",
  "dependencies": ["task-design-review"],
  "assigned_to": "agent-junior-001"
}
```

---

### task_get

获取指定任务的详细信息。

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `task_id` | `string` | ✅ | 任务 ID |

**返回值**: 任务完整信息（标题、状态、依赖、分配、结果）。

---

### task_list

列出所有任务，可按状态过滤。

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `status` | `string` | ❌ | 状态过滤：`Pending`、`Ready`、`Running`、`Completed`、`Failed` |
| `assigned_to` | `string` | ❌ | 按分配 Agent 过滤 |

**返回值**: 任务列表。

---

### task_update

更新任务状态或结果。

| 参数 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `task_id` | `string` | ✅ | 任务 ID |
| `status` | `string` | ❌ | 新状态 |
| `result` | `object` | ❌ | 任务执行结果 |

**返回值**: 更新后的任务信息。

---

## 工具注册表

`ToolRegistry` 是线程安全的工具管理中心，基于 `DashMap` 实现并发安全存取。

```rust
pub struct ToolRegistry {
    tools: DashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// 注册新工具
    pub fn register(&self, tool: Arc<dyn Tool>);

    /// 注销工具
    pub fn unregister(&self, name: &str) -> bool;

    /// 获取工具实例
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>>;

    /// 列出所有已注册工具名
    pub fn list_tools(&self) -> Vec<String>;

    /// 导出所有工具的 JSON Schema
    pub fn get_schemas(&self) -> Value;

    /// 创建包含所有内置工具的默认注册表
    pub fn create_default_registry() -> Self;
}
```

### 初始化流程

```
程序启动
  ↓
ToolRegistry::create_default_registry()
  ↓
注册所有内置工具（16+）
  ↓
应用 SecurityPolicy 包装
  ↓
FileOperationGuard 附加
  ↓
工具就绪，可供 Agent 调用
```

---

## 错误处理

所有工具共享统一的错误类型体系：

| 错误变体 | 描述 | 示例场景 |
|----------|------|----------|
| `InvalidParams` | 参数验证失败 | 缺少必需参数、类型不匹配 |
| `ExecutionError` | 执行过程中出错 | 命令返回非零退出码 |
| `Io` | IO 操作错误 | 文件不存在、权限不足 |
| `SecurityViolation` | 安全策略违规 | 访问受限路径、未授权操作 |
| `NotFound` | 资源未找到 | 工具名不存在、目标文件不存在 |

---

## 安全约束

所有工具执行前均经过安全检查：

1. **路径验证** (`PathValidator`)：确保文件操作不越出允许范围
2. **权限检查** (`PermissionKind`)：验证当前角色是否有权使用该工具
3. **审计记录** (`AuditLogger`)：所有工具调用均被记录到审计链
4. **敏感信息脱敏** (`Sanitizer`)：结果中的密钥、令牌等信息自动脱敏

详见 [权限系统参考](./permissions.md) 和 [安全架构](../architecture/security.md)。

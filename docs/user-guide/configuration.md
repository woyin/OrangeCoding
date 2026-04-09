# 配置参考

> CEAIR 使用 JSONC（带注释的 JSON）格式进行配置，支持项目级和全局级的层级覆盖。

## 目录

- [配置文件位置和格式](#配置文件位置和格式)
- [项目级 vs 全局级配置](#项目级-vs-全局级配置)
- [AI 提供者配置](#ai-提供者配置)
- [Agent 配置](#agent-配置)
- [工具配置](#工具配置)
- [安全策略](#安全策略)
- [Category 覆盖](#category-覆盖)
- [Hook 配置](#hook-配置)
- [Skill 配置](#skill-配置)
- [TUI 配置](#tui-配置)
- [日志配置](#日志配置)
- [完整配置示例](#完整配置示例)

---

## 配置文件位置和格式

### JSONC 格式

CEAIR 使用 JSONC（JSON with Comments）格式，在标准 JSON 基础上支持：

- **单行注释**：`// 这是注释`
- **多行注释**：`/* 这是多行注释 */`
- **尾随逗号**：对象和数组末尾允许多余的逗号

```jsonc
{
  // 这是单行注释
  "key": "value",

  /* 这是
     多行注释 */
  "array": [
    "item1",
    "item2",   // 尾随逗号没问题
  ],
}
```

### 配置文件位置

| 级别 | 路径 | 说明 |
|------|------|------|
| **全局配置** | `~/.ceair/config.jsonc` | 用户级全局设置 |
| **项目配置** | `.opencode/ceair.jsonc` | 项目级设置 |

---

## 项目级 vs 全局级配置

配置按以下优先级合并（后者覆盖前者）：

```
内置默认值 < 全局配置 (~/.ceair/config.jsonc) < 项目配置 (.opencode/ceair.jsonc)
```

### 适合放在全局配置中的设置

```jsonc
// ~/.ceair/config.jsonc
{
  // API Key 通常是全局的
  "providers": {
    "openai": {
      "api_key": "sk-xxxxxxxx"
    },
    "anthropic": {
      "api_key": "sk-ant-xxxxxxxx"
    }
  },
  // 个人偏好
  "tui": {
    "theme": "dark",
    "show_token_usage": true
  }
}
```

### 适合放在项目配置中的设置

```jsonc
// .opencode/ceair.jsonc
{
  // 项目特定的 AI 设置
  "ai": {
    "provider": "anthropic",
    "model": "claude-opus-4-6"
  },
  // 项目特定的工具限制
  "tools": {
    "allowed_paths": ["src/", "tests/", "docs/"],
    "blocked_paths": ["secrets/", ".env"]
  },
  // 项目特定的 Agent 覆盖
  "agents": {
    "sisyphus": {
      "temperature": 0.3
    }
  }
}
```

---

## AI 提供者配置

### 支持的提供者

| 提供者 | 标识 | 可用模型 | 特殊功能 |
|--------|------|---------|---------|
| **OpenAI** | `openai` | gpt-4, gpt-4o, gpt-4-turbo, gpt-5.4, gpt-5.4-mini | 函数调用、流式输出、视觉 |
| **Anthropic** | `anthropic` | claude-opus-4-6, claude-sonnet-4-6, claude-haiku | 思维块、工具使用 |
| **DeepSeek** | `deepseek` | deepseek-chat, deepseek-coder | 代码专精 |
| **通义千问** | `qianwen` | qwen-turbo, qwen-plus | 阿里 DashScope API |
| **文心一言** | `wenxin` | ernie-bot, ernie-bot-turbo | 百度云 API |

### 基本配置

```jsonc
{
  "ai": {
    "provider": "openai",             // 默认提供者
    "api_key": "sk-xxxxxxxx",         // API Key（推荐用环境变量）
    "model": "gpt-4",                 // 默认模型
    "temperature": 0.7,               // 生成温度（0.0-2.0）
    "max_tokens": 4096,               // 最大输出 Token
    "base_url": "https://api.openai.com/v1"  // API 端点
  }
}
```

### 多提供者配置

```jsonc
{
  "providers": {
    "openai": {
      "api_key": "${OPENAI_API_KEY}",
      "base_url": "https://api.openai.com/v1",
      "api_type": "openai"
    },
    "anthropic": {
      "api_key": "${ANTHROPIC_API_KEY}",
      "base_url": "https://api.anthropic.com",
      "api_type": "anthropic"
    },
    "deepseek": {
      "api_key": "${DEEPSEEK_API_KEY}",
      "base_url": "https://api.deepseek.com",
      "api_type": "openai"           // DeepSeek 兼容 OpenAI API
    },
    "qianwen": {
      "api_key": "${DASHSCOPE_API_KEY}",
      "base_url": "https://dashscope.aliyuncs.com/api/v1",
      "api_type": "dashscope",
      "headers": {
        "X-DashScope-SSE": "enable"
      }
    },
    "wenxin": {
      "api_key": "${WENXIN_API_KEY}",
      "api_secret": "${WENXIN_API_SECRET}",
      "base_url": "https://aip.baidubce.com",
      "api_type": "wenxin",
      "auth_type": "oauth"           // 文心使用 OAuth 认证
    }
  }
}
```

### 环境变量支持

配置值中可使用 `${ENV_VAR}` 语法引用环境变量：

```jsonc
{
  "ai": {
    "api_key": "${OPENAI_API_KEY}",    // 从环境变量读取
    "base_url": "${OPENAI_BASE_URL}"   // 可选的自定义端点
  }
}
```

### 模型角色路由

CEAIR 使用模型角色系统为不同类型的任务选择合适的模型：

| 角色 | 说明 | 典型用途 |
|------|------|---------|
| `Default` | 默认角色 | 通用对话 |
| `Smol` | 快速/轻量 | 简单任务、快速响应 |
| `Slow` | 深度/重量 | 复杂推理、深度分析 |
| `Plan` | 规划专用 | 架构规划、方案设计 |
| `Commit` | 提交信息 | Git 提交消息生成 |

### Fallback 链

当主模型不可用时，自动降级到备选模型：

```jsonc
{
  "model_fallback": {
    "claude-opus-4-6": ["claude-sonnet-4-6", "gpt-5.4"],
    "gpt-5.4": ["gpt-5.4-mini", "claude-sonnet-4-6"],
    "deepseek-coder": ["deepseek-chat", "gpt-5.4"]
  }
}
```

---

## Agent 配置

### 全局 Agent 设置

```jsonc
{
  "agent": {
    "max_iterations": 50,             // 单次任务最大迭代数
    "timeout_secs": 300,              // 任务超时时间（秒）
    "auto_approve_tools": false       // 是否自动批准工具调用
  }
}
```

### 单个 Agent 覆盖

```jsonc
{
  "agents": {
    // 覆盖 Sisyphus 的默认配置
    "sisyphus": {
      "model": "claude-sonnet-4-6",     // 换用更快的模型
      "temperature": 0.5,               // 降低创造性
      "max_tokens": 8192,               // 增加输出长度
      "thinking_level": "High",         // 推理深度
      "max_iterations": 100,            // 增加迭代上限
      "timeout_secs": 600               // 增加超时时间
    },

    // 覆盖 Hephaestus 的配置
    "hephaestus": {
      "model": "gpt-5.4",
      "thinking_level": "XHigh",        // 使用极限推理
      "temperature": 0.3
    },

    // 覆盖 Junior 的配置
    "junior": {
      "model": "claude-sonnet-4-6",     // 固定模型（跳过 Category 路由）
      "default_category": "deep"
    },

    // 覆盖 Librarian 的配置
    "librarian": {
      "model": "qwen-turbo"            // 使用通义千问替代 minimax
    }
  }
}
```

### Thinking Level 配置

控制模型的推理深度：

| 级别 | 值 | 说明 | 适用场景 |
|------|------|------|---------|
| 关闭 | `"Off"` | 不使用推理 | 最简单的任务 |
| 最小 | `"Minimal"` | 极简推理 | 快速响应场景 |
| 低 | `"Low"` | 基本推理 | 常规编码任务 |
| 中等 | `"Medium"` | 标准推理（默认） | 大多数任务 |
| 高 | `"High"` | 深度推理 | 复杂问题 |
| 极高 | `"XHigh"` | 极限推理 | 最困难的问题 |

---

## 工具配置

### 路径权限

```jsonc
{
  "tools": {
    // 允许访问的路径列表（相对于项目根目录）
    "allowed_paths": [
      ".",                              // 当前目录及子目录
      "/usr/local/include"              // 也可以是绝对路径
    ],

    // 禁止访问的路径列表（优先级高于 allowed_paths）
    "blocked_paths": [
      ".env",                           // 环境变量文件
      "secrets/",                       // 密钥目录
      "node_modules/",                  // 依赖目录
      ".git/objects/"                   // Git 内部对象
    ],

    // 最大可读/写文件大小（字节）
    "max_file_size": 10485760           // 10 MB
  }
}
```

**路径优先级规则：**
`blocked_paths` 的优先级始终高于 `allowed_paths`。即使一个路径被 `allowed_paths` 包含，如果它匹配 `blocked_paths` 中的任何模式，访问仍会被拒绝。

### 工具执行参数

```jsonc
{
  "tools": {
    // Bash 工具配置
    "bash": {
      "timeout": 30,                    // 命令超时（秒）
      "max_output_lines": 1000          // 最大输出行数
    },

    // Python 工具配置
    "python": {
      "interpreter": "python3",         // Python 解释器路径
      "timeout": 60                     // 执行超时（秒）
    },

    // 浏览器工具配置
    "browser": {
      "headless": true,                 // 无头模式
      "timeout": 30                     // 页面加载超时
    },

    // Web 搜索配置
    "web_search": {
      "engine": "brave",                // 搜索引擎：brave 或 jina
      "api_key": "${BRAVE_API_KEY}"
    }
  }
}
```

---

## 安全策略

### 工具安全

```jsonc
{
  "security": {
    // 是否需要用户确认写入操作
    "require_approval": true,

    // 自动批准的工具列表（无需确认）
    "auto_approve_tools": [
      "read",
      "grep",
      "find",
      "lsp"
    ],

    // 始终需要确认的工具
    "always_confirm_tools": [
      "bash",
      "delete",
      "ssh"
    ],

    // 沙箱模式（实验性）
    "sandbox": {
      "enabled": false,
      "type": "container"               // container 或 chroot
    }
  }
}
```

### 数据安全

```jsonc
{
  "security": {
    // 审计日志
    "audit": {
      "enabled": true,
      "log_tool_calls": true,           // 记录所有工具调用
      "log_api_requests": false,        // 是否记录 API 请求体
      "data_masking": true              // 自动脱敏敏感数据
    },

    // 密钥混淆
    "key_obfuscation": {
      "enabled": true,
      "patterns": [
        "sk-*",
        "api_key",
        "password",
        "secret"
      ]
    }
  }
}
```

---

## Category 覆盖

Category 系统基于任务意图自动路由到最优模型。你可以覆盖默认的 Category 配置。

### 内置 Category

| Category | 默认模型 | 变体 | 用途 |
|----------|---------|------|------|
| `visual-engineering` | `gemini-3.1-pro` | — | 视觉/UI 工程 |
| `ultrabrain` | `gpt-5.4` | `xhigh` | 超级大脑深度推理 |
| `deep` | `gpt-5.4` | `medium` | 深度思考/自主解决 |
| `artistry` | `gemini-3.1-pro` | `high` | 创意工作 |
| `quick` | `gpt-5.4-mini` | — | 快速任务 |
| `unspecified-low` | `claude-sonnet-4-6` | — | 默认低难度 |
| `unspecified-high` | `claude-opus-4-6` | `max` | 默认高难度 |
| `writing` | `gemini-3-flash` | — | 文档写作 |

### Category 覆盖配置

```jsonc
{
  "categories": {
    // 覆盖 deep 类别
    "deep": {
      "model": "claude-opus-4-6",        // 改用 Claude
      "variant": "high",                 // 推理等级
      "temperature": 0.3,
      "max_tokens": 8192,
      "thinking": true,
      "reasoning_effort": "high"
    },

    // 覆盖 quick 类别
    "quick": {
      "model": "deepseek-chat",          // 使用 DeepSeek
      "temperature": 0.7,
      "max_tokens": 2048
    },

    // 覆盖 writing 类别
    "writing": {
      "model": "qwen-plus",             // 使用通义千问写文档
      "temperature": 0.8,
      "text_verbosity": "detailed"
    }
  }
}
```

### Category 字段详解

| 字段 | 类型 | 说明 |
|------|------|------|
| `model` | `String` | 使用的模型名 |
| `variant` | `String` | 推理变体：`max`, `xhigh`, `high`, `medium`, `low` |
| `temperature` | `f64` | 生成温度 |
| `top_p` | `f64` | 核采样参数 |
| `max_tokens` | `u32` | 最大输出 Token |
| `prompt_append` | `String` | 追加到提示末尾的文本 |
| `thinking` | `bool` | 是否启用思维链 |
| `reasoning_effort` | `String` | 推理努力程度 |
| `text_verbosity` | `String` | 文本详细程度 |
| `tools` | `Object` | 按工具名启用/禁用 |
| `is_unstable_agent` | `bool` | 是否强制后台模式 |

### 意图分类规则

CEAIR 的 Intent Gate 系统自动分类用户意图：

| 意图类型 | 分配 Category | 说明 |
|---------|---------------|------|
| `Research` | `deep` | 调研和探索 |
| `Implementation` | `unspecified-high` | 功能实现 |
| `Fix` | `unspecified-low` | Bug 修复 |
| `Investigation` | `ultrabrain` | 深度调查 |
| `Refactor` | `deep` | 代码重构 |
| `Planning` | `unspecified-high` | 规划任务 |
| `QuickFix` | `quick` | 快速修复 |

---

## Hook 配置

Hook 系统允许在关键事件点注入自定义逻辑。

### Hook 事件

| 事件 | 触发时机 | 说明 |
|------|---------|------|
| `PreSession` | 会话开始前 | 初始化检查 |
| `PostSession` | 会话结束后 | 清理和报告 |
| `PreMessage` | 消息发送前 | 消息过滤/修改 |
| `PostMessage` | 消息返回后 | 结果处理 |
| `PreToolCall` | 工具调用前 | 安全检查 |
| `PostToolCall` | 工具调用后 | 结果审计 |
| `PreCompaction` | 上下文压缩前 | 保护关键信息 |
| `PostCompaction` | 上下文压缩后 | 验证压缩结果 |

### Hook 配置示例

```jsonc
{
  "hooks": {
    // 内联处理器
    "pre_tool_call": [
      {
        "priority": 10,                 // 优先级（数值越小优先级越高）
        "action": "continue"            // 允许执行
      }
    ],

    // 基于脚本的处理器
    "post_message": [
      {
        "priority": 20,
        "script": ".ceair/hooks/log-message.sh",
        "timeout": 5                     // 脚本超时（秒）
      }
    ],

    // 安全检查 Hook
    "pre_tool_call": [
      {
        "priority": 1,                  // 最高优先级
        "script": ".ceair/hooks/security-check.sh",
        "action_on_failure": "block:安全检查未通过"
      }
    ]
  }
}
```

### Hook 处理器动作

| 动作 | 格式 | 说明 |
|------|------|------|
| 继续 | `"continue"` | 允许操作继续执行 |
| 阻止 | `"block:原因"` | 阻止操作并返回原因 |
| 修改 | `"modify:内容"` | 修改操作内容 |
| 跳过 | `"skip"` | 跳过当前操作 |

### Hook 脚本示例

`.ceair/hooks/security-check.sh`：

```bash
#!/bin/bash
# 检查工具调用是否涉及敏感文件
TOOL_NAME="$1"
TOOL_ARGS="$2"

if echo "$TOOL_ARGS" | grep -q "\.env\|secrets\|password"; then
    echo "block:检测到对敏感文件的访问"
    exit 0
fi

echo "continue"
```

---

## Skill 配置

Skill 系统为 Agent 提供领域知识和专业能力。

### Skill 结构

每个 Skill 包含以下内容：

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | `String` | Skill 名称 |
| `description` | `String` | 功能描述 |
| `version` | `String` | 版本号 |
| `rules` | `Vec<String>` | 领域知识规则 |
| `context_files` | `Vec<String>` | 关联的上下文文件 |
| `tools` | `Vec<String>` | 关联的工具列表 |
| `enabled` | `bool` | 是否启用 |

### Skill 来源

| 来源 | 优先级 | 位置 |
|------|--------|------|
| 内置 | 最低 | 程序内部 |
| 用户全局 | 中 | `~/.ceair/skills/` |
| 项目 | 最高 | `.ceair/skills/` |

### Skill 文件格式

创建 `SKILL.md` 文件定义 Skill：

```markdown
---
name: rust-development
description: Rust 开发最佳实践
version: "1.0"
tools:
  - bash
  - read
  - write
  - edit
---

## 规则

1. 使用 `cargo clippy` 进行代码检查
2. 所有公开 API 必须有文档注释
3. 错误处理使用 `thiserror` 或 `anyhow`
4. 异步代码使用 `tokio` 运行时
5. 测试覆盖率目标 > 80%
```

### Skill 配置示例

```jsonc
{
  "skills": {
    // 启用/禁用特定 Skill
    "rust-development": {
      "enabled": true
    },
    "python-development": {
      "enabled": false
    },

    // Skill 发现路径
    "discovery_paths": [
      ".ceair/skills/",
      "~/.ceair/skills/"
    ]
  }
}
```

---

## TUI 配置

终端界面相关设置。

```jsonc
{
  "tui": {
    "theme": "dark",                     // 主题：dark 或 light
    "show_token_usage": true,            // 显示 Token 使用量
    "show_timestamps": true,             // 显示消息时间戳
    "markdown_rendering": true,          // 启用 Markdown 渲染
    "syntax_highlighting": true          // 启用语法高亮
  }
}
```

---

## 日志配置

```jsonc
{
  "logging": {
    "level": "info",                     // 日志级别：trace, debug, info, warn, error
    "format": "pretty",                  // 格式：pretty, json, compact
    "file": "~/.ceair/logs/ceair.log",   // 日志文件路径
    "rotation": "daily"                  // 日志轮转：daily, hourly, size
  }
}
```

---

## 完整配置示例

以下是一个完整的项目级配置文件示例：

```jsonc
// .opencode/ceair.jsonc — 完整配置示例
{
  // === AI 提供者配置 ===
  "ai": {
    "provider": "anthropic",
    "model": "claude-opus-4-6",
    "temperature": 0.7,
    "max_tokens": 4096
  },

  // === 多提供者配置 ===
  "providers": {
    "openai": {
      "api_key": "${OPENAI_API_KEY}",
      "base_url": "https://api.openai.com/v1"
    },
    "anthropic": {
      "api_key": "${ANTHROPIC_API_KEY}",
      "base_url": "https://api.anthropic.com"
    },
    "deepseek": {
      "api_key": "${DEEPSEEK_API_KEY}",
      "base_url": "https://api.deepseek.com"
    }
  },

  // === Agent 全局设置 ===
  "agent": {
    "max_iterations": 50,
    "timeout_secs": 300,
    "auto_approve_tools": false
  },

  // === Agent 覆盖 ===
  "agents": {
    "sisyphus": {
      "temperature": 0.5,
      "thinking_level": "High"
    },
    "hephaestus": {
      "thinking_level": "XHigh",
      "timeout_secs": 600
    }
  },

  // === 工具配置 ===
  "tools": {
    "allowed_paths": ["."],
    "blocked_paths": [".env", "secrets/"],
    "max_file_size": 10485760,
    "bash": {
      "timeout": 30
    }
  },

  // === 安全配置 ===
  "security": {
    "require_approval": true,
    "audit": {
      "enabled": true,
      "data_masking": true
    }
  },

  // === Category 覆盖 ===
  "categories": {
    "deep": {
      "model": "claude-opus-4-6",
      "thinking": true
    }
  },

  // === TUI 配置 ===
  "tui": {
    "theme": "dark",
    "show_token_usage": true,
    "show_timestamps": true
  },

  // === 日志配置 ===
  "logging": {
    "level": "info",
    "format": "pretty"
  },

  // === Hook 配置 ===
  "hooks": {
    "pre_tool_call": [
      {
        "priority": 10,
        "action": "continue"
      }
    ]
  },

  // === Skill 配置 ===
  "skills": {
    "discovery_paths": [
      ".ceair/skills/",
    ],
  },

  // === 会话配置 ===
  "session": {
    "compaction": {
      "enabled": true,
      "max_tokens": 100000,
      "keep_recent": 10
    },
    "memory": {
      "enabled": false
    }
  },
}
```

---

## 默认值速查

| 配置项 | 默认值 |
|--------|--------|
| `ai.provider` | `"openai"` |
| `ai.model` | `"gpt-4"` |
| `ai.temperature` | `0.7` |
| `ai.max_tokens` | `4096` |
| `agent.max_iterations` | `50` |
| `agent.timeout_secs` | `300` |
| `agent.auto_approve_tools` | `false` |
| `tools.allowed_paths` | `["."]` |
| `tools.max_file_size` | `10485760` (10 MB) |
| `tools.bash.timeout` | `30` (秒) |
| `tui.theme` | `"dark"` |
| `tui.show_token_usage` | `true` |
| `tui.show_timestamps` | `true` |
| `session.compaction.enabled` | `true` |
| `session.compaction.max_tokens` | `100000` |
| `session.compaction.keep_recent` | `10` |
| `session.memory.enabled` | `false` |
| `security.sandbox.enabled` | `false` |
| `logging.level` | `"info"` |

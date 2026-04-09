# 快速入门指南

> CEAIR — 基于 Rust 构建的终端 AI 编程代理

## 目录

- [系统要求](#系统要求)
- [安装方法](#安装方法)
- [首次配置](#首次配置)
- [API Key 设置](#api-key-设置)
- [快速开始](#快速开始)
- [常见问题](#常见问题)

---

## 系统要求

### Rust 版本

CEAIR 要求 **Rust 1.75** 或更高版本。推荐使用 `rustup` 管理 Rust 工具链：

```bash
# 安装 rustup（如果尚未安装）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 确认版本
rustc --version   # 需要 >= 1.75
cargo --version
```

### 操作系统支持

| 操作系统 | 支持状态 | 说明 |
|---------|---------|------|
| **macOS** (Intel/Apple Silicon) | ✅ 完全支持 | 主要开发平台 |
| **Linux** (x86_64/aarch64) | ✅ 完全支持 | 需要 OpenSSL 开发库 |
| **Windows** (x86_64) | ⚠️ 实验性 | 部分终端功能受限 |

### 系统依赖

**macOS：**

```bash
# Xcode 命令行工具（通常已安装）
xcode-select --install
```

**Linux (Debian/Ubuntu)：**

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev
```

**Linux (Fedora/RHEL)：**

```bash
sudo dnf install -y gcc openssl-devel pkg-config
```

---

## 安装方法

### 方法一：通过 Cargo 安装（推荐）

```bash
cargo install ceair-cli
```

安装完成后，`ceair` 命令将添加到 `~/.cargo/bin/` 目录下。确保该目录已加入 `PATH`：

```bash
# 在 ~/.bashrc 或 ~/.zshrc 中添加
export PATH="$HOME/.cargo/bin:$PATH"
```

### 方法二：从源码编译

```bash
# 克隆仓库
git clone https://github.com/woyin/ceair_cli.git
cd ceair_cli

# 编译 release 版本
cargo build --release

# 可执行文件位于 target/release/ceair
./target/release/ceair --version
```

如需安装到系统路径：

```bash
cargo install --path crates/ceair-cli
```

### 方法三：开发模式运行

```bash
# 在项目根目录直接运行
cargo run --release -- [参数]

# 例如：
cargo run --release -- --help
```

### 验证安装

```bash
ceair --version
ceair --help
```

---

## 首次配置

CEAIR 使用 JSONC（带注释的 JSON）格式进行配置。首次运行时，建议创建项目级配置文件。

### 初始化项目配置

在你的项目根目录下创建 `.opencode/ceair.jsonc`：

```bash
mkdir -p .opencode
```

创建最小配置文件 `.opencode/ceair.jsonc`：

```jsonc
{
  // CEAIR 项目配置
  "ai": {
    "provider": "openai",
    "model": "gpt-4",
    "temperature": 0.7,
    "max_tokens": 4096
  },
  "agent": {
    "max_iterations": 50,
    "timeout_secs": 300,
    "auto_approve_tools": false
  },
  "tools": {
    "allowed_paths": ["."],
    "max_file_size": 10485760  // 10 MB
  }
}
```

### 配置文件层级

CEAIR 按以下顺序查找和合并配置（后者覆盖前者）：

1. **内置默认值** — 程序内部的默认配置
2. **全局配置** — `~/.ceair/config.jsonc`
3. **项目配置** — `.opencode/ceair.jsonc`（当前目录）

---

## API Key 设置

CEAIR 支持多种 AI 提供者。你需要至少配置一个提供者的 API Key。

### 方法一：环境变量（推荐）

```bash
# OpenAI
export OPENAI_API_KEY="sk-xxxxxxxxxxxxxxxx"

# Anthropic
export ANTHROPIC_API_KEY="sk-ant-xxxxxxxxxxxxxxxx"

# DeepSeek
export DEEPSEEK_API_KEY="sk-xxxxxxxxxxxxxxxx"

# 通义千问 (Qianwen)
export DASHSCOPE_API_KEY="sk-xxxxxxxxxxxxxxxx"

# 文心一言 (Wenxin)
export WENXIN_API_KEY="xxxxxxxxxxxxxxxx"
export WENXIN_API_SECRET="xxxxxxxxxxxxxxxx"
```

建议将环境变量添加到 `~/.bashrc` 或 `~/.zshrc`：

```bash
echo 'export OPENAI_API_KEY="sk-xxxxxxxx"' >> ~/.zshrc
source ~/.zshrc
```

### 方法二：配置文件

在 `.opencode/ceair.jsonc` 中直接设置：

```jsonc
{
  "ai": {
    "provider": "anthropic",
    "api_key": "sk-ant-xxxxxxxxxxxxxxxx",
    "model": "claude-opus-4-6"
  }
}
```

> ⚠️ **安全提示：** 如果在配置文件中保存 API Key，请确保将该文件加入 `.gitignore`，避免意外提交到版本控制系统。

### 方法三：加密存储

CEAIR 支持加密存储 API Key（通过 `ceair-config` 模块）。首次使用时会提示设置加密密码：

```bash
ceair config set-key openai sk-xxxxxxxxxxxxxxxx
```

### 多提供者配置

你可以同时配置多个 AI 提供者，不同的 Agent 会使用不同的模型：

```jsonc
{
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
  }
}
```

---

## 快速开始

### 启动 CEAIR

```bash
# 在项目目录下启动
cd your-project
ceair
```

启动后你将看到终端交互界面（TUI），默认使用 **Sisyphus** Agent（主编排器）。

### 基本交互

在输入框中直接输入自然语言指令：

```
> 帮我分析一下这个项目的架构
```

CEAIR 会自动调用合适的工具（读取文件、搜索代码等）来完成你的请求。

### 使用斜杠命令

输入 `/` 开头的命令执行特定操作：

```
/help          — 显示帮助信息
/model         — 切换 AI 模型
/compact       — 压缩上下文（节省 Token）
/new           — 开始新会话
/exit          — 退出
```

### 深度初始化项目

首次使用建议执行深度初始化，让 CEAIR 全面了解你的项目：

```
/init-deep
```

这将：
- 扫描项目结构
- 创建 `.sisyphus/boulder.json` 状态文件
- 初始化 Agent 工作状态

### 全自动开发模式

启动 UltraWork 全自动模式，让 CEAIR 自主完成开发任务：

```
/ulw-loop
```

或在对话中使用关键词触发：

```
> ulw 帮我实现用户认证模块
```

### 切换 Agent

使用 Tab 键在不同 Agent 之间切换，每个 Agent 有不同的专长：

| 快捷键 | Agent | 用途 |
|--------|-------|------|
| Tab 1 | Sisyphus | 综合编排 |
| Tab 2 | Hephaestus | 深度开发 |
| Tab 3 | Prometheus | 战略规划 |
| Tab 4 | Atlas | 任务执行 |
| Tab 5 | Oracle | 架构分析 |

更多 Agent 信息请参阅 [Agent 文档](agents.md)。

---

## 常见问题

### Q: 编译时出现 OpenSSL 相关错误？

**Linux 用户：** 安装 OpenSSL 开发库：

```bash
# Ubuntu/Debian
sudo apt install libssl-dev

# Fedora
sudo dnf install openssl-devel
```

### Q: API Key 无效或连接超时？

1. 确认 API Key 正确且未过期
2. 检查网络连接，必要时配置代理：

```jsonc
{
  "ai": {
    "base_url": "https://your-proxy.com/v1"
  }
}
```

### Q: 如何查看 Token 使用量？

使用 `/usage` 命令查看当前会话的 Token 消耗统计。也可以在配置中启用实时显示：

```jsonc
{
  "tui": {
    "show_token_usage": true
  }
}
```

### Q: 工具执行需要手动确认？

默认情况下，写入操作（文件修改、命令执行等）需要用户确认。如需自动批准：

```jsonc
{
  "agent": {
    "auto_approve_tools": true
  }
}
```

> ⚠️ **注意：** 自动批准可能导致意外的文件修改，建议在版本控制环境下使用。

---

## 下一步

- 📖 [Agent 详细文档](agents.md) — 了解 11 个专业 Agent
- ⌨️ [斜杠命令参考](commands.md) — 所有可用命令
- ⚙️ [配置参考](configuration.md) — 完整配置选项
- 🔄 [工作流文档](workflows.md) — 自动化开发流程

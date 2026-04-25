# API Switch

> 个人 API 管理与转发中心
> 
> 统一管理多个 AI API 渠道，一个入口访问所有服务，自动故障转移，永不断线。

## ✨ 核心特色

- **永不中断** — 模型填 `auto` 自动匹配最优渠道；指定模型在渠道失效后自动回退到可用渠道
- **多渠道路由** — 支持 OpenAI / Anthropic / Google Gemini / Azure OpenAI / 自定义兼容渠道
- **智能熔断** — 连续失败自动熔断，可配置禁用状态码、重试状态码、禁用关键词
- **轻量便携** — 单文件 EXE，数据跟随程序，拷走即用
- **系统托盘** — 右键快速切换优先 API，关闭窗口自动隐藏到托盘

## 🚀 快速开始

1. 下载最新 `api-switch.exe`
2. 双击运行，数据库自动创建在同目录下
3. 进入「渠道管理」添加 API 渠道，拉取并选择模型
4. 在「API 管理」中启用模型条目
5. 将客户端 API 地址指向 `http://127.0.0.1:9090`，模型填 `auto` 或指定模型名，无需 API Key

## 📦 下载

前往 [Releases](https://github.com/wang1970/API-Switch/releases) 下载最新版本。

## 🔧 支持的 API 渠道

| 渠道 | 认证方式 | 说明 |
|------|----------|------|
| OpenAI | Bearer Token | 标准 OpenAI API |
| Anthropic | x-api-key | Claude 系列模型 |
| Google Gemini | Query Parameter | 使用 OpenAI 兼容端点 |
| Azure OpenAI | api-key Header | 需配置 Deployment 名 |
| Custom | Bearer Token | 兼容 OpenAI 格式的第三方服务 |

## 🛡️ 熔断与容错

- **自动熔断** — 连续失败达到阈值自动跳过该渠道，恢复时间可配置
- **自动禁用** — 匹配到指定状态码（默认 401）或关键词（余额不足、账号禁用等）自动禁用条目
- **故障转移** — 当前渠道失败自动尝试下一个可用渠道
- **504/524 不重试** — 网关超时直接返回，不浪费配额

## ⚙️ 配置说明

代理默认监听 `9090` 端口，可在「系统设置」中修改。

### 客户端接入

```
API Base URL: http://127.0.0.1:9090
API Key: 任意（可在设置中开启强制验证）
Model: auto（自动匹配）或指定模型名
```

### 路由规则

- `model: auto` — 按 API 池中启用条目的优先级自动选择
- `model: gpt-4o` — 精确匹配，失败后回退到 `auto` 流程
- 托盘右键可随时切换优先渠道

## 🏗️ 技术栈

- **桌面框架**: Tauri v2 (Rust + Web)
- **后端**: Rust + Axum + SQLite (WAL)
- **前端**: React 19 + TypeScript + Tailwind CSS v4
- **协议适配**: 5 种 API 类型独立适配器，互不影响

## 📁 文件结构

```
api-switch.exe          # 主程序（绿色便携版）
api-switch.db           # 数据库（首次运行自动创建）
```

所有数据存储在 EXE 同目录下，删除即可完全卸载。

## 📜 开源协议

[MIT License](LICENSE)

## ⭐ Star

如果觉得好用，欢迎到 [GitHub](https://github.com/wang1970/API-Switch) 给个 Star！

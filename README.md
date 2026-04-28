# API Switch

> Personal AI API Management & Forwarding Hub
> 多渠道路由 · 自动故障转移 · 一键测速 · 桌面便携

Manage multiple AI API providers through a single endpoint with automatic failover — never go down.

管理多个 AI API 渠道，统一入口，自动故障转移，永不宕机。

---

## ✨ 核心功能

| 功能 | 说明 |
|------|------|
| **多渠道路由** | 一个入口访问多个 AI 服务商，按模型自动匹配或手动指定 |
| **永不宕机** | 模型设置为 `auto`，自动匹配最优渠道；失败自动冷却并切换到下一个可用渠道 |
| **一键测速** | 渠道 & 模型逐个测速，成功显示绿色响应时间，失败显示红色 ✗，帮你排除不可用渠道和模型 |
| **熔断优化** | 自动禁用不可恢复模型（401/403/410），冷却中的模型不参与路由，成功自动恢复 |
| **渠道自动校对** | 添加渠道时一键拉取模型，自动检测 API 类型、校对 Base URL，支持中转站模型发现 |
| **智能模型预选** | 拉取模型后自动选中 6 个月内新模型 + 已有模型，新增条目默认开启 |
| **托盘快捷切换** | 右键系统托盘图标，直接选择模型发起对话，无需打开主窗口 |
| **中英双语** | 界面和用户指南支持中英文切换 |
| **绿色便携** | 单文件 EXE，数据存储在同目录，复制即用 |

---

## 🚀 快速开始

1. 从 [Releases](https://github.com/wang1970/API-Switch/releases) 下载对应平台版本
2. 运行 — 数据库自动创建在同目录下
3. 进入 **渠道管理** 添加 API 渠道，拉取并选择模型
4. 进入 **API 管理** 查看和启用模型
5. 将客户端 API 地址指向 `http://127.0.0.1:9090`，模型名设为 `auto` 或指定模型名

### 客户端配置

```
API Base URL: http://127.0.0.1:9090
API Key: 任意（可在设置中开启强制验证）
Model: auto（智能匹配）或指定模型名
```

### 路由规则

| 模式 | 行为 |
|------|------|
| `model: auto` | 从已启用且未冷却的条目中，按优先级自动选择 |
| `model: gpt-4o` | 精确匹配同名条目，失败时 fallback 到 auto 流程 |
| 托盘右键 | 随时切换优先模型 |

---

## 📦 下载

| 平台 | 文件 |
|------|------|
| Windows x64 | `api-switch-*-windows-x64.exe` |
| macOS Intel | `api-switch-*-macos-x64` |
| macOS Apple Silicon | `api-switch-*-macos-arm64` |
| Linux x64 | `api-switch-*-linux-x64` |

访问 [Releases](https://github.com/wang1970/API-Switch/releases) 获取最新版本。

---

## 🔧 支持的 API 类型

| 类型 | 认证方式 | 说明 |
|------|---------|------|
| OpenAI | Bearer Token | 标准 OpenAI API |
| Anthropic | x-api-key | Claude 系列模型，完整格式转换 |
| Google Gemini | Query Parameter | OpenAI 兼容端点 |
| Azure OpenAI | api-key Header | Deployment 名称路由 |
| Custom | Bearer Token | 任何 OpenAI 兼容的第三方服务（中转站等） |

---

## 🛡️ 容错机制

- **模型冷却** — 任意上游失败自动冷却 300s，冷却中的模型不参与路由
- **自动恢复** — 请求成功后自动清除冷却状态
- **自动禁用** — 收到 401/403/410 等状态码时自动关闭不可恢复的模型（可在设置中自定义）
- **故障转移** — 自动尝试下一个可用渠道，全部失败返回 502
- **用户开关神圣** — `enabled` 只由用户手动控制，系统不会自动启用已关闭的条目

---

## 📖 使用指南

- [中文指南](GUIDE_CN.md)
- [English Guide](GUIDE.md)

---

## ⚙️ 配置

代理默认监听端口 `9090`，可在 **设置 → 代理设置** 中修改。

冷却恢复时间默认 600s（可在 **设置 → 熔断** 中通过滑块调整，范围 300-1800s）。

---

## 🏗️ 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri v2（Rust + Web） |
| 后端 | Rust + Axum + SQLite（WAL 模式） |
| 前端 | React 19 + TypeScript + Tailwind CSS v4 |
| 协议适配 | 5 种独立适配器模块，互不影响 |

---

## 📁 文件结构

```
api-switch.exe          # 主程序（绿色便携版）
api-switch.db           # 数据库（首次运行自动创建）
```

所有数据存储在程序同目录下。删除这两个文件即可完全卸载。

---

## 📜 License

[MIT License](LICENSE)

---

## ⭐ Star

如果觉得有用，欢迎在 [GitHub](https://github.com/wang1970/API-Switch) 上点个 Star！

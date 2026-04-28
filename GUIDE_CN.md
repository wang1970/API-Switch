# API Switch 使用指南

## 快速开始

### 1. 添加渠道

进入「渠道管理」，点击添加渠道，填写：

| 配置项 | 说明 |
|--------|------|
| 名称 | 自定义，方便识别 |
| API 类型 | 选择对应的服务商类型 |
| Base URL | 上游 API 地址（不需要带 `/v1`） |
| API Key | 服务商提供的密钥 |

添加后点击「拉取模型」获取可用模型列表，勾选需要使用的模型。

### 2. 启用模型

进入「API 管理」，开启需要使用的模型条目（绿点 = 可用）。

### 3. 开始使用

将客户端的 API Base URL 设为：

```
http://127.0.0.1:9090/v1
```

API Key 留空即可（未开启访问密钥验证时）。

请求时 model 填 `auto` 即可自动匹配最优渠道，也可以填具体模型名称。

---

## CODING PLAN / API-Switch 推荐配置

### MiniMax（硅基流动）

| 配置项 | 值 |
|--------|-----|
| API 类型 | `openai` 或 `anthropic` |
| Base URL | `https://api.minimaxi.com` 或 `https://api.minimax.chat` |
| API Key | 你的 Key |
| API 管理添加模型 | `MiniMax-M2.7`（需手动填写） |

### CODING PLAN

| 配置项 | 值 |
|--------|-----|
| API 类型 | `openai` |
| Base URL | `https://api.rcouyi.com` |
| API Key | 你的 Key |
| 拉取模型 | 不支持，需手动添加 |
| API 管理添加模型 | `gemini-2.0-flash`、`gemini-2.5-pro` 等（需手动填写） |

> 此类中转站的模型列表接口不可用，拉取会失败。在渠道中添加 Key 后，直接到「API 管理」点击「添加 API」手动填写模型名称即可正常使用。

---

## 常见问题

### 代理启动失败：端口被占用

修改「系统设置 → 代理设置 → 监听端口」为其他端口，重启代理。

### 请求返回 401

1. 如果开启了「强制验证访问密钥」，请求必须携带 Access Key：
   ```
   Authorization: Bearer sk-xxx
   ```
2. 如果未开启，检查渠道的 API Key 是否正确。

### 请求返回 "No available provider"

1. 确认渠道已启用。
2. 确认对应的模型条目已启用（绿点）。
3. 确认模型未处于冷却中（红点）。冷却中的模型不参与路由。

### 模型显示红点（冷却中）

模型请求失败后会被自动冷却，冷却期间不参与路由。默认冷却时间 300 秒，可在「系统设置 → 熔断机制」中调整。冷却到期后模型会自动恢复。

### 拉取模型失败

1. 检查 Base URL 是否正确（不需要带 `/v1` 后缀）。
2. 检查 API Key 是否有效。
3. 检查网络是否能访问上游 API。

### Claude 渠道配置

- API 类型选择 `claude`
- Base URL 填写 `https://api.anthropic.com`
- 模型名称需要与上游一致，如 `claude-sonnet-4-20250514`

### Gemini 渠道配置

- API 类型选择 `gemini`
- Base URL 填写 `https://generativelanguage.googleapis.com`
- API Key 填写 Google AI Studio 的 Key

### Azure OpenAI 配置

- API 类型选择 `azure`
- Base URL 填写 Endpoint 地址
- 模型名称填写部署名称（Deployment Name）

### 托盘菜单模型顺序不对

托盘显示前 5 个可用的启用模型，按 API 管理页的排序。在 API 管理页拖拽调整顺序即可。

### 日志中看到 (auto) 前缀

表示请求的 model 为 `auto`，括号后是实际命中的模型名称。

---

> 本文档会持续更新，如果遇到其他问题欢迎提 [Issue](https://github.com/wang1970/API-Switch/issues)。

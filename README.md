# API Switch

> Personal API Management & Forwarding Hub
>
> Manage multiple AI API providers through a single endpoint with automatic failover — never go down.

## ✨ Features

- **Never Goes Down** — Set model to `auto` for optimal channel matching; if a specific model's channel fails, it automatically falls back to available ones
- **Multi-Provider Routing** — OpenAI / Anthropic / Google Gemini / Azure OpenAI / Custom compatible providers
- **Smart Circuit Breaker** — Auto-trip on consecutive failures, configurable disable/retry status codes and keywords
- **Portable** — Single EXE file, data stored alongside the executable, copy and run anywhere
- **System Tray** — Right-click to switch priority API, close to tray instead of exiting

## 🚀 Quick Start

1. Download the latest `api-switch.exe` from [Releases](https://github.com/wang1970/API-Switch/releases)
2. Double-click to run — database is auto-created in the same directory
3. Go to **Channels** to add your API providers, fetch and select models
4. Go to **API Mgmt** and enable the model entries you want to use
5. Point your client's API base URL to `http://127.0.0.1:9090`, set model to `auto` or any specific name — no API Key required

## 🔧 Supported Providers

| Provider | Auth Method | Description |
|----------|-------------|-------------|
| OpenAI | Bearer Token | Standard OpenAI API |
| Anthropic | x-api-key | Claude series models |
| Google Gemini | Query Parameter | OpenAI-compatible endpoint |
| Azure OpenAI | api-key Header | Deployment name required |
| Custom | Bearer Token | Any OpenAI-compatible third-party service |

## 🛡️ Circuit Breaker & Fault Tolerance

- **Auto Circuit Breaker** — Skip channel after consecutive failures, configurable recovery time
- **Auto Disable** — Auto-disable entries on specific status codes (default: 401) or error keywords (insufficient balance, account disabled, etc.)
- **Failover** — Automatically try the next available channel on failure
- **504/524 No Retry** — Gateway timeouts return immediately without wasting quota

## ⚙️ Configuration

Proxy listens on port `9090` by default, configurable in **Settings**.

### Client Setup

```
API Base URL: http://127.0.0.1:9090
API Key: anything (enforce validation in Settings if needed)
Model: auto (smart match) or any specific model name
```

### Routing Rules

- `model: auto` — Selects automatically by priority from enabled entries
- `model: gpt-4o` — Exact match, falls back to `auto` flow on failure
- Switch priority anytime via tray right-click menu

## 🏗️ Tech Stack

- **Desktop**: Tauri v2 (Rust + Web)
- **Backend**: Rust + Axum + SQLite (WAL mode)
- **Frontend**: React 19 + TypeScript + Tailwind CSS v4
- **Protocol Adapters**: 5 independent adapter modules, isolated from each other

## 📁 File Structure

```
api-switch.exe          # Main program (portable)
api-switch.db           # Database (auto-created on first run)
```

All data is stored alongside the executable. Delete both files to completely remove.

## 📜 License

[MIT License](LICENSE)

## ⭐ Star

If you find it useful, consider giving it a Star on [GitHub](https://github.com/wang1970/API-Switch)!

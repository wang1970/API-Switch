# API Switch

> Personal AI API Management & Forwarding Hub

Manage multiple AI API providers through a single endpoint with automatic failover — never go down.

---

## ✨ Features

| Feature | Description |
|---------|-------------|
| **Multi-Provider Routing** | One endpoint for multiple AI providers, auto-match or manually specify models |
| **Never Goes Down** | Set model to `auto` for optimal channel matching; auto-cooldown on failure and switch to next available |
| **One-Click Speed Test** | Test all channels & models sequentially — green response time for success, red ✗ for failure |
| **Smart Circuit Breaker** | Auto-disable unrecoverable models (401/403/410), cooldown skips failed models, auto-recover on success |
| **Auto Channel Calibration** | One-click model fetch with auto API type detection, Base URL correction, relay station model discovery |
| **Smart Model Pre-selection** | Auto-select models released within 6 months + existing models; new entries enabled by default |
| **System Tray** | Right-click tray icon to switch models directly without opening the main window |
| **Bilingual** | UI and user guide support Chinese/English |
| **Portable** | Single EXE file, data stored alongside the executable, copy and run anywhere |

---

## 🚀 Quick Start

1. Download from [Releases](https://github.com/wang1970/API-Switch/releases)
2. Run — database is auto-created in the same directory
3. Go to **Channels** to add API providers, fetch and select models
4. Go to **API Management** to view and enable models
5. Point your client's API base URL to `http://127.0.0.1:9090`, set model to `auto` or any specific name

### Client Setup

```
API Base URL: http://127.0.0.1:9090
API Key: anything (enforce validation in Settings if needed)
Model: auto (smart match) or any specific model name
```

### Routing Rules

| Mode | Behavior |
|------|----------|
| `model: auto` | Automatically select from enabled & non-cooled entries by priority |
| `model: gpt-4o` | Exact match, falls back to auto flow on failure |
| Tray right-click | Switch priority model anytime |

---

## 📦 Downloads

| Platform | File |
|----------|------|
| Windows x64 | `api-switch-*-windows-x64.exe` |
| macOS Intel | `api-switch-*-macos-x64` |
| macOS Apple Silicon | `api-switch-*-macos-arm64` |
| Linux x64 | `api-switch-*-linux-x64` |

Visit [Releases](https://github.com/wang1970/API-Switch/releases) for the latest version.

---

## 🔧 Supported Providers

| Type | Auth Method | Description |
|------|-------------|-------------|
| OpenAI | Bearer Token | Standard OpenAI API |
| Anthropic | x-api-key | Claude series models, full format conversion |
| Google Gemini | Query Parameter | OpenAI-compatible endpoint |
| Azure OpenAI | api-key Header | Deployment name routing |
| Custom | Bearer Token | Any OpenAI-compatible third-party service |

---

## 🛡️ Fault Tolerance

- **Model Cooldown** — Any upstream failure triggers 300s cooldown; cooled models are skipped in routing
- **Auto Recovery** — Successful request automatically clears cooldown state
- **Auto Disable** — Auto-disable entries on 401/403/410 status codes (configurable in Settings)
- **Failover** — Automatically try the next available channel; returns 502 if all fail
- **User Controls Are Sacred** — `enabled` toggle is only controlled by the user; the system never auto-enables

---

## 📖 User Guide

- [English Guide](GUIDE.md)
- [中文指南](GUIDE_CN.md)

---

## ⚙️ Configuration

Proxy listens on port `9090` by default, configurable in **Settings → Proxy**.

Cooldown recovery time defaults to 600s (adjustable via slider in **Settings → Circuit Breaker**, range 300-1800s).

---

## 🏗️ Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop | Tauri v2 (Rust + Web) |
| Backend | Rust + Axum + SQLite (WAL mode) |
| Frontend | React 19 + TypeScript + Tailwind CSS v4 |
| Protocol Adapters | 5 independent adapter modules, isolated from each other |

---

## 📁 File Structure

```
api-switch.exe          # Main program (portable)
api-switch.db           # Database (auto-created on first run)
```

All data is stored alongside the executable. Delete both files to completely remove.

---

## 📜 License

[MIT License](LICENSE)

---

## ⭐ Star

If you find it useful, consider giving it a Star on [GitHub](https://github.com/wang1970/API-Switch)!

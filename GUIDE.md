# API Switch User Guide

## Quick Start

### 1. Add a Channel

Go to **Channel Management**, click **Add Channel**, and fill in:

| Field | Description |
|-------|-------------|
| Name | Custom name for easy identification |
| API Type | Select the corresponding provider type |
| Base URL | Upstream API address (no need to include `/v1`) |
| API Key | Key provided by the provider |

After adding, click **Fetch Models** to get the available model list, then check the models you want to use.

### 2. Enable Models

Go to **API Management** and enable the model entries you want to use (green dot = available).

### 3. Start Using

Set your client's API Base URL to:

```
http://127.0.0.1:9090/v1
```

Leave the API Key empty (when access key verification is not enabled).

Use `auto` as the model name for automatic optimal channel matching, or specify an exact model name.

---

## Recommended Channel Configurations

### MiniMax (SiliconFlow)

| Field | Value |
|-------|-------|
| API Type | `openai` or `anthropic` |
| Base URL | `https://api.minimaxi.com` or `https://api.minimax.chat` |
| API Key | Your Key |
| Add Model Manually | `MiniMax-M2.7` |

### CODING PLAN

| Field | Value |
|-------|-------|
| API Type | `openai` |
| Base URL | `https://api.rcouyi.com` |
| API Key | Your Key |
| Fetch Models | Not supported, add manually |
| Add Model Manually | `gemini-2.0-flash`, `gemini-2.5-pro`, etc. |

> The model list API for relay services like this is unavailable, so fetching will fail. After adding your Key in the channel, go directly to **API Management** and click **Add API** to manually enter the model name.

---

## FAQ

### Proxy Failed to Start: Port in Use

Change the listening port in **Settings → Proxy Settings** to another port, then restart the proxy.

### Request Returns 401

1. If **Enforce Access Key Verification** is enabled, the request must include an Access Key:
   ```
   Authorization: Bearer sk-xxx
   ```
2. If not enabled, check that the channel's API Key is correct.

### Request Returns "No available provider"

1. Confirm the channel is enabled.
2. Confirm the corresponding model entry is enabled (green dot).
3. Confirm the model is not in cooldown (red dot). Models in cooldown do not participate in routing.

### Model Shows Red Dot (Cooldown)

Models that fail requests are automatically cooled down and do not participate in routing during the cooldown period. Default cooldown is 300 seconds, adjustable in **Settings → Circuit Breaker**. Models recover automatically after cooldown expires.

### Failed to Fetch Models

1. Check that the Base URL is correct (no need to include the `/v1` suffix).
2. Check that the API Key is valid.
3. Check that the network can access the upstream API.

### Claude Channel Configuration

- Select API type `claude`
- Base URL: `https://api.anthropic.com`
- Model name must match the upstream, e.g. `claude-sonnet-4-20250514`

### Gemini Channel Configuration

- Select API type `gemini`
- Base URL: `https://generativelanguage.googleapis.com`
- API Key: Google AI Studio Key

### Azure OpenAI Configuration

- Select API type `azure`
- Base URL: Endpoint address
- Model name: Deployment Name

### Tray Menu Model Order is Incorrect

The tray shows the top 5 enabled available models, sorted by the order in API Management. Drag to adjust the order in API Management.

### Log Shows (auto) Prefix

Indicates that the requested model is `auto`, and the text after the parentheses is the actual model matched.

---

> This document is continuously updated. If you encounter other issues, feel free to open an [Issue](https://github.com/wang1970/API-Switch/issues).

# API Switch - 项目计划书

> Personal API Management & Forwarding Center
> 版本: 0.4.1 | 更新日期: 2026-04-28

---

## 1. 项目概述

**API Switch** 是一款基于 Tauri v2 的桌面应用，用于统一管理和转发多个 AI API 渠道。面向个人本地使用，默认信任本机环境，不按公网多用户服务进行安全模型设计。

### 核心价值

- **多渠道路由**：一个入口访问多个 AI 服务商，按模型自动匹配或手动指定
- **提升可用性**：模型冷却 + 自动故障转移，降低单渠道故障对使用的影响
- **用量可视化**：实时 Dashboard + 请求日志 + Token 消耗统计
- **轻量桌面**：Tauri v2 架构，内存占用低，跨平台

---

## 2. 技术架构

### 2.1 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| 桌面框架 | Tauri v2 | Rust 后端 + Web 前端 |
| 后端语言 | Rust 1.85+ | 高性能异步运行时 |
| HTTP 框架 | axum 0.7 | 代理服务器 & API 路由 |
| HTTP 客户端 | reqwest 0.12 | 转发请求到上游 (rustls-tls) |
| 数据库 | rusqlite 0.31 (bundled) | 嵌入式 SQLite，WAL 模式 |
| 前端框架 | React 19 + TypeScript 5.8 | UI 渲染层 |
| UI 组件 | Radix UI + Tailwind CSS v4 | 无障碍组件库 |
| 状态管理 | TanStack React Query v5 | 服务端状态缓存与自动刷新 |
| 图表 | Recharts v3 | Dashboard 可视化 |
| 国际化 | i18next + react-i18next | 中/英双语 |
| 拖拽 | @dnd-kit | API 管理排序 |

### 2.2 整体架构

```
┌─────────────────────────────────────────────────────┐
│                    Tauri App Window                   │
│  ┌───────────────────────────────────────────────┐  │
│  │              React Frontend (Vite)             │  │
│  │ Dashboard │ Channel │ API 管理 │ Token │ Logs │ Settings │ Guide │  │
│  └──────────────┬────────────────────────────────┘  │
│                 │ Tauri IPC (invoke)                  │
│  ┌──────────────▼────────────────────────────────┐  │
│  │           Tauri Commands Layer                 │  │
│  └──────────────┬────────────────────────────────┘  │
│                 │                                     │
│  ┌──────────────▼────────────────────────────────┐  │
│  │              AppState (Arc<Database>)          │  │
│  └──────────────┬────────────────────────────────┘  │
│                 │                                     │
│  ┌──────────────▼────────────────────────────────┐  │
│  │           SQLite (rusqlite + Mutex)            │  │
│  │  channels │ api_entries │ access_keys │ logs   │  │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘

         ▲ 并行运行
┌─────────────────────────────────────────────────────┐
│              Axum Proxy Server (0.0.0.0:port)        │
│  ┌─────────────┐  ┌─────────┐  ┌────────────────┐  │
│  │   /health   │  │ /v1/models│  │/v1/chat/completions│
│  └─────────────┘  └─────────┘  └───────┬────────┘  │
│                                       │             │
│  ┌──────────▼──────────────────────────────────┐   │
│  │  Auth → Router → Forwarder (retry+failover)  │   │
│  │         ↕ Cooldown (per entry, DB 持久化)     │   │
│  └──────────┬──────────────────────────────────┘   │
│             │ reqwest → Upstream APIs               │
└─────────────────────────────────────────────────────┘
```

---

## 3. 模块详解

### 3.1 后端模块 (`src-tauri/src/`)

| 模块 | 文件 | 职责 |
|------|------|------|
| 入口 | `main.rs`, `lib.rs` | Tauri 初始化、托盘菜单、代理自启 |
| 数据库 | `database/mod.rs`, `schema.rs` | SQLite 连接、建表、兼容迁移 |
| 数据访问 | `database/dao/*.rs` | 渠道/条目/密钥/日志/配置 CRUD |
| 代理服务器 | `proxy/server.rs` | Axum HTTP 服务器、graceful shutdown |
| 请求处理 | `proxy/handlers.rs` | 入口路由、Access Key 验证 |
| 智能路由 | `proxy/router.rs` | AUTO/显式模型匹配、冷却过滤 |
| 请求转发 | `proxy/forwarder.rs` | Failover、冷却、日志、SSE 流式处理 |
| 冷却机制 | `proxy/circuit_breaker.rs` | 内存三态熔断器（辅助 DB 冷却） |
| 协议适配 | `proxy/protocol/*.rs` | 5 种 API 类型独立适配器 |
| 认证 | `proxy/auth.rs` | Bearer Key 提取与验证 |
| 命令层 | `commands/*.rs` | Tauri IPC 接口 |

### 3.2 前端模块 (`src/`)

| 页面 | 文件 | 功能 |
|------|------|------|
| API 管理 | `ApiPoolPage.tsx` | 拖拽排序、启停、状态点（绿/红/灰）、测试对话、一键测速、响应时间显示 |
| 渠道管理 | `ChannelPage.tsx` | 统一添加/编辑弹窗、模型拉取/选择 |
| 令牌管理 | `TokenPage.tsx` | 密钥 CRUD + 复制 |
| 使用日志 | `LogPage.tsx` | 分页、成功/失败筛选、点击行展开详情 |
| 数据看板 | `DashboardPage.tsx` | 统计卡片 + 4 图表 |
| 系统设置 | `SettingsPage.tsx` | 代理、安全、冷却、托盘、通用设置 |
| 使用指南 | 侧边栏菜单项 | 按语言切换中/英文 GUIDE（GUIDE_CN.md / GUIDE.md） |
| 测试对话 | `TestChatDialog.tsx` | 直接调 Tauri 命令请求上游，不走代理 |

---

## 4. 数据流

### 4.1 请求代理流程

```
Client → POST /v1/chat/completions
  │
  ├─ 1. auth::extract_access_key()       ← 从 Header 提取并验证密钥
  ├─ 2. 解析 JSON body → model / stream
  ├─ 3. router::resolve()                ← 从 DB 查询 enabled + 未冷却 entries
  ├─ 4. forwarder::forward_with_retry()
  │     ├─ 遍历 entries:
  │     │   ├─ adapter.build_chat_url() + apply_auth() + transform_request()
  │     │   ├─ reqwest::send()
  │     │   ├─ 成功 → 清除冷却 → 返回客户端
  │     │   └─ 失败 → 设置冷却 → 继续下一个
  │     └─ 全部失败 → 502 AllProvidersFailed
  └─ 5. insert_usage_log()
```

### 4.2 冷却机制

```text
正常请求 → 直接返回，并清除该模型 cooldown_until
不正常请求 → 写失败日志，设置 cooldown_until = now + 300s，继续 failover
正式路由 → 只选择 enabled 且未冷却的模型
用户禁用 → 仍由 enabled=false 表达，系统不会自动改 enabled
```

### 4.3 AUTO 机制

| 场景 | 行为 |
|---|---|
| `model = "auto"` | 使用 enabled + 未冷却条目，按 `sort_index, created_at` 稳定排序依次尝试 |
| 显式模型且存在 enabled 同名条目 | 同名条目优先，再追加 AUTO 池作为 fallback |
| 显式模型不存在 | 直接 fallback 到 AUTO 池 |
| disabled / 冷却中条目 | 不进入正式代理路由，仅测试对话可直接调用 |

---

## 5. 数据库兼容开发规范

> **规则**：每个版本新增数据库字段或配置项时，必须在启动检查机制中补齐，确保老用户升级后数据库自动兼容。

| 检查项 | 机制 | 位置 | 说明 |
|--------|------|------|------|
| 新增表字段 | `ensure_column()` | `schema.rs` → `ensure_*_columns()` | `PRAGMA table_info` 检查，不存在则 `ALTER TABLE ADD COLUMN` |
| 新增 config key | `INSERT OR IGNORE` | `schema.rs` → `defaults` 数组 | 不覆盖用户已有值，只补缺失的 key |
| 旧默认值迁移 | `UPDATE ... WHERE value = '旧值'` | `schema.rs` → `create_tables()` 末尾 | 只迁移未修改的旧默认值 |
| 前端类型同步 | `types.ts` | `ApiEntry` / `AppSettings` | 新增后端字段必须同步前端类型 |

**每次新增字段/配置必须做的事**：

1. `schema.rs`：建表 SQL 中加新字段
2. `schema.rs`：`ensure_*_columns()` 中加 `ensure_column()` 调用
3. `schema.rs`：`defaults` 数组中加新 config key 默认值
4. `config_dao.rs`：`AppSettings` 中加字段 + `get_settings()` 读取 + `update_settings()` 写入
5. `types.ts`：`ApiEntry` / `AppSettings` 中加对应字段
6. 如有旧值需迁移，在 `create_tables()` 末尾加 `UPDATE` 语句

---

## 6. 表结构

| 表名 | 用途 | 关键字段 |
|------|------|----------|
| `channels` | API 渠道 | id, name, api_type, base_url, api_key, available_models(JSON), selected_models(JSON), enabled |
| `api_entries` | 路由池条目 | id, channel_id, model, display_name, sort_index, enabled, cooldown_until, response_ms |
| `access_keys` | 访问密钥 | id, name, key(UUID), enabled |
| `usage_logs` | 请求日志 | 25+ 字段，含 token 统计、延迟、错误信息 |
| `config` | 全局配置 | KV 存储 |

---

## 7. 协议适配

5 种 API 类型各自独立实现 `ProtocolAdapter` trait，互不影响：

| API 类型 | 认证方式 | 聊天端点 | 模型列表端点 | 说明 |
|----------|---------|---------|-------------|------|
| `openai` | Bearer | `/v1/chat/completions` | `/v1/models` | 标准 OpenAI |
| `claude` | x-api-key | `/v1/messages` | `/v1/models` | 完整格式转换 |
| `gemini` | ?key= 查询参数 | `/v1beta/openai/chat/completions` | `/v1beta/openai/models` | Google OpenAI 兼容端点 |
| `azure` | api-key header | `/openai/deployments/{model}/chat/completions` | `/openai/models` | Deployment 名路由 |
| `custom` | Bearer | 用户 base_url 完整路径 | 用户 base_url 完整路径 | 不自动拼接 /v1 |

---

## 8. 设计取舍

| 项 | 当前取舍 | 原因 |
|---|---|---|
| API Key 明文存储 | 接受 | 个人本地工具降低复杂度 |
| Access Key 可关闭 | 接受 | 本机使用优先降低门槛 |
| SQLite + Mutex | 接受 | 单机低并发场景足够 |
| 冷却状态 DB 持久化 | 接受 | 重启后坏模型不会立即恢复 |
| CORS 宽松 | 接受 | 非公网服务 |
| Custom base_url 不拼接 /v1 | 接受 | 用户填写完整版本路径 |
| 不新增 API 类型处理中转站 | 接受 | 用 openai 类型 + 手动添加模型即可 |

---

## 9. 待开发 / 改进项

### P1 — 个人使用体验与稳定性

- [ ] **Responses API 支持**: 新增 `/v1/responses` 路由，支持 OpenAI Responses API 格式（GPT-5.5 等新模型需要），包含请求/响应格式转换
- [ ] **客户端断开精准检测**: 细分 client_gone / runtime cancellation / 其他 drop
- [ ] **前端统一 Toast 错误提示**: 替代零散 `alert()` 和静默失败
- [ ] **程序放外网**: 打包发布到公网，支持外部访问
- [ ] **错误冷却策略优化 (个人模式)**:
    - **问题**: 当前所有非永久禁用错误（401/403/410 除外）都会进入 `cool_down` (默认 5 分钟)，导致对**不可恢复错误**进行无意义的反复重试，浪费用户时间。
    - **核心思路**: **中间状态不落地**。连续失败计数完全在**内存**中维护，重启归零，无需数据库字段。数据库只存持久状态。
    - **规则**:
        1. **任何冷却的错误都计数**（不论 4xx/5xx/网络）。每失败一次，内存计数器 +1。
        2. **达到阈值**（如 3 次连续失败）→ **永久禁用** (`enabled=false`)，并设 `cooldown_until=now+1day`（24 小时后自动恢复尝试一次）。禁用后**路由完全不可见**（AUTO 和指定模型均跳过）。
        3. **阈值内**（未达到）：正常冷却（`cooldown_until=now+N秒`），正常路由可见。
        4. **强制开启**：用户手动 `enabled=true` → 清除计数 + 清除冷却，重新开始计数。
        5. **成功请求**：清除计数 + 清除冷却。
    - **补充点**:
        - **24 小时恢复尝试**: "1Day 冷却" 不是永久禁用，24 小时后 `cooldown_until` 过期，模型自动回到池中尝试一次。如果又失败 → 再禁 1Day。这样用户不手动干预也可能在服务恢复后自动回归。
        - **AUTO 和指定模型**: 统一通过 `enabled + cooldown_until` 过滤，不区分两种路由模式。
        - **用户感知**: UI 上不新增"自动禁用"状态点，复用现有灰点 (`!enabled`) + 冷却剩余时间显示。但如果用户看到"已禁用 23h"会困惑，建议在冷却提示文案上区分：`"冷却 30s"` vs `"故障 23h"`。
        - **重启**: 内存计数清零，但 `cooldown_until` 和 `enabled` 在数据库中持久化，重启后 24 小时冷却依然生效。
        - **测试对话**: 测试对话（`test_chat`）不走正式路由，应该允许跳过冷却状态直接测试，便于用户验证入口是否恢复。
    - **牵连点**:
        - **后端**:
            - `server.rs`: 新增 `failure_counts: HashMap<String, u32>` 到 `ProxyState`。
            - `forwarder.rs`: 修改 `cool_down_entry` 计数+阈值判断逻辑；修改 `record_circuit_success` 清除计数；修改 `spawn_cool_down_entry` 同步逻辑。
            - `pool.rs`: 修改 `toggle_entry` (手动开启时) 清除计数。
            - `router.rs`: 无需改动（已通过 `enabled + cooldown_until` 过滤）。
        - **前端**:
            - `ApiPoolPage.tsx`: 修改冷却文案显示，区分"临时冷却"和"长时间冷却"。
            - 新增 i18n 文案（如 `"cooldownLong"`）。
        - **数据库**: **零改动** (复用 `enabled`, `cooldown_until`)。
    - **收益**: 无意义重试次数从 ∞ 降到 3 次；24h 后自动恢复一次；用户可手动强制开启。

### P2 — 常用体验增强

- [x] **模型/渠道测速**: 对指定模型或渠道进行延迟测试（TTFB），排序展示结果，帮助用户选择最优渠道
- [ ] **CLI 配置片段生成**: PowerShell / bash 环境变量片段复制
- [ ] **auto 模式实际模型名可见**: 日志和测试对话展示实际命中模型
- [ ] **响应式布局优化**: 改善小窗口、分屏使用体验

### P3 — 可选增强

- [ ] **上游关键 header 记录**: request-id / rate-limit 记录到日志
- [ ] **SSE ping / timeout 配置化**: 当前 ping=10s, idle timeout=300s 为代码常量
- [ ] **自动更新**: 当前仅检查更新，Tauri updater 可后续集成
- [ ] **Gemini 原生格式验证**: 原生格式转换函数作为备选
- [ ] **Azure deployment 端到端验证**: 待有 Azure 资源后验证
- [ ] **监听地址可配置**: 127.0.0.1 / 0.0.0.0 可选

### 未来愿景（非核心）

- [ ] API Key 加密存储（系统 Keychain / DPAPI）
- [ ] 多用户隔离（按 Access Key 配额）
- [ ] 插件系统（日志脱敏、请求改写）

---

## 10. 推荐渠道配置

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

> 此类中转站的模型列表接口不可用，拉取会失败。直接到「API 管理」点击「添加 API」手动填写模型名称即可。

---

## 11. 验证矩阵

| API 类型 | 拉取模型 | 非流式聊天 | 流式聊天 | 工具调用 | 图片输入 | 错误码透传 | 状态 |
|---|---|---|---|---|---|---|---|
| OpenAI | ✅ | ✅ | ✅ | 待验证 | 待验证 | ✅ | 主要链路可用 |
| Custom | ✅ | ✅ | ✅ | 依赖上游 | 依赖上游 | ✅ | OpenAI 兼容上游优先 |
| Claude | ✅ | ✅ | ✅ | 待验证 | 待验证 | ✅ | 格式转换复杂 |
| Gemini | ✅ | 待验证 | 待验证 | 待验证 | 待验证 | 待验证 | 使用 OpenAI 兼容端点 |
| Azure | ✅ | 待验证 | 待验证 | 待验证 | 待验证 | 待验证 | 缺 Azure 资源 |

---

## 12. 开发环境

```bash
# 前置要求
- Rust 1.85+ (rustup)
- Node.js 18+ / pnpm
- Tauri CLI (pnpm add -D @tauri-apps/cli)

# 开发
pnpm install
pnpm dev              # 启动 tauri dev
pnpm typecheck        # TypeScript 类型检查

# 构建
pnpm build            # 生产构建

# 数据库位置
# Windows: EXE 同目录下 api-switch.db（绿色便携版）
```

---

## 13. 文件索引

```
api-switch/
├── src-tauri/
│   ├── Cargo.toml                          # Rust 依赖
│   ├── tauri.conf.json                     # Tauri 配置
│   └── src/
│       ├── main.rs                         # 入口
│       ├── lib.rs                          # Tauri setup、托盘、代理自启
│       ├── error.rs                        # AppError 枚举
│       ├── database/
│       │   ├── mod.rs                      # Database struct + 连接管理
│       │   ├── schema.rs                   # 建表 + 兼容迁移
│       │   └── dao/
│       │       ├── channel_dao.rs          # 渠道 CRUD + 模型管理
│       │       ├── api_entry_dao.rs        # 路由池条目 + 冷却
│       │       ├── access_key_dao.rs       # 访问密钥
│       │       ├── usage_dao.rs            # 日志 + 统计
│       │       └── config_dao.rs           # KV 配置
│       ├── commands/
│       │   ├── channel.rs                  # 渠道命令
│       │   ├── pool.rs                     # 池命令
│       │   ├── token.rs                    # 密钥命令
│       │   ├── usage.rs                    # 统计命令
│       │   ├── config.rs                   # 配置命令
│       │   ├── proxy_cmd.rs               # 代理控制命令
│       │   └── test_chat.rs              # 测试对话（直接调适配器）
│       └── proxy/
│           ├── server.rs                   # Axum 服务器
│           ├── handlers.rs                 # 请求处理
│           ├── router.rs                   # 智能路由
│           ├── auth.rs                     # 认证
│           ├── forwarder.rs                # 转发 + 冷却 + 日志
│           ├── circuit_breaker.rs          # 内存熔断器（辅助）
│           └── protocol/                   # 协议适配
│               ├── mod.rs                  # ProtocolAdapter trait + 工厂
│               ├── common.rs               # join_url
│               ├── openai.rs               # OpenAI
│               ├── claude.rs               # Anthropic
│               ├── gemini.rs               # Gemini + 原生格式备选
│               ├── azure.rs                # Azure OpenAI
│               └── custom.rs               # 自定义
├── src/
│   ├── main.tsx                            # React 入口
│   ├── App.tsx                             # 主布局 + 导航 + 使用指南
│   ├── types.ts                            # 类型定义
│   ├── lib/
│   │   ├── api.ts                          # Tauri IPC 封装
│   │   └── utils.ts                        # cn() 工具
│   ├── components/
│   │   ├── ui/                             # Radix UI 组件
│   │   ├── proxy/
│   │   │   ├── ProxyToggle.tsx             # 代理启停
│   │   │   └── TestChatDialog.tsx          # 测试对话
│   │   └── WelcomeGuide.tsx               # 首次启动引导
│   ├── pages/
│   │   ├── DashboardPage.tsx               # 数据看板
│   │   ├── ChannelPage.tsx                 # 渠道管理
│   │   ├── ApiPoolPage.tsx                 # API 管理
│   │   ├── TokenPage.tsx                   # 令牌管理
│   │   ├── LogPage.tsx                     # 使用日志
│   │   └── SettingsPage.tsx                # 系统设置
│   └── i18n/locales/                       # 中英文翻译
├── GUIDE.md                                # 使用指南（英文）
├── GUIDE_CN.md                             # 使用指南（中文）
├── package.json
└── PLAN.md
```

---

## 14. 变更日志

### 2026-04-28 — API 池一键测速 / 使用指南中英双语 / 渠道智能选模 / 体验优化（v0.3.0-dev）

| # | 改动项 | 说明 |
|---|--------|------|
| 1 | **API 池一键测速** | 新增测速按钮，逐个测试所有模型延迟，测试中显示旋转图标，成功显示绿色响应时间，失败显示红色 ✗，测试中列表不跳动 |
| 2 | **API 池响应时间字段** | `api_entries` 表新增 `response_ms TEXT DEFAULT ''`，兼容迁移自动补齐；新增 `test_entry_latency` 和 `update_entry_response_ms` Tauri 命令 |
| 3 | **渠道测速体验优化** | 改为测试所有渠道（不限于已启用），使用本地 state 逐个回填结果，避免列表跳动 |
| 4 | **渠道列表 nowrap** | 状态、响应时间、模型数列添加 `whitespace-nowrap`，防止换行 |
| 5 | **使用指南中英双语** | `GUIDE.md` → `GUIDE_CN.md`（中文原版），新建英文 `GUIDE.md`；侧边栏按 `i18n.language` 自动切换 |
| 6 | **渠道空时自动弹窗** | 进入渠道页时若无渠道自动弹出添加对话框，每次进入都触发 |
| 7 | **模型智能预选** | 拉取模型后自动选中 6 个月内发布的新模型 + 当前渠道已有模型 |
| 8 | **新增模型默认开启** | `sync_entries_for_channel` 新建条目 `enabled` 从 0 改为 1 |
| 9 | **选择同步修复** | 渠道保存时无论选择是否为空都调用 `selectModels`，清空选择能正确删除已有条目 |
| 10 | **API 池缓存刷新** | 渠道保存后同时 invalidate `entries` 和 `channels`，切换页面即时看到数据 |

### 2026-04-29 — 错误冷却策略优化 (个人模式) / 空模型名修复

| # | 改动项 | 说明 |
|---|--------|------|
| 1 | **冷却策略改为"连续失败关闭"** | 任何错误都计数，达到阈值（默认 3 次）→ `enabled=false` + 24h 冷却。计数纯内存，重启归零。用户手动开启时清除计数+冷却。 |
| 2 | **`ProxyState` 新增 `failure_counts`** | `HashMap<String, u32>` 内存计数器，与 `AppState` 共享，Tauri 命令和代理服务器共用。 |
| 3 | **`forwarder.rs` 冷却逻辑重写** | `cool_down_entry` / `spawn_cool_down_entry` 均改为：计数+1 → 未达阈值则临时冷却 → 已达阈值则永久禁用+24h。 |
| 4 | **`record_circuit_success` / `spawn_record_circuit_success` 清除计数** | 成功请求时清除内存计数。 |
| 5 | **`toggle_entry` 手动开启时重置** | 用户手动开启入口时清除 `failure_counts` + `cooldown_until`。 |
| 6 | **设置页标签语义更新** | "连续失败次数"→"连续失败关闭次数"，"恢复等待时间(秒)"→"冷却恢复时间(秒)"，英文同步。 |
| 7 | **空模型名 `""` 归一化为 `auto`** | `handlers.rs` + `router.rs` 同时处理，避免空字符串误走指定模型路径。 |
| 8 | **数据库零改动** | 计数器纯内存，复用 `enabled` / `cooldown_until` 字段。 |

代理核心（forwarder / handlers / router / circuit_breaker / server）扫描发现的问题及修复计划。

| # | 优先级 | 模块 | 问题 | 修复方案 | 状态 |
|---|--------|------|------|----------|------|
| 1 | 🔴 | forwarder | 流式 poll 内同步调用 DB 写日志 | `log_usage` 改为 `tokio::spawn` 异步写入 | ✅ |
| 2 | 🔴 | server | 代理 HTTP client 缺 read_timeout / gzip | 添加 `read_timeout(120s)` + `gzip(true)`，connect_timeout 30→15s | ✅ |
| 3 | 🔴 | forwarder | 流式错误后 stream drop 可能重复冷却 | `StreamLogGuard::drop` 检查错误类型，upstream_error 跳过重复冷却 | ✅ |
| 4 | 🟡 | circuit_breaker | `try_read`/`try_write` 静默失败 | `is_available` lock 竞争时返回 false（原为 true） | ✅ |
| 5 | 🟡 | handlers | 上游错误信息透传泄露内部细节 | 截断 error_body 至 300 字符 | ✅ |
| 6 | 🟡 | router | failover 不按延迟排序 | 解析 `response_ms` 按延迟升序排列可用 entry | ✅ |
| 7 | 🟢 | handlers | body 限制 10MB | 改为 32MB | ✅ |
| 8 | 🟢 | router | 每次请求查 DB | 保留现状，低并发场景可接受 | — |

### v0.4.1 发布前验证清单

流式路径和 circuit breaker 行为变更引入了新的失败模式，发布前需验证以下场景：

| # | 场景 | 关注点 |
|---|------|--------|
| 1 | **流式请求日志完整性** | 正常完成 / 客户端断开 / 上游错误 三种场景下日志是否写入 |
| 2 | **并发 circuit breaker** | 10+ 并发请求下无误跳过可用 entry |
| 3 | **未测速 entry 排序** | `response_ms` 为空的 entry 不应被永久排到最后 |
| 4 | **压缩兼容性** | gzip 上游 / 无压缩上游 / 不支持 identity 的上游 |
| 5 | **慢速渠道连接** | connect_timeout 15s 是否过短（偏远地区/慢服务器） |
| 6 | **长思考模型** | read_timeout 120s 是否够用（深度推理模型可能 >2min） |
| 7 | **错误截断** | 300 字符是否覆盖常见上游错误格式 |
| 8 | **新建渠道保存** | 不拉取模型直接保存功能正常 |
| 9 | **添加模型弹窗** | 无控件溢出 |

### 2026-04-28 — 渠道保存按钮修复 / gzip 解压支持

| # | 改动项 | 说明 |
|---|--------|------|
| 1 | **渠道新增保存按钮解锁** | 新建渠道时保存按钮不再强制要求先拉取模型，填完名称/URL/API Key 即可保存 |
| 2 | **gzip 解压支持** | reqwest 启用 `gzip` feature，修复上游返回 gzip 压缩响应时 `error decoding response body` 错误 |
| 3 | **添加模型弹窗精简** | 移除 AddApiDialog 中模型元信息提示框，减少无效信息干扰 |

### 2026-05-XX — 设置页恢复等待时间改为滑块控件

设置页 Circuit Breaker 区域的"恢复等待时间(秒)"从数字输入框改为滑块控件，操作更直观。

| # | 改动项 | 说明 |
|---|--------|------|
| 1 | **Slider 组件** | 新建 `src/components/ui/slider.tsx`，自定义范围滑块，支持 min/max/step/value |
| 2 | **恢复等待时间控件** | 从 `<Input type="number">` 改为 `<Slider>`，范围 300-1800s，步长 30s |
| 3 | **默认值调整** | `circuit_recovery_secs` 默认值从 300 改为 600 秒 |

### 2026-04-27 — 智能模型拉取 / API 池模型目录增强 / 自动禁用修正（v0.3.0-dev）

围绕"尽量让用户少填少猜"的目标，模型拉取、API 池展示、自动禁用策略和测试交互都做了收敛与修正。

| # | 改动项 | 说明 |
|---|--------|------|
| 1 | **单按钮智能拉模型** | 移除两步化检测 UI，渠道页只保留一个“拉取模型”按钮；后台先校对 API 类型与 Base URL，再执行多方式模型拉取 fallback |
| 2 | **URL / 类型自动回填** | 支持从错误的 endpoint/path 回退到正确的 base site；识别成功后自动回填 `api_type` 与 `base_url` |
| 3 | **会话内校对标志** | 校对状态只存在于当前 Add/Edit 弹窗内存，不落库；用户修改 URL / Key / API 类型后自动失效并重新校对 |
| 4 | **避免 Gemini 误判** | 收紧类型校对条件：只有命中 Gemini 权威 `/v1beta/openai/*` 路径才判为 `gemini`，避免 OpenAI-compatible 网关被误判 |
| 5 | **模型拉取与校对解耦** | 校对只负责推导推荐保存值；真正拉模型时仍会按多种协议/路径 fallback，避免特殊网关被“猜错后锁死” |
| 6 | **过滤 `auto` 模型** | 上游返回的 `auto` 不再进入渠道模型列表，避免被误保存到 API 池 |
| 7 | **API 池本地模型目录** | 新增 `models.json` + `modelsCatalog.ts` 本地索引，不落库；API 池卡片和手动添加弹窗实时显示发布、能力、上下文、输出等元信息 |
| 8 | **API 池文案压缩** | 标题改为 `渠道 / 模型`，冷却提示改为内联 `(冷却 5m)`；模型元信息压成一行，适配中英文短标签 |
| 9 | **近似模型匹配** | 支持 `provider/model`、`-free`、`-preview` 等后缀清洗和相似度匹配，提升聚合网关模型名识别率 |
| 10 | **自动禁用默认值恢复** | 自动禁用状态码默认改为 `401,403,410`，并在设置页显式开放输入框让用户自行增减 |
| 11 | **正式代理自动禁用生效** | 正式代理链路在收到命中状态码时会直接 `enabled=false`，同时保留 cooldown 作为“系统关闭”标识 |
| 12 | **日志尝试路径修复** | 使用日志详情中的 `attempt_path` 从对象数组正确格式化，不再显示 `[object Object]` |
| 13 | **测试对话关闭修复** | `TestChatDialog` 增加请求序号隔离与关闭强制收尾，避免 X 掉卡住请求后下一个测试持续转圈 |
| 14 | **渠道响应时间** | 编辑渠道保存时，自动将 URL 探测的 `latency_ms` 换算成秒，保存到 `response_ms` 字段；渠道列表"响应"列显示响应时间 |
| 15 | **渠道批量测速** | 表头"响应"列加刷新按钮，点击后逐个测试所有启用渠道的 URL 响应时间，测试中显示旋转图标，完成后更新显示；超时显示红色 ✗ |
| 16 | **托盘菜单恢复** | 恢复"Open Main Window"菜单项（最顶部），右键托盘图标可直接打开主窗口 |

### 2026-04-26 — 个人版模型冷却机制（v0.2.0-dev）

放弃 NEW-API 风格的复杂状态码/关键词路由策略，改为个人版稳定优先的"模型冷却"机制。

| # | 改动项 | 说明 |
|---|--------|------|
| 1 | **数据库兼容检查** | `api_entries` 新增 `cooldown_until INTEGER`；启动时自动补字段 |
| 2 | **正式路由过滤冷却模型** | 冷却中模型不参与 AUTO 和显式模型路由 |
| 3 | **失败统一冷却** | 任意上游非正常设置 `cooldown_until = now + 300s`，继续 failover |
| 4 | **成功清除冷却** | 非流式/流式成功后清除 `cooldown_until` |
| 5 | **用户开关不被系统修改** | 移除 `toggle_entry(false)` 自动禁用；`enabled` 只由用户控制 |
| 6 | **取消复杂策略配置** | 删除自动禁用状态码、自动重试状态码、自动禁用关键词、504/524 特判 |
| 7 | **默认冷却参数** | 连续失败 1 次，冷却 300 秒；启动迁移旧默认值 `4/60` → `1/300` |
| 8 | **设置页精简** | 熔断卡片只保留连续失败次数和恢复等待时间 |
| 9 | **API 池状态点** | 红点=冷却中、灰点=未开启、绿点=已开启未冷却 |
| 10 | **测试对话直连上游** | 改用 Tauri `test_chat` 命令直接请求上游，不走代理端口，不触发 fallback |
| 11 | **日志点击行展开** | 移除三角图标，点击整行展开详情 |
| 12 | **侧边栏使用指南** | 系统设置下方新增"使用指南"菜单，外链 GitHub GUIDE.md |
| 13 | **API 管理光标** | 拖拽手柄改为 `cursor-pointer`，避免 Windows 上 `cursor-grab` 锯齿 |

### 2026-04-26 — 托盘菜单同步刷新修复（v0.2.0-dev）

API 管理页排序/开关/创建模型、Channel 选择/更新/删除后，都会刷新系统托盘菜单。

### 2026-04-25 — 转发核心对齐 NEW-API（v0.2.0-dev）

- enabled-only 正式路由
- Claude SSE 标准 frame
- 流式日志结束原因（done/upstream_error/timeout/dropped）
- 重试路径记录（attempt_path）
- HTTP 连接超时 + 流式 idle timeout
- HTTP Client 复用
- SSE Ping 保活
- AUTO 排序稳定性修正

### 2026-04-25 — UI 体验优化

- 令牌管理表格重构
- 渠道默认类型改为 custom
- 移除拖拽滚动

### 2026-04-25 — v0.1.0 首版发布

- 绿色便携版（数据库 EXE 同目录）
- 托盘菜单、首次启动引导
- 主题切换、更新检查
- 实时日志推送

### 2026-04-24 — 协议适配模块化重构

- 单体 `protocol.rs` 拆分为 5 个独立适配器
- `ProtocolAdapter` trait 统一接口
- 88 个单元测试

---

*本文档随开发持续更新。*

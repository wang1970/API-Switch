# API Switch - 项目计划书

> Personal API Management & Forwarding Center
> 版本: 0.1.5-dev | 生成日期: 2026-04-24

---

## 1. 项目概述

**API Switch** 是一款基于 Tauri v2 的桌面应用，用于统一管理和转发多个 AI API 渠道。用户可配置多个 API 渠道（OpenAI / Anthropic / Gemini / Azure / 自定义兼容），系统作为本地代理服务器运行，对外暴露统一的 OpenAI 兼容接口，内部实现智能路由、自动故障转移、熔断保护和用量统计。

### 核心价值
- **多渠道路由**：一个入口访问多个 AI 服务商，按模型自动匹配或手动指定
- **高可用**：熔断器 + 自动故障转移，单点故障不影响使用
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
| 状态管理 | TanStack React Query v5 | ChannelPage 已接入（useQuery/useMutation），其余页面待迁移 |
| 图表 | Recharts v3 | Dashboard 可视化 |
| 国际化 | i18next + react-i18next | 中/英双语 |
| 拖拽 | @dnd-kit | API Pool 排序 |
| 表单 | react-hook-form + zod v4 | 已安装，当前页面使用 useState 管理表单 |

### 2.2 整体架构

```
┌─────────────────────────────────────────────────────┐
│                    Tauri App Window                   │
│  ┌───────────────────────────────────────────────┐  │
│  │              React Frontend (Vite)             │  │
│  │  Dashboard │ Channel │ Pool │ Logs │ Settings  │  │
│  └──────────────┬────────────────────────────────┘  │
│                 │ Tauri IPC (invoke)                  │
│  ┌──────────────▼────────────────────────────────┐  │
│  │           Tauri Commands Layer                 │  │
│  │  channel │ pool │ token │ usage │ config │ proxy│  │
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
│  │         ↕ CircuitBreaker (per entry)         │   │
│  └──────────┬──────────────────────────────────┘   │
│             │ reqwest → Upstream APIs               │
└─────────────────────────────────────────────────────┘
```

---

## 3. 模块详解

### 3.1 后端模块 (`src-tauri/src/`)

#### `main.rs` — 入口
- 初始化 Tauri App，注册所有 commands
- 创建 `AppState`（持有 `Arc<Database>` 和 `Arc<RwLock<Option<ProxyServer>>>`）

#### `database/mod.rs` — 数据库管理
- SQLite 连接，WAL 模式 + 外键约束
- `Mutex<Connection>` 包装，通过 `lock_conn!` 宏安全访问
- 委托 `schema::create_tables()` 建表，`ensure_columns()` 兼容迁移

#### `database/schema.rs` — 表结构（5 张核心表）

| 表名 | 用途 | 关键字段 |
|------|------|----------|
| `channels` | API 渠道 | id, name, api_type, base_url, api_key, available_models(JSON), selected_models(JSON), enabled |
| `api_entries` | 路由池条目 | id, channel_id, model, display_name, sort_index, enabled (FK→channels CASCADE) |
| `access_keys` | 访问密钥 | id, name, key(UUID), enabled |
| `usage_logs` | 请求日志 | 25+ 字段，含 token 统计、延迟、熔断状态、错误信息 |
| `app_settings` | 全局配置 | KV 存储 (listen_port, circuit_failure_threshold, circuit_recovery_secs, locale, theme 等) |

#### `database/dao/` — 数据访问层
- **channel_dao.rs** (210行): CRUD + 模型管理（available/selected JSON 序列化）
- **api_entry_dao.rs**: 条目 CRUD + 启停 + 排序 + 路由查询（JOIN channel 获取完整信息）
- **access_key_dao.rs**: 密钥 CRUD + 启停 + key 查找
- **usage_dao.rs** (580行): 日志插入 + 分页查询 + Dashboard 聚合统计 + 图表数据
- **config_dao.rs**: 设置读写 (KV JSON)

#### `proxy/server.rs` — 代理服务器
- `ProxyServer`: 持有 port、`ProxyState`、`shutdown_tx`
- 基于 tokio TcpListener + axum serve，支持 graceful shutdown
- 路由: `/health`, `/v1/chat/completions`, `/v1/models`
- CORS 中间件 (`tower-http`)，允许 WebView 跨域访问

#### `proxy/handlers.rs` (138行) — 请求处理
- `health_check`: 健康检查
- `handle_chat_completions`: 核心代理逻辑
  - 使用 `HeaderMap` + `Bytes` 独立提取器（避免 axum 0.7 `Request<Body>` 非 Send 问题）
  - 提取 Access Key → 解析请求 body → 提取 model/stream → 路由解析 → 转发（含重试）
- `handle_list_models`: 返回池中所有可用模型列表
- `ProxyError` 枚举: Unauthorized / NoAvailableProvider / AllProvidersFailed / Internal

#### `proxy/router.rs` — 智能路由
- 接收 `enabled_entries`（仅启用的）和 `all_entries`（含禁用的）两组列表
- `model == "auto"`: 仅使用 enabled 条目（AUTO 选择范围）
- 精确匹配 model: 从 ALL 条目中查找（含 disabled），匹配到的 + enabled 条目作为 fallback（保证不断链）
- 错误模型名: fallback 到 enabled 条目（AUTO 行为）
- 熔断条目自动跳过

#### `proxy/auth.rs` — 认证
- 从 `Authorization: Bearer <key>` 提取密钥
- `access_key_required` 开启时强制验证，关闭时仅用于身份追踪

#### `proxy/forwarder.rs` (499行) — 请求转发
- 按条目列表逐个尝试（failover）
- 通过 `protocol::get_adapter()` 获取适配器，统一调用 trait 方法
- 支持流式 (SSE) 和非流式响应
  - SSE: 根据 `adapter.needs_sse_transform()` 分两条路径
    - OpenAI/Custom/Gemini/Azure: 原始字节透传，旁路提取 usage
    - Claude: 解析→转换→重建（Anthropic SSE → OpenAI chunk 格式），部分 SSE 行缓存时 `cx.waker().wake_by_ref()` 防止流挂起
- Token 用量统计（从响应中提取 prompt/completion tokens）
- 记录首次 token 延迟 (first_token_ms)
- 成功/失败反馈到熔断器
- 所有结果写入 usage_logs

#### `proxy/circuit_breaker.rs` — 熔断器
- 三态: Closed → Open → HalfOpen
- 连续失败达到阈值 → Open（恢复期从 DB 配置 `circuit_recovery_secs` 读取，默认 60 秒）
- 恢复期结束 → HalfOpen（允许探测）
- 探测成功 → Closed，失败 → 继续保持 Open
- 内存态，不持久化（重启重置）

#### `proxy/protocol/` — 协议适配（trait 模块化架构）
- **设计理念**: 5 种 API 类型各自独立实现，互不影响。修改一个类型的逻辑不会引发其他类型的 BUG
- **`mod.rs` (含测试)** — `ProtocolAdapter` trait 定义 + `get_adapter()` 工厂函数（按 `api_type` 返回对应适配器）
- **`common.rs`** — 共享工具函数（`join_url` URL 去重拼接）
- **`openai.rs`** — OpenAI 适配器：Bearer 认证、`/v1/chat/completions`、请求/响应透传、SSE 透传
- **`claude.rs`** — Anthropic 适配器：`x-api-key` 认证、`/v1/messages`、完整的 OpenAI ↔ Anthropic 格式转换（system prompt 提取、tool_calls/tool_result 转换、图片 base64 格式转换）、SSE 事件流转换（message_start/content_block_delta/message_delta 等）
- **`gemini.rs`** — Google Gemini 适配器：query-param 认证（`?key=...`）、使用 Google 官方 OpenAI 兼容端点 `/v1beta/openai/`、请求/响应透传。同时提供原生格式转换函数（`transform_request_to_gemini`/`transform_response_from_gemini`/`transform_gemini_sse_line`）作为备选
- **`azure.rs`** — Azure OpenAI 适配器：`api-key` header 认证、deployment 名嵌入 URL 路径、请求体移除 model 字段（Azure 用 deployment 名路由）、`parse_models_response` 使用 `model` 字段作为 display_name
- **`custom.rs`** — 自定义适配器：Bearer 认证、不拼接 `/v1` 前缀（用户 base_url 已含版本路径）、请求/响应透传

#### `commands/` — Tauri Command 层

| 模块 | 命令 | 说明 |
|------|------|------|
| `channel` | list/create/update/delete, fetch_models, select_models, test_chat | 渠道管理 + 模型拉取 + 连通性测试 |
| `pool` | list/toggle/reorder/create_entry | API 池管理 + 拖拽排序 |
| `token` | list/create/delete/toggle_access_key | 访问密钥管理 |
| `usage` | get_usage_logs, get_dashboard_stats, get_model_consumption, get_call_trend, get_model/user_distribution, get_model/user_ranking | 统计数据 |
| `config` | get_settings, update_settings | 全局配置 |
| `proxy_cmd` | start_proxy, stop_proxy, get_proxy_status | 代理服务器控制 |
| `test_chat` | test_chat | API 池测试对话（直接通过适配器调用上游，不走路由） |

### 3.2 前端模块 (`src/`)

#### `types.ts` — 类型定义 (234行)
- `ApiType = "openai" | "claude" | "gemini" | "azure" | "custom"`
- 每种类型配有默认 URL、URL 提示、API Key 提示（中英双语）
- 完整的 Channel / ApiEntry / AccessKey / UsageLog / DashboardStats 等接口定义

#### `lib/api.ts` — Tauri IPC 封装 (154行)
- 所有 `invoke()` 调用集中管理
- 完整覆盖后端所有 command

#### `lib/utils.ts` — 工具函数
- `cn()` (clsx + tailwind-merge) 用于条件类名

#### 页面组件

| 页面 | 文件 | 功能 |
|------|------|------|
| API Pool | `ApiPoolPage.tsx` (358行) | 条目列表 + 拖拽排序 + 新建弹窗 + 测试对话弹窗（耗时+token显示） |
| Channel | `ChannelPage.tsx` (752行) | 统一添加/编辑弹窗（基础配置+内嵌模型选择+一键保存同步） + TanStack React Query + 状态切换联动禁用关联条目 |
| Token | `TokenPage.tsx` | 密钥列表 + 创建/删除 + 启停 + Key 复制 |
| Logs | `LogPage.tsx` (267行) | 请求日志分页 + 多维筛选 + 详情展开 |
| Dashboard | `DashboardPage.tsx` (321行) | 统计概览 + 4图表（消耗趋势/调用趋势/模型分布/用户趋势） |
| Settings | `SettingsPage.tsx` (148行) | 代理端口/开关（联动启停） + 熔断参数 + 语言/主题切换 |

#### `App.tsx` — 主布局
- 左侧 6 页导航栏 (Sidebar) + 右侧内容区
- 图标: Layers/Route/KeyRound/FileText/BarChart3/Settings/Power

#### i18n
- 中/英双语，支持切换并持久化到 localStorage + 数据库

---

## 4. 数据流

### 4.1 请求代理流程

```
Client → POST /v1/chat/completions
  │
  ├─ 1. auth::extract_access_key()  ← 从 Header 提取并验证密钥
  ├─ 2. 解析 JSON body → model / stream
  ├─ 3. router::resolve()           ← 从 DB 查询 enabled entries，过滤熔断，按优先级排序
├─ 4. forwarder::forward_with_retry()
│     ├─ 遍历 entries:
│     │   ├─ protocol::get_adapter(api_type) → adapter
│     │   ├─ adapter.build_chat_url() + adapter.apply_auth() + adapter.transform_request()
│     │   ├─ reqwest::send()              ← HTTP 转发到上游
│     │   ├─ 成功 → circuit_breaker::record_success()
│     │   │         → adapter.transform_response() / adapter.needs_sse_transform()
│     │   │         → 解析响应/token → 返回客户端
│     │   └─ 失败 → circuit_breaker::record_failure() → 尝试下一个
  │     └─ 全部失败 → 502 AllProvidersFailed
  └─ 5. insert_usage_log()           ← 无论成败，记录完整日志
```

### 4.2 前端数据流

```
ChannelPage: React Component → TanStack Query (useQuery/useMutation)
  → lib/api.ts → Tauri invoke() → Rust Command → Database (SQLite)
  → 返回 → Query Cache → 自动失效 & 重新获取 → Component Re-render

其余页面: React Component → useState + 直接 invoke()
  → lib/api.ts → Tauri invoke() → Rust Command → Database (SQLite)
  → 返回 → setState → Component Re-render
```

> TanStack React Query 已在 ChannelPage 接入（useQuery/useMutation/optimistic update），
> 其余页面（ApiPool/Token/Log/Dashboard/Settings）待迁移。

---

## 5. 已完成功能 ✓

### 5.1 后端核心
- [x] SQLite 数据库初始化 + 5 张表 + 自动迁移（ensure_columns）
- [x] 渠道 CRUD（含 api_type 字段，5 种类型）
- [x] API Entry 路由池管理（创建/启停/排序/按 channel 级联删除）
- [x] Access Key 管理（UUID 生成 + 启停 + 验证）
- [x] Axum 代理服务器（graceful shutdown）
- [x] OpenAI 兼容协议代理（`/v1/chat/completions`, `/v1/models`）
- [x] **协议适配模块化重构**: 5 种 API 类型各自独立适配器（`proxy/protocol/` 目录），通过 `ProtocolAdapter` trait 统一接口，修改一种类型不影响其他类型
- [x] 智能路由（按模型匹配 + auto fallback + sort_index 优先级）
- [x] 请求转发 + 自动故障转移（逐条目重试）
- [x] **模型自动拉取**: `fetch_models` 已实现，各适配器通过 `build_models_url` + `apply_auth` + `parse_models_response` 分别处理不同上游格式
- [x] **协议适配器单元测试**: 88 个测试覆盖 5 种适配器（OpenAI 10 + Claude 30 + Custom 9 + Azure 10 + Gemini 15 + 公共 7 + 工厂 7），全部通过
- [x] 熔断器（三态 + 可配置阈值/恢复时间，recovery_secs 从 DB 配置注入）
- [x] 用量日志记录（25+ 字段，含 token/延迟/错误）
- [x] Dashboard 统计聚合（总量/今日 Token/输入 Token/输出 Token）
- [x] 图表数据接口（趋势/分布/排名）
- [x] 全局配置（KV 存储）
- [x] 健康检查端点
- [x] **应用图标替换**: `icon.jpg` 通过 sharp + png-to-ico 生成所有尺寸图标（PNG/ICO/ICNS）
- [x] **CORS 中间件**: `tower-http` CorsLayer 允许所有 origin，支持 WebView 跨域访问代理
- [x] **API 池测试对话**: `TestChatDialog` 组件，通过 HTTP fetch 走代理端口（测试完整链路），流式 SSE 拆分连接时间(TTFB)+思考时间，显示 token 统计

### 5.2 前端 UI
- [x] 中英双语国际化 (i18next)
- [x] 亮色/暗色/跟随系统主题
- [x] Dashboard 页面（统计卡片 + 6 种 Recharts 图表）
- [x] Channel 管理页面（平铺列表 + 操作列导入/编辑/删除 + 创建/编辑弹窗 + 导入模型弹窗含拉取 + 连通性测试弹窗 + TanStack React Query）
- [x] API Pool 页面（拖拽排序 @dnd-kit + 新建弹窗 + 测试对话弹窗 + 渠道/模型双行显示）
- [x] **TestChatDialog 组件**: 测试对话弹窗，HTTP fetch 走代理端口（流式 SSE），连接时间 🔗 + 思考时间 💭 拆分显示，IN/OUT token 统计
- [x] **渠道添加/编辑统一弹窗**: 基础配置 + 内嵌模型拉取/选择，保存时同步到 API 池，新增模型默认 disabled
- [x] **路由 fallback 不断链**: 精确匹配条目（含 disabled）+ enabled 条目作为 auto-fallback，错误模型名直接 fallback 到 AUTO
- [x] Token 密钥管理页面（独立页面，创建/删除/启停 + Key 复制）
- [x] Log 日志页面（分页 + 多维筛选 + 详情展开）
- [x] Settings 设置页面（代理/熔断/语言/主题）
- [x] 类型定义完整（ApiType 5 种 + URL/Key 提示配置）
- [x] API 层封装完整（lib/api.ts 覆盖所有 command）

---

## 6. 待开发 / 改进项

### ~~P0 — 关键缺失~~
- ~~**cargo-tauri CLI 安装**: `@tauri-apps/cli` 已在 devDependencies，`pnpm tauri dev` 可直接使用~~

### P1 — 功能增强
- [ ] **Gemini 原生格式验证**: 当前 Gemini 适配器使用 Google OpenAI 兼容端点 (`/v1beta/openai/`)，原生格式转换函数已实现但未接入 trait（可作为备选方案）
- [ ] **Azure deployment 验证**: Azure 适配器已实现完整 URL 路径 + api-key 认证 + 模型列表解析，待有 Azure 资源后端到端验证
- [ ] **请求速率限制**: 当前无 RPM/TPM 限流，高并发下可能打爆上游
- [ ] **熔断状态持久化**: 当前内存态，重启后丢失所有熔断历史
- [ ] **系统托盘**: `Cargo.toml` 已启用 `tray-icon` feature，但未实现托盘菜单
- [x] **系统托盘菜单**: 顶层平铺 5 个优先 API（CheckMenuItem），分隔线，退出。点击条目设为最高优先级。双击托盘图标弹出主窗口
- [x] **托盘设置**: 设置页新增"系统托盘"卡片，支持"跟随系统启动"和"启动最小化"开关
- [x] **AppState Clone**: 支持 Clone 以便 TrayIconBuilder 闭包捕获

### P2 — 体验优化
- [ ] **实时日志推送**: 当前日志靠轮询，可改为 Tauri Event 实时推送
- [ ] **日志导出**: 支持导出 CSV/JSON
- [ ] **API Key 安全**: 当前 api_key 明文存储在 SQLite，建议加密或使用系统密钥链
- [ ] **请求/响应 Mock**: 前端开发时缺少 Mock 数据，开发体验不佳
- [ ] **错误提示优化**: 前端统一的 Toast 错误提示
- [ ] **响应式布局**: 当前 min 800×600，小屏适配
- [ ] **TanStack React Query 全量接入**: ChannelPage 已接入，其余 5 个页面待迁移

### P3 — 未来愿景
- [ ] **多用户隔离**: 按 Access Key 做用量配额限制
- [ ] **Web UI**: 除桌面端外提供 Web 管理界面
- [ ] **插件系统**: 支持自定义中间件（如日志脱敏、请求改写）
- [ ] **集群部署**: 支持 SQLite → PostgreSQL 迁移，多实例负载均衡
- [ ] **OpenTelemetry**: 接入分布式追踪
- [ ] **自动更新**: Tauri updater 集成

---

## 7. 开发环境

```bash
# 前置要求
- Rust 1.85+ (rustup)
- Node.js 18+ / pnpm
- Tauri CLI (via npm: pnpm add -D @tauri-apps/cli)

# 开发
pnpm install
pnpm dev              # 启动 tauri dev (前端 Vite + 后端 cargo build)
pnpm typecheck        # TypeScript 类型检查

# 构建
pnpm build            # 生产构建

# 数据库位置
# Windows: %LOCALAPPDATA%/api-switch/api-switch.db (自动创建)
```

---

## 8. 已知问题 & 风险

| # | 问题 | 严重度 | 说明 |
|---|------|--------|------|
| 1 | ~~开发环境未验证~~ | ~~高~~ | ✅ cargo check + pnpm typecheck 均通过 |
| 2 | API Key 明文存储 | 中 | SQLite 中 api_key 未加密 |
| 3 | 熔断状态不持久 | 低 | 重启后所有熔断重置 |
| 4 | ~~路由层 ApiEntry 重复克隆~~ | 🟡中 | `router::resolve()` 对匹配的 ApiEntry 调用 `(*e).clone()`，`model="auto"` 时克隆所有条目。设计上应引入轻量级 `RouteEntry` 结构（仅 id/model/sort_index），由数据库层提供独立接口，避免克隆大对象。已尝试重构但因涉及链路过长（router/handlers/forwarder/dao）编译错误较多，暂缓处理，留待后续 P2-P3 优化。 |
| 5 | ~~SSE 流式回归~~ | ~~🔴高~~ | ✅ 已修复：OpenAI 兼容类型恢复原始字节透传 |
| 6 | ~~Gemini 原生格式内存泄漏~~ | ~~🔴高~~ | ✅ 已修复：`str.to_lowercase().leak()` → `.to_string()` |
| 7 | ~~CircuitBreaker recovery_secs 硬编码~~ | ~~🟡中~~ | ✅ 已修复：从 DB 配置 `circuit_recovery_secs` 注入到构造函数 |
| 8 | Release 构建未完成 | 🟡中 | cargo check 通过，`cargo build --release` 超时未完成，需手动构建 |

---

## 9. 文件索引

```
api-switch/
├── src-tauri/
│   ├── Cargo.toml                          # Rust 依赖
│   ├── tauri.conf.json                     # Tauri 配置
│   └── src/
│       ├── main.rs                         # 入口 + AppState
│       ├── error.rs                        # AppError 枚举
│       ├── database/
│       │   ├── mod.rs                      # Database struct + 连接管理
│       │   ├── schema.rs                   # 建表 + 迁移
│       │   └── dao/
│       │       ├── channel_dao.rs          # 渠道 CRUD + 模型管理
│       │       ├── api_entry_dao.rs        # 路由池条目
│       │       ├── access_key_dao.rs       # 访问密钥
│       │       ├── usage_dao.rs            # 日志 + 统计
│       │       └── config_dao.rs           # KV 配置
│       ├── commands/
│       │   ├── mod.rs                      # Command 注册
│       │   ├── channel.rs                  # 渠道命令 (含 test_chat, 263行)
│       │   ├── pool.rs                     # 池命令
│       │   ├── token.rs                    # 密钥命令
│       │   ├── usage.rs                    # 统计命令
│       │   ├── config.rs                   # 配置命令
│       │   ├── proxy_cmd.rs               # 代理控制命令
│       │   └── test_chat.rs              # API 池测试对话命令 (直接适配器调用)
│       └── proxy/
│           ├── mod.rs                      # 模块导出
│           ├── server.rs                   # Axum 服务器
│           ├── handlers.rs                 # 请求处理 (138行)
│           ├── router.rs                   # 智能路由 (enabled+all双列表, fallback不断链)
│           ├── auth.rs                     # 认证
│           ├── forwarder.rs                # 转发 + 重试 (499行)
│           ├── circuit_breaker.rs          # 熔断器
│           └── protocol/                   # 协议适配 (trait 模块化架构)
│               ├── mod.rs                  # ProtocolAdapter trait + get_adapter() + 88个测试 (1200行)
│               ├── common.rs               # join_url 共享工具 (16行)
│               ├── openai.rs               # OpenAI 适配器 (88行)
│               ├── claude.rs               # Anthropic 适配器 (732行)
│               ├── gemini.rs               # Gemini 适配器 + 原生格式转换函数 (545行)
│               ├── azure.rs                # Azure OpenAI 适配器 (128行)
│               └── custom.rs               # 自定义适配器 (91行)
├── src/
│   ├── main.tsx                            # React 入口
│   ├── App.tsx                             # 主布局 + 路由
│   ├── types.ts                            # 完整类型定义 (234行)
│   ├── lib/
│   │   ├── api.ts                          # Tauri IPC 封装 (154行)
│   │   └── utils.ts                        # cn() 工具
│   ├── components/
│   │   ├── ui/                             # Radix UI 组件
│   │   └── proxy/
│   │       ├── ProxyToggle.tsx             # 代理启停按钮
│   │       └── TestChatDialog.tsx          # 测试对话弹窗 (流式SSE, 连接+思考拆分)
│   └── pages/
│       ├── DashboardPage.tsx               # Dashboard (451行)
│       ├── ChannelPage.tsx                 # 渠道管理 (统一弹窗+模型选择)
│       ├── ApiPoolPage.tsx                 # API 池 (513行)
│       ├── TokenPage.tsx                   # 密钥管理
│       ├── LogPage.tsx                     # 日志 (267行)
│       └── SettingsPage.tsx                # 设置 (148行)
├── package.json
└── PLAN.md                                 # ← 本文件
```

## 10. 变更日志

### 2026-04-24 — Protocol 接入 + 测试对话 + 代理自启（v0.1.5-dev）

**改动文件**: 13 个文件，+550 行 / -100 行

| # | 改动项 | 说明 |
|---|--------|------|
| 1 | **protocol 模块接入 forwarder** | `forwarder.rs` 全面改用 `get_adapter()` 构建 URL/认证/转换请求/响应/SSE，替代硬编码 OpenAI 逻辑 |
| 2 | **protocol 模块接入 channel** | `commands/channel.rs` 的 `fetch_models_from_api()` 改用适配器，删除 60+ 行 `match api_type` |
| 3 | **proxy/mod.rs 模块导出** | 添加 `mod protocol` + `pub(crate)` 导出 `ProxyState`/`forward_with_retry`/`resolve`/`circuit_breaker` |
| 4 | **SSE 流式防挂起** | `transform_sse_chunk` 返回 `None` 时添加 `cx.waker().wake_by_ref()`，防止 Claude 流式挂起 |
| 5 | **测试对话功能** | 新增 `TestChatDialog.tsx` + `commands/test_chat.rs`，API 池每条目旁加测试按钮，通过 Tauri IPC 直接调用 forwarder（绕过 WebView CORS） |
| 6 | **耗时 + Token 显示** | 后端返回 `latency_ms` + `usage`，前端在 assistant 消息底部显示秒数和 IN/OUT tokens |
| 7 | **CORS 中间件** | `tower-http` CorsLayer 允许所有 origin，支持 WebView 跨域访问代理 |
| 8 | **代理自动启动** | `lib.rs` setup 中根据 `proxy_enabled` 配置自动启动代理服务器 |
| 9 | **代理开关联动** | 设置页"启用代理"开关改为实际调用 start/stop proxy，状态读取代理实际运行状态 |
| 10 | **i18n 补充** | 中英文添加 `apiPool.testChat` 系列 key |

**编译状态**: `cargo check` 0 errors, 12 warnings（均为 dead_code） | `pnpm typecheck` 0 errors | 92 tests passed

---

### 2026-04-24 — 路由 fallback + 测试对话流式拆分 + 渠道弹窗统一（v0.1.5-dev）

**改动文件**: 9 个文件，+492 行 / -244 行

| # | 改动项 | 说明 |
|---|--------|------|
| 1 | **路由 fallback 不断链** | `router.rs` 改为双列表（enabled + all），精确匹配查所有条目（含 disabled），后追加 enabled 作为 auto-fallback，错误模型名直接 fallback 到 AUTO |
| 2 | **handlers.rs 传双列表** | `handle_chat_completions` 同时查 `get_enabled_entries_for_routing()` + `get_entries_for_routing_all()` 传给 router |
| 3 | **get_entries_for_routing_all** | `api_entry_dao.rs` 新增方法，查所有条目（含 disabled），供路由精确匹配和测试对话使用 |
| 4 | **新条目默认 disabled** | `sync_entries_for_channel` 新增条目时 `enabled` 从 `1` 改为 `0` |
| 5 | **test_chat 直接适配器调用** | 不走路由/forwarder，直接通过 `get_adapter()` 调上游，支持 disabled 条目测试 |
| 6 | **TestChatDialog 流式 SSE** | 改回 HTTP fetch 走代理端口（测试完整链路），`stream: true`，拆分 🔗连接时间 (TTFB) + 💭思考时间 |
| 7 | **渠道添加/编辑统一弹窗** | `ChannelPage.tsx` 统一为同一个界面流程：基础配置 + 内嵌模型拉取/选择，保存时同步到 API 池 |
| 8 | **API Key 明文切换** | 弹窗内添加密码/明文切换按钮 |

**编译状态**: `cargo check` 0 errors, 15 warnings（均为 dead_code） | `pnpm typecheck` 0 errors

---

### 2026-04-24 — UI 精简 + 图标替换 + 渠道禁用联动（v0.1.5-dev）

**改动文件**: 25 个文件，+429 行 / -204 行

| # | 改动项 | 说明 |
|---|--------|------|
| 1 | **应用图标替换** | `icon.jpg` 通过 sharp + png-to-ico 生成所有尺寸图标（16 个文件），替换默认 Tauri 图标 |
| 2 | **Dashboard 精简** | 移除右上角时间筛选按钮、模型排行/用户排行 tab 及 TopListCard、RPM/TPM/成功率/延迟卡片，统计卡片改为 4 张统一格式（标题→今日→总计） |
| 3 | **渠道页精简** | 移除搜索框、备注字段，状态列改为圆角标签（绿/灰），操作列新增电源按钮切换启用/禁用 |
| 4 | **渠道禁用联动** | 禁用渠道时 `UPDATE api_entries SET enabled=0 WHERE channel_id`，启用时不修改条目状态 |
| 5 | **模型 chip 修复** | 模型选择标签的 `×` 按钮改用 `&times;` HTML 实体修复显示问题 |

**编译状态**: `cargo check` 0 errors | `pnpm typecheck` 0 errors

---

### 2026-04-24 — 系统托盘 + 优先级切换（v0.1.5-dev）

**改动文件**: 8 个文件，+120 行 / -30 行

| # | 改动项 | 说明 |
|---|--------|------|
| 1 | **系统托盘** | `TrayIconBuilder` 构建托盘：图标 + 菜单 + 双击事件。顶层平铺 5 个优先 API（CheckMenuItem），点击设为最高优先级，分隔线 + 退出 |
| 2 | **双击托盘弹出窗口** | `on_tray_icon_event` 处理，无 menu_id 时触发 `window.show()` |
| 3 | **托盘设置** | 设置页新增"系统托盘"卡片：跟随系统启动（`autostart`）、启动最小化（`start_minimized`），默认均为 false |
| 4 | **启动窗口逻辑** | 根据 `start_minimized` 决定是否隐藏窗口（不再无条件隐藏） |
| 5 | **AppSettings 扩展** | 新增 `autostart: bool` + `start_minimized: bool` 字段，DB 读写已接入 |

---

### 2026-04-24 — UI 精简 + 图标替换 + 渠道禁用联动（v0.1.5-dev）

### 2026-04-23 — 协议适配模块化重构（v0.1.2-dev）

**改动文件**: `protocol/` 目录（新建） / `forwarder.rs` / `channel.rs` / `handlers.rs` / `proxy/mod.rs`

**重构目标**: 将单体 `protocol.rs`（643行，所有类型 match 分支混在一起）拆分为 5 个独立适配器，通过 `ProtocolAdapter` trait 统一接口。修改一种 API 类型的逻辑不会引发其他类型的 BUG。

| # | 改动项 | 说明 |
|---|--------|------|
| 1 | **删除 `protocol.rs`** | 移除旧的 643 行单体文件 |
| 2 | **新建 `protocol/` 目录** | trait 模块化架构，每个类型一个文件 |
| 3 | **`ProtocolAdapter` trait** | 定义 12 个接口方法（URL 构建、认证、请求/响应转换、SSE、模型列表解析） |
| 4 | **`get_adapter()` 工厂** | 按 `api_type` 返回 `Box<dyn ProtocolAdapter + Send>` |
| 5 | **OpenAI 适配器** | Bearer 认证、`/v1/chat/completions`、请求/响应/SSE 全透传 |
| 6 | **Claude 适配器** | `x-api-key` 认证、完整 OpenAI ↔ Anthropic 格式转换（system prompt、tool_calls、SSE 事件流） |
| 7 | **Gemini 适配器** | query-param 认证、Google OpenAI 兼容端点 `v1beta/openai/`、额外提供原生格式转换函数 |
| 8 | **Azure 适配器** | `api-key` header 认证、deployment 名嵌入 URL、请求体移除 model 字段 |
| 9 | **Custom 适配器** | Bearer 认证、不拼接 `/v1`（用户 base_url 已含版本路径） |
| 10 | **forwarder.rs 重构** | `protocol::get_adapter()` + trait 方法替代所有 `match api_type` 分支 |
| 11 | **channel.rs 重构** | `get_adapter()` 替代 `protocol::build_auth_headers` 等模块函数 |
| 12 | **handlers.rs 修复** | `Request<Body>` 改为 `HeaderMap + Bytes` 独立提取器（修复 axum 0.7 Send 问题） |
| 13 | **Claude 3 个 bug 修复** | ① `custom` 适配器 `join_url` 误去重 `/v1` ② Claude `parse_models_response` 字段错误（`owned_by`→`display_name`）③ Claude 请求转换 assistant tool_calls + tool result 处理不完整 |
| 14 | **88 个单元测试** | OpenAI 10 + Claude 30 + Custom 9 + Azure 10 + Gemini 15 + 公共 7 + 工厂 7，全部通过 |

**编译状态**: `cargo check` 通过（0 错误，2 个无害 dead_code warning）

---

### 2026-04-23 — Proxy 代理核心修复（v0.1.1-dev）

**改动文件**: `forwarder.rs` / `protocol.rs` / `channel.rs` / `types.ts`

| # | 修复项 | 根因 | 影响 |
|---|--------|------|------|
| 1 | **SSE 流式透传回归** | refactor 将 `append_and_parse_sse` 从旁路解析(返回 void, 原始 chunk 透传)改为重建替换(返回 Bytes)，对 OpenAI 兼容类型破坏 chunk 边界导致客户端 JSON 解析失败 | 🔴 所有 OpenAI 兼容流式请求失败 |
| 2 | **URL 路径拼接重复** | 缺少 `endpoint_path()` / `models_endpoint_path()` 统一接口 + `join_url()` 去重 `/v1` | 🔴 custom 类型路由 404 |
| 3 | **custom 类型误加 `/v1`** | channel 命令层对 custom 类型也拼接 `/v1/chat/completions` | 🔴 自定义渠道请求路径错误 |
| 4 | **NDJSON 缺失 SSE 前缀** | 某些上游返回裸 NDJSON 而非 SSE 格式，客户端无法识别事件 | 🟡 部分上游流式失败 |
| 5 | **protocol 适配层完善** | `transform_sse_line` / `transform_response` 对 OpenAI 类型确保 no-op，仅 Anthropic 做格式转换 | ✅ 正确性保障 |

**修复策略**: `append_and_parse_sse` 分两条路径 — OpenAI 兼容类型直接 `chunk.clone()` 原始透传(旁路提取 usage)，仅 Anthropic 走解析→转换→重建。`cargo check` 编译通过。

---

*本文档由 Agent 自动分析源码生成，涵盖项目全部模块。后续开发请在对应章节更新状态。*
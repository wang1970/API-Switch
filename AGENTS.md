# API Switch — Agent Instructions

## 联动一致性检查规则

当用户要求检查"A 和 B 是否联动一致"、"是否同步"、"是否一致"时，**禁止只做静态值对比**。必须完成以下全部步骤：

1. **静态层**：对比类型定义、UI 选项、默认值是否一致（必要但不充分）
2. **数据流层**：端到端追踪一次实际调用链：
   - 前端调用处的**实际传参**（注意 `as any`、`Partial<T>` 等绕过类型检查的写法）
   - Tauri IPC / API 的序列化与反序列化（Rust `#[derive(Deserialize)]` 是否要求完整对象）
   - 后端写入 DB 的逻辑
   - 从前端重新读回并渲染的路径
3. **如果数据流中任何一环可能失败**，必须报告，不能说"一致"

### 本项目已知陷阱

- `updateSettings()` 的 Rust 端期望完整 `AppSettings` 对象，**不能只传部分字段**。调用方必须 `{ ...DEFAULT_SETTINGS, ...currentSettings, changed_key: value }` 传完整对象。
- `as any` 是危险信号：看到 `as any` 时必须追问"为什么需要绕过类型检查"，并验证运行时是否真的兼容。

## 项目架构速查

- 前端：React + TypeScript + TanStack Query + i18next
- 后端：Tauri (Rust) + SQLite
- 通信：Tauri `invoke` 命令，Rust 端 `#[tauri::command]` 函数
- 设置存储：SQLite `config` 表 key-value，Rust `AppSettings` 结构体
- 路由核心：`src-tauri/src/proxy/router.rs`，读取 `settings.default_sort_mode` 决定排序策略

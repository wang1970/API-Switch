# 流式转发核心对比与重构思路（内部讨论稿）

> 用途：内部论证，不进入 Git 提交历史，仅供当前阶段设计讨论。
> 背景：当前版本整体已经比较稳定，任何涉及流式转发核心的调整都必须谨慎推进。

---

## 一、目标

本报告不是为了立即重写 `forwarder.rs`，而是为了明确：

1. **NEW-API 为什么稳定**
2. **我们当前核心的优势和特色是什么**
3. **如果未来要增强稳定性，应该怎么做，才能不破坏现有特色**
4. **哪些点可以借鉴，哪些点不应照搬**

---

## 二、当前判断

### 结论

当前不建议直接大改流式转发核心。

原因：

- 当前版本已经进入“**比较稳**”阶段
- 现有逻辑虽然存在一些结构问题，但功能完整，且与项目现有特性深度耦合
- 一旦贸然重构，容易影响：
  - 协议适配器输出
  - 日志记录
  - 流结束状态判定
  - 冷却 / 自动关闭策略
  - 实际模型命中与 attempt path 记录

因此建议：

> **先论证，再抽象，再局部替换。**

---

## 三、NEW-API 的稳定性来源（结论版）

对 `D:\Work\new-api` 的 relay 核心对比后，认为 NEW-API 稳定主要来自以下几点：

### 1. 流式处理职责拆分清晰
NEW-API 的 `stream_scanner.go` 将流式处理拆成：

- scanner：从 upstream 读取 SSE 行
- dataChan：中间通道
- dataHandler：负责写下游
- StreamStatus：专门记录流结束原因
- writeMutex：保护并发写

特点：

- 上游读取与下游写出解耦
- 流结束原因有独立状态模型
- ping、timeout、scanner error、client gone 等状态都能清楚归类

### 2. 对 SSE 输入过滤保守
NEW-API 只处理：

- `data: ...`
- `[DONE]`

其余内容几乎全部忽略。

这意味着：

- 对下游客户端更友好
- 更少主动污染字节流
- 更少兼容性问题

### 3. ping 是“可配置功能”，不是硬编码主链路
NEW-API 中 heartbeat/ping：

- 可以开关
- 可以配置间隔
- 可以按场景禁用
- 有相关测试覆盖

### 4. 流结束状态独立建模
NEW-API 使用 `StreamStatus`，能区分：

- Done
- EOF
- Timeout
- ClientGone
- PingFail
- ScannerErr
- Panic

这比“只看请求最终成败”更细粒度。

---

## 四、我们的特色（必须保留）

这部分是重点。

我们不能把项目改成 NEW-API 的样子，因为我们有自己的使用场景和特色。

### 1. 统一 `ProtocolAdapter` 体系
我们已经有：

- `openai`
- `claude`
- `custom`
- `gemini`
- `azure`

统一走 adapter 抽象。

这是当前架构的重要优点，不能为了流式稳定性把它拆散。

### 2. 个人工具导向的冷却 / 关闭模型逻辑
项目不是多用户公共网关，而是**个人桌面工具**。

当前已有的特色逻辑包括：

- 连续失败计数
- 达到阈值后关闭模型
- 1Day 冷却
- 手动开启时重置计数

这些逻辑非常符合本项目定位，必须保留。

### 3. 日志与实际命中信息非常有价值
当前日志体系中有一些非常重要的信息：

- `requested_model`
- `resolved_model`
- `attempt_path`
- `stream_end_reason`
- 实际尝试路径和状态码

这些能力非常适合个人排障，不应削弱。

### 4. Tauri 本地桌面工具定位
项目运行在本地，不是 server-only relay。意味着：

- 状态可以带有本地交互性
- 可以更偏向用户体验而不是网关吞吐
- 可以更强调“可解释性”和“可见性”

---

## 五、当前核心的主要问题（仅限结构，不代表立即要重构）

当前 `forwarder.rs` 的流式核心并不是“不工作”，而是**职责过重**。

### 主要问题：单个 `poll_fn` 承担了太多职责
它同时负责：

- 读取 upstream bytes stream
- 处理 SSE transform
- idle timeout
-（之前）ping 注入
- 成功/失败 usage log
- 冷却/关闭 side effect
- stream_end_reason 状态分流
- token 统计

这会导致：

> 流式输出字节本身和业务 side effect 强耦合。

### 风险

一旦某个逻辑处理不严谨，影响的不是“某个状态字段”，而是：

- 直接打断流
- 直接污染字节输出
- 直接影响下游 JSON/SSE 解析

这也是为什么 `: PING` 注入问题会显得危险——因为它插在了“真实字节输出主路径”里。

---

## 六、未来可接受的演进方向（不是现在立刻做）

### 总原则

不是“照搬 NEW-API”。
而是：

> **借用 NEW-API 稳定的流壳，但保留我们自己的协议层、日志层、冷却层和个人策略层。**

---

## 七、建议中的目标架构（讨论稿）

### 1. 保留不动的层

#### A. Router / Retry / Cooldown 层
保留现有逻辑：

- `resolve(...)`
- `forward_with_retry(...)`
- `attempts`
- `cool_down_entry(...)`
- `disable_entry(...)`
- `failure_counts`

#### B. ProtocolAdapter 层
继续保留：

- `transform_request`
- `transform_sse_line`
- `extract_sse_usage`
- `build_chat_url`
- `build_models_url`

#### C. Logging / AttemptPath 层
保留：

- `requested_model`
- `resolved_model`
- `attempt_path`
- `stream_end_reason`

---

### 2. 未来建议抽出来的层

建议新增类似：

```text
src-tauri/src/proxy/stream_core.rs
```

职责仅限：

- 从 upstream 读取 SSE
- 标准化为安全的下游输出帧
- 记录 first token / usage / done / timeout / upstream error / client gone
- 返回明确的 `StreamOutcome`

示例目标：

```rust
struct StreamMetrics {
    prompt_tokens: i64,
    completion_tokens: i64,
    first_token_ms: i64,
}

enum StreamOutcome {
    Done,
    Timeout,
    UpstreamError(String),
    ClientGone,
    Dropped,
}
```

然后由 `forwarder.rs` 外层继续处理：

- log_usage
- cooldown / disable
- circuit success/failure
- attempt path

这样就能把“字节流输出核心”和“业务 side effect”拆开。

---

## 八、关于 PING 的思考

### 当前判断

PING 不应该继续作为默认行为硬编码在主流输出路径中。

### 推荐方向

未来如果恢复 heartbeat，应该是：

```rust
struct StreamRelayOptions {
    idle_timeout: Duration,
    heartbeat: Option<HeartbeatMode>,
}

enum HeartbeatMode {
    Disabled,
    Comment(Duration),
}
```

默认：

```rust
heartbeat = Disabled
```

### 原因

这样做的好处：

- heartbeat 是否存在是显式配置，不再隐式埋在流输出里
- 可以按客户端特性关闭
- 更容易做兼容性测试
- 结构更清晰

---

## 九、如果未来真的要动，建议顺序

### 阶段 1：只抽取，不改变行为
把当前流式核心从 `forwarder.rs` 中抽出成独立函数，例如：

```rust
relay_sse_stream(...)
```

这一阶段目标：

- 不改冷却逻辑
- 不改 adapter
- 不改日志结构
- 只把代码从 `poll_fn` 中拆出

### 阶段 2：side effect 收口
定义统一的“流结束后处理函数”，例如：

```rust
handle_stream_finish(...)
```

把：

- log_usage
- cooldown
- disable
- record success/failure
- stream_end_reason

统一放到这个层里。

### 阶段 3：heartbeat 变可选
把 heartbeat 改成配置项，而不是主链路硬编码。

### 阶段 4：增加测试覆盖
重点测：

- 长流
- 慢上游
- 无 [DONE]
- client disconnect
- timeout
- 无 ping / 有 ping
- SSE comment 对下游兼容性

---

## 十、当前建议

### 现在不建议做的事

- 直接把 `forwarder.rs` 全面重写成 NEW-API 风格
- 为了“结构优雅”去牺牲现有日志和策略特性
- 引入过多 goroutine/状态对象而破坏当前可读性

### 现在建议做的事

1. 保持当前核心稳定运行
2. PING 问题先停留在“临时禁用”层面
3. 继续观察实际使用情况
4. 收集更多下游不兼容案例
5. 等论证充分后，再决定是否抽 `stream_core.rs`

---

## 十一、最终态度

这个项目目前已经不是“快速试验阶段”，而是进入了：

> **稳定优先，任何核心重构都必须经过多轮论证。**

因此建议：

- 当前仅记录思路
- 暂不做大规模核心重构
- 等更多实际数据、问题类型和论证结论收集齐全后，再进入设计落地阶段

---

## 十二、简版结论

### 我们应该借鉴 NEW-API 的：

- 流式处理职责拆分
- 流状态建模
- heartbeat 配置化
- 更保守的下游输出策略

### 我们绝不能丢掉的：

- `ProtocolAdapter` 统一层
- 个人工具导向的冷却 / 关闭策略
- `attempt_path` / `resolved_model` / `requested_model` 体系
- Tauri 本地工具的使用体验导向

### 当前最合适的策略：

> **先写报告，先论证，先观察，暂不贸然重构。**

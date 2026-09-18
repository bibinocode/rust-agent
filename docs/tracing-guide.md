# tracing 详细入门与实践指南

这份教程配合本项目的 `Cargo.toml`、`src/main.rs` 和 [Tokio 教程](tokio-guide.md) 使用。你将学会记录结构化日志、跟踪异步任务、定位错误，以及控制日志输出。

## 1. tracing 是什么，为什么不用 println!

`println!` 很适合刚开始调试，但并发程序会带来几个问题：

- 多个请求的输出交错，很难看出哪一条属于哪个请求。
- 希望开发环境输出细节，运行时只输出重要信息。
- 希望按 `task_id`、耗时、模型名称等字段搜索日志。
- 希望知道一条日志属于哪次调用、哪个子步骤。

`tracing` 提供结构化事件和执行上下文；配合 subscriber，你可以决定记录哪些内容、怎样格式化、输出到哪里。

| 概念 | 作用 | 例子 |
| --- | --- | --- |
| Event（事件） | 某个时刻发生的事情 | “请求失败”，包含错误字段 |
| Span（上下文） | 描述一段操作及其关联字段 | “处理任务”，包含 `task_id` |
| Subscriber | 接收事件和 span 的通知 | 决定是否记录，以及如何处理 |
| Layer | 可组合的订阅处理层 | 日志格式化、过滤、链路导出 |

可以先这样理解：**event 记录发生了什么，span 说明这件事处于什么操作中。**

`tracing` 本身不等于远程监控系统。若要跨进程链路追踪，还需要上下文传播、导出组件和后端服务。

## 2. 当前项目已有的依赖

项目当前配置：

```toml
[dependencies]
tracing = "0.1.44"
tracing-subscriber = "0.3.23"
```

两个 crate 的分工：

- `tracing`：提供 `info!`、`warn!`、`error!`、span 和 `#[instrument]` 等埋点接口。
- `tracing-subscriber`：提供常用 subscriber、格式化输出和可组合的 layer。

当前默认功能足够运行基础示例，并支持 `#[instrument]`。后面介绍的 `EnvFilter` 和 JSON 输出需要显式添加功能开关，不能直接假定当前配置已经启用它们。

### 示例怎么运行

本文带 `main` 的 Rust 代码块是相互独立的示例。任选一个保存为 `examples/tracing_demo.rs`，运行：

```powershell
cargo run --example tracing_demo
```

不要把多个 `main` 合并到同一个文件。第 7 节和第 12 节分别标注了额外功能要求；其他完整示例可使用项目当前依赖。

## 3. 最小示例：初始化后记录事件

```rust
use tracing::{debug, error, info, trace, warn};

fn main() {
    tracing_subscriber::fmt::init();

    trace!("非常细的执行信息");
    debug!("调试信息");
    info!("Agent 启动");
    warn!("即将达到请求配额");
    error!("一次模拟请求失败");
}
```

默认的格式化 subscriber 通常输出 `INFO` 及以上级别，因此这里会看到 `info!`、`warn!`、`error!`，而不会看到 `debug!` 和 `trace!`。

输出可能包含时间、级别、target 和消息。具体外观取决于版本、终端颜色支持和格式配置，不要依赖某一段时间戳或 ANSI 颜色序列。

如果没有安装 subscriber，普通 `tracing` 事件通常不会变成可见的终端日志。事件宏也不会像 `println!` 一样无条件打印。

### 3.1 对照当前 main.rs

你目前已调用 `dotenv::dotenv().ok()`，下一行正好可以初始化日志：

```rust
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), String> {
    // 忽略加载 .env 的失败：本地没有文件时，也可以使用系统环境变量。
    dotenv::dotenv().ok();

    // 安装全局 subscriber，应在业务日志产生前完成。
    tracing_subscriber::fmt::init();

    info!("Agent 启动完成");
    Ok(())
}
```

`.ok()` 会把 `Result` 转成 `Option`，这里连这个 `Option` 也被丢弃，因此加载失败被忽略。它与 `.await?` 中“遇到错误提前返回”的 `?` 含义不同。

此代码只是建议的入口写法；本教程不要求修改现有入口。

## 4. 日志级别怎么选

严重程度从低到高：`TRACE < DEBUG < INFO < WARN < ERROR`。

| 级别 | 适合记录 | Agent 例子 |
| --- | --- | --- |
| `TRACE` | 非常细的内部过程 | 状态机每次转换 |
| `DEBUG` | 定位问题需要的细节 | 重试次数、请求参数摘要 |
| `INFO` | 正常运行的关键节点 | 启动、任务完成、耗时 |
| `WARN` | 出现异常但仍可恢复 | 暂时限流，准备退避重试 |
| `ERROR` | 操作失败，需要关注 | 重试耗尽，任务最终失败 |

日志级别不改变程序控制流：

- `error!(...)` 不会自动返回 `Err`，也不会让进程退出。
- `warn!(...)` 不会自动重试。
- `info!(...)` 也不是操作成功的证明，成功状态需要代码保证。

通常在最终处理错误的边界记录错误，底层负责补充信息并向上返回，避免每层都重复打印同一错误。

## 5. 结构化字段：比拼接字符串更有用

```rust
use tracing::info;

fn main() {
    tracing_subscriber::fmt::init();

    let task_id = 42_u64;
    let model = "demo-model";
    let elapsed_ms = 125_u64;
    let tools = vec!["search", "calculator"];

    info!(task_id, model, elapsed_ms, success = true, "任务完成");
    info!(available_tools = ?tools, "工具列表");

    let error = std::io::Error::other("模拟网络异常");
    tracing::error!(task_id, error = %error, "请求失败");
}
```

### 5.1 字段语法拆解

| 写法 | 含义 |
| --- | --- |
| `task_id` | 字段名和变量名相同的简写 |
| `elapsed_ms = 125_u64` | 显式指定字段名和值 |
| `success = true` | 布尔字段 |
| `error = %error` | 使用 `Display` 格式记录 |
| `available_tools = ?tools` | 使用 `Debug` 格式记录 |
| `"任务完成"` | 人类可读的消息，通常对应 `message` 字段 |

`%value` 类似格式化字符串的 `{}`，`?value` 类似 `{:?}`。字符串、数值和布尔值通常可以直接作为字段。

结构化字段便于过滤、聚合和机器处理。比如 `elapsed_ms = 125` 可以保留数值语义；只写 `"耗时 125ms"` 则需要重新解析文本。

### 5.2 这里的 ? 不是错误传播

你之前问过 `.await?`。现在又看到 `?tools`，注意它们不是同一种语法：

```text
request().await?           Rust 的后缀 ?：失败则提前返回
info!(tools = ?tools)      tracing 宏的字段语法：使用 Debug 格式
info!(error = %error)      tracing 宏的字段语法：使用 Display 格式
```

`tracing` 宏里的 `?tools` 不会取出 `Some`，也不会让函数返回错误。

同样，`Debug` 不保证自动脱敏。对配置对象直接写 `?config`，可能把凭据也打印出来。

## 6. 控制输出格式和固定级别

```rust
use tracing::{debug, info, Level};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_ansi(false)
        .compact()
        .init();

    debug!(retry = 1, "准备请求");
    info!("日志配置完成");
}
```

这里的 builder 方法逐步构造 subscriber，最后 `.init()` 才安装它。

- `with_max_level(Level::DEBUG)`：允许 `DEBUG`、`INFO`、`WARN`、`ERROR`，过滤 `TRACE`。
- `with_target(true)`：显示事件来源，默认 target 通常是模块路径。
- `with_file(true)` / `with_line_number(true)`：辅助定位日志产生位置。
- `with_thread_ids(true)`：显示线程 ID；异步任务会迁移，线程 ID 不能替代任务 ID。
- `with_ansi(false)`：关闭颜色转义，便于重定向到文件或检查输出。
- `compact()`：紧凑格式；`pretty()` 更适合本地阅读。

格式化 subscriber 默认写入标准输出。希望日志走标准错误时，可以配置 `.with_writer(std::io::stderr)`，避免与程序的正常数据输出混在一起。

不要在业务循环中反复调用 `.init()`。全局 subscriber 通常只能安装一次，再次初始化可能 panic。可处理初始化失败时使用 `.try_init()`；库代码通常应由调用它的应用负责全局初始化。

## 7. 用 RUST_LOG 动态控制日志

固定写死 `DEBUG` 不方便部署。常用方案是读取 `RUST_LOG`，让运行环境决定过滤规则。

### 7.1 先开启 env-filter 功能

将 `Cargo.toml` 中原来的 `tracing-subscriber` 配置替换为：

```toml
tracing-subscriber = { version = "0.3.23", features = ["env-filter"] }
```

不要重复添加同名依赖。以下完整示例**需要 `env-filter` 功能**：

```rust
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,rust_agent=debug"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .try_init()
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    // 显式 target 使示例放在 examples/ 中时，仍匹配 rust_agent 规则。
    debug!(target: "rust_agent", "应用调试日志");
    info!("日志系统已初始化");
    Ok(())
}
```

这里错误转换只是为了适配示例入口的错误类型。实际应用也可以统一使用 `Box<dyn Error + Send + Sync>` 等错误类型。

`.env` 中可以添加这一行：

```dotenv
RUST_LOG=info,rust_agent=debug
```

该规则表示通常允许 `INFO` 及以上；匹配 `rust_agent` target 的事件和 span 则允许到 `DEBUG`。

Cargo 包名是 `rust-agent`，但 Rust crate 路径通常是 `rust_agent`。此外，独立 example 具有自己的 crate 名；如果不指定 target，过滤规则需要匹配 example 的模块路径，例如 `tracing_demo=debug`。

PowerShell 中也可以这样设置：

```powershell
$env:RUST_LOG = 'warn,rust_agent=debug'
cargo run
```

常用规则：

| RUST_LOG | 作用 |
| --- | --- |
| `info` | 通常只看 INFO 及以上 |
| `debug` | 包括依赖库的 DEBUG，可能很多 |
| `warn,rust_agent=debug` | 应用详细，其他来源只看警告及错误 |
| `off` | 关闭受此过滤器控制的记录 |

注意几点：

1. 先加载 `.env`，再构建 `EnvFilter`；否则刚构建的过滤器读取不到之后加载的值。
2. 本项目当前未启用 `env-filter`，仅设置 `RUST_LOG` 并不能保证改变基础 subscriber 的行为；显式配置最清晰。
3. 上例对缺失和无效规则都使用默认值；希望无效配置直接启动失败时，应区分这两种情况。
4. 环境变量在构建过滤器时读取，运行后修改文件不会自动热更新；动态更新需要 `reload` 等机制。
5. 如果使用编译期最大日志级别功能移除了事件，运行时过滤器无法恢复它们。

## 8. Span：给一次操作建立上下文

事件描述一个时间点，span 表示一次操作，可以承载稳定字段，并包含子操作。

```rust
use tracing::{info, info_span};

fn main() {
    tracing_subscriber::fmt::init();

    let task = info_span!("agent_task", task_id = 42_u64);
    task.in_scope(|| {
        info!("开始处理任务");

        let tool = info_span!("tool_call", tool_name = "calculator");
        tool.in_scope(|| {
            info!("开始计算");
            info!(answer = 4, "计算完成");
        });

        info!("任务完成");
    });
}
```

逻辑上下文类似这样：

```text
agent_task{task_id=42}
  ├─ 开始处理任务
  ├─ tool_call{tool_name="calculator"}
  │    ├─ 开始计算
  │    └─ 计算完成 answer=4
  └─ 任务完成
```

创建 span 不等于进入 span。`in_scope` 在同步闭包执行期间进入该上下文，闭包返回时退出。**不要用 `in_scope(|| async { ... })` 包装异步函数体**：那只是在上下文中创建 Future，实际执行可能发生在闭包返回以后。

Span 的生命周期与“当前正在进入 span”的时间也不同。一个异步操作可以在每次被轮询时进入，暂停时退出，最后完成时关闭。

需要完整父子上下文时，要注意父 span 自身也可能被级别或 target 过滤掉。

## 9. 在 Tokio 中正确使用 span

### 9.1 使用 Instrument 包装 Future

```rust
use tokio::time::{sleep, Duration};
use tracing::{info, info_span, Instrument};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let task_span = info_span!("agent_task", task_id = 7_u64);
    let handle = tokio::spawn(
        async {
            info!("开始请求");
            sleep(Duration::from_millis(20)).await;
            info!("请求完成");
        }
        .instrument(task_span),
    );

    handle.await?;
    Ok(())
}
```

`.instrument(span)` 会让 Future 每次执行时进入 span，在暂停、交还控制权时退出，因此不会把别的任务误关联进来。

不要让手动 `span.enter()` 得到的 guard 跨越 `.await`。否则当前线程转而执行其他任务时，上下文仍可能处于错误状态。

### 9.2 spawn 不会自动继承当前 span

新任务需要显式关联上下文：

- `.instrument(span)`：指定某个 span。
- `.in_current_span()`：在包装 Future 时捕获当前 span。

二者都来自 `tracing::Instrument`。如果期望 `.in_current_span()` 捕获请求上下文，调用它时必须已经处于请求 span 中。

`spawn_blocking` 的同步闭包也不会自动获得所需 span，可以先取 `Span::current()`，再在闭包内部使用 `span.in_scope(|| ...)`。因为闭包是同步代码，这种 `in_scope` 用法是合适的。

## 10. #[instrument]：自动跟踪函数

对于每个都要跟踪的业务函数，手动创建 span 比较繁琐。`#[instrument]` 可以为每次调用自动创建并进入函数 span，并正确包装异步函数。

```rust
use tokio::time::{sleep, Duration};
use tracing::{info, instrument};

#[instrument(
    name = "model_request",
    skip(api_key, prompt),
    fields(prompt_bytes = prompt.len()),
    err
)]
async fn call_model(
    task_id: u64,
    api_key: &str,
    prompt: &str,
) -> Result<String, std::io::Error> {
    if api_key.is_empty() {
        return Err(std::io::Error::other("缺少 API key"));
    }

    info!("开始模拟模型调用");
    sleep(Duration::from_millis(20)).await;
    Ok(format!("任务 {task_id} 的模拟响应"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let answer = call_model(42, "demo-key", "解释 Tokio").await?;
    info!(answer_bytes = answer.len(), "收到响应");
    Ok(())
}
```

属性参数解释：

| 配置 | 作用 |
| --- | --- |
| `name = "model_request"` | 自定义 span 名；默认是函数名 |
| `level = "debug"` | 修改 span 级别；默认 INFO |
| `skip(api_key, prompt)` | 不自动记录这些参数 |
| `skip_all` | 不自动记录任何参数，只记录明确指定的字段 |
| `fields(prompt_bytes = prompt.len())` | 增加自定义字段 |
| `err` | 返回 Err 时生成错误事件，默认使用 Display 和 ERROR 级别 |
| `ret` | 返回时记录返回值，默认使用 Debug |

默认会记录未跳过的函数参数，许多类型会使用 `Debug`，因此参数可能需要实现相应格式化能力。对于客户端对象、`self`、大型结构体，常用 `skip(self, client)` 或 `skip_all`。

`err` 只是记录错误，不会吞掉错误或替你重试；`?` 依然控制错误传播。使用 `err` 后，如果上层再记录同一错误，要考虑是否造成重复日志。

对模型原文、用户输入、凭据等，不宜直接启用 `ret` 或默认参数记录。可以记录长度、状态、请求 ID 等摘要。这里 `prompt.len()` 是 UTF-8 字节数，不是中文字符数。

## 11. 事后补充字段与耗时

有些信息在操作开始时不知道，例如响应 token 数或最终状态，可以先声明空字段，再调用 `record` 更新。

```rust
use std::time::Instant;
use tokio::time::{sleep, Duration};
use tracing::{field, info, instrument, Span};

#[instrument(fields(output_tokens = field::Empty, status = field::Empty))]
async fn run_task(task_id: u64) {
    let started = Instant::now();
    sleep(Duration::from_millis(20)).await;

    Span::current().record("output_tokens", 128_u64);
    Span::current().record("status", "ok");

    info!(elapsed_ms = started.elapsed().as_millis() as u64, "任务完成");
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    run_task(42).await;
}
```

span 字段需要在创建时声明，不能通过 `record` 随意增加从未声明过的字段。字段更新也不会回头修改已经写出的日志行。

### 11.1 Span 自动生命周期日志

如果需要观察创建、进入、退出、关闭等通知，可在 fmt builder 上配置：

```text
.with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
```

`CLOSE` 在 span 最终关闭时产生相应格式化事件；`NEW` 记录创建；`ENTER` / `EXIT` 记录进入退出；`FULL` 输出所有这些通知，在异步高并发下可能非常多。

开启计时的格式化输出通常会包含 busy / idle 时间。busy 表示 span 处于 entered 状态的时间，不等于精确 CPU 使用时间；idle 也不等于纯网络延迟。要测量一次业务操作从开始到结束的墙钟耗时，显式使用 `Instant` 更直接。

关闭发生在 span 的所有相关句柄释放后，不能假定某一个局部变量离开作用域时必然已经关闭。

## 12. JSON 日志与 Layer

生产日志经常交给采集器，需要机器可解析的格式。可以开启 JSON 输出。

先替换依赖配置：

```toml
tracing-subscriber = { version = "0.3.23", features = ["env-filter", "json"] }
```

以下完整示例**需要 `env-filter` 和 `json` 功能**：

```rust
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn main() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let output = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(true);

    tracing_subscriber::registry()
        .with(filter)
        .with(output)
        .init();

    info!(task_id = 42_u64, elapsed_ms = 125_u64, "任务完成");
}
```

这次没有直接使用 `fmt()`，而是把 registry、过滤 layer、格式化 layer 组合起来。

```text
tracing 埋点 → registry + filter + formatting layer → JSON 输出
```

随着需求增加，可以组合更多 layer，例如终端输出和文件输出。但“全局过滤”和“某一 layer 自己的过滤”是不同概念，配置多个输出时应确认过滤规则作用的范围。

JSON 默认一般将事件字段放在 `fields` 对象内。`with_current_span` 和 `with_span_list` 控制当前及祖先 span 信息，字段布局需要以实际配置的输出为准。

## 13. Agent 风格完整示例

这个例子不访问网络、不读取真实凭据，直接使用当前依赖即可运行。它演示每个任务的上下文、业务错误、超时和按完成顺序收集结果。

```rust
use std::time::Instant;
use tokio::task::JoinSet;
use tokio::time::{sleep, timeout, Duration};
use tracing::{error, info, info_span, warn, Instrument};

async fn mock_tool(id: u64) -> Result<String, String> {
    info!("工具开始执行");
    let wait_ms = if id == 3 { 200 } else { 20 };
    sleep(Duration::from_millis(wait_ms)).await;

    if id == 2 {
        return Err("模拟参数错误".to_owned());
    }
    Ok("模拟结果".to_owned())
} 

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(false)
        .init();

    let mut tasks = JoinSet::new();
    for id in 1..=3_u64 {
        let span = info_span!("agent_task", task_id = id, tool = "demo");
        tasks.spawn(
            async move {
                let started = Instant::now();
                let success = match timeout(Duration::from_millis(80), mock_tool(id)).await {
                    Ok(Ok(response)) => {
                        info!(response_bytes = response.len(), "工具成功");
                        true
                    }
                    Ok(Err(error)) => {
                        error!(error = %error, "工具业务失败");
                        false
                    }
                    Err(error) => {
                        warn!(error = %error, timeout_ms = 80_u64, "工具超时");
                        false
                    }
                };

                info!(success, elapsed_ms = started.elapsed().as_millis() as u64, "任务结束");
                success
            }
            .instrument(span),
        );
    }

    let mut succeeded = 0_u64;
    let mut failed = 0_u64;
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(true) => succeeded += 1,
            Ok(false) => failed += 1,
            Err(error) => {
                failed += 1;
                error!(error = %error, "任务 panic 或被取消");
            }
        }
    }

    info!(succeeded, failed, "所有任务已收集");
}
```

预期结果：任务 1 成功，任务 2 业务失败，任务 3 超时；最后统计 `succeeded=1`、`failed=2`。日志顺序可能交错，但每条任务内部日志都包含对应 `agent_task` 上下文。

示例只创建三个任务；真实任务流仍需要并发限制，具体参见 Tokio 教程。这里记录错误后继续处理，因此不会仅因为输出了 `ERROR` 就返回非零退出码。

`join_next` 外层的日志不自动处于子任务 span 中。如果希望任务 panic 时也能标识业务 ID，需要额外维护任务标识与业务 ID 的关联，而不能只依赖被取消任务内部的日志。

超时只停止本地 Future 的继续执行，不保证撤销已经发生的外部副作用。

## 14. 日志文件、测试和常见陷阱

### 14.1 写文件与非阻塞输出

格式化日志默认写标准输出，也可以自定义 writer。但文件写入可能阻塞调用线程，高吞吐应用常用额外的 `tracing-appender` crate 提供后台写入和滚动文件。

使用 `tracing_appender::non_blocking` 时要保留返回的 `WorkerGuard`，使它活到应用退出前。若 guard 很早被丢弃，后台写入可能提前停止，日志可能丢失。

非阻塞队列仍有容量和丢失/背压策略，不是无限可靠的存储。正常退出应先等待业务任务结束，再让日志 guard 释放；进程强制终止时不能保证完整刷盘。

### 14.2 测试中的初始化

多个测试反复调用 `.init()` 容易冲突。常见做法是通过 `Once` 在测试进程中初始化一次，或有意忽略 `.try_init()` 的“已经初始化”错误。

fmt builder 的 `.with_test_writer()` 可配合 Rust 测试框架捕获输出。需要展示成功测试的日志时可以运行：

```powershell
cargo test -- --nocapture
```

全局 subscriber 由测试进程共享，不代表每个测试都有独立配置。同步的 `with_default` 作用域也不能直接包一个异步 Future 就认为覆盖了它后续的执行；涉及异步局部 subscriber 时，需要正确的 Future 包装和上下文传播。

### 14.3 问题速查

| 现象 | 常见原因 | 修复方向 |
| --- | --- | --- |
| 没有任何日志 | subscriber 未初始化或事件被过滤 | 在入口初始化，检查级别和 target |
| DEBUG 看不到 | 默认过滤到 INFO | 调整 max level 或 EnvFilter |
| RUST_LOG 没效果 | 未启用/配置 EnvFilter，或初始化顺序不对 | 启用 feature，在读取前加载环境变量 |
| 提示 EnvFilter/json 不存在 | 对应 feature 未开启 | 按第 7、12 节修改依赖配置 |
| 重复初始化 panic | 全局 subscriber 已安装 | 初始化一次，或处理 try_init 错误 |
| 异步日志属于错误任务 | enter guard 跨 await | 改用 Instrument 或 instrument 属性 |
| spawn 后没有父上下文 | 新任务没有显式关联 span | 使用 instrument / in_current_span |
| record 后没有字段 | span 创建时未声明字段，或 span 被过滤 | 声明 Empty 字段并检查过滤 |
| 输出 ERROR 后程序继续 | 日志宏不控制业务流程 | 显式 return Err 或设置退出状态 |
| 同一错误反复出现 | 每层都记录，再加 instrument(err) | 选择主要记录边界，避免重复 |
| 文件中出现颜色乱码 | ANSI 转义被重定向 | with_ansi(false) |
| 日志暴露输入或配置 | 默认参数 Debug 或 ret 记录过多 | skip/skip_all，只记录必要字段 |

## 15. 推荐练习顺序

1. 运行第 3 节，确认默认能看到哪些级别。
2. 运行第 6 节，将 `DEBUG` 改成 `WARN`，比较输出。
3. 在第 5 节分别尝试普通字段、`%` 和 `?`，与错误传播的后缀 `?` 区分。
4. 运行第 8 节，观察父 span 和子 span 的字段如何出现在事件旁边。
5. 运行第 13 节，确认成功、业务失败和超时都能对应到 `task_id`。
6. 按第 7 节启用过滤，通过 `RUST_LOG` 调整应用和依赖的级别。
7. 按第 12 节输出 JSON，观察数值字段与文本字段的区别。

当前 Agent 项目可以先采用“入口初始化一次 + 任务级 span + 结构化结果日志”。随着实际需求增加，再加入 EnvFilter、文件输出或链路导出。

## 16. 官方资料

- [tracing API](https://docs.rs/tracing)
- [tracing-subscriber API](https://docs.rs/tracing-subscriber)
- [instrument 属性](https://docs.rs/tracing/latest/tracing/attr.instrument.html)
- [Instrument trait](https://docs.rs/tracing/latest/tracing/trait.Instrument.html)
- [EnvFilter](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html)
- [tracing-appender](https://docs.rs/tracing-appender)

运行 `cargo doc --open -p tracing` 或 `cargo doc --open -p tracing-subscriber`，可查看项目当前实际解析版本的文档。遇到在线示例无法编译时，优先核对版本、feature 和需要导入的 trait。

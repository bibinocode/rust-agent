# anyhow 详细入门与实践指南

本教程结合当前 `src/main.rs`，解释 `anyhow::Result`、`?`、错误上下文和异步任务中的错误处理。可与 [Tokio 教程](tokio-guide.md)、[tracing 教程](tracing-guide.md) 配合阅读。

## 1. anyhow 解决什么问题

一个 Agent 程序可能依次执行：

1. 加载 `.env`。
2. 初始化日志。
3. 读取配置文件。
4. 解析 JSON。
5. 调用模型或工具。

这些步骤的错误类型通常不同，例如 `std::io::Error`、`serde_json::Error` 和日志初始化错误。如果入口函数要接收这些错误，就需要一种统一表示方式。

`anyhow` 提供适合应用层使用的错误容器，让不同类型的错误可以通过 `?` 向上传播，同时允许你补充“正在做什么”的上下文。

它主要提供：

| 接口 | 用途 |
| --- | --- |
| `anyhow::Error` | 容纳不同具体类型的错误，并支持上下文和错误链 |
| `anyhow::Result<T>` | 默认错误类型为 `anyhow::Error` 的 Result |
| `Context` trait | 给 Result 或 Option 补充错误上下文 |
| `anyhow!` | 构造一个错误值 |
| `bail!` | 构造错误并立即返回 |
| `ensure!` | 条件不满足时立即返回错误 |

**anyhow 不会自动重试、修复错误，也不会自动记录日志。** 它负责表示和传播错误；如何恢复、重试或结束程序，仍由业务代码决定。

当前项目已配置 `anyhow = "1.0.104"`，本文示例无需新增依赖。

### 如何运行示例

本文带 `main` 的 Rust 代码块都是独立示例，可以任选一个保存到 `examples/anyhow_demo.rs`，运行：

```powershell
cargo run --example anyhow_demo
```

不要把多个 `main` 拼到同一文件。最后的测试示例放入 `tests/anyhow_basics.rs`。

## 2. 先理解普通 Result

标准库的 `Result<T, E>` 有两个泛型参数：

- `T`：成功时的值类型。
- `E`：失败时的错误类型。

对应两种值：

```text
Result<T, E>
  ├─ Ok(value: T)
  └─ Err(error: E)
```

例如 `Result<String, std::io::Error>` 表示成功返回字符串，失败返回 I/O 错误。

`Result<(), E>` 中的 `()` 是单元类型，表示没有需要返回的业务数据。成功仍然需要写 `Ok(())`，因为函数必须返回一个 Result。

### 2.1 anyhow::Result<()> 是什么

常用形式可以理解为：

```text
anyhow::Result<T> ≈ std::result::Result<T, anyhow::Error>
```

因此：

```text
anyhow::Result<()>      成功没有业务返回值，失败返回 anyhow::Error
anyhow::Result<String>  成功返回 String，失败返回 anyhow::Error
```

它仍然是标准库的 Result，不是另一套异步机制。`anyhow::Result` 也允许显式指定第二个错误类型参数，但应用代码通常使用默认值。

导入后可以写得更短：

```rust
use anyhow::Result;

fn greeting(name: &str) -> Result<String> {
    Ok(format!("你好，{name}"))
}

fn main() -> Result<()> {
    let message = greeting("Rust")?;
    println!("{message}");
    Ok(())
}
```

这里的 `Result` 来自 `anyhow`，可以从 `use` 看出来。这个简单函数没有实际失败分支，只是演示返回类型；真实项目中，不会失败的函数通常直接返回 `String` 即可。

## 3. 对照当前 main.rs

你当前的入口采用了这样的结构：

```text
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv()?;
    // 构建 subscriber
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}
```

逐项解释：

1. Tokio 宏负责运行异步入口，和 anyhow 各司其职。
2. `anyhow::Result<()>` 让入口能够传播多种不同的错误。
3. `dotenvy::dotenv()?` 成功时取得加载文件的路径；这里没有保存路径，所以结果被丢弃。失败则立即返回错误。
4. 日志初始化也可以使用 `?`，其错误会转换成 `anyhow::Error`。
5. 所有步骤成功后返回 `Ok(())`。

需要留意：**`dotenvy::dotenv()?` 会把找不到 `.env` 也视为启动失败。** 如果你的程序允许仅依赖系统环境变量，就应有意选择处理方式，而不是无条件认为所有 `.env` 错误都可以忽略。文件缺失和内容解析失败也不应混为一谈。

如果旧入口是 `Result<(), String>`，不同错误通常不能直接用 `?` 转成 String。改为 anyhow 后，符合条件的标准错误类型可以自动转换，不需要每一步都写 `.map_err(|e| e.to_string())`。

## 4. ? 与 anyhow 的关系

`?` 是 Rust 语言的运算符，不是 anyhow 发明的语法。

对 Result 使用时，可以近似理解为：

```text
match operation() {
    Ok(value) => value,
    Err(error) => return Err(error.into()),
}
```

anyhow 让许多错误类型能转换到统一的 `anyhow::Error`。通常适用的是实现 `std::error::Error + Send + Sync + 'static` 的具体错误类型。

- `Send`：错误值可以在线程间转移。
- `Sync`：错误值的共享引用可以安全跨线程使用。
- `'static`：错误内部不能依赖随时可能失效的短生命周期借用；不代表错误必须活到程序结束。

### 4.1 自动传播不同错误类型

```rust
use anyhow::Result;

fn parse_count(text: &str) -> Result<u32> {
    let number = text.parse::<u32>()?;
    Ok(number)
}

fn main() -> Result<()> {
    let count = parse_count("3")?;
    let value: serde_json::Value = serde_json::from_str(r#"{"enabled":true}"#)?;

    println!("count={count}, config={value}");
    Ok(())
}
```

整数解析错误和 JSON 解析错误不是同一种类型，但都可以传播到这个入口。

### 4.2 .await? 怎么读

```text
let response = request().await?;
               └ 调用 ┘ └等待┘└错误传播
```

`.await` 得到异步函数的输出，如果这个输出是 Result，接着使用 `?` 解包成功值或提前返回错误。

不要写普通调用的 `request().await()?`。这会尝试把等待结果当作函数再调用。

### 4.3 Result<T, String> 不能一概直接 ?

`String` 本身没有实现标准错误 trait，因此不能把所有 `Result<T, String>` 都直接当作标准错误来源自动转进 anyhow。

适配这种接口时可以显式转换：

```rust
use anyhow::{anyhow, Result};

fn legacy_call() -> std::result::Result<u32, String> {
    Err("旧接口返回的字符串错误".to_owned())
}

fn run() -> Result<u32> {
    let value = legacy_call().map_err(|message| anyhow!(message))?;
    Ok(value)
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
    }
}
```

这里旧接口原本就只返回文本，因此没有更具体的底层错误类型可保留。新接口设计应优先考虑标准错误类型或 anyhow，而不是到处返回 String。

## 5. Context：告诉你是在做什么时失败

仅看到 `invalid digit found in string`，你可能不知道解析的是端口、并发数还是 token 限制。

通过 `Context` 可以增加业务含义，同时保留原始错误：

```rust
use anyhow::{Context, Result};

fn load_concurrency(text: &str) -> Result<usize> {
    text.parse::<usize>()
        .context("解析 Agent 并发数失败")
}

fn run() -> Result<()> {
    let count = load_concurrency("invalid")
        .context("初始化 Agent 配置失败")?;
    println!("并发数：{count}");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("最外层：{error}");
        eprintln!("完整错误链：{error:#}");
    }
}
```

错误链逻辑上是：

```text
初始化 Agent 配置失败
  → 解析 Agent 并发数失败
    → invalid digit found in string
```

### 5.1 为什么要 use anyhow::Context

`.context()` 和 `.with_context()` 由 `Context` trait 提供。忘记导入时，即使安装了 anyhow，也可能报“找不到方法”。

```text
use anyhow::{Context, Result};
```

### 5.2 context 与 with_context

| 方法 | 上下文求值方式 | 适用情况 |
| --- | --- | --- |
| `.context("固定信息")` | 参数表达式立即求值 | 简短静态说明 |
| `.with_context(|| format!(...))` | 仅在失败时调用闭包 | 需要构造动态字符串 |

例如：

```rust
use anyhow::{Context, Result};

fn parse_limit(setting_name: &str, text: &str) -> Result<usize> {
    text.parse::<usize>()
        .with_context(|| format!("配置项 {setting_name} 必须是非负整数"))
}

fn main() -> Result<()> {
    let limit = parse_limit("MAX_TASKS", "8")?;
    println!("任务上限：{limit}");
    Ok(())
}
```

`context(format!(...))` 并非错误，但成功时也会先构造字符串。

上下文建议描述动作和对象，例如“读取工具配置失败”“解析模型响应失败”。不要只重复“发生错误”，也不要无意把 API key、完整请求或私密输入拼进去。

### 5.3 不要过早转成字符串

如果把一个有类型的错误先 `.to_string()`，再包装成新错误，就会失去原来的错误类型及其 source 链。

优先使用 `.context(...)` 或 `.with_context(...)` 补充信息。保留原始错误后，既可以查看来源，也可以在确有需要时判断错误类型。

## 6. Option 也可以使用 context

`Option<T>` 表示“有值或没有值”，本身没有错误说明。

anyhow 的 `Context` trait 可以将 Option 转为 Result：

```rust
use anyhow::{Context, Result};

fn first_tool(tools: &[String]) -> Result<&str> {
    let first = tools.first().context("未配置任何可用工具")?;
    Ok(first.as_str())
}

fn main() -> Result<()> {
    let tools = vec!["calculator".to_owned()];
    println!("第一个工具：{}", first_tool(&tools)?);
    Ok(())
}
```

转换关系：

```text
Some(value).context("说明") → Ok(value)
None.context("说明")        → Err(带说明的 anyhow::Error)
```

返回 Result 的函数通常不能直接对 Option 使用 `?`。需要先把“缺失”转换成错误，这正是上面 `.context(...)?` 的作用。

如果“没有工具”是正常业务状态，应保留 Option 或显式分支处理，不必把它强行定义为错误。

## 7. anyhow!、bail! 与 ensure!

当错误不是底层库产生，而是业务检查发现时，可以自己构造错误。

```rust
use anyhow::{anyhow, bail, ensure, Result};

fn validate(model: &str, concurrency: usize) -> Result<()> {
    ensure!(concurrency > 0, "并发数必须大于 0");
    ensure!(concurrency <= 16, "并发数超过本例上限：{concurrency}");

    if model.trim().is_empty() {
        bail!("模型名称不能为空");
    }

    if model == "unsupported-demo" {
        return Err(anyhow!("本例不支持模型 {model}"));
    }

    Ok(())
}

fn main() -> Result<()> {
    validate("demo-model", 3)?;
    println!("配置有效");
    Ok(())
}
```

三者的区别：

| 写法 | 构造错误 | 立即返回当前函数 |
| --- | --- | --- |
| `anyhow!("信息")` | 是 | 否 |
| `bail!("信息")` | 是 | 是 |
| `ensure!(条件, "信息")` | 条件为 false 时 | 条件为 false 时 |

`bail!(...)` 可理解为 `return Err(anyhow!(...))`。

`ensure!(condition, ...)` 可理解为 `if !condition { bail!(...) }`。

不要把 `ensure!` 和 `assert!` 混为一谈：前者返回可处理的错误，后者失败时 panic。用户提供了无效参数，一般应返回错误，而不是使程序 panic。

## 8. 怎样显示错误链

anyhow 的格式化方式不同，显示的信息也不同：

| 写法 | 通常显示 |
| --- | --- |
| `format!("{error}")` | 最外层错误说明 |
| `format!("{error:#}")` | 以冒号连接的完整错误链 |
| `format!("{error:?}")` | Debug 报告，包含错误链；可包含已捕获的 backtrace |
| `error.chain()` | 逐层遍历错误来源 |
| `error.root_cause()` | 最底层原因 |

```rust
use anyhow::{Context, Result};

fn parse() -> Result<u32> {
    "bad".parse::<u32>().context("读取重试次数失败")
}

fn main() {
    if let Err(error) = parse() {
        for (index, cause) in error.chain().enumerate() {
            eprintln!("第 {index} 层：{cause}");
        }
        eprintln!("根因：{}", error.root_cause());
    }
}
```

### 8.1 错误链与 backtrace 不一样

- 错误链回答“哪一层操作失败、底层是什么原因”。
- Backtrace 帮助定位捕获错误时的调用栈。

需要调查时，可以在启动程序前设置：

```powershell
$env:RUST_BACKTRACE = '1'
cargo run --example anyhow_demo
```

`RUST_LIB_BACKTRACE=1` 可用于控制错误 backtrace；若显式设置，它会影响 anyhow 是否捕获错误堆栈。具体显示还取决于是否捕获成功以及采用的格式化方式。

Backtrace 不是业务上下文的替代品，异步代码的物理调用栈也不等于完整逻辑任务链。耗时定位和任务关联仍可借助 tracing span。

## 9. anyhow 与 tracing 各自负责什么

推荐分工：

```text
底层函数执行操作
  → context 补充说明
  → ? 向上传播
  → 应用边界决定记录、重试或退出
```

anyhow 不会自动调用 `error!`，`error!` 也不会替你返回 `Err`。

### 9.1 %error 可能只记录最外层

对 anyhow 错误写 `tracing::error!(error = %error, ...)`，通常只记录 Display 的最外层信息。

希望记录完整错误链可以使用：

```text
tracing::error!(error = %format!("{error:#}"), "任务失败");
```

也可以使用 `error = ?error` 记录 Debug 报告，但内容可能包含堆栈等更多信息。

这也意味着 `#[instrument(err)]` 默认按 Display 记录错误时，不应假定它一定展开 anyhow 的整个错误链。

### 9.2 避免重复记录

不建议每一层都“记录 ERROR 后再 ?”，否则同一失败可能出现多遍。一般由底层补充上下文，最了解任务结果的边界统一记录。

完整错误链也可能包含底层库携带的 URL、请求内容等信息；字段命名为 `error` 不会使其中的数据自动脱敏。

## 10. Tokio：handle.await?? 为什么有两个问号

当任务自身返回 `anyhow::Result<T>` 时，等待 JoinHandle 会再包一层 Result：

```text
Result<Result<T, anyhow::Error>, tokio::task::JoinError>
       └────── 业务结果 ──────┘  └──── 任务错误 ──────┘
```

第一层 `?` 处理任务取消或 panic 等 JoinError；第二层 `?` 处理业务返回的错误。

```rust
use anyhow::{Context, Result};

async fn work() -> Result<u32> {
    let count = "3".parse::<u32>().context("解析任务数量失败")?;
    Ok(count)
}

#[tokio::main]
async fn main() -> Result<()> {
    let handle = tokio::spawn(work());

    let count = handle.await??;
    println!("任务数量：{count}");
    Ok(())
}
```

如果想分别添加上下文，可以把同一处等待改为：

```text
let count = handle
    .await
    .context("工作任务被取消或发生 panic")?
    .context("工作任务业务处理失败")?;
```

`anyhow::Error` 支持 Send 和 Sync，适合在这些任务之间返回。但它不会自动使任务捕获的其他变量满足 Send：如果你跨 `.await` 持有 `Rc` 等非 Send 数据，仍然可能无法 `spawn`。

### 10.1 anyhow 不会捕获 panic

`?` 处理的是 Result 的错误返回，不是任意 panic。Tokio 在可以展开栈的配置下，通常会通过 JoinError 报告子任务 panic；panic-abort 配置可能直接终止进程。不要依靠 anyhow 把 panic 自动变成普通业务错误。

## 11. 错误类型判断：downcast_ref

anyhow 隐藏了函数签名里的具体错误类型，但不意味着原始错误对象已经被转成字符串。

必要时可以查看其具体类型：

```rust
use anyhow::{Context, Result};
use std::num::ParseIntError;

fn parse_limit(text: &str) -> Result<u32> {
    text.parse::<u32>().context("并发限制解析失败")
}

fn main() {
    if let Err(error) = parse_limit("wrong") {
        if let Some(parse_error) = error.downcast_ref::<ParseIntError>() {
            println!("识别到整数解析错误：{parse_error}");
        }
        println!("完整错误：{error:#}");
    }
}
```

anyhow 自己添加的 Context 支持保留这种向下转换能力。不过不要假设任意第三方包装层都能被 `downcast_ref` 自动递归穿透；需要时应检查 `chain()` 中的具体来源。

不要通过 `error.to_string().contains("timeout")` 决定是否重试。错误文案可能变化，也可能在不同语言环境下变化。优先判断具体错误类型、错误码或 SDK 提供的方法。

如果调用方经常需要稳定区分“鉴权失败、限流、参数错误”，这提示你应该使用明确的业务错误枚举，而不只是返回 anyhow。

## 12. anyhow、thiserror 和 Box<dyn Error> 怎么选

| 方案 | 适用场景 | 特点 |
| --- | --- | --- |
| `anyhow::Result<T>` | 应用入口、脚本、任务编排 | 方便统一传播，添加上下文 |
| 明确的错误枚举，常配合 `thiserror` | 公共库、需要匹配错误分支的业务接口 | 对调用方保留稳定的错误契约 |
| `Box<dyn Error + Send + Sync>` | 需要基础的动态错误容器 | 标准库能力，不自带 anyhow 的上下文工具 |
| `Result<T, String>` | 简单临时接口 | 易丢失类型和 source 链 |

`thiserror` 帮助你为自定义错误实现标准错误相关 trait，并不是 anyhow 的完全替代品。常见组合是：底层返回明确的错误枚举，上层 Agent 编排使用 anyhow 补充上下文。

anyhow 更接近支持上下文和诊断信息的动态错误容器，不应把它简单理解为给 String 换了个名字。

## 13. 结合当前项目的完整示例

下面不访问模型、不读取真实 `.env`，仅模拟 Agent 参数校验、工具执行、超时和错误记录。可直接在当前依赖下运行。

```rust
use anyhow::{bail, ensure, Context, Result};
use std::process::ExitCode;
use tokio::time::{sleep, timeout, Duration};
use tracing::{error, info, instrument, Level};
use tracing_subscriber::FmtSubscriber;

#[instrument]
async fn run_tool(task_id: u64) -> Result<String> {
    ensure!(task_id > 0, "task_id 必须大于 0");
    sleep(Duration::from_millis(10)).await;

    if task_id == 2 {
        bail!("模拟工具参数错误");
    }

    Ok(format!("任务 {task_id} 已完成"))
}

async fn run_agent(task_id: u64) -> Result<()> {
    let response = timeout(Duration::from_millis(100), run_tool(task_id))
        .await
        .context("工具执行超过 100ms")?
        .with_context(|| format!("Agent 调用工具失败，task_id={task_id}"))?;

    info!(task_id, response_bytes = response.len(), "Agent 任务成功");
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_ansi(false)
        .finish();

    if let Err(error) = tracing::subscriber::set_global_default(subscriber) {
        // 日志初始化失败时，不能假定 tracing 已经可用。
        eprintln!("初始化日志失败：{error}");
        return ExitCode::FAILURE;
    }

    match run_agent(2).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error!(error = %format!("{error:#}"), "Agent 运行失败");
            ExitCode::FAILURE
        }
    }
}
```

本例故意使用任务 2，因此预期记录如下错误链，并以非零状态退出：

```text
Agent 调用工具失败，task_id=2: 模拟工具参数错误
```

把 `run_agent(2)` 改为 `run_agent(1)`，应成功；改为 `run_agent(0)`，应得到参数校验失败。

注意两层错误：`timeout` 外层表示超时，内部 Result 表示工具业务错误，因此分别添加了上下文。

为什么这里没有让 `main` 返回 anyhow::Result？因为示例选择自己记录错误并控制退出码，避免记录完整错误后再返回 Err，让运行时终止输出重复报告。小程序直接使用 `main() -> anyhow::Result<()>` 也完全合理，标准终止处理会报告错误并返回失败状态。

## 14. 如何给当前入口补上下文

对于当前项目入口，可以考虑下面的写法。本例保留“`.env` 加载失败就停止启动”的现有语义：

```rust
use anyhow::{Context, Result};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().context("加载 .env 环境配置失败")?;

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .context("安装全局日志订阅器失败")?;

    info!("Agent 初始化完成");
    Ok(())
}
```

变化只有两个方面：导入 `Context`，并给容易缺少业务含义的错误补充说明。

教程不需要实际读取你的 `.env`。示例若在没有 `.env` 的目录运行，按设计会失败；使用时应先确定部署环境是否要求该文件存在。

## 15. 测试与常见问题

### 15.1 测试关键上下文与底层类型

下面的测试验证包装上下文后，底层整数解析错误仍可以被识别：

```rust
use anyhow::{Context, Result};

fn parse_limit(text: &str) -> Result<usize> {
    text.parse::<usize>().context("解析并发上限失败")
}

#[test]
fn preserves_parse_error_and_context() {
    let error = parse_limit("invalid").unwrap_err();

    assert_eq!(error.to_string(), "解析并发上限失败");
    assert!(error.downcast_ref::<std::num::ParseIntError>().is_some());
    assert!(error.chain().count() >= 2);
}
```

保存为 `tests/anyhow_basics.rs` 后运行：

```powershell
cargo test --test anyhow_basics
```

测试应关注你控制的业务语义和错误类别，避免把第三方底层错误的完整文案固定为稳定契约。

### 15.2 速查表

| 问题 | 原因或处理方向 |
| --- | --- |
| 找不到 `.context()` | 检查是否导入 `anyhow::Context`，来源类型是否满足约束 |
| Result 函数中不能对 Option 用 `?` | 先通过 `.context(...)` 或 `ok_or_else` 转为 Result |
| `String` 错误不能直接 `?` | 适配旧接口时显式 `map_err` 构造 anyhow 错误 |
| 只显示“Agent 启动失败” | `{error}` 通常只显示最外层，改用 `{error:#}` 查看链 |
| `.await??` 不知道处理什么 | 分别检查任务错误或超时错误与业务错误两层 Result |
| 写了 anyhow 仍然 panic | anyhow 不自动捕获 panic，检查 unwrap/expect/assert |
| 错误没有出现在日志里 | anyhow 只传播错误，需要边界记录或让 main 返回 Err |
| `error!` 后进程仍成功退出 | 日志宏不改变退出状态，应明确返回错误或失败退出码 |
| `.env` 不存在就退出 | 当前使用 `dotenvy::dotenv()?`，这是预期的错误传播 |
| 同一错误重复输出 | 多层日志、instrument(err)、main 返回 Err 可能叠加 |
| 无法区分业务错误类别 | 考虑明确的错误枚举，避免解析错误字符串 |

## 16. 学习顺序与官方文档

建议按这个顺序练习：

1. 对比标准 `Result<T, E>` 和 `anyhow::Result<T>`。
2. 修改第 4 节的输入，观察 `?` 如何使后续代码不再执行。
3. 运行第 5 节，对比 `{error}` 与 `{error:#}`。
4. 给 Option 使用 `context`，理解“缺失”到“错误”的转换。
5. 分别触发 `ensure!`、`bail!`，确认它们返回错误而不是 panic。
6. 运行第 13 节，切换任务 ID，观察错误链和退出状态。
7. 在项目实际读取配置和调用工具的位置补充具体上下文。

官方参考：

- [anyhow 文档](https://docs.rs/anyhow)
- [Context trait](https://docs.rs/anyhow/latest/anyhow/trait.Context.html)
- [Error 的格式化与类型判断](https://docs.rs/anyhow/latest/anyhow/struct.Error.html)
- [Rust 错误处理章节](https://doc.rust-lang.org/book/ch09-00-error-handling.html)

本地执行 `cargo doc --open -p anyhow`，可以查看当前项目实际依赖版本的 API 文档。

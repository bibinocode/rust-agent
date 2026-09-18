# Tokio 详细入门与实践指南

这份教程面向正在学习 Rust、准备开发 Agent 或网络服务的读者，结合本项目的 `src/main.rs` 讲解 Tokio。目标是让你理解代码为什么这样写，并能自己实现并发请求、超时、任务通信与优雅退出。

## 目录

1. [Tokio 是什么](#1-tokio-是什么)
2. [读懂当前 main.rs](#2-读懂当前-mainrs)
3. [async、Future 和 await](#3-asyncfuture-和-await)
4. [运行时与调度](#4-运行时与调度)
5. [顺序执行、join! 与 spawn](#5-顺序执行join-与-spawn)
6. [所有权、Send 与 static](#6-所有权send-与-static)
7. [任务错误与取消](#7-任务错误与取消)
8. [超时、select! 与取消安全](#8-超时select-与取消安全)
9. [Channel 与任务通信](#9-channel-与任务通信)
10. [共享状态与锁](#10-共享状态与锁)
11. [阻塞操作与 CPU 密集任务](#11-阻塞操作与-cpu-密集任务)
12. [异步文件、网络与定时器](#12-异步文件网络与定时器)
13. [Agent 风格完整示例](#13-agent-风格完整示例)
14. [日志与异步测试](#14-日志与异步测试)
15. [常见问题速查](#15-常见问题速查)
16. [练习路线与官方资料](#16-练习路线与官方资料)

## 1. Tokio 是什么

Tokio 是 Rust 的异步运行时及配套工具库，适合处理大量需要等待的工作，例如 HTTP 请求、数据库访问、TCP 通信、定时任务和任务之间的消息传递。

Rust 语言提供 `async` / `.await` 语法，但这些语法本身不会创建调度器，也不会自动提供网络事件循环。Tokio 负责驱动异步任务，在任务等待 I/O 或时间到达时，让线程有机会处理其他任务。

可以这样分工：

| 组成 | 负责什么 |
| --- | --- |
| `async fn` / `async {}` | 描述可以暂停、恢复的计算 |
| `Future` | 表示一个需要被轮询才能推进的异步计算 |
| `.await` | 等待 Future 完成；未就绪时允许当前任务暂停 |
| Tokio runtime | 轮询任务，调度任务，提供 I/O 和时间驱动 |
| Tokio 工具 | 任务、定时器、通道、异步锁等 |

### 1.1 并发不等于并行

- **并发**：多个工作在时间上交错推进。例如等待请求 A 的响应时处理请求 B。
- **并行**：多个工作在不同 CPU 核心上同时执行。

单线程 Tokio 也可以处理并发 I/O。多线程 Tokio 可以让不同任务并行执行，但不会让同一个任务内部的所有表达式自动并行。

Tokio 主要提高等待期间的资源利用率。压缩、图像处理、大量哈希等 CPU 密集计算不会因为加了 `async` 就变快。

## 2. 读懂当前 main.rs

项目已经配置：

```toml
[dependencies]
tokio = { version = "1.53.1", features = ["full"] }
```

`full` 开启 Tokio 的主要稳定功能，适合入门阶段。它不代表所有可选功能，例如后面测试时间控制使用的 `test-util` 仍需单独开启。Cargo 中 `"1.53.1"` 是兼容版本要求，实际解析的版本由 `Cargo.lock` 记录。

项目当前入口是：

```rust
#[tokio::main]
async fn main() -> Result<(), String> {
    Ok(())
}
```

逐行解释：

1. `#[tokio::main]` 是过程宏，负责构建 Tokio runtime，并在 runtime 中运行你的异步入口。
2. `async fn main()` 允许函数体使用 `.await`。
3. `Result<(), String>` 表示成功时不返回业务数据，失败时返回字符串错误。
4. `Ok(())` 是最后一个表达式，也是函数返回值；如果加分号就会丢弃这个值。

`Some` 是 `Option` 的枚举变体，用于构造 `Some(value)`，不是类型。这里既不需要 `Some<Result<...>>`，也不需要给 `Result` 再包一层 `Option`。

这个宏在概念上相当于下面的同步入口，注意这里只是便于理解的近似展开：

```rust
fn main() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;

    runtime.block_on(async { Ok(()) })
}
```

一般应用直接使用宏即可，不必手动管理 runtime。不要在已有异步任务中再随意创建 runtime 并调用 `block_on`，这可能触发嵌套运行时相关的 panic。

### 2.1 如何运行本教程示例

后文标为 `rust` 的代码块都是独立的可编译示例；带 `main` 的代码可以单独放入 `examples/tokio_demo.rs`，然后运行：

```powershell
cargo run --example tokio_demo
```

一次只使用一个示例，不要把多个 `main` 拼到同一个文件里。这样可以保留当前 `src/main.rs`。最后的测试示例则放入 `tests/tokio_basics.rs`。

## 3. async、Future 和 await

调用 `async fn` 会生成 Future，函数体此时通常尚未执行。这个 Future 需要被 `.await`，或交给运行时驱动，才会向前推进。

```rust
use tokio::time::{sleep, Duration};

async fn fetch_name() -> String {
    println!("开始模拟请求");
    sleep(Duration::from_millis(100)).await;
    "Rust Agent".to_owned()
}

#[tokio::main]
async fn main() {
    let future = fetch_name();
    println!("已创建 Future，fetch_name 的函数体还没有执行");

    let name = future.await;
    println!("结果：{name}");
}
```

预期顺序：先打印“已创建 Future”，然后打印“开始模拟请求”，最后输出结果。

### 3.1 await 在等待什么

可以把 Future 想象为状态机。执行器轮询它时，可能得到：

- `Poll::Ready(value)`：已经完成，返回结果。
- `Poll::Pending`：暂时不能完成，通过唤醒机制在以后安排再次轮询。

当被等待的 Future 返回 `Pending`，当前任务可以暂停，线程可以去处理其他任务。若 Future 已经就绪，`.await` 可以直接继续执行，**不是每遇到 `.await` 都一定切换任务**。

### 3.2 异步等待与阻塞等待

| 写法 | 效果 |
| --- | --- |
| `tokio::time::sleep(duration).await` | 等待定时器，不占着工作线程睡眠 |
| `std::thread::sleep(duration)` | 当前操作系统线程阻塞 |
| 异步网络读取 `.await` | 数据未到时允许任务暂停 |
| 很长的纯计算循环 | 持续占用线程，直到循环结束或主动让出 |

不要在异步任务中直接使用长时间的同步阻塞操作。任务调度通常依靠任务合作；运行时不会像操作系统调度线程那样随时抢占任意 Rust 代码。

## 4. 运行时与调度

默认的 `#[tokio::main]` 使用多线程 runtime；本项目的 `full` 已包含需要的功能。

| 模式 | 配置 | 适用场景 |
| --- | --- | --- |
| 多线程 | `#[tokio::main]` | 通用网络应用、并发 Agent |
| 当前线程 | `#[tokio::main(flavor = "current_thread")]` | 单线程环境、理解并发、部分测试 |
| 指定工作线程数 | `#[tokio::main(worker_threads = 2)]` | 需要明确控制资源时 |

当前线程模式依然支持 `tokio::spawn`，但它不会让任务在多个工作线程上并行。

`#[tokio::main]` 驱动的入口 Future 由 `block_on` 执行，不应简单认为入口本身就是普通的工作线程任务。需要独立调度的工作可以通过 `spawn` 提交。

对应用而言，最重要的习惯是：在入口创建 runtime，在内部使用异步函数组织逻辑，避免每个业务函数各自创建 runtime。

## 5. 顺序执行、join! 与 spawn

下面三种写法解决不同问题。

### 5.1 连续 await 是顺序执行

```rust
use tokio::time::{sleep, Duration, Instant};

async fn work(name: &'static str) -> &'static str {
    sleep(Duration::from_millis(100)).await;
    name
}

#[tokio::main]
async fn main() {
    let start = Instant::now();
    let a = work("A").await;
    let b = work("B").await;
    println!("顺序：{a}, {b}，耗时 {:?}", start.elapsed());

    let start = Instant::now();
    let (a, b) = tokio::join!(work("A"), work("B"));
    println!("并发：{a}, {b}，耗时 {:?}", start.elapsed());
}
```

通常顺序部分约需 200ms，并发部分约需 100ms，具体取决于调度和系统负载，不应断言毫秒数精确相等。

`join!` 在**同一个任务**中轮询多个 Future，直到全部完成，不会为每个分支创建任务。如果一个分支进行阻塞调用或长时间计算，其他分支也会受到影响。

当多个分支返回 `Result` 时，可以使用 `try_join!`：全部成功则返回结果元组；某个分支返回错误时提前返回，并丢弃其他尚未完成的分支 Future。丢弃是否安全，需要结合第 8 节判断。

### 5.2 spawn 创建独立任务

```rust
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let first = tokio::spawn(async {
        sleep(Duration::from_millis(100)).await;
        10
    });

    let second = tokio::spawn(async {
        sleep(Duration::from_millis(100)).await;
        20
    });

    // 两个任务已经提交。这里依次等待 handle，并不把任务变成顺序执行。
    let a = first.await?;
    let b = second.await?;
    println!("总和：{}", a + b);
    Ok(())
}
```

`spawn` 返回 `JoinHandle<T>`，它用于等待任务结果。任务提交后可以由 runtime 调度，不需要先 `.await` 这个 handle 才开始。

| 需求 | 优先考虑 |
| --- | --- |
| 后一步依赖前一步的结果 | 顺序 `.await` |
| 同一作用域内等待少量独立操作 | `join!` / `try_join!` |
| 需要独立调度、后台任务 | `spawn` |
| 动态数量的任务，按完成顺序收集 | `JoinSet` |

`spawn` 不是“越多越好”。大量任务会消耗内存，还可能挤爆远端服务，需要明确并发上限。

## 6. 所有权、Send 与 static

`tokio::spawn` 的 Future 通常需要满足 `Send + 'static`，它的返回值也有对应约束。

- **`Send`**：值可以安全地跨线程转移；多线程调度时，任务可能在不同线程上继续执行。
- **`'static`**：任务不能依赖随时可能失效的外部借用；不表示任务必须运行到程序结束。

即使使用当前线程 runtime，`tokio::spawn` 仍然要求 `Send`。

### 6.1 async move 转移捕获值的所有权

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let prompt = String::from("解释 Rust 所有权");

    let handle = tokio::spawn(async move {
        println!("收到任务：{prompt}");
        prompt.len()
    });

    println!("字符串 UTF-8 字节数：{}", handle.await?);
    Ok(())
}
```

这里 `String` 被移动到任务中，因此外层之后不能继续使用原来的 `prompt`。需要同时使用时，可以先 `clone()`；多任务共享较大对象时可以考虑 `Arc<T>`。

注意：`async move` 只会移动捕获的值。假如捕获的是 `&String`，移动进去的仍然是引用，它不会自动变成拥有所有权的 `String`。

### 6.2 future cannot be sent between threads safely

看到这个错误，重点检查**跨越 `.await` 仍然存活的变量**。例如 `Rc<T>` 通常不是 `Send`，`std::sync::MutexGuard` 也不适合跨 `.await` 保留。

常见处理方式：

1. 缩小变量作用域，让它在 `.await` 前释放。
2. 需要跨线程共享所有权时，使用满足条件的 `Arc<T>` 替代 `Rc<T>`。
3. 必须使用非 `Send` Future 时，学习 `LocalSet` 和 `spawn_local`。仅切换成单线程 runtime 并不能解除 `spawn` 的约束。

`Arc` 只提供线程安全的引用计数，不会自动让任意内部对象变得线程安全。

## 7. 任务错误与取消

异步代码中要区分“任务执行出了问题”和“业务返回了错误”。

```rust
type TaskError = Box<dyn std::error::Error + Send + Sync>;

async fn request() -> Result<String, TaskError> {
    Ok("模拟模型响应".to_owned())
}

#[tokio::main]
async fn main() -> Result<(), TaskError> {
    let handle = tokio::spawn(request());

    // 第一层 ? 处理 JoinError，第二层 ? 处理 request 的业务错误。
    let response = handle.await??;
    println!("{response}");
    Ok(())
}
```

这里 `handle.await` 的类型是：

```text
Result<Result<String, TaskError>, tokio::task::JoinError>
       └──────── 业务结果 ────────┘  └── 任务错误 ──┘
```

任务 panic 或被取消时，通常通过 `JoinError` 体现；对于按 abort 策略编译的 panic，进程可能直接终止。业务失败则由内部 `Result` 表达。

入门可以用 `String` 表示错误，但实际项目更适合保留原始错误类型。跨任务返回动态错误时，经常需要 `Box<dyn Error + Send + Sync>`，而不只是 `Box<dyn Error>`。

### 7.1 丢弃 JoinHandle 不等于停止任务

- 丢弃普通 `JoinHandle`：任务被分离，通常仍继续运行。
- 调用 `handle.abort()`：请求取消异步任务，不保证调用返回时任务已完成清理。
- 再 `handle.await`：观察取消或完成结果。存在竞争时，任务可能已正常结束。
- runtime 关闭：未完成的异步任务不保证执行完毕。

取消通常要等执行器重新取得任务控制权，不能立即中断没有让出执行权的死循环。`spawn_blocking` 一旦开始执行，也不能靠 `abort()` 强行停止。

`JoinSet` 与普通 handle 不同：丢弃 `JoinSet` 会取消它管理的异步任务。需要可靠退出时应明确等待、协作通知或取消并收集任务，而不是仅依赖作用域结束。

## 8. 超时、select! 与取消安全

### 8.1 给操作添加 timeout

```rust
use tokio::time::{sleep, timeout, Duration};

#[tokio::main]
async fn main() {
    let result = timeout(Duration::from_millis(50), async {
        sleep(Duration::from_millis(200)).await;
        "请求成功"
    })
    .await;

    match result {
        Ok(value) => println!("{value}"),
        Err(error) => println!("请求超时：{error}"),
    }
}
```

若被包装的 Future 自身返回 `Result<T, E>`，最终会得到 `Result<Result<T, E>, Elapsed>`，同样要处理两层结果。

`timeout` 不是操作系统级的强制中断。如果 Future 长时间不让出执行权，超时不能按时生效，甚至可能在超过期限后返回成功。

特别注意：`timeout(duration, handle)` 超时后会丢弃这个 `JoinHandle`，但对应的独立任务仍可能继续运行。若要在超时后取消任务，应保留 handle，将 `&mut handle` 交给 `timeout`，超时后调用 `abort()`，再等待 handle。

### 8.2 select! 等待多个事件

```rust
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    println!("等待 3 秒，或按 Ctrl+C 退出");

    tokio::select! {
        _ = sleep(Duration::from_secs(3)) => {
            println!("计时结束");
        }
        result = tokio::signal::ctrl_c() => {
            result?;
            println!("收到退出信号");
        }
    }

    Ok(())
}
```

`select!` 并发轮询多个分支，在某个分支完成并匹配其模式后执行对应分支体，取消其余尚未完成的分支 Future。像 `join!` 一样，它不自动创建独立任务。

默认情况下，它通过随机选择起始检查位置提供一定公平性。`biased;` 可以指定按源码顺序检查，但防止后面分支饥饿就成为调用方的责任。

### 8.3 取消安全为什么重要

“丢弃 Future”意味着停止继续轮询，不意味着业务副作用自动回滚。例如：

- HTTP 请求已经发送，客户端超时后远端仍可能继续执行。
- 一段文件或网络数据可能已经部分写入。
- `read_exact` 等操作被取消时，可能已消耗部分输入；重新调用未必能从原始位置开始。

Tokio 的 `mpsc::Receiver::recv` 是适合放入 `select!` 循环的取消安全操作之一。但不要由此推断所有异步方法都安全，使用前查看对应 API 的 **Cancellation safety** 说明。

`select!` 中取消 `Sender::send(value)` 不会把消息发出去，但这个被移动进去的值可能被丢弃；需要确保业务数据不丢失时，可以研究 `reserve()` 和 permit。

对于支付、写数据库、触发外部工具等副作用操作，超时后的重试还需要幂等键或状态查询，不能仅凭本地超时认定远端没有执行。

## 9. Channel 与任务通信

相比让所有任务直接修改共享变量，通道可以明确数据和控制信号的流向。

| 类型 | 用途 | 需要注意 |
| --- | --- | --- |
| `mpsc` | 多个发送者，一个接收者；任务队列 | 有界通道可产生背压 |
| `oneshot` | 一次性请求响应 | 只传一个值，发送本身不需要 await |
| `broadcast` | 多订阅者接收每条消息 | 慢订阅者可能收到 `Lagged` 错误 |
| `watch` | 广播最新状态 | 中间更新可能被覆盖，适合配置和退出状态 |

### 9.1 有界任务队列

```rust
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (tx, mut rx) = mpsc::channel::<String>(2);

    let producer = tokio::spawn(async move {
        for number in 1..=3 {
            if tx.send(format!("任务 {number}")).await.is_err() {
                // 接收端已关闭，停止生产。
                break;
            }
        }
        // tx 离开作用域，发送端被关闭。
    });

    while let Some(message) = rx.recv().await {
        println!("处理：{message}");
    }

    producer.await?;
    Ok(())
}
```

容量为 2 表示通道内最多暂存 2 条消息；队列满时 `send().await` 等待空间，这就是背压。它避免生产者无限制地把消息积压在内存里。

所有 sender 都被丢弃、并且缓冲区消息被取完后，`recv().await` 返回 `None`。如果你保留了某个 sender 的 clone，接收循环可能会一直等待。

注意：队列容量不等于正在执行的任务数量。若消费者不断取出消息再无限 `spawn`，仍可能产生大量在途任务，需要单独限制执行并发。

## 10. 共享状态与锁

共享计数器、缓存或会话状态时，经常使用 `Arc<Mutex<T>>`。

- `Arc` 让多个任务共同拥有数据。
- `Mutex` 控制同一时刻谁可以访问数据。

### 10.1 短临界区可以使用标准库 Mutex

```rust
use std::sync::{Arc, Mutex};
use tokio::task::JoinSet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let counter = Arc::new(Mutex::new(0_u32));
    let mut tasks = JoinSet::new();

    for _ in 0..10 {
        let counter = Arc::clone(&counter);
        tasks.spawn(async move {
            {
                let mut value = counter.lock().expect("计数器锁中毒");
                *value += 1;
            } // guard 在这里释放，不跨 await。

            tokio::task::yield_now().await;
        });
    }

    while let Some(result) = tasks.join_next().await {
        result?;
    }

    assert_eq!(*counter.lock().expect("计数器锁中毒"), 10);
    Ok(())
}
```

这里持锁时间极短，且不跨 `.await`，使用标准库锁很合适。如果存在严重竞争或很长临界区，同步锁等待仍会阻塞工作线程，应调整设计。

### 10.2 什么时候使用 tokio::sync::Mutex

需要异步等待锁，或确实必须持锁跨 `.await` 时，可以使用 Tokio 的异步 Mutex：通过 `lock().await` 获取 guard。

但异步锁不代表不会死锁，也不代表高性能。若持有一个全局锁去等待网络响应，所有竞争者仍会排队，请求实际被串行化。

优先考虑这样的结构：

```text
短时间加锁读取输入 → 释放锁 → await 网络操作 → 再短时间加锁更新结果
```

如果解锁期间数据可能改变，更新时还需要版本检查等一致性设计。对于连接、会话等有状态资源，也可以由一个专属任务持有，通过消息与它交互。

## 11. 阻塞操作与 CPU 密集任务

### 11.1 使用 spawn_blocking 包装同步调用

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = tokio::task::spawn_blocking(|| {
        // 模拟没有异步接口的旧库；实际项目可替换成真实同步调用。
        std::thread::sleep(std::time::Duration::from_millis(50));
        String::from("同步库处理完成")
    })
    .await?;

    println!("{text}");
    Ok(())
}
```

`spawn_blocking` 把同步闭包交给专门处理阻塞工作的线程池，避免长时间占用异步工作线程。它接收普通闭包，不应在里面返回一个未被驱动的 async 块。

已开始执行的阻塞闭包不能通过 `abort` 强制取消。长时间操作需要自己设计取消标志、分块处理或使用底层库提供的超时机制。runtime 退出也可能等待阻塞任务结束。

### 11.2 CPU 密集计算如何处理

少量计算可以放在普通函数中；持续的大量计算可以使用有限并发的 `spawn_blocking`，或 Rayon 等计算线程池。阻塞线程池的上限通常较大，不要把它误当作按 CPU 核数自动限流的计算池。

在 `async fn` 中写一个持续计算十秒的循环，它仍然会占着线程十秒。`yield_now().await` 可以提供让出机会，但不能替代合理的计算资源管理，也不保证立刻调度某个指定任务。

## 12. 异步文件、网络与定时器

### 12.1 文件操作

常见 API 包括 `tokio::fs::read_to_string(path).await`、`tokio::fs::write(path, bytes).await` 和 `tokio::fs::File`。

异步接口可以避免应用任务直接阻塞在普通文件 I/O 上，但 Tokio 的普通文件系统操作主要借助阻塞线程池实现，不意味着所有文件访问都由内核原生异步完成。

### 12.2 网络操作

`tokio::net::TcpListener` 和 `TcpStream` 提供异步 TCP；扩展 trait `tokio::io::AsyncReadExt` / `AsyncWriteExt` 提供常用读写方法。

TCP 是字节流，一次读取不保证得到完整消息。实现协议时需要明确消息边界，例如长度字段或换行分隔，并处理 EOF、部分读写和超时。不要把一次 `read` 当作一个完整请求。

本项目使用的 `async-openai` 属于更高层的客户端库。调用异步 SDK 时，按具体 API 返回的 Future 使用 `.await`，一般无需自己实现 TCP 或 HTTP。

### 12.3 sleep 与 interval

- `sleep(duration)`：从当前等待点开始等待一段时间。
- `interval(duration)`：按周期产生 tick，适合心跳或定期刷新。

`interval` 的第一次 `tick().await` 通常立即完成。如果你希望第一次也延迟，可以使用 `interval_at` 指定起点。

处理速度赶不上周期时，要考虑 `MissedTickBehavior`：默认 `Burst` 可能补赶 tick；`Skip` 会跳过错过的周期；`Delay` 会调整后续调度。对于健康检查，通常不希望恢复后突然执行一大批过期检查。

如果在循环的每轮 `select!` 中重新创建 `sleep`，其他分支频繁获胜可能反复重置计时。要表示固定截止时间，应把定时器放在循环外并 pin，或显式使用 deadline。

## 13. Agent 风格完整示例

下面模拟一个 Agent 同时执行多个工具任务，不需要 API key 或网络访问。它演示：

1. 用 `JoinSet` 管理动态任务。
2. 最多保留 3 个尚未收集结果的任务，控制资源使用。
3. 给每个工具调用设置超时。
4. 按完成顺序收集结果，分别处理业务错误和任务错误。
5. 收到 Ctrl+C 后停止接收新任务，等待已经启动的任务结束。

将此代码保存为 `examples/tokio_agent.rs`，运行 `cargo run --example tokio_agent`。

```rust
use std::collections::VecDeque;
use tokio::task::JoinSet;
use tokio::time::{sleep, timeout, Duration};

async fn mock_tool(id: u32) -> Result<String, String> {
    // 任务 4 故意超过超时；任务 5 故意产生业务错误。
    let delay_ms = if id == 4 { 800 } else { 100 + u64::from(id) * 30 };
    sleep(Duration::from_millis(delay_ms)).await;

    if id == 5 {
        return Err("模拟工具参数错误".to_owned());
    }

    Ok(format!("工具 {id} 执行成功"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    const MAX_IN_FLIGHT: usize = 3;
    let mut pending: VecDeque<u32> = (1..=6).collect();
    let mut tasks = JoinSet::new();
    let mut stopping = false;

    // 只创建一次信号 Future，跨循环继续等待它。
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        while !stopping && tasks.len() < MAX_IN_FLIGHT {
            let Some(id) = pending.pop_front() else {
                break;
            };

            tasks.spawn(async move {
                let result = match timeout(Duration::from_millis(400), mock_tool(id)).await {
                    Ok(result) => result,
                    Err(_) => Err(format!("工具 {id} 超时")),
                };
                (id, result)
            });
        }

        if tasks.is_empty() {
            break;
        }

        tokio::select! {
            signal = &mut shutdown, if !stopping => {
                // 信号监听出错也停止接收新任务，并排空当前任务。
                if let Err(error) = signal {
                    eprintln!("退出信号监听失败：{error}");
                }
                stopping = true;
                println!("停止接收任务，等待已启动任务结束");
            }
            completed = tasks.join_next() => {
                match completed {
                    Some(Ok((id, Ok(message)))) => println!("[{id}] {message}"),
                    Some(Ok((id, Err(error)))) => eprintln!("[{id}] 业务失败：{error}"),
                    Some(Err(error)) => eprintln!("任务异常：{error}"),
                    None => break,
                }
            }
        }
    }

    println!("处理结束，未启动任务数：{}", pending.len());
    Ok(())
}
```

正常运行时，任务 4 超时，任务 5 报业务错误，其余成功；输出顺序不应该作为程序正确性的依据。业务失败在本例中被记录后继续处理，所以最终 `main` 仍返回成功。若需要整个批次失败，应累计失败状态并返回错误。

这里直接把工具 Future 放进 `timeout`，没有额外包一层 `spawn`，因此超时时会丢弃工具 Future。不过真实远端工具的副作用依然不保证被撤销。

停止后没有清空 `pending`，便于统计未启动工作。本例的等待时间由单任务超时约束；真实服务还应考虑退出总期限和任务持久化。

### 13.1 什么时候用 Semaphore

本例通过 `JoinSet::len()` 限制任务数量。当多个不同位置提交的任务要共享同一个并发预算，例如模型 API 全局最多 4 个请求时，可以使用 `Arc<Semaphore>`：

```text
获取 owned permit → 启动/执行请求 → 请求结束或任务取消 → permit 释放
```

如果先创建十万个任务，再让它们在任务内部等待 permit，请求并发虽然受限，但十万个任务依然占内存。要同时限制任务数量，可以在 `spawn` 前获取 permit，或使用本例的窗口调度。

Semaphore 限制的是同时执行数量，不是每秒请求次数。API 有每分钟请求或 token 配额时，还需要独立的速率控制和重试退避。

## 14. 日志与异步测试

### 14.1 用 tracing 记录任务上下文

本项目已经包含 `tracing` 和 `tracing-subscriber`，可以这样使用：

```rust
use tracing::{info, Instrument};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let handle = tokio::spawn(
        async {
            info!("开始处理");
            tokio::task::yield_now().await;
            info!("处理完成");
        }
        .instrument(tracing::info_span!("agent_task", task_id = 1)),
    );

    handle.await?;
    Ok(())
}
```

`instrument` 把 span 关联到异步 Future 的执行，避免把 `span.enter()` 的 guard 跨 `.await` 保留而导致日志上下文混乱。

初始化全局 subscriber 通常只做一次；测试中多次初始化可以使用 `try_init()`。模型调用日志建议记录请求 ID、耗时、状态和 token 用量，避免直接输出 API key 或完整敏感上下文。

### 14.2 使用 tokio::test

将下面代码保存到 `tests/tokio_basics.rs`，执行 `cargo test --test tokio_basics`：

```rust
#[tokio::test]
async fn channel_preserves_message() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    tx.send("hello").await.unwrap();
    drop(tx);

    assert_eq!(rx.recv().await, Some("hello"));
    assert_eq!(rx.recv().await, None);
}
```

`#[tokio::test]` 为测试创建 runtime，默认是当前线程模式。需要测试多线程行为时，可以配置 `flavor = "multi_thread"`。

对定时逻辑，不建议靠“现实时间恰好过去 100ms”断言。可以为 Tokio 额外启用 `test-util`，再使用 `#[tokio::test(start_paused = true)]` 和时间推进 API，测试更快、更稳定。

暂停 Tokio 时间不等于暂停真实世界：同步系统时间、网络、外部服务和阻塞线程中的 sleep 不会因此一起虚拟化。

## 15. 常见问题速查

| 问题或报错 | 可能原因 | 处理方向 |
| --- | --- | --- |
| 调用了异步函数却没反应 | Future 未被驱动 | `.await` 或提交到任务中 |
| 加了 async 仍然很慢 | 顺序等待，或同步阻塞/计算过重 | 判断依赖关系，使用并发或隔离阻塞工作 |
| `future cannot be sent ...` | 非 Send 值跨 await 保留 | 检查 guard、Rc 和动态错误类型 |
| 借用值生命周期不够长 | spawn 捕获局部借用 | 转移所有权、clone 或使用 Arc |
| 后台任务没执行完程序就退出 | main 结束，没有等待任务 | 保存 handle 或用 JoinSet 收集 |
| timeout 后工作仍在继续 | 超时包装的是 JoinHandle，或远端已执行 | 显式取消本地任务，查询远端状态 |
| 接收通道一直不结束 | 还有 sender 存活 | 检查 clone 的持有者并显式关闭 |
| `there is no reactor running` | 在 runtime 上下文外使用相关设施 | 把操作移入 Tokio 驱动的异步入口 |
| `Cannot start a runtime from within a runtime` | 在异步上下文嵌套 block_on | 沿调用链使用 await |
| 锁住后所有请求卡住 | 持锁跨慢操作、死锁或同步锁竞争 | 缩短临界区，检查锁顺序，考虑消息传递 |
| 内存持续上涨 | 无界队列或无限创建任务 | 有界通道、并发窗口、负载限制 |
| `tokio::spawn(async { ... })` 编译不过 | 捕获局部变量、错误类型缺少 Send 等 | 阅读首个报错对应的跨 await 变量 |

排查时先运行 `cargo check`。生命周期或 `Send` 报错通常不是 Tokio 的“特殊语法问题”，而是在指出任务可能活得更久或移动到其他线程，需要把数据所有权说明清楚。

## 16. 练习路线与官方资料

建议按下面顺序练习，每一步都先预测输出，再实际运行：

1. **基础**：运行第 3 节，理解创建 Future 与执行函数体的区别。
2. **并发**：运行第 5 节，比较顺序 await、join 和 spawn。
3. **所有权**：在第 6 节移除 `move`，观察编译器如何提示生命周期问题，再修复。
4. **通道**：把第 9 节容量改成 1，并给消费者增加 sleep，观察生产者何时等待。
5. **任务管理**：运行第 13 节，把并发上限改成 1、3、6，比较完成时间。
6. **退出**：在任务较长时按 Ctrl+C，确认不再启动新任务，而已启动任务被收集。
7. **真实 Agent**：将 `mock_tool` 替换成项目使用的异步 SDK 调用，保留超时、并发控制和错误分层；按当前 SDK 文档确认接口。

后续学习时优先阅读这些官方资料：

- [Tokio 官方教程](https://tokio.rs/tokio/tutorial)
- [Tokio API 文档](https://docs.rs/tokio)
- [任务与 spawn](https://tokio.rs/tokio/tutorial/spawning)
- [共享状态](https://tokio.rs/tokio/tutorial/shared-state)
- [通道](https://tokio.rs/tokio/tutorial/channels)
- [select 与取消](https://tokio.rs/tokio/tutorial/select)
- [优雅退出](https://tokio.rs/tokio/topics/shutdown)

在项目中运行 `cargo doc --open -p tokio`，可以查看当前实际依赖版本的 API 文档。优先核对方法的功能开关、返回类型、取消安全说明以及 panic 条件。

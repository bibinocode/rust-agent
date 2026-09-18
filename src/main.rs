// Agent 主函数

use std::io::{self, Write};

use futures::{StreamExt, pin_mut};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use rust_agent::constant::{API_BASE_URL, MODEL};
use rust_agent::llm::stream::{chat_stream, chat_stream_with_retry};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // cargo run：实时输出；cargo run -- --retry：重试并返回完整回复。
    let args: Vec<String> = std::env::args().skip(1).collect();
    let retry = match args.as_slice() {
        [] => false,
        [flag] if flag == "--retry" => true,
        _ => anyhow::bail!("用法：cargo run [-- --retry]"),
    };

    // 初始化环境变量
    dotenvy::dotenv()?;

    // 创建一个输出 INFO 及以上级别日志的订阅器，并将它安装为全局默认订阅器
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    tracing::info!("AI_BASE_URL: {:?}", API_BASE_URL.as_str());
    tracing::info!("AI_MODEL: {:?}", MODEL.as_str());

    let system = Some("你是一个全能助手");
    let prompt = "我要去美加墨世界杯观看比赛，如何安排？";

    if retry {
        tracing::info!("重试模式：等待完整回复");
        let text = chat_stream_with_retry(MODEL.as_str(), system, prompt).await?;
        println!("{text}");
    } else {
        tracing::info!("流式模式：实时输出回复");
        let stream = chat_stream(MODEL.as_str(), system, prompt).await;
        pin_mut!(stream);

        while let Some(chunk) = stream.next().await {
            print!("{}", chunk?);
            // 分片可能不含换行，主动刷新才能立即显示。
            io::stdout().flush()?;
        }
        println!();
    }
    Ok(())
}

// Agent 主函数

use tracing::{Level};
use tracing_subscriber::FmtSubscriber;

use rust_agent::llm::structured::chat_complete_structured;
use rust_agent::constant::{API_BASE_URL, MODEL};

#[tokio::main]
async fn main() -> anyhow::Result<()> {

    // 初始化环境变量
    dotenvy::dotenv()?;

    // 创建一个输出 INFO 及以上级别日志的订阅器，并将它安装为全局默认订阅器
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    tracing::info!("AI_BASE_URL: {:?}", API_BASE_URL.as_str());
    tracing::info!("AI_MODEL: {:?}", MODEL.as_str());


    let res = chat_complete_structured(&MODEL, Some("你是一个全能助手"), "我要去美加墨世界杯观看比赛，如何安排？").await?;

    tracing::info!("AI回答: {:#?}", res);

    Ok(())
}

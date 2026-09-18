// Agent 主函数

use tracing::{Level};
use tracing_subscriber::FmtSubscriber;

use crate::llm::complete::chat_complete;


mod llm;

#[tokio::main]
async fn main() -> anyhow::Result<()> {

    // 初始化环境变量
    dotenvy::dotenv()?;

    // 创建一个输出 INFO 及以上级别日志的订阅器，并将它安装为全局默认订阅器
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    let url = std::env::var("AI_BASE_URL")?;
    let model = std::env::var("AI_MODEL")?;
    tracing::info!("AI_BASE_URL: {:?}", url);


    let res = chat_complete(&model, Some("你是一个专业的地理问答机器人"), "爱尔兰的首都是哪里?").await?;

    tracing::info!("res: {:?}", res);

    Ok(())
}

use anyhow::{ Ok, Result, };
use async_openai::{
    Client,  config::OpenAIConfig, types::chat::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
};

/// 调用非流式 Chat Completions，返回第一条回复的文本。
#[warn(dead_code)]
pub  async fn chat_complete(model: &str, system: Option<&str>, prompt: &str) -> Result<String> {


    
    let api_base = std::env::var("AI_BASE_URL")?;
    let api_key = std::env::var("AI_API_KEY")?;
    tracing::info!("AI_BASE_URL: {:?}", api_base);

    let config: OpenAIConfig = OpenAIConfig::new().with_api_base(api_base).with_api_key(api_key);

    let client = Client::with_config(config);

    let mut messages = vec![];



    if let Some(system) = system {
        messages.push(
            ChatCompletionRequestSystemMessageArgs::default()
            .content(system)
            .build()?
            .into()
        );
    }


    messages.push(
        ChatCompletionRequestUserMessageArgs::default().content(prompt).build()?.into()
    );

    let req = CreateChatCompletionRequestArgs::default().model(model)
    .messages(messages).max_tokens(2048u32).build()?;

    let res = client.chat().create(req).await?;

    tracing::info!("res: {:#?}", res);

    // 拿到 第一条回复的文本
    let content = res.choices.into_iter().next().and_then(|c|c.message.content).ok_or_else(||anyhow::anyhow!("No content found"))?;
    
    Ok(content)
}

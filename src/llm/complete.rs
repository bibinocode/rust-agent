use anyhow::{ Ok, Result, };
use crate::constant::{API_BASE_URL, API_KEY, PROXY};
use async_openai::{
    Client,  config::OpenAIConfig, types::chat::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
};

/// 调用非流式 Chat Completions，返回第一条回复的文本。
#[warn(dead_code)]
pub  async fn chat_complete(model: &str, system: Option<&str>, prompt: &str) -> Result<String> {



    let config: OpenAIConfig = OpenAIConfig::new().with_api_base(API_BASE_URL.as_str()).with_api_key(API_KEY.as_str());

      let proxy = PROXY.as_str().trim();

    let http_client = if !proxy.is_empty() {
        reqwest::Client::builder()
            .proxy(reqwest::Proxy::all(proxy)?)
            .build()?
    } else {
        reqwest::Client::new()
    };

    let client = Client::with_config(config)
        .with_http_client(http_client);

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

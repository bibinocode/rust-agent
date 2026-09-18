use crate::constant::{API_BASE_URL, API_KEY, PROXY};
use anyhow::{Context, Result, anyhow};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
};
use async_stream::try_stream;
use backon::{ExponentialBuilder, Retryable};
use futures::{Stream, StreamExt};

pub async fn chat_stream(
    model: &str,
    system: Option<&str>,
    prompt: &str,
) -> impl Stream<Item = Result<String>> {
    try_stream! {

        let config: OpenAIConfig = OpenAIConfig::new().with_api_base(API_BASE_URL.as_str()).with_api_key(API_KEY.as_str());

          let proxy = PROXY.as_str().trim();

        let http_client = if !proxy.is_empty() {
            reqwest::Client::builder()
                .proxy(reqwest::Proxy::all(proxy).context("代理地址无效")?)
                .build().context("创建 HTTP 客户端失败")?
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
            .build().context("构建 system 消息失败")?
                .into()
            );
        }


        messages.push(
            ChatCompletionRequestUserMessageArgs::default()
                .content(prompt).build().context("构建 user 消息失败")?.into()
        );

        let req = CreateChatCompletionRequestArgs::default().model(model)
        .messages(messages).max_tokens(2048u32).build()
        .context("构建流式请求失败")?;


        let mut stream = client.chat().create_stream(req).await
            .context("创建 Chat Completions 流失败")?;

        // next() 返回 Option<Result<Chunk>>：先判断流结束，再处理分片错误。
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("读取 Chat Completions 分片失败")?;
            // 角色、统计信息等分片可能没有文本，不能当作错误。
            if let Some(choice) = chunk.choices.into_iter().find(|choice| choice.index == 0) {
                if choice.delta.refusal.is_some() {
                    Err(anyhow!("模型拒绝了本次请求"))?;
                }
                if let Some(text) = choice.delta.content {
                    if !text.is_empty() {
                        // try_stream! 自动包装成 Ok(text)。保留空格等原始文本。
                        yield text;
                    }
                }
            }
        }



    }
}

/// 收集完整的流式回复，失败时按指数退避最多重试 3 次（共最多 4 次尝试）。
///
/// 每次重试重新请求并丢弃前一次的部分文本，不是断点续传。
/// 此包装函数完成后才返回 String；需要实时显示时使用 chat_stream。
/// 当前对所有返回错误重试，包括不能通过重试恢复的参数或鉴权错误。
pub async fn chat_stream_with_retry(
    model: &str,
    system: Option<&str>,
    prompt: &str,
) -> Result<String> {
    let op = || async {
        // chat_stream 保留现有 async 接口，因此先 await 得到流。
        let stream = chat_stream(model, system, prompt).await;
        futures::pin_mut!(stream);

        // 放在单次尝试内部，避免失败后的重试拼接出重复内容。
        let mut output = String::new();
        while let Some(chunk) = stream.next().await {
            output.push_str(&chunk?);
        }
        Ok::<String, anyhow::Error>(output)
    };

    op.retry(ExponentialBuilder::default().with_max_times(3))
        .await
        .context("流式请求在重试后仍然失败")
}

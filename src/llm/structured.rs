use anyhow::{ Ok, Result, };
use crate::constant::{API_BASE_URL, API_KEY,PROXY};
use async_openai::{
    Client, config::OpenAIConfig, types::chat::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs, ResponseFormat, ResponseFormatJsonSchema,
    },
};
use reqwest;

use crate::models::action_plan::ActionPlan; 

/// 调用非流式 Chat Completions，返回第一条回复的结构化结果。
#[warn(dead_code)]
pub  async fn chat_complete_structured(model: &str, system: Option<&str>, prompt: &str) -> Result<ActionPlan> {

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
    
    // 序列化契约要求输出所有字段；Option 仍允许 null，但字段不能省略。
    let schema = action_plan_schema();
    // 转换为 JSON 值（不是 JSON 字符串）。
    let schema_json = schema.as_value().clone();
    // 给大模型 设置结构化输出的 schema 
    let format_setting = ResponseFormat::JsonSchema {
        json_schema: ResponseFormatJsonSchema{
            description:Some("A setu-by-setup agent action plan with diffifulty and time estimate".into()),
            name:"action_plan" .into(),
            schema:schema_json,
            strict:Some(true)
        }
      };


    let req = CreateChatCompletionRequestArgs::default().model(model)
    .messages(messages).response_format(format_setting).max_tokens(2048u32).build()?;

    let res = client.chat().create(req).await?;

    tracing::info!("res: {:#?}", res);

    // 拿到 第一条回复的文本
    let content = res
        .choices
        .into_iter()
        .next()
        .and_then(|c|c.message.content)
        .ok_or_else(||anyhow::anyhow!("No content found"))
        .and_then(|s|serde_json::from_str(&s).map_err(Into::into))?;
    
    Ok(content)
}

fn action_plan_schema() -> schemars::Schema {
    schemars::generate::SchemaSettings::default()
        .with(|settings| settings.contract = schemars::generate::Contract::Serialize)
        .into_generator()
        .into_root_schema_for::<ActionPlan>()
}

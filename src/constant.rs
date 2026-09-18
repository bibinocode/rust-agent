use std::sync::LazyLock;

// 利用 LazyLock 第一次访问时读取并缓存下来
pub static API_BASE_URL: LazyLock<String> = LazyLock::new(|| std::env::var("AI_BASE_URL").expect("未配置AI_BASE_URL"));
pub static API_KEY: LazyLock<String> = LazyLock::new(|| std::env::var("AI_API_KEY").expect("未配置AI_API_KEY"));
pub static MODEL: LazyLock<String> = LazyLock::new(|| std::env::var("AI_MODEL").expect("未配置AI_MODEL"));

// 允许未配置代理的情况
pub static PROXY: LazyLock<String> = LazyLock::new(|| std::env::var("AI_PROXY").unwrap_or_default());
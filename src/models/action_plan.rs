use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug,Serialize,Deserialize,JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionPlan {
    // 目标
    pub goal:String,
    // 动作设置
    pub setup:Vec<ActionSetup>,
    // 难度
    pub difficulty:Difficulty,
    // 估计耗时（分钟）
    pub estimated_minutes:u32
}


#[derive(Debug,Serialize,Deserialize,JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct  ActionSetup {
    // 动作索引
    pub index:u8,
    // 动作描述
    pub description:String,
    // 工具提示
    pub tool_hint:Option<String>
}

#[derive(Debug,Serialize,Deserialize,JsonSchema)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

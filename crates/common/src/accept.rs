use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtraArgs {
    pub context: Context,
    pub meta: Meta,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Context {
    pub values: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Meta {
    pub values: MetaValue,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MetaValue {}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceArguments {
    pub extra_args: ExtraArgs,
}

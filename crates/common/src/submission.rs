use crate::{accept, transfer, transfer_factory};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ExerciseCommandData {
    #[serde(rename = "templateId")]
    pub template_id: String,
    #[serde(rename = "contractId")]
    pub contract_id: String,
    pub choice: String,
    #[serde(rename = "choiceArgument")]
    pub choice_argument: ChoiceArgumentsVariations,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum ChoiceArgumentsVariations {
    TransferFactory(transfer_factory::ChoiceArguments),
    Accept(accept::ChoiceArguments),
    Generic(serde_json::Value),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExerciseCommand {
    #[serde(rename = "ExerciseCommand")]
    pub exercise_command: ExerciseCommandData,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum Command {
    ExerciseCommand(ExerciseCommand),
}

#[derive(Serialize, Deserialize, Default)]
pub struct Submission {
    #[serde(rename = "actAs")]
    pub act_as: Vec<String>,
    #[serde(rename = "readAs", default, skip_serializing_if = "Option::is_none")]
    pub read_as: Option<Vec<String>>,
    #[serde(rename = "commandId")]
    pub command_id: String,
    #[serde(rename = "disclosedContracts")]
    pub disclosed_contracts: Vec<transfer::DisclosedContract>,
    pub commands: Vec<Command>,
}

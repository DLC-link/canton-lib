use canton_api_client::models;
use crate::{accept, filters, transfer, transfer_factory};
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
    // pub transaction_format: Box<models::TransactionFormat>,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransactionFormat {
    #[serde(rename = "eventFormat", skip_serializing_if = "Option::is_none")]
    pub event_format: Option<EventFormat>,
    #[serde(rename = "transactionShape")]
    pub transaction_shape: Option<TransactionShape>,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventFormat {
    #[serde(rename = "filtersByParty")]
    pub filters_by_party: std::collections::HashMap<String, filters::Filters>,
    #[serde(rename = "filtersForAnyParty", skip_serializing_if = "Option::is_none")]
    pub filters_for_any_party: Option<filters::Filters>,
    /// If enabled, values served over the API will contain more information than strictly necessary to interpret the data. In particular, setting the verbose flag to true triggers the ledger to include labels for record fields. Optional
    #[serde(rename = "verbose")]
    pub verbose: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TransactionShape {
    TransactionShapeAcsDelta(serde_json::Value),
    TransactionShapeLedgerEffects(serde_json::Value),
    TransactionShapeUnspecified(serde_json::Value),
    Unrecognized(Box<models::Unrecognized>),
}

impl Default for TransactionShape {
    fn default() -> Self {
        Self::TransactionShapeAcsDelta(Default::default())
    }
}


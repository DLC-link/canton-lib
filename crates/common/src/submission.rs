use crate::{accept, filters, transfer, transfer_factory};
use canton_api_client::models;
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
// `transfer_factory::ChoiceArguments` is ~10x larger than the other variants,
// so every enum value pays the larger size. Boxing it would be the clippy-
// recommended fix, but it's a breaking API change for downstream callers that
// construct `TransferFactory(args)` (cbtc-lib has several). Suppress the lint
// here and revisit when those callers can be updated in lock-step.
#[allow(clippy::large_enum_variant)]
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
    #[serde(rename = "submissionId", skip_serializing_if = "Option::is_none")]
    pub submission_id: Option<String>,
    #[serde(rename = "workflowId", skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    #[serde(rename = "domainId", skip_serializing_if = "Option::is_none")]
    pub domain_id: Option<String>,
    #[serde(rename = "userId", skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(rename = "deduplicationPeriod", skip_serializing_if = "Option::is_none")]
    pub deduplication_period: Option<DeduplicationPeriod>,
    #[serde(rename = "disclosedContracts")]
    pub disclosed_contracts: Vec<transfer::DisclosedContract>,
    pub commands: Vec<Command>,
    #[serde(rename = "transactionFormat", skip_serializing_if = "Option::is_none")]
    pub transaction_format: Option<TransactionFormat>,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransactionFormat {
    #[serde(rename = "eventFormat", skip_serializing_if = "Option::is_none")]
    pub event_format: Option<EventFormat>,
    #[serde(rename = "transactionShape")]
    pub transaction_shape: Option<String>,
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
pub enum DeduplicationPeriod {
    DeduplicationPeriodOneOf(models::DeduplicationPeriodOneOf),
    DeduplicationPeriodOneOf1(models::DeduplicationPeriodOneOf1),
    DeduplicationPeriodOneOf2(models::DeduplicationPeriodOneOf2),
}

impl Default for DeduplicationPeriod {
    fn default() -> Self {
        Self::DeduplicationPeriodOneOf(Default::default())
    }
}

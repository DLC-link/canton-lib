use crate::{accept, allocation_factory, filters, transfer, transfer_factory};
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
// The factory variants (`TransferFactory`/`AllocationFactory`) are much larger
// than the others, so every enum value pays the larger size. Boxing would be the
// clippy-recommended fix, but it's a breaking API change for downstream callers
// that construct these variants (cbtc-lib has several). Suppress the lint here and
// revisit when those callers can be updated in lock-step.
//
// Variant order is load-bearing for untagged deserialization. Serde tries each
// variant in declaration order and takes the first that fits; unknown fields do
// not stop a match. So each variant must precede every variant it could be
// mistaken for:
//   - `AllocationFactory` precedes `Accept`, which only requires `extraArgs`.
//   - `TransferFactoryV2` precedes `AcceptV2`, because a V2 factory payload also
//     satisfies the V2 instruction shape: it carries `actors` and `extraArgs`.
//   - Both V2 variants precede `Accept`, for the same reason as `AllocationFactory`.
// The two V1 factory variants are safe in front of the V2 ones, because both
// require `expectedAdmin` and no V2 payload carries it.
#[allow(clippy::large_enum_variant)]
pub enum ChoiceArgumentsVariations {
    TransferFactory(transfer_factory::ChoiceArguments),
    AllocationFactory(allocation_factory::ChoiceArguments),
    TransferFactoryV2(transfer_factory::v2::ChoiceArguments),
    AcceptV2(accept::v2::ChoiceArguments),
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
    #[serde(
        rename = "deduplicationPeriod",
        skip_serializing_if = "Option::is_none"
    )]
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn empty_extra_args() -> serde_json::Value {
        json!({ "context": { "values": {} }, "meta": { "values": {} } })
    }

    fn v1_transfer() -> serde_json::Value {
        json!({
            "sender": "alice::1220ab",
            "receiver": "bob::1220cd",
            "amount": "1.0",
            "instrumentId": { "admin": "admin::1220ef", "id": "CBTC" },
            "requestedAt": "2026-09-01T00:00:00Z",
            "executeBefore": "2026-09-08T00:00:00Z",
            "inputHoldingCids": ["00abc"],
            "meta": { "values": {} }
        })
    }

    fn v2_transfer() -> serde_json::Value {
        json!({
            "sender": { "owner": "alice::1220ab", "provider": null, "id": "" },
            "receiver": { "owner": "bob::1220cd", "provider": null, "id": "" },
            "amount": "1.0",
            "instrumentId": { "admin": "admin::1220ef", "id": "CBTC" },
            "requestedAt": "2026-09-01T00:00:00Z",
            "executeBefore": "2026-09-08T00:00:00Z",
            "inputHoldingCids": ["00abc"],
            "meta": { "values": {} }
        })
    }

    #[test]
    fn v2_factory_payload_deserializes_as_transfer_factory_v2() {
        let payload = json!({
            "transfer": v2_transfer(),
            "actors": ["alice::1220ab"],
            "extraArgs": empty_extra_args()
        });

        let parsed: ChoiceArgumentsVariations = serde_json::from_value(payload).unwrap();
        assert!(
            matches!(parsed, ChoiceArgumentsVariations::TransferFactoryV2(_)),
            "a V2 factory payload must not be swallowed by AcceptV2 or Accept"
        );
    }

    #[test]
    fn v2_instruction_payload_deserializes_as_accept_v2() {
        let payload = json!({
            "actors": ["bob::1220cd"],
            "extraArgs": empty_extra_args()
        });

        let parsed: ChoiceArgumentsVariations = serde_json::from_value(payload).unwrap();
        assert!(
            matches!(parsed, ChoiceArgumentsVariations::AcceptV2(_)),
            "a V2 instruction payload must not be swallowed by Accept"
        );
    }

    #[test]
    fn v1_factory_payload_still_deserializes_as_transfer_factory() {
        let payload = json!({
            "expectedAdmin": "admin::1220ef",
            "transfer": v1_transfer(),
            "extraArgs": empty_extra_args()
        });

        let parsed: ChoiceArgumentsVariations = serde_json::from_value(payload).unwrap();
        assert!(matches!(
            parsed,
            ChoiceArgumentsVariations::TransferFactory(_)
        ));
    }

    #[test]
    fn v1_instruction_payload_still_deserializes_as_accept() {
        let payload = json!({ "extraArgs": empty_extra_args() });

        let parsed: ChoiceArgumentsVariations = serde_json::from_value(payload).unwrap();
        assert!(matches!(parsed, ChoiceArgumentsVariations::Accept(_)));
    }
}

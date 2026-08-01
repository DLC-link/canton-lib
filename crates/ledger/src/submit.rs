use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Serialize;

use common::{
    consts::TRANSACTION_SHAPE_LEDGER_EFFECTS,
    submission::{EventFormat, Submission, TransactionFormat},
};

pub struct Params {
    pub ledger_host: String,
    pub access_token: String,
    pub request: Submission,
}

#[derive(Serialize)]
struct SubmitAndWaitRequest<'a> {
    // Outer `commands` field of the JSON Ledger API request body — wraps an
    // entire `Submission`, which itself owns a `commands: Vec<Command>`.
    commands: &'a Submission,
    #[serde(rename = "transactionFormat")]
    transaction_format: TransactionFormat,
}

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

/// Submit commands and wait for the resulting flat transaction.
///
/// Calls the JSON Ledger API v2 endpoint
/// `POST /v2/commands/submit-and-wait-for-transaction` and returns the raw
/// `JsSubmitAndWaitForTransactionResponse` body as a string.
///
/// If `params.request.transaction_format` is set, that value is moved to the
/// top-level `transactionFormat` field of the request. If unset, a default
/// `TransactionFormat` is built that mirrors the defaults the deprecated tree
/// endpoint applied server-side: `transactionShape =
/// TRANSACTION_SHAPE_LEDGER_EFFECTS`, with one entry in `filtersByParty` per
/// party in `actAs ∪ readAs`.
///
/// # Errors
///
/// Returns `Err` if:
/// - `transaction_format` is unset *and* `act_as` is empty (the default would
///   produce an empty `filtersByParty` map, which the API rejects with an
///   opaque error);
/// - the HTTP request fails;
/// - the server returns a non-success status (the body is included in the
///   error message).
pub async fn wait_for_transaction(params: Params) -> Result<String, String> {
    let url = format!(
        "{}/v2/commands/submit-and-wait-for-transaction",
        params.ledger_host
    );

    let mut request = params.request;
    let transaction_format = if let Some(tf) = request.transaction_format.take() {
        tf
    } else {
        if request.act_as.is_empty() {
            return Err(
                "wait_for_transaction: Submission.act_as must contain at least one party \
                 when transaction_format is not provided"
                    .to_string(),
            );
        }
        default_transaction_format(&request)
    };

    let body = SubmitAndWaitRequest {
        commands: &request,
        transaction_format,
    };

    let response = http_client()
        .post(url)
        .json(&body)
        .bearer_auth(&params.access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = response.status();
    let body_raw = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response in wait_for_transaction: {e}"))?;

    if !status.is_success() {
        return Err(format!(
            "Submit request failed in wait_for_transaction [{status}]: {body_raw:?}"
        ));
    }
    log::trace!("Submit success: {body_raw}");

    Ok(body_raw)
}

#[deprecated(
    since = "0.5.0",
    note = "the `submit-and-wait-for-transaction-tree` JSON Ledger API endpoint is removed in Canton 3.5.0; migrate to `wait_for_transaction` (note: the response body shape changes from `transactionTree.eventsById` to `transaction.events`)"
)]
pub async fn wait_for_transaction_tree(params: Params) -> Result<String, String> {
    let url = format!(
        "{}/v2/commands/submit-and-wait-for-transaction-tree",
        params.ledger_host
    );
    let response = http_client()
        .post(url)
        .json(&params.request)
        .bearer_auth(&params.access_token)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = response.status();
    let body_raw = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response in wait_for_transaction_tree: {e}"))?;

    if !status.is_success() {
        return Err(format!(
            "Submit request failed in wait_for_transaction_tree [{status}]: {body_raw:?}"
        ));
    }
    log::trace!("Submit success: {body_raw}");

    Ok(body_raw)
}

fn default_transaction_format(request: &Submission) -> TransactionFormat {
    let mut filters_by_party: HashMap<String, common::filters::Filters> = HashMap::new();
    for party in &request.act_as {
        filters_by_party.entry(party.clone()).or_default();
    }
    if let Some(read_as) = &request.read_as {
        for party in read_as {
            filters_by_party.entry(party.clone()).or_default();
        }
    }

    TransactionFormat {
        transaction_shape: Some(TRANSACTION_SHAPE_LEDGER_EFFECTS.to_string()),
        event_format: Some(EventFormat {
            filters_by_party,
            filters_for_any_party: None,
            verbose: true,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::submission::{
        ChoiceArgumentsVariations, Command, ExerciseCommand, ExerciseCommandData,
    };

    fn sample_submission(act_as: Vec<String>, read_as: Option<Vec<String>>) -> Submission {
        Submission {
            act_as,
            read_as,
            command_id: "test-cmd".to_string(),
            commands: vec![Command::ExerciseCommand(ExerciseCommand {
                exercise_command: ExerciseCommandData {
                    template_id: "tid".to_string(),
                    contract_id: "cid".to_string(),
                    choice: "Choice".to_string(),
                    choice_argument: ChoiceArgumentsVariations::Generic(serde_json::json!({})),
                },
            })],
            ..Default::default()
        }
    }

    #[test]
    fn default_transaction_format_includes_act_as_and_read_as_parties() {
        let submission =
            sample_submission(vec!["alice".to_string()], Some(vec!["bob".to_string()]));
        let tf = default_transaction_format(&submission);

        assert_eq!(
            tf.transaction_shape.as_deref(),
            Some(TRANSACTION_SHAPE_LEDGER_EFFECTS)
        );
        let event_format = tf.event_format.expect("event_format is set");
        assert!(event_format.verbose);
        assert_eq!(event_format.filters_by_party.len(), 2);
        assert!(event_format.filters_by_party.contains_key("alice"));
        assert!(event_format.filters_by_party.contains_key("bob"));
    }

    #[test]
    fn default_transaction_format_dedups_overlapping_parties() {
        let submission = sample_submission(
            vec!["alice".to_string(), "bob".to_string()],
            Some(vec!["bob".to_string(), "carol".to_string()]),
        );
        let tf = default_transaction_format(&submission);
        let event_format = tf.event_format.unwrap();

        assert_eq!(event_format.filters_by_party.len(), 3);
        for party in ["alice", "bob", "carol"] {
            assert!(event_format.filters_by_party.contains_key(party));
        }
    }

    #[test]
    fn request_body_lifts_transaction_format_to_top_level() {
        let submission = sample_submission(vec!["alice".to_string()], None);
        let tf = default_transaction_format(&submission);
        let body = SubmitAndWaitRequest {
            commands: &submission,
            transaction_format: tf,
        };

        let json = serde_json::to_value(&body).expect("serialize");
        assert!(
            json.get("commands").is_some(),
            "outer `commands` field present"
        );
        assert!(
            json.get("transactionFormat").is_some(),
            "top-level `transactionFormat` present"
        );
        // The wrapped Submission carries its own `commands` array.
        assert!(json["commands"].get("commands").is_some());
        // The wrapped Submission must NOT also carry `transactionFormat`
        // (otherwise the request would double-nest it).
        assert!(json["commands"].get("transactionFormat").is_none());
        assert_eq!(
            json["transactionFormat"]["transactionShape"].as_str(),
            Some(TRANSACTION_SHAPE_LEDGER_EFFECTS)
        );
    }

    #[test]
    fn caller_provided_transaction_format_takes_precedence() {
        let custom = TransactionFormat {
            transaction_shape: Some("CUSTOM_SHAPE".to_string()),
            event_format: None,
        };
        let mut submission = sample_submission(vec!["alice".to_string()], None);
        submission.transaction_format = Some(custom.clone());

        // Mirror the take()-then-default logic from wait_for_transaction.
        let chosen = submission
            .transaction_format
            .take()
            .unwrap_or_else(|| default_transaction_format(&submission));

        assert_eq!(chosen, custom);
        assert!(submission.transaction_format.is_none());
    }
}

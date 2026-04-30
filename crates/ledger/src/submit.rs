use common::submission::{EventFormat, Submission, TransactionFormat};
use serde::Serialize;
use std::collections::HashMap;

pub struct Params {
    pub ledger_host: String,
    pub access_token: String,
    pub request: Submission,
}

#[derive(Serialize)]
struct SubmitAndWaitRequest<'a> {
    commands: &'a Submission,
    #[serde(rename = "transactionFormat")]
    transaction_format: TransactionFormat,
}

pub async fn wait_for_transaction(params: Params) -> Result<String, String> {
    let client = reqwest::Client::new();

    let url = format!(
        "{}/v2/commands/submit-and-wait-for-transaction",
        params.ledger_host
    );

    let mut request = params.request;
    let transaction_format = request
        .transaction_format
        .take()
        .unwrap_or_else(|| default_transaction_format(&request));

    let body = SubmitAndWaitRequest {
        commands: &request,
        transaction_format,
    };

    let response = client
        .post(url.to_string())
        .json(&body)
        .bearer_auth(&params.access_token)
        .send()
        .await
        .map_err(|e| format!("{}", e))?;

    let status = response.status();
    let body_raw = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response in wait_for_transaction: {}", e))?;

    if !status.is_success() {
        return Err(format!(
            "Submit request failed in wait_for_transaction [{}]: {:?}",
            status, body_raw
        ));
    }
    log::trace!("Submit success: {}", body_raw);

    Ok(body_raw)
}

#[deprecated(
    since = "0.5.0",
    note = "the `submit-and-wait-for-transaction-tree` endpoint is removed in Canton 3.5.0; use `wait_for_transaction` instead"
)]
pub async fn wait_for_transaction_tree(params: Params) -> Result<String, String> {
    wait_for_transaction(params).await
}

fn default_transaction_format(request: &Submission) -> TransactionFormat {
    let mut filters_by_party: HashMap<String, common::filters::Filters> = HashMap::new();
    for party in &request.act_as {
        filters_by_party
            .entry(party.clone())
            .or_insert_with(common::filters::Filters::default);
    }
    if let Some(read_as) = &request.read_as {
        for party in read_as {
            filters_by_party
                .entry(party.clone())
                .or_insert_with(common::filters::Filters::default);
        }
    }

    TransactionFormat {
        transaction_shape: Some("TRANSACTION_SHAPE_LEDGER_EFFECTS".to_string()),
        event_format: Some(EventFormat {
            filters_by_party,
            filters_for_any_party: None,
            verbose: true,
        }),
    }
}

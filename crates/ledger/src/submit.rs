pub struct Params {
    pub ledger_host: String,
    pub access_token: String,
    pub request: common::submission::Submission,
}

pub async fn wait_for_transaction_tree(params: Params) -> Result<String, String> {
    let client = reqwest::Client::new();

    let url = format!(
        "{}/v2/commands/submit-and-wait-for-transaction-tree",
        params.ledger_host
    );
    let response = client
        .post(url.to_string())
        .json(&params.request)
        .bearer_auth(&params.access_token)
        .send()
        .await
        .map_err(|e| format!("{}", e))?;

    let status = response.status();
    let body_raw = response.text().await.map_err(|e| {
        format!(
            "Failed to read response in wait_for_transaction_tree: {}",
            e
        )
    })?;

    if !status.is_success() {
        return Err(format!(
            "Submit request failed in wait_for_transaction_tree [{}]: {:?}",
            status, body_raw
        ));
    }
    log::trace!("Submit success: {}", body_raw);

    Ok(body_raw)
}

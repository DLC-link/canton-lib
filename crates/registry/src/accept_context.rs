use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct Params {
    pub registry_url: String,
    pub decentralized_party_id: String,
    pub transfer_offer_contract_id: String,
    pub request: Request,
}

#[derive(Debug, Serialize)]
pub struct Request {
    pub meta: Meta,
}

#[derive(Debug, Serialize)]
pub struct Meta {
    pub values: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub choice_context_data: ChoiceContextData,
    pub disclosed_contracts: Vec<common::transfer::DisclosedContract>,
}

#[derive(Debug, Deserialize)]
pub struct ChoiceContextData {
    pub values: serde_json::Value,
}

/// Get the choice context for accepting a transfer offer.
/// This retrieves the disclosed contracts and context data needed to accept the transfer.
///
/// # Example
/// ```ignore
/// use registry::accept_context;
///
/// let params = accept_context::Params {
///     registry_url: "https://api.utilities.digitalasset-dev.com".to_string(),
///     decentralized_party_id: "cbtc-network::1220...".to_string(),
///     transfer_offer_contract_id: "00abc123...".to_string(),
///     request: accept_context::Request {
///         meta: accept_context::Meta {
///             values: String::new(),
///         },
///     },
/// };
///
/// let response = accept_context::get(params).await?;
/// ```
pub async fn get(params: Params) -> Result<Response, String> {
    let url = format!(
        "{}/api/token-standard/v0/registrars/{}/registry/transfer-instruction/v1/{}/choice-contexts/accept",
        params.registry_url, params.decentralized_party_id, params.transfer_offer_contract_id
    );

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&params.request)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to registry: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read response body".to_string());
        return Err(format!(
            "Registry request failed with status {}: {}",
            status, body
        ));
    }

    let response_data: Response = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse registry response: {e}"))?;

    Ok(response_data)
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_get_accept_context() {
        // This is an integration test that requires a valid transfer offer contract ID
        // Skip in CI or when no .env is present
        dotenvy::dotenv().ok();

        let registry_url = std::env::var("REGISTRY_URL").unwrap_or_default();
        let decentralized_party_id = std::env::var("DECENTRALIZED_PARTY_ID").unwrap_or_default();

        if registry_url.is_empty() || decentralized_party_id.is_empty() {
            return;
        }

        // Note: This test requires a valid transfer_offer_contract_id
        // which would come from a real transfer. This is just a placeholder test.
    }
}

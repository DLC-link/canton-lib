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

/// The V1 accept choice-context route.
pub fn accept_context_url(
    registry_url: &str,
    decentralized_party_id: &str,
    transfer_offer_contract_id: &str,
) -> String {
    format!(
        "{registry_url}/api/token-standard/v0/registrars/{decentralized_party_id}/registry/transfer-instruction/v1/{transfer_offer_contract_id}/choice-contexts/accept"
    )
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
    let url = accept_context_url(
        &params.registry_url,
        &params.decentralized_party_id,
        &params.transfer_offer_contract_id,
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
    use super::*;

    #[test]
    fn v1_accept_context_url_keeps_its_v1_path() {
        assert_eq!(
            accept_context_url("https://registry.example", "admin::1220ab", "00offer"),
            "https://registry.example/api/token-standard/v0/registrars/admin::1220ab/registry/transfer-instruction/v1/00offer/choice-contexts/accept"
        );
    }
}

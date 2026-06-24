use serde::Serialize;

/// The allocation choice whose context is being fetched. Each variant maps to a
/// distinct `choice-contexts/{..}` endpoint on the registry's allocation API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationChoice {
    /// `Allocation_ExecuteTransfer` — settle the leg (exercised by the executor).
    ExecuteTransfer,
    /// `Allocation_Withdraw` — the sender reclaims a still-pending allocation.
    Withdraw,
    /// `Allocation_Cancel` — release the allocation back to the sender.
    Cancel,
}

impl AllocationChoice {
    /// The URL path segment under `choice-contexts/` for this choice.
    fn path_segment(self) -> &'static str {
        match self {
            Self::ExecuteTransfer => "execute-transfer",
            Self::Withdraw => "withdraw",
            Self::Cancel => "cancel",
        }
    }
}

#[derive(Debug)]
pub struct Params {
    pub registry_url: String,
    pub decentralized_party_id: String,
    pub allocation_contract_id: String,
    pub choice: AllocationChoice,
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

/// The choice context returned by the allocation `choice-contexts/{..}`
/// endpoints. Identical in shape to the transfer-instruction accept context:
/// `choiceContextData` plus the contracts to disclose when exercising.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub choice_context_data: ChoiceContextData,
    pub disclosed_contracts: Vec<common::transfer::DisclosedContract>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ChoiceContextData {
    pub values: serde_json::Value,
}

/// Get the choice context (and disclosed contracts) needed to exercise one of
/// the `Allocation_ExecuteTransfer`, `Allocation_Withdraw`, or
/// `Allocation_Cancel` choices on an allocation contract.
///
/// # Errors
///
/// Returns an error string if the request cannot be sent, the registry returns
/// a non-success status, or the response body cannot be parsed.
pub async fn get(params: Params) -> Result<Response, String> {
    let url = format!(
        "{}/api/token-standard/v0/registrars/{}/registry/allocations/v1/{}/choice-contexts/{}",
        params.registry_url,
        params.decentralized_party_id,
        params.allocation_contract_id,
        params.choice.path_segment()
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
            "Registry request failed with status {status}: {body}"
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
    fn path_segments_match_registry_endpoints() {
        assert_eq!(
            AllocationChoice::ExecuteTransfer.path_segment(),
            "execute-transfer"
        );
        assert_eq!(AllocationChoice::Withdraw.path_segment(), "withdraw");
        assert_eq!(AllocationChoice::Cancel.path_segment(), "cancel");
    }

    #[test]
    fn response_deserializes_context_and_disclosed_contracts() {
        let json = r#"{
            "choiceContextData": { "values": { "k": "v" } },
            "disclosedContracts": [
                {
                    "templateId": "pkg:Mod:T",
                    "contractId": "00disclosed",
                    "createdEventBlob": "blob",
                    "synchronizerId": "sync::1220"
                }
            ]
        }"#;
        let response: Response = serde_json::from_str(json).unwrap();
        assert_eq!(response.disclosed_contracts.len(), 1);
        assert_eq!(response.disclosed_contracts[0].contract_id, "00disclosed");
        assert_eq!(response.choice_context_data.values["k"], "v");
    }
}

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

    let response = crate::post_json(&url, &params.request)
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

/// The V2 transfer-instruction choice-context routes.
///
/// One function covers accept, reject and withdraw, because V2 serves all
/// three under the same path with a different trailing segment. The request
/// and response bodies are identical to V1's, so [`Request`], [`Meta`],
/// [`Response`] and [`ChoiceContextData`] are reused.
pub mod v2 {
    /// The transfer-instruction choice whose context is being fetched.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum InstructionChoice {
        /// `TransferInstruction_Accept` — the receiver accepts the transfer.
        Accept,
        /// `TransferInstruction_Reject` — the receiver rejects the transfer.
        Reject,
        /// `TransferInstruction_Withdraw` — the sender reclaims the transfer.
        Withdraw,
    }

    impl InstructionChoice {
        /// The URL path segment under `choice-contexts/` for this choice.
        fn path_segment(self) -> &'static str {
            match self {
                Self::Accept => "accept",
                Self::Reject => "reject",
                Self::Withdraw => "withdraw",
            }
        }
    }

    #[derive(Debug)]
    pub struct Params {
        pub registry_url: String,
        pub decentralized_party_id: String,
        pub transfer_instruction_id: String,
        pub choice: InstructionChoice,
        pub request: super::Request,
    }

    pub fn context_url(
        registry_url: &str,
        decentralized_party_id: &str,
        transfer_instruction_id: &str,
        choice: InstructionChoice,
    ) -> String {
        format!(
            "{registry_url}/api/token-standard/v0/registrars/{decentralized_party_id}/registry/transfer-instruction/v2/{transfer_instruction_id}/choice-contexts/{}",
            choice.path_segment()
        )
    }

    pub async fn get(params: Params) -> Result<super::Response, String> {
        let url = context_url(
            &params.registry_url,
            &params.decentralized_party_id,
            &params.transfer_instruction_id,
            params.choice,
        );

        let response = crate::post_json(&url, &params.request)
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

        response
            .json()
            .await
            .map_err(|e| format!("Failed to parse registry response: {e}"))
    }
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

    #[test]
    fn v2_context_url_covers_all_three_choices() {
        use v2::InstructionChoice;

        let url = |choice| {
            v2::context_url(
                "https://registry.example",
                "admin::1220ab",
                "00instruction",
                choice,
            )
        };
        let base = "https://registry.example/api/token-standard/v0/registrars/admin::1220ab/registry/transfer-instruction/v2/00instruction/choice-contexts";

        assert_eq!(url(InstructionChoice::Accept), format!("{base}/accept"));
        assert_eq!(url(InstructionChoice::Reject), format!("{base}/reject"));
        assert_eq!(url(InstructionChoice::Withdraw), format!("{base}/withdraw"));
    }
}

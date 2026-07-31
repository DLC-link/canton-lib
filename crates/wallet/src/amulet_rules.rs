use reqwest::header;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct AmuletRulesWrapper {
    #[serde(rename = "amulet_rules")]
    pub amulet_rules: AmuletRules,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AmuletRules {
    pub contract: AmuletRulesContract,
    #[serde(rename = "domain_id")]
    pub domain_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AmuletRulesContract {
    #[serde(rename = "template_id")]
    pub template_id: String,
    #[serde(rename = "contract_id")]
    pub contract_id: String,
    pub created_event_blob: String,
}

pub struct Params {
    pub token: String,
    pub wallet_api_host: String,
}

pub async fn get(params: Params) -> Result<AmuletRules, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|err| format!("Failed to get reqwest builder: {}", err))?;

    let url = format!(
        "{}/api/validator/v0/scan-proxy/amulet-rules",
        params.wallet_api_host
    );
    let resp = client
        .get(&url)
        .header(header::AUTHORIZATION, format!("Bearer {}", params.token))
        .send()
        .await
        .map_err(|e| format!("Amulet HTTP request error: {:?}", e))?;

    let wrapper: AmuletRulesWrapper = resp
        .json()
        .await
        .map_err(|e| format!("Amulet json parsing error: {:?}", e))?;
    Ok(wrapper.amulet_rules)
}

#[cfg(test)]
mod integration_tests {
    //! Live integration test for the amulet-rules query on the wallet API.
    //! It authenticates with the client-credentials flow and needs these
    //! env vars (a `.env` file is loaded when present): `WALLET_API_HOST`,
    //! `KEYCLOAK_URL` (full token endpoint URL), `AMULET_CLIENT_ID`,
    //! `AMULET_CLIENT_SECRET`.

    use super::*;
    use keycloak::login::{ClientCredentialsParams, client_credentials};
    use std::env;

    fn var(name: &str) -> String {
        env::var(name).unwrap_or_else(|_| panic!("{name} must be set for integration tests"))
    }

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_get_amulet_rules() {
        dotenvy::dotenv().ok();

        let login = client_credentials(ClientCredentialsParams {
            client_id: var("AMULET_CLIENT_ID"),
            client_secret: var("AMULET_CLIENT_SECRET"),
            url: var("KEYCLOAK_URL"),
        })
        .await
        .expect("keycloak client-credentials login failed");

        let rules = get(Params {
            token: login.access_token,
            wallet_api_host: var("WALLET_API_HOST"),
        })
        .await
        .expect("failed to get amulet rules");

        assert!(!rules.contract.contract_id.is_empty());
    }
}

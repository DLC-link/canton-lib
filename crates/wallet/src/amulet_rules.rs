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
mod tests {
    use super::*;
    use keycloak::login::{ClientCredentialsParams, client_credentials, client_credentials_url};
    use std::env;
    use tokio;

    #[tokio::test]
    async fn test_get_amulet_rules_integration() {
        dotenvy::dotenv().ok();

        let params = ClientCredentialsParams {
            client_id: env::var("KEYCLOAK_CLIENT_ID").expect("KEYCLOAK_CLIENT_ID must be set"),
            client_secret: env::var("LIB_TEST_AMULET_CLIENT_SECRET")
                .expect("LIB_TEST_AMULET_CLIENT_SECRET must be set"),
            url: client_credentials_url(
                &env::var("KEYCLOAK_HOST").expect("KEYCLOAK_HOST must be set"),
                &env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set"),
            ),
        };
        let result = client_credentials(params).await.unwrap();

        // Call the function
        let result = get(Params {
            token: result.access_token,
            wallet_api_host: env::var("WALLET_API_HOST").expect("WALLET_API_HOST must be set"),
        })
        .await;

        match result {
            Ok(rules) => {
                assert!(!rules.contract.contract_id.is_empty());
            }
            Err(e) => panic!("Failed to get amulet rules: {:?}", e),
        }
    }
}

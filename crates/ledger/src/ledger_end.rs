use crate::client::Client;
use canton_api_client::apis::default_api as canton_api;
use serde::{Deserialize, Serialize};

pub struct Params {
    pub access_token: String,
    pub ledger_host: String,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub offset: i64,
}

pub async fn get_with_client(client: &Client) -> Result<Response, String> {
    let ledger_end = canton_api::get_v2_state_ledger_end(&client.configuration)
        .await
        .map_err(|e| format!("Error getting ledger end: {}", e))?;

    Ok(Response {
        offset: ledger_end.offset,
    })
}

/// Get the ledger end offset, this exists if we ever want to implement our own reqwest solution here
pub async fn get(params: Params) -> Result<Response, String> {
    let canton_client = Client::new(params.access_token, params.ledger_host);

    let ledger_end = canton_api::get_v2_state_ledger_end(&canton_client.configuration)
        .await
        .map_err(|e| format!("Error getting ledger end: {}", e))?;

    Ok(Response {
        offset: ledger_end.offset,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use keycloak::login::{ClientCredentialsParams, client_credentials, client_credentials_url};
    use std::env;

    #[tokio::test]
    async fn test_get_ledger_end() {
        dotenvy::dotenv().ok();

        let params = ClientCredentialsParams {
            client_id: env::var("KEYCLOAK_CLIENT_ID").expect("KEYCLOAK_CLIENT_ID must be set"),
            client_secret: env::var("LIB_TEST_LEDGER_END_CLIENT_SECRET")
                .expect("LIB_TEST_LEDGER_END_CLIENT_SECRET must be set"),
            url: client_credentials_url(
                &env::var("KEYCLOAK_HOST").expect("KEYCLOAK_HOST must be set"),
                &env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set"),
            ),
        };
        let result = client_credentials(params).await.unwrap();

        let params = Params {
            access_token: result.access_token,
            ledger_host: env::var("LEDGER_HOST").expect("LEDGER_HOST must be set"),
        };

        let response = get(params).await.expect("Failed to get ledger end");
        assert!(response.offset >= 0);
    }
}

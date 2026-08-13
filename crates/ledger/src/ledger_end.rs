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
        offset: ledger_end
            .offset
            .ok_or_else(|| "Ledger end response missing offset".to_string())?,
    })
}

/// Get the ledger end offset, this exists if we ever want to implement our own reqwest solution here
pub async fn get(params: Params) -> Result<Response, String> {
    let canton_client = Client::new(params.access_token, params.ledger_host);

    let ledger_end = canton_api::get_v2_state_ledger_end(&canton_client.configuration)
        .await
        .map_err(|e| format!("Error getting ledger end: {}", e))?;

    Ok(Response {
        offset: ledger_end
            .offset
            .ok_or_else(|| "Ledger end response missing offset".to_string())?,
    })
}

#[cfg(test)]
mod integration_tests {
    //! Live integration test for the ledger end query. It authenticates
    //! with the client-credentials flow and needs these env vars (a `.env`
    //! file is loaded when present): `LEDGER_HOST`, `KEYCLOAK_URL` (full
    //! token endpoint URL), `KEYCLOAK_CLIENT_AUTH_CLIENT_ID`,
    //! `KEYCLOAK_CLIENT_AUTH_CLIENT_SECRET`.

    use super::*;
    use keycloak::login::{ClientCredentialsParams, client_credentials};
    use std::env;

    fn var(name: &str) -> String {
        env::var(name).unwrap_or_else(|_| panic!("{name} must be set for integration tests"))
    }

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_get_ledger_end() {
        dotenvy::dotenv().ok();

        let login = client_credentials(ClientCredentialsParams {
            client_id: var("KEYCLOAK_CLIENT_AUTH_CLIENT_ID"),
            client_secret: var("KEYCLOAK_CLIENT_AUTH_CLIENT_SECRET"),
            url: var("KEYCLOAK_URL"),
        })
        .await
        .expect("keycloak client-credentials login failed");

        let response = get(Params {
            access_token: login.access_token,
            ledger_host: var("LEDGER_HOST"),
        })
        .await
        .expect("failed to get ledger end");
        assert!(response.offset >= 0);
    }
}

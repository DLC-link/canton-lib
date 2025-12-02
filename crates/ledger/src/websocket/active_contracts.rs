use crate::{common, utils};
use canton_api_client::models;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::client::Request;

#[derive(Debug, Clone)]
pub struct Params {
    pub ledger_host: String,
    pub party: String,
    pub filter: common::IdentifierFilter,
    pub access_token: String,
    pub ledger_end: i64,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct ContractEntry {
    #[serde(rename = "JsActiveContract")]
    pub js_active_contract: models::JsActiveContract,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct ContractMessage {
    #[serde(rename = "workflowId")]
    pub workflow_id: Option<String>,
    #[serde(rename = "contractEntry")]
    pub contract_entry: Option<ContractEntry>,
}

pub async fn get_with_callback<F, Fut>(params: Params, mut callback: F) -> Result<(), String>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let ws_host = utils::http_to_ws(&params.ledger_host.clone());
    let ws_url = format!(
        "{}/v2/state/active-contracts",
        ws_host.trim_end_matches('/')
    );

    // Parse URL to extract host
    let parsed_url = url::Url::parse(&ws_url).map_err(|e| format!("Invalid URL: {e}"))?;
    let host = parsed_url
        .host_str()
        .ok_or_else(|| "Could not extract host from URL".to_string())?;

    let protocol_header = format!("jwt.token.{}, daml.ws.auth", params.access_token);
    let request = Request::builder()
        .uri(parsed_url.as_str())
        .header("Sec-WebSocket-Protocol", protocol_header)
        .header("Sec-WebSocket-Key", utils::random_16_byte_string())
        .header("Sec-WebSocket-Version", "13")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Host", host)
        .body(())
        .map_err(|e| format!("Failed to build request: {e}"))?;

    let (ws_stream, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| format!("WebSocket connection error: {e}"))?;

    let (mut write, mut read) = ws_stream.split();

    // Setup request
    let cumulative_vec: Vec<common::CumulativeFilter> = vec![common::CumulativeFilter {
        identifier_filter: params.filter,
    }];

    let mut filters_by_party: HashMap<String, common::Filters> = HashMap::new();
    filters_by_party.insert(
        params.party.clone(),
        common::Filters {
            cumulative: Some(cumulative_vec),
        },
    );
    let request = common::GetActiveContractsRequest {
        filter: Some(common::TransactionFilter {
            filters_by_party,
            filters_for_any_party: None,
        }),
        verbose: false,
        active_at_offset: params.ledger_end,
    };
    let event = serde_json::to_value(&request).map_err(|e| format!("Serialization error: {e}"))?;

    let mut error: Option<String> = None;

    // Send messages if needed
    match write
        .send(Message::Text(event.to_string()))
        .await
        .map_err(|e| format!("Error sending message: {e}"))
    {
        Ok(_) => {}
        Err(e) => {
            error = Some(e);
        }
    };

    while let Some(message) = read.next().await {
        match message {
            Ok(Message::Text(text)) => {
                if text.contains("A security-sensitive error has been received") {
                    error = Some(format!(
                        "Received security-sensitive error from server: {}",
                        text
                    ));
                    break;
                }
                callback(text).await;
            }
            Ok(Message::Binary(_)) => {
                log::info!("Received unhandled binary message.");
            }
            Ok(Message::Close(_)) => {
                break;
            }
            Err(e) => {
                error = Some(format!("WebSocket error: {e}"));
                break;
            }
            msg => match msg {
                Ok(other) => {
                    log::info!("Received other type of message: {:?}", other);
                }
                Err(e) => {
                    log::error!("Error receiving message: {}", e);
                }
            },
        }
    }

    match write
        .close()
        .await
        .map_err(|e| format!("Error closing connection: {e}"))
    {
        Ok(_) => {}
        Err(e) => {
            log::error!("Error closing websocket connection: {}", e);
        }
    };

    if let Some(err) = error {
        return Err(err);
    }

    Ok(())
}

pub async fn get(params: Params) -> Result<Vec<models::JsActiveContract>, String> {
    let ws_host = utils::http_to_ws(&params.ledger_host.clone());
    let ws_url = format!(
        "{}/v2/state/active-contracts",
        ws_host.trim_end_matches('/')
    );

    // Parse URL to extract host
    let parsed_url = url::Url::parse(&ws_url).map_err(|e| format!("Invalid URL: {e}"))?;
    let host = parsed_url
        .host_str()
        .ok_or_else(|| "Could not extract host from URL".to_string())?;

    let protocol_header = format!("jwt.token.{}, daml.ws.auth", params.access_token);
    let request = Request::builder()
        .uri(parsed_url.as_str())
        .header("Sec-WebSocket-Protocol", protocol_header)
        .header("Sec-WebSocket-Key", utils::random_16_byte_string())
        .header("Sec-WebSocket-Version", "13")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Host", host)
        .body(())
        .map_err(|e| format!("Failed to build request: {e}"))?;

    let (ws_stream, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| format!("WebSocket connection error: {e}"))?;

    let (mut write, mut read) = ws_stream.split();

    // Setup request
    let cumulative_vec: Vec<common::CumulativeFilter> = vec![common::CumulativeFilter {
        identifier_filter: params.filter,
    }];

    let mut filters_by_party: HashMap<String, common::Filters> = HashMap::new();
    filters_by_party.insert(
        params.party.clone(),
        common::Filters {
            cumulative: Some(cumulative_vec),
        },
    );
    let request = common::GetActiveContractsRequest {
        filter: Some(common::TransactionFilter {
            filters_by_party,
            filters_for_any_party: None,
        }),
        verbose: false,
        active_at_offset: params.ledger_end,
    };
    let event = serde_json::to_value(&request).map_err(|e| format!("Serialization error: {e}"))?;

    let mut error: Option<String> = None;

    // Send messages if needed
    match write
        .send(Message::Text(event.to_string()))
        .await
        .map_err(|e| format!("Error sending message: {e}"))
    {
        Ok(_) => {}
        Err(e) => {
            error = Some(e);
        }
    };

    let mut result: Vec<models::JsActiveContract> = Vec::new();
    while let Some(message) = read.next().await {
        match message {
            Ok(Message::Text(text)) => {
                if text.contains("A security-sensitive error has been received") {
                    error = Some(format!(
                        "Received security-sensitive error from server: {}",
                        text
                    ));
                    break;
                }
                let d: ContractMessage = serde_json::from_str(&text)
                    .map_err(|e| format!("Error deserializing JSON: {e}"))?;

                if let Some(ce) = d.contract_entry {
                    result.push(ce.js_active_contract);
                }
            }
            Ok(Message::Binary(_)) => {
                log::warn!("Received unhandled binary message.");
            }
            Ok(Message::Close(_)) => {
                break;
            }
            Err(e) => {
                error = Some(format!("WebSocket error: {e}"));
                break;
            }
            msg => match msg {
                Ok(other) => {
                    log::info!("Received other type of message: {:?}", other);
                }
                Err(e) => {
                    log::error!("Error receiving message: {}", e);
                }
            },
        }
    }

    match write
        .close()
        .await
        .map_err(|e| format!("Error closing connection: {e}"))
    {
        Ok(_) => {}
        Err(e) => {
            log::error!("Error closing websocket connection: {}", e);
        }
    };

    if let Some(err) = error {
        return Err(err);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger_end;
    use keycloak::login::{ClientCredentialsParams, client_credentials, client_credentials_url};
    use std::env;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_get() {
        dotenvy::dotenv().ok();

        let ledger_host = env::var("LEDGER_HOST").expect("LEDGER_HOST must be set");
        let party_id = env::var("PARTY_ID").expect("PARTY_ID must be set");

        let params = ClientCredentialsParams {
            client_id: env::var("KEYCLOAK_CLIENT_ID").expect("KEYCLOAK_CLIENT_ID must be set"),
            client_secret: env::var("LIB_TEST_LEDGER_END_CLIENT_SECRET")
                .expect("LIB_TEST_LEDGER_END_CLIENT_SECRET must be set"),
            url: client_credentials_url(
                &env::var("KEYCLOAK_HOST").expect("KEYCLOAK_HOST must be set"),
                &env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set"),
            ),
        };
        let login_response = client_credentials(params).await.unwrap();

        let params = ledger_end::Params {
            access_token: login_response.access_token.clone(),
            ledger_host: ledger_host.to_string(),
        };

        let ledger_end_response = ledger_end::get(params)
            .await
            .expect("Failed to get ledger end");

        // Run the connection with a timeout instead of spawning and aborting
        let result = tokio::time::timeout(
            Duration::from_secs(1000),
            get(Params {
                ledger_host: ledger_host.to_string(),
                party: party_id.to_string(),
                filter: common::IdentifierFilter::WildcardIdentifierFilter(
                    common::WildcardIdentifierFilter {
                        wildcard_filter: common::WildcardFilter {
                            value: common::WildcardFilterValue {
                                include_created_event_blob: true,
                            },
                        },
                    },
                ),
                access_token: login_response.access_token,
                ledger_end: ledger_end_response.offset,
            }),
        )
        .await;

        if let Ok(connection_result) = result {
            match connection_result {
                Ok(_) => {}
                Err(_e) => {}
            }
        }
    }
}

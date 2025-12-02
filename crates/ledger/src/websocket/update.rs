use crate::{common, utils};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::{handshake::client::Request, protocol::Message};

pub struct Params {
    pub ledger_host: String,
    pub party: String,
    pub filter: common::IdentifierFilter,
    pub access_token: String,
    pub ledger_end: i64,
}

pub async fn subscribe(
    params: Params,
    message_handler: fn(String) -> Result<(), String>,
) -> Result<(), String> {
    let ws_host = utils::http_to_ws(&params.ledger_host.clone());
    let ws_url = format!("{}/v2/updates", ws_host.trim_end_matches('/'));

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

    let mut map = std::collections::HashMap::new();
    map.insert(
        params.party.clone(),
        common::Filters {
            cumulative: Some(vec![common::CumulativeFilter {
                identifier_filter: params.filter,
            }]),
        },
    );
    let (mut write, mut read) = ws_stream.split();
    let event = common::UpdateRequest {
        filter: Some(common::TransactionFilter {
            filters_by_party: { map },
            filters_for_any_party: None,
        }),
        verbose: true,
        begin_exclusive: params.ledger_end,
        end_inclusive: None,
    };
    let event = serde_json::to_value(&event).map_err(|e| format!("Serialization error: {e}"))?;

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
                if let Err(e) = message_handler(text) {
                    log::error!("Error handling message: {e}");
                }
            }
            Ok(Message::Binary(_)) => {
                log::info!("Received unhandled binary message.");
            }
            Ok(Message::Ping(data)) => {
                // tungstenite usually auto-pongs, but it's fine to be explicit:
                let _ = write.send(Message::Pong(data)).await;
            }
            Ok(Message::Close(_)) => {
                return Ok(());
            }
            Err(e) => {
                return Err(format!("WebSocket error: {e}"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger_end;
    use keycloak::login::{ClientCredentialsParams, client_credentials, client_credentials_url};
    use std::env;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_websocket_connection() {
        dotenvy::dotenv().ok();

        let ledger_host = env::var("LEDGER_HOST").expect("LEDGER_HOST must be set");
        let party_id =
            env::var("DECENTRALIZED_PARTY_ID").expect("DECENTRALIZED_PARTY_ID must be set");

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
            subscribe(
                Params {
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
                },
                |_msg| Ok(()),
            ),
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

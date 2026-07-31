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
mod integration_tests {
    //! Live integration test for the websocket update subscription. It
    //! authenticates with the client-credentials flow and needs these env
    //! vars (a `.env` file is loaded when present): `LEDGER_HOST`,
    //! `PARTY_ID_1`, `KEYCLOAK_URL` (full token endpoint URL),
    //! `KEYCLOAK_CLIENT_AUTH_CLIENT_ID`,
    //! `KEYCLOAK_CLIENT_AUTH_CLIENT_SECRET`.

    use super::*;
    use crate::ledger_end;
    use keycloak::login::{ClientCredentialsParams, client_credentials};
    use std::env;
    use tokio::time::Duration;

    fn var(name: &str) -> String {
        env::var(name).unwrap_or_else(|_| panic!("{name} must be set for integration tests"))
    }

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_websocket_connection() {
        dotenvy::dotenv().ok();
        let ledger_host = var("LEDGER_HOST");

        let login = client_credentials(ClientCredentialsParams {
            client_id: var("KEYCLOAK_CLIENT_AUTH_CLIENT_ID"),
            client_secret: var("KEYCLOAK_CLIENT_AUTH_CLIENT_SECRET"),
            url: var("KEYCLOAK_URL"),
        })
        .await
        .expect("keycloak client-credentials login failed");

        let ledger_end_response = ledger_end::get(ledger_end::Params {
            access_token: login.access_token.clone(),
            ledger_host: ledger_host.clone(),
        })
        .await
        .expect("failed to get ledger end");

        // The subscription never completes on its own: staying connected
        // and error-free until the timeout fires is the success case.
        let result = tokio::time::timeout(
            Duration::from_secs(15),
            subscribe(
                Params {
                    ledger_host,
                    party: var("PARTY_ID_1"),
                    filter: common::IdentifierFilter::WildcardIdentifierFilter(
                        common::WildcardIdentifierFilter {
                            wildcard_filter: common::WildcardFilter {
                                value: common::WildcardFilterValue {
                                    include_created_event_blob: true,
                                },
                            },
                        },
                    ),
                    access_token: login.access_token,
                    ledger_end: ledger_end_response.offset,
                },
                |_msg| Ok(()),
            ),
        )
        .await;

        if let Ok(connection_result) = result {
            connection_result.expect("websocket subscription failed");
        }
    }
}

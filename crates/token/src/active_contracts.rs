#[derive(Debug, Clone)]
pub struct Params {
    pub ledger_host: String,
    pub party: String,
    pub access_token: String,
}

pub async fn get(params: Params) -> Result<Vec<ledger::models::JsActiveContract>, String> {
    use ledger::ledger_end;
    use ledger::websocket::active_contracts;

    let ledger_end_result = ledger_end::get(ledger_end::Params {
        access_token: params.access_token.clone(),
        ledger_host: params.ledger_host.clone(),
    })
    .await?;

    let result = active_contracts::get(active_contracts::Params {
        ledger_host: params.ledger_host,
        party: params.party,
        filter: ledger::common::IdentifierFilter::InterfaceIdentifierFilter(
            ledger::common::InterfaceIdentifierFilter {
                interface_filter: ledger::common::InterfaceFilter {
                    value: ledger::common::InterfaceFilterValue {
                        interface_id: Some(common::consts::INTERFACE_HOLDING.to_string()),
                        include_interface_view: true,
                        include_created_event_blob: true,
                    },
                },
            },
        ),
        access_token: params.access_token,
        ledger_end: ledger_end_result.offset,
    })
    .await?;

    let filtered: Vec<ledger::models::JsActiveContract> = result
        .into_iter()
        .filter(|ac| {
            // Note: Filter out CBTC related contracts only
            if let Some(view) = ac.created_event.interface_views.clone() {
                for iv in view {
                    let value = iv.view_value.unwrap_or_default().unwrap_or_default();
                    let instrument_id = value.get("instrumentId").unwrap_or_default();
                    let instrument = instrument_id
                        .get("id")
                        .unwrap_or_default()
                        .as_str()
                        .unwrap_or_default();

                    let lock = value.get("lock").unwrap_or_default();

                    // Note: We have to check the lock value to be null
                    if instrument.to_lowercase().eq("cbtc") && lock.as_null().is_some() {
                        return true;
                    }
                }
            }
            false
        })
        .collect();
    Ok(filtered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use keycloak::login::{PasswordParams, password, password_url};
    use std::env;

    #[tokio::test]
    async fn test_get_by_party() {
        dotenvy::dotenv().ok();

        let ledger_host = env::var("LEDGER_HOST").expect("LEDGER_HOST must be set");
        let party_id = env::var("PARTY_ID").expect("PARTY_ID must be set");

        let params = PasswordParams {
            client_id: env::var("KEYCLOAK_CLIENT_ID").expect("KEYCLOAK_CLIENT_ID must be set"),
            username: env::var("KEYCLOAK_USERNAME").expect("KEYCLOAK_USERNAME must be set"),
            password: env::var("KEYCLOAK_PASSWORD").expect("KEYCLOAK_PASSWORD must be set"),
            url: password_url(
                &env::var("KEYCLOAK_HOST").expect("KEYCLOAK_HOST must be set"),
                &env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set"),
            ),
        };
        let login_response = password(params).await.unwrap();

        let contracts = get(Params {
            ledger_host: ledger_host.to_string(),
            party: party_id,
            access_token: login_response.access_token,
        })
        .await
        .unwrap();

        assert!(!contracts.is_empty());
    }
}

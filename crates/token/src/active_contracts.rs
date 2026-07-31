#[derive(Debug, Clone)]
pub struct Params {
    pub ledger_host: String,
    pub party: String,
    pub access_token: String,
    /// The instrument whose holdings to fetch
    pub instrument_id: common::transfer::InstrumentId,
}

pub async fn get(params: Params) -> Result<Vec<ledger::models::JsActiveContract>, String> {
    use ledger::ledger_end;
    use ledger::websocket::active_contracts;

    let wanted_instrument = params.instrument_id.clone();

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
            // Note: Filter out the requested instrument's contracts only
            if let Some(view) = ac.created_event.interface_views.clone() {
                for iv in view {
                    let value = iv.view_value.unwrap_or_default().unwrap_or_default();
                    let instrument_id = value.get("instrumentId").unwrap_or_default();
                    let instrument = instrument_id
                        .get("id")
                        .unwrap_or_default()
                        .as_str()
                        .unwrap_or_default();
                    let admin = instrument_id
                        .get("admin")
                        .unwrap_or_default()
                        .as_str()
                        .unwrap_or_default();

                    let lock = value.get("lock").unwrap_or_default();

                    // Note: We have to check the lock value to be null
                    if instrument.eq_ignore_ascii_case(&wanted_instrument.id)
                        && admin == wanted_instrument.admin
                        && lock.as_null().is_some()
                    {
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
// Each test holds SERIAL_LOCK across its awaits on purpose: the lock spans
// the whole test body so tests never interleave, and `#[tokio::test]` runs
// on a single-thread runtime, so a held std guard cannot deadlock it.
#[allow(clippy::await_holding_lock)]
mod integration_tests {
    //! Live integration test for the holdings ACS query. Shared setup, the
    //! required env vars, and the run command are documented in
    //! [`crate::test_utils`].

    use super::*;
    use crate::test_utils::{IntegrationTestState, SERIAL_LOCK};

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_get_by_party() {
        let _guard = SERIAL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let state = IntegrationTestState::from_env();

        let contracts = get(Params {
            ledger_host: state.ledger_host.clone(),
            party: state.party_1.clone(),
            access_token: state.access_token().await,
            instrument_id: state.instrument.clone(),
        })
        .await
        .expect("active_contracts::get failed");

        assert!(
            !contracts.is_empty(),
            "party 1 should hold at least one active contract"
        );
    }
}

#[derive(Debug, Clone)]
pub struct Params {
    pub ledger_host: String,
    pub party: String,
    pub access_token: String,
    /// The instrument whose holdings to fetch
    pub instrument_id: common::transfer::InstrumentId,
    /// When `Some`, keep only holdings whose account label matches. `None`
    /// keeps every holding the party owns, which is what every V1 caller
    /// wants and what this function did before Token Standard V2.
    pub account: Option<common::transfer::v2::Account>,
}

const PROVIDER_META_KEY: &str = "cip-112/account.provider";
const ACCOUNT_ID_META_KEY: &str = "cip-112/account.id";

/// Does this holding sit under `account`?
///
/// A V2 holding carries its account label in the metadata of its V1 interface
/// view: `accountToMeta` writes `cip-112/account.provider` and
/// `cip-112/account.id` (`Conversions.daml:187-201`). It writes each key only
/// when the value is present — a provider of `None`, or an empty id, produces
/// no key at all. So an absent key means "unlabelled", and a basic account
/// matches exactly the holdings that carry neither key.
pub(crate) fn matches_account(
    view: &serde_json::Value,
    account: &common::transfer::v2::Account,
) -> bool {
    let values = view.get("meta").and_then(|meta| meta.get("values"));
    let read = |key: &str| -> Option<&str> {
        values
            .and_then(|values| values.get(key))
            .and_then(|v| v.as_str())
    };

    read(PROVIDER_META_KEY) == account.provider.as_deref()
        && read(ACCOUNT_ID_META_KEY).unwrap_or("") == account.id
}

pub async fn get(params: Params) -> Result<Vec<ledger::models::JsActiveContract>, String> {
    use ledger::ledger_end;
    use ledger::websocket::active_contracts;

    let wanted_instrument = params.instrument_id.clone();
    let params_account = params.account.clone();

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
        .filter(|ac| wanted(ac, &wanted_instrument, params_account.as_ref()))
        .collect();
    Ok(filtered)
}

/// Does this contract belong in the result: an unlocked holding of
/// `instrument`, and under `account` when one is given?
///
/// Split out of [`get`], which opens a websocket and so cannot be reached by a
/// unit test. Keeping the rules here means each one has tests.
pub(crate) fn wanted(
    ac: &ledger::models::JsActiveContract,
    instrument: &common::transfer::InstrumentId,
    account: Option<&common::transfer::v2::Account>,
) -> bool {
    let Some(views) = ac.created_event.interface_views.clone() else {
        return false;
    };

    for iv in views {
        let value = iv.view_value.unwrap_or_default().unwrap_or_default();
        let instrument_id = value.get("instrumentId").unwrap_or_default();
        let id = instrument_id
            .get("id")
            .unwrap_or_default()
            .as_str()
            .unwrap_or_default();
        let admin = instrument_id
            .get("admin")
            .unwrap_or_default()
            .as_str()
            .unwrap_or_default();

        // A locked holding is not spendable, so `lock` must be null.
        let lock = value.get("lock").unwrap_or_default();

        if id == instrument.id && admin == instrument.admin && lock.as_null().is_some() {
            if let Some(account) = account
                && !matches_account(&value, account)
            {
                continue;
            }
            return true;
        }
    }

    false
}

#[cfg(test)]
mod label_tests {
    use super::*;
    use serde_json::json;

    fn view_with_meta(values: serde_json::Value) -> serde_json::Value {
        json!({
            "instrumentId": { "admin": "admin::1220ef", "id": "CBTC" },
            "amount": "1.0",
            "lock": null,
            "meta": { "values": values }
        })
    }

    fn instrument() -> common::transfer::InstrumentId {
        common::transfer::InstrumentId {
            admin: "admin::1220ef".to_string(),
            id: "CBTC".to_string(),
        }
    }

    /// An active contract carrying one Holding interface view.
    fn holding(view: serde_json::Value) -> ledger::models::JsActiveContract {
        let created_event = ledger::models::CreatedEvent {
            contract_id: "00cid".to_string(),
            interface_views: Some(vec![ledger::models::JsInterfaceView {
                interface_id: common::consts::INTERFACE_HOLDING.to_string(),
                view_value: Some(Some(view)),
                ..Default::default()
            }]),
            ..Default::default()
        };

        ledger::models::JsActiveContract {
            created_event: Box::new(created_event),
            ..Default::default()
        }
    }

    #[test]
    fn an_unlocked_holding_of_the_instrument_is_wanted() {
        assert!(wanted(
            &holding(view_with_meta(json!({}))),
            &instrument(),
            None
        ));
    }

    #[test]
    fn a_holding_of_another_instrument_is_not_wanted() {
        let view = json!({
            "instrumentId": { "admin": "admin::1220ef", "id": "OTHER" },
            "amount": "1.0",
            "lock": null,
        });
        assert!(!wanted(&holding(view), &instrument(), None));
    }

    #[test]
    fn a_locked_holding_is_not_wanted() {
        let view = json!({
            "instrumentId": { "admin": "admin::1220ef", "id": "CBTC" },
            "amount": "1.0",
            "lock": { "holders": ["alice::1220ab"] },
        });
        assert!(
            !wanted(&holding(view), &instrument(), None),
            "a locked holding is not spendable"
        );
    }

    #[test]
    fn an_account_filter_reaches_the_selection() {
        let labelled = holding(view_with_meta(json!({ "cip-112/account.id": "treasury" })));
        let basic = common::transfer::v2::Account::basic("alice::1220ab");

        assert!(
            !wanted(&labelled, &instrument(), Some(&basic)),
            "dropping the account check from `wanted` would let a basic \
             account spend a labelled holding"
        );

        let own = common::transfer::v2::Account {
            owner: Some("alice::1220ab".to_string()),
            provider: None,
            id: "treasury".to_string(),
        };
        assert!(wanted(&labelled, &instrument(), Some(&own)));
    }

    #[test]
    fn no_account_keeps_a_labelled_holding() {
        let labelled = holding(view_with_meta(json!({ "cip-112/account.id": "treasury" })));
        assert!(
            wanted(&labelled, &instrument(), None),
            "a V1 caller passes no account and must keep every holding"
        );
    }

    #[test]
    fn a_basic_account_matches_a_holding_with_no_label_keys() {
        let view = view_with_meta(json!({}));
        assert!(matches_account(
            &view,
            &common::transfer::v2::Account::basic("alice::1220ab")
        ));
    }

    #[test]
    fn a_basic_account_rejects_a_labelled_holding() {
        let view = view_with_meta(json!({ "cip-112/account.id": "treasury" }));
        assert!(
            !matches_account(
                &view,
                &common::transfer::v2::Account::basic("alice::1220ab")
            ),
            "a basic account must not sweep up a labelled holding"
        );

        let view = view_with_meta(json!({ "cip-112/account.provider": "prov::1220cd" }));
        assert!(!matches_account(
            &view,
            &common::transfer::v2::Account::basic("alice::1220ab")
        ));
    }

    #[test]
    fn a_labelled_account_matches_only_its_own_label() {
        let account = common::transfer::v2::Account {
            owner: Some("alice::1220ab".to_string()),
            provider: Some("prov::1220cd".to_string()),
            id: "treasury".to_string(),
        };

        let exact = view_with_meta(json!({
            "cip-112/account.provider": "prov::1220cd",
            "cip-112/account.id": "treasury"
        }));
        assert!(matches_account(&exact, &account));

        let other_id = view_with_meta(json!({
            "cip-112/account.provider": "prov::1220cd",
            "cip-112/account.id": "operating"
        }));
        assert!(!matches_account(&other_id, &account));

        let other_provider = view_with_meta(json!({
            "cip-112/account.provider": "prov::9999",
            "cip-112/account.id": "treasury"
        }));
        assert!(!matches_account(&other_provider, &account));

        let unlabelled = view_with_meta(json!({}));
        assert!(
            !matches_account(&unlabelled, &account),
            "a labelled account must not claim an unlabelled holding"
        );
    }

    #[test]
    fn a_missing_meta_block_reads_as_no_label() {
        let view = json!({ "instrumentId": { "admin": "a", "id": "CBTC" }, "lock": null });
        assert!(matches_account(
            &view,
            &common::transfer::v2::Account::basic("alice::1220ab")
        ));
    }
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
            account: None,
        })
        .await
        .expect("active_contracts::get failed");

        assert!(
            !contracts.is_empty(),
            "party 1 should hold at least one active contract"
        );
    }

    /// The V2 read, which filters on the account label.
    ///
    /// Every account this library builds is basic, so no devnet holding
    /// carries a label key and the filter must keep the whole V1 set. That is
    /// what this asserts: it catches a filter that drops holdings a V2 caller
    /// then cannot spend. It cannot catch a filter that keeps too much —
    /// `an_account_filter_reaches_the_selection` covers that direction.
    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_get_by_party_v2() {
        let _guard = SERIAL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let state = IntegrationTestState::from_env();
        let access_token = state.access_token().await;

        let unfiltered = get(Params {
            ledger_host: state.ledger_host.clone(),
            party: state.party_1.clone(),
            access_token: access_token.clone(),
            instrument_id: state.instrument.clone(),
            account: None,
        })
        .await
        .expect("V1 active_contracts::get failed");

        let filtered = get(Params {
            ledger_host: state.ledger_host.clone(),
            party: state.party_1.clone(),
            access_token,
            instrument_id: state.instrument.clone(),
            account: Some(common::transfer::v2::Account::basic(state.party_1.clone())),
        })
        .await
        .expect("V2 active_contracts::get failed");

        assert!(
            !filtered.is_empty(),
            "party 1 should hold at least one active contract"
        );

        let unfiltered_cids: Vec<&String> = unfiltered
            .iter()
            .map(|c| &c.created_event.contract_id)
            .collect();
        let filtered_cids: Vec<&String> = filtered
            .iter()
            .map(|c| &c.created_event.contract_id)
            .collect();

        assert_eq!(
            filtered_cids, unfiltered_cids,
            "party 1 holds no labelled holding, so the basic account must \
             select exactly the unfiltered set"
        );
    }
}

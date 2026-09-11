use canton_api_client::models::JsActiveContract;
use common::decimal::DamlDecimal;
use common::transfer::InstrumentId;

/// A token holding contract
#[derive(Debug, Clone)]
pub struct Holding {
    pub contract_id: String,
    pub amount: DamlDecimal,
    /// The instrument, admin included. The payload's `registrar` is the
    /// admin, which is what `Holding.daml:57` writes into the V1 view.
    pub instrument_id: InstrumentId,
    pub owner: String,
    /// The account id. The Daml template calls this field `label`, and
    /// `registryAccount` (`TokenApiUtilsV2.daml:57-59`) derives the V2
    /// account from `owner` and this field, with no provider.
    pub account_id: String,
}

impl Holding {
    /// Parse a Holding from a JsActiveContract
    pub fn from_active_contract(contract: &JsActiveContract) -> Result<Self, String> {
        let contract_id = contract.created_event.contract_id.clone();

        let args = contract
            .created_event
            .create_argument
            .as_ref()
            .and_then(|v| v.as_object())
            .ok_or("createArgument is not an object")?;

        let amount = DamlDecimal::parse(
            args.get("amount")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'amount' field")?,
        )
        .map_err(|e| format!("Invalid 'amount' field: {}", e))?;

        let instrument = args
            .get("instrument")
            .and_then(|v| v.as_object())
            .ok_or("Missing 'instrument' field")?;

        let id = instrument
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'instrument.id' field")?
            .to_string();

        // The template's `registrar` is the instrument admin: `Holding.daml:57`
        // writes it into the V1 view as `instrumentId.admin`. The payload's
        // `instrument.source` holds the same party, and the template's `ensure`
        // clause forces the two to agree, so reading either is correct. Read
        // `registrar`, because that is the field the view reads.
        let admin = args
            .get("registrar")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'registrar' field")?
            .to_string();

        let owner = args
            .get("owner")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'owner' field")?
            .to_string();

        // An empty label is a real unlabelled account, so only an absent field
        // is an error. Every registry-holding version from 0.0.1 declares
        // `label`, so a required read cannot break an existing holding.
        let account_id = args
            .get("label")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'label' field")?
            .to_string();

        Ok(Self {
            contract_id,
            amount,
            instrument_id: InstrumentId { admin, id },
            owner,
            account_id,
        })
    }

    /// Check if this holding is locked (being used in another transaction)
    /// Returns true if the holding has a non-null lock field
    pub fn is_locked_in_contract(contract: &JsActiveContract) -> bool {
        contract
            .created_event
            .create_argument
            .as_ref()
            .and_then(|v| v.as_object())
            .and_then(|args| args.get("lock"))
            .is_some_and(|lock| !lock.is_null())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canton_api_client::models::CreatedEvent;
    use serde_json::json;

    /// An active contract carrying a concrete utility-registry Holding
    /// payload. `Holding` reads `createArgument`, not an interface view.
    fn contract(argument: Option<serde_json::Value>) -> JsActiveContract {
        JsActiveContract {
            created_event: Box::new(CreatedEvent {
                contract_id: "00cid".to_string(),
                create_argument: argument,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    /// The eight fields `Utility.Registry.Holding.V0.Holding` declares. Every
    /// registry-holding version from 0.0.1 to 0.3.1 declares the same set, and
    /// all 519,386 active mainnet holdings carried all eight on 11 Sep 2026.
    fn payload() -> serde_json::Value {
        json!({
            "operator": "operator::1220aa",
            "provider": "provider::1220bb",
            "registrar": "admin::1220ef",
            "owner": "alice::1220ab",
            "instrument": {
                "source": "admin::1220ef",
                "id": "CBTC",
                "scheme": "RegistrarInternalScheme",
            },
            "label": "",
            "amount": "1.25",
            "lock": null,
        })
    }

    fn cbtc() -> InstrumentId {
        InstrumentId {
            admin: "admin::1220ef".to_string(),
            id: "CBTC".to_string(),
        }
    }

    #[test]
    fn parses_every_field_of_a_holding() {
        let holding = Holding::from_active_contract(&contract(Some(payload())))
            .expect("the fixture is a valid holding");

        assert_eq!(holding.contract_id, "00cid");
        assert_eq!(holding.amount, DamlDecimal::parse("1.25").unwrap());
        assert_eq!(holding.instrument_id, cbtc());
        assert_eq!(holding.owner, "alice::1220ab");
        assert_eq!(holding.account_id, "");
    }

    #[test]
    fn reads_the_id_out_of_the_instrument_object() {
        let mut argument = payload();
        argument["instrument"]["id"] = json!("OTHER");

        let holding = Holding::from_active_contract(&contract(Some(argument))).unwrap();

        assert_eq!(
            holding.instrument_id,
            InstrumentId {
                admin: "admin::1220ef".to_string(),
                id: "OTHER".to_string(),
            }
        );
    }

    /// The admin comes from the top-level `registrar`, never from
    /// `instrument.source`. The template's `ensure` clause forces the two to
    /// agree, so this fixture is impossible on the ledger. That is the point:
    /// giving them different values is the only way to prove which one the
    /// parser reads.
    #[test]
    fn the_admin_comes_from_registrar_not_from_instrument_source() {
        let mut argument = payload();
        argument["instrument"]["source"] = json!("decoy::1220cc");

        let holding = Holding::from_active_contract(&contract(Some(argument))).unwrap();

        assert_eq!(holding.instrument_id.admin, "admin::1220ef");
    }

    #[test]
    fn rejects_a_holding_with_no_registrar() {
        let mut argument = payload();
        argument.as_object_mut().unwrap().remove("registrar");

        let error = Holding::from_active_contract(&contract(Some(argument))).unwrap_err();

        assert_eq!(error, "Missing 'registrar' field");
    }

    /// An empty label is a real unlabelled account, so it parses. Only an
    /// absent field is an error, which the next test covers.
    #[test]
    fn an_empty_label_is_the_unlabelled_account() {
        let mut argument = payload();
        argument["label"] = json!("");

        let holding = Holding::from_active_contract(&contract(Some(argument))).unwrap();

        assert_eq!(holding.account_id, "");
    }

    #[test]
    fn reads_a_non_empty_label_as_the_account_id() {
        let mut argument = payload();
        argument["label"] = json!("treasury");

        let holding = Holding::from_active_contract(&contract(Some(argument))).unwrap();

        assert_eq!(holding.account_id, "treasury");
    }

    #[test]
    fn rejects_a_holding_with_no_label() {
        let mut argument = payload();
        argument.as_object_mut().unwrap().remove("label");

        let error = Holding::from_active_contract(&contract(Some(argument))).unwrap_err();

        assert_eq!(error, "Missing 'label' field");
    }

    #[test]
    fn rejects_a_contract_with_no_create_argument() {
        let error = Holding::from_active_contract(&contract(None)).unwrap_err();

        assert_eq!(error, "createArgument is not an object");
    }

    #[test]
    fn rejects_a_create_argument_that_is_not_an_object() {
        let error = Holding::from_active_contract(&contract(Some(json!("a string")))).unwrap_err();

        assert_eq!(error, "createArgument is not an object");
    }

    #[test]
    fn rejects_a_holding_with_no_amount() {
        let mut argument = payload();
        argument.as_object_mut().unwrap().remove("amount");

        let error = Holding::from_active_contract(&contract(Some(argument))).unwrap_err();

        assert_eq!(error, "Missing 'amount' field");
    }

    #[test]
    fn rejects_an_amount_that_is_not_a_decimal() {
        let mut argument = payload();
        argument["amount"] = json!("not a number");

        let error = Holding::from_active_contract(&contract(Some(argument))).unwrap_err();

        // The detail comes from the decimal parser, so derive it rather than
        // pinning wording this crate does not own.
        let detail = DamlDecimal::parse("not a number").unwrap_err();
        assert_eq!(error, format!("Invalid 'amount' field: {detail}"));
    }

    #[test]
    fn rejects_an_amount_that_is_a_json_number() {
        let mut argument = payload();
        argument["amount"] = json!(1.25);

        let error = Holding::from_active_contract(&contract(Some(argument))).unwrap_err();

        assert_eq!(error, "Missing 'amount' field");
    }

    #[test]
    fn rejects_a_holding_with_no_instrument() {
        let mut argument = payload();
        argument.as_object_mut().unwrap().remove("instrument");

        let error = Holding::from_active_contract(&contract(Some(argument))).unwrap_err();

        assert_eq!(error, "Missing 'instrument' field");
    }

    #[test]
    fn rejects_an_instrument_with_no_id() {
        let mut argument = payload();
        argument["instrument"].as_object_mut().unwrap().remove("id");

        let error = Holding::from_active_contract(&contract(Some(argument))).unwrap_err();

        assert_eq!(error, "Missing 'instrument.id' field");
    }

    #[test]
    fn rejects_a_holding_with_no_owner() {
        let mut argument = payload();
        argument.as_object_mut().unwrap().remove("owner");

        let error = Holding::from_active_contract(&contract(Some(argument))).unwrap_err();

        assert_eq!(error, "Missing 'owner' field");
    }

    #[test]
    fn a_holding_with_a_lock_is_locked() {
        let mut argument = payload();
        argument["lock"] = json!({ "holders": ["alice::1220ab"] });

        assert!(Holding::is_locked_in_contract(&contract(Some(argument))));
    }

    #[test]
    fn a_holding_with_a_null_lock_is_not_locked() {
        let mut argument = payload();
        argument["lock"] = json!(null);

        assert!(!Holding::is_locked_in_contract(&contract(Some(argument))));
    }

    #[test]
    fn a_holding_with_no_lock_field_is_not_locked() {
        let mut argument = payload();
        argument.as_object_mut().unwrap().remove("lock");

        assert!(!Holding::is_locked_in_contract(&contract(Some(argument))));
    }

    #[test]
    fn a_contract_with_no_create_argument_is_not_locked() {
        assert!(!Holding::is_locked_in_contract(&contract(None)));
    }
}

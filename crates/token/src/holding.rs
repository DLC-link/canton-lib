use canton_api_client::models::JsActiveContract;
use common::decimal::DamlDecimal;

/// A token holding contract
#[derive(Debug, Clone)]
pub struct Holding {
    pub contract_id: String,
    pub amount: DamlDecimal,
    pub instrument_id: String,
    pub owner: String,
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

        let instrument_id = instrument
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'instrument.id' field")?
            .to_string();

        let owner = args
            .get("owner")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'owner' field")?
            .to_string();

        Ok(Self {
            contract_id,
            amount,
            instrument_id,
            owner,
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

    fn payload() -> serde_json::Value {
        json!({
            "amount": "1.25",
            "instrument": { "admin": "admin::1220ef", "id": "CBTC" },
            "owner": "alice::1220ab",
        })
    }

    #[test]
    fn parses_every_field_of_a_holding() {
        let holding = Holding::from_active_contract(&contract(Some(payload())))
            .expect("the fixture is a valid holding");

        assert_eq!(holding.contract_id, "00cid");
        assert_eq!(holding.amount, DamlDecimal::parse("1.25").unwrap());
        assert_eq!(holding.instrument_id, "CBTC");
        assert_eq!(holding.owner, "alice::1220ab");
    }

    #[test]
    fn reads_the_id_out_of_the_instrument_object() {
        let mut argument = payload();
        argument["instrument"]["id"] = json!("OTHER");

        let holding = Holding::from_active_contract(&contract(Some(argument))).unwrap();

        assert_eq!(holding.instrument_id, "OTHER");
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
        assert!(!Holding::is_locked_in_contract(&contract(Some(payload()))));
    }

    #[test]
    fn a_contract_with_no_create_argument_is_not_locked() {
        assert!(!Holding::is_locked_in_contract(&contract(None)));
    }
}

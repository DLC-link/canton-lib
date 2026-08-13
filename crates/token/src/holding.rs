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

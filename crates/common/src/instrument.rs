use serde::{Deserialize, Serialize};

/// Which instrument a holding, transfer or allocation refers to.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstrumentId {
    /// The party that administers the instrument. A payload may call the same
    /// party `registrar` or `source`.
    pub admin: String,
    /// The ticker, such as `CBTC`. Unique per admin, not globally, so compare
    /// a whole `InstrumentId`: this field alone admits another admin's token
    /// of the same name.
    pub id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cbtc() -> InstrumentId {
        InstrumentId {
            admin: "cbtc-network::1220ab".to_string(),
            id: "CBTC".to_string(),
        }
    }

    #[test]
    fn two_instruments_with_the_same_fields_are_equal() {
        assert_eq!(cbtc(), cbtc());
    }

    #[test]
    fn a_different_admin_makes_a_different_instrument() {
        let attacker = InstrumentId {
            admin: "attacker::1220ff".to_string(),
            ..cbtc()
        };

        assert_ne!(cbtc(), attacker);
    }

    #[test]
    fn a_different_ticker_makes_a_different_instrument() {
        let legacy = InstrumentId {
            id: "CBTCV0RC8".to_string(),
            ..cbtc()
        };

        assert_ne!(cbtc(), legacy);
    }
}

use serde::{Deserialize, Serialize};

/// Which instrument a holding, transfer or allocation refers to.
///
/// This type mirrors `Splice.Api.Token.HoldingV1.InstrumentId`. The Token
/// Standard declares it in its Holding module, not its transfer module,
/// because every part of the standard refers to an instrument.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstrumentId {
    /// The party that administers the instrument.
    ///
    /// The utility registry calls the same party `registrar` in its `Holding`
    /// template, and `source` in `InstrumentIdentifier`. `Holding.daml:57`
    /// maps `registrar` to `admin` when it builds the view. Another registry
    /// app would use its own template name, and this field would still be
    /// `admin`.
    pub admin: String,
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

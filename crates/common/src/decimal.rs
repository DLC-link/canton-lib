use rust_decimal::Decimal;
use rust_decimal::RoundingStrategy::MidpointNearestEven;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};
use std::str::FromStr;

pub const DAML_DECIMAL_SCALE: u32 = 10;

#[derive(Debug)]
pub enum DamlDecimalError {
    InvalidScale { expected: u32, actual: u32 },
    ParseError(String),
}

impl fmt::Display for DamlDecimalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DamlDecimalError::InvalidScale { expected, actual } => {
                write!(
                    f,
                    "expected at most {} decimal places, got {}",
                    expected, actual
                )
            }
            DamlDecimalError::ParseError(msg) => write!(f, "failed to parse decimal: {}", msg),
        }
    }
}

impl std::error::Error for DamlDecimalError {}

/// A Daml Decimal (Numeric 10) value — at most 10 decimal places.
///
/// Construction validates the scale invariant. Arithmetic rounds results
/// to 10 decimal places using banker's rounding (HalfEven), matching
/// Daml's behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DamlDecimal(Decimal);

impl DamlDecimal {
    /// Creates a DamlDecimal from a Decimal, validating scale ≤ 10.
    pub fn new(value: Decimal) -> Result<Self, DamlDecimalError> {
        if value.scale() > DAML_DECIMAL_SCALE {
            return Err(DamlDecimalError::InvalidScale {
                expected: DAML_DECIMAL_SCALE,
                actual: value.scale(),
            });
        }
        Ok(DamlDecimal(value))
    }

    /// Parses a string into a DamlDecimal, validating scale ≤ 10.
    pub fn parse(s: &str) -> Result<Self, DamlDecimalError> {
        let value =
            Decimal::from_str(s).map_err(|e| DamlDecimalError::ParseError(e.to_string()))?;
        Self::new(value)
    }

    /// Returns the inner Decimal value.
    pub fn value(&self) -> Decimal {
        self.0
    }
}

impl Add for DamlDecimal {
    type Output = DamlDecimal;
    fn add(self, rhs: Self) -> Self::Output {
        DamlDecimal(self.0 + rhs.0)
    }
}

impl Sub for DamlDecimal {
    type Output = DamlDecimal;
    fn sub(self, rhs: Self) -> Self::Output {
        DamlDecimal(self.0 - rhs.0)
    }
}

impl Mul for DamlDecimal {
    type Output = DamlDecimal;
    fn mul(self, rhs: Self) -> Self::Output {
        DamlDecimal(
            (self.0 * rhs.0).round_dp_with_strategy(DAML_DECIMAL_SCALE, MidpointNearestEven),
        )
    }
}

impl Div for DamlDecimal {
    type Output = DamlDecimal;
    fn div(self, rhs: Self) -> Self::Output {
        DamlDecimal(
            (self.0 / rhs.0).round_dp_with_strategy(DAML_DECIMAL_SCALE, MidpointNearestEven),
        )
    }
}

impl Neg for DamlDecimal {
    type Output = DamlDecimal;
    fn neg(self) -> Self::Output {
        DamlDecimal(-self.0)
    }
}

impl AddAssign for DamlDecimal {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for DamlDecimal {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl MulAssign for DamlDecimal {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl DivAssign for DamlDecimal {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

impl Serialize for DamlDecimal {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for DamlDecimal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        DamlDecimal::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for DamlDecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for DamlDecimal {
    type Err = DamlDecimalError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_valid_zero_scale() {
        let d = Decimal::from_str("42").unwrap();
        assert!(DamlDecimal::new(d).is_ok());
    }

    #[test]
    fn new_valid_ten_scale() {
        let d = Decimal::from_str("0.0500000000").unwrap();
        assert!(DamlDecimal::new(d).is_ok());
    }

    #[test]
    fn new_invalid_eleven_scale() {
        let d = Decimal::from_str("0.00000000001").unwrap();
        let err = DamlDecimal::new(d).unwrap_err();
        match err {
            DamlDecimalError::InvalidScale { expected, actual } => {
                assert_eq!(expected, 10);
                assert_eq!(actual, 11);
            }
            _ => panic!("expected InvalidScale error"),
        }
    }

    #[test]
    fn parse_valid() {
        let d = DamlDecimal::parse("100.0").unwrap();
        assert_eq!(d.value(), Decimal::from_str("100.0").unwrap());
    }

    #[test]
    fn parse_large() {
        let d = DamlDecimal::parse("40000000000.0000000000").unwrap();
        assert_eq!(
            d.value(),
            Decimal::from_str("40000000000.0000000000").unwrap()
        );
    }

    #[test]
    fn parse_invalid_string() {
        let err = DamlDecimal::parse("not_a_number").unwrap_err();
        match err {
            DamlDecimalError::ParseError(_) => {}
            _ => panic!("expected ParseError"),
        }
    }

    #[test]
    fn parse_excess_scale() {
        let err = DamlDecimal::parse("1.00000000001").unwrap_err();
        match err {
            DamlDecimalError::InvalidScale { expected, actual } => {
                assert_eq!(expected, 10);
                assert_eq!(actual, 11);
            }
            _ => panic!("expected InvalidScale error"),
        }
    }

    #[test]
    fn value_returns_inner() {
        let d = DamlDecimal::parse("3.14").unwrap();
        assert_eq!(d.value(), Decimal::from_str("3.14").unwrap());
    }

    #[test]
    fn add_basic() {
        let a = DamlDecimal::parse("1.5").unwrap();
        let b = DamlDecimal::parse("2.3").unwrap();
        assert_eq!((a + b).value(), Decimal::from_str("3.8").unwrap());
    }

    #[test]
    fn sub_basic() {
        let a = DamlDecimal::parse("5.0").unwrap();
        let b = DamlDecimal::parse("2.3").unwrap();
        assert_eq!((a - b).value(), Decimal::from_str("2.7").unwrap());
    }

    #[test]
    fn mul_rounds_to_10dp() {
        // 0.1234567890 * 0.1234567890 = 0.01524157875019052100
        // Rounded to 10dp with HalfEven: 0.0152415788
        let a = DamlDecimal::parse("0.1234567890").unwrap();
        let result = a * a;
        assert_eq!(
            result.value(),
            Decimal::from_str("0.0152415788").unwrap()
        );
    }

    #[test]
    fn mul_no_rounding_needed() {
        // When the exact result has ≤10dp, rounding is a no-op
        let a = DamlDecimal::parse("0.3333333333").unwrap();
        let b = DamlDecimal::parse("3").unwrap();
        assert_eq!(
            (a * b).value(),
            Decimal::from_str("0.9999999999").unwrap()
        );
    }

    #[test]
    fn div_rounds_to_10dp() {
        // 1 / 3 = 0.3333333333... rounded to 10dp
        let a = DamlDecimal::parse("1").unwrap();
        let b = DamlDecimal::parse("3").unwrap();
        assert_eq!(
            (a / b).value(),
            Decimal::from_str("0.3333333333").unwrap()
        );
    }

    #[test]
    fn neg_preserves_value() {
        let a = DamlDecimal::parse("3.14").unwrap();
        let neg_a = -a;
        assert_eq!(neg_a.value(), Decimal::from_str("-3.14").unwrap());
        assert_eq!((-neg_a).value(), a.value());
    }

    #[test]
    fn add_assign_works() {
        let mut a = DamlDecimal::parse("1.0").unwrap();
        let b = DamlDecimal::parse("2.5").unwrap();
        a += b;
        assert_eq!(a.value(), Decimal::from_str("3.5").unwrap());
    }

    #[test]
    fn sub_assign_works() {
        let mut a = DamlDecimal::parse("5.0").unwrap();
        let b = DamlDecimal::parse("2.5").unwrap();
        a -= b;
        assert_eq!(a.value(), Decimal::from_str("2.5").unwrap());
    }

    #[test]
    fn mul_assign_works() {
        let mut a = DamlDecimal::parse("2.0").unwrap();
        let b = DamlDecimal::parse("3.0").unwrap();
        a *= b;
        assert_eq!(a.value(), Decimal::from_str("6.0").unwrap());
    }

    #[test]
    fn div_assign_works() {
        let mut a = DamlDecimal::parse("6.0").unwrap();
        let b = DamlDecimal::parse("2.0").unwrap();
        a /= b;
        assert_eq!(a.value(), Decimal::from_str("3.0").unwrap());
    }

    #[test]
    fn serde_round_trip() {
        let d = DamlDecimal::parse("3.14").unwrap();
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(json, "\"3.14\"");
        let deserialized: DamlDecimal = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, d);
    }

    #[test]
    fn deserialize_rejects_excess_scale() {
        let result: Result<DamlDecimal, _> =
            serde_json::from_str("\"1.00000000001\"");
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_option_some() {
        let result: Option<DamlDecimal> =
            serde_json::from_str("\"3.14\"").unwrap();
        assert_eq!(result, Some(DamlDecimal::parse("3.14").unwrap()));
    }

    #[test]
    fn deserialize_option_null() {
        let result: Option<DamlDecimal> =
            serde_json::from_str("null").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn display_and_fromstr_round_trip() {
        let d = DamlDecimal::parse("42.1234567890").unwrap();
        let s = d.to_string();
        let parsed: DamlDecimal = s.parse().unwrap();
        assert_eq!(parsed, d);
    }

    #[test]
    fn ordering_works() {
        let a = DamlDecimal::parse("1.0").unwrap();
        let b = DamlDecimal::parse("2.0").unwrap();
        assert!(a < b);
        assert!(b > a);
        assert_eq!(a, a);
    }
}

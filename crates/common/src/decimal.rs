use rust_decimal::Decimal;
use std::fmt;
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

/// Validates that a Decimal has at most 10 decimal places.
pub fn validate_daml_decimal(value: &Decimal) -> Result<(), DamlDecimalError> {
    let scale = value.scale();
    if scale > DAML_DECIMAL_SCALE {
        return Err(DamlDecimalError::InvalidScale {
            expected: DAML_DECIMAL_SCALE,
            actual: scale,
        });
    }
    Ok(())
}

/// Parses a string into a Decimal and validates it has at most 10 decimal places.
pub fn parse_daml_decimal(s: &str) -> Result<Decimal, DamlDecimalError> {
    let value =
        Decimal::from_str(s).map_err(|e| DamlDecimalError::ParseError(e.to_string()))?;
    validate_daml_decimal(&value)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_zero_decimal_places() {
        let val = Decimal::from_str("42").unwrap();
        assert!(validate_daml_decimal(&val).is_ok());
    }

    #[test]
    fn validate_five_decimal_places() {
        let val = Decimal::from_str("3.14159").unwrap();
        assert!(validate_daml_decimal(&val).is_ok());
    }

    #[test]
    fn validate_ten_decimal_places() {
        let val = Decimal::from_str("0.0500000000").unwrap();
        assert!(validate_daml_decimal(&val).is_ok());
    }

    #[test]
    fn validate_eleven_decimal_places_fails() {
        let val = Decimal::from_str("0.00000000001").unwrap();
        let err = validate_daml_decimal(&val).unwrap_err();
        match err {
            DamlDecimalError::InvalidScale { expected, actual } => {
                assert_eq!(expected, 10);
                assert_eq!(actual, 11);
            }
            _ => panic!("expected InvalidScale error"),
        }
    }

    #[test]
    fn parse_valid_decimal_string() {
        let val = parse_daml_decimal("100.0").unwrap();
        assert_eq!(val, Decimal::from_str("100.0").unwrap());
    }

    #[test]
    fn parse_large_decimal_string() {
        let val = parse_daml_decimal("40000000000.0000000000").unwrap();
        assert_eq!(val, Decimal::from_str("40000000000.0000000000").unwrap());
    }

    #[test]
    fn parse_invalid_string_fails() {
        let err = parse_daml_decimal("not_a_number").unwrap_err();
        match err {
            DamlDecimalError::ParseError(_) => {}
            _ => panic!("expected ParseError"),
        }
    }

    #[test]
    fn parse_excess_scale_fails() {
        let err = parse_daml_decimal("1.00000000001").unwrap_err();
        match err {
            DamlDecimalError::InvalidScale { expected, actual } => {
                assert_eq!(expected, 10);
                assert_eq!(actual, 11);
            }
            _ => panic!("expected InvalidScale error"),
        }
    }
}

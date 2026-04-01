# Daml Decimal Support via `rust_decimal`

## Problem

Canton-lib currently represents Daml `Decimal` (aka `Numeric 10`) values inconsistently:

- `ContextValue::Decimal` uses `f64` — subject to floating-point precision loss, unsuitable for financial data
- `Transfer::amount` uses `String` — safe for serialization but provides no arithmetic support
- `wallet/mining_rounds.rs` uses `String` for numerous fields that are `Decimal` in Daml
- The Daml ledger API transmits numeric values as strings with up to 38 significant digits and scale 0-37

The goal is to use a proper fixed-precision decimal type across the codebase, matching Daml's `Decimal` type (10 decimal places).

## Decision

Use `rust_decimal::Decimal` directly — no custom wrapper type. Add validation functions called at system boundaries (parsing input, deserializing API responses, before ledger submission). Validation returns `Result` errors on failure rather than silently rounding.

### Why no wrapper

A wrapper adds type ceremony (conversions, trait forwarding) without meaningful benefit when:
- The underlying type already supports all needed operations (arithmetic, comparison, Display)
- Precision enforcement is a boundary concern, not an invariant on every instance
- The Daml ledger itself validates numeric values on submission

## Design

### Dependency

Add `rust_decimal` to the workspace `Cargo.toml` and to crates that use it:

- `crates/common/Cargo.toml` — runtime dependency (types and validation)
- `crates/wallet/Cargo.toml` — runtime dependency (mining round fields)
- `crates/registry/Cargo.toml` — runtime dependency (`transfer_factory::get()` takes `Transfer` which contains `Decimal`)

```toml
# workspace Cargo.toml [workspace.dependencies]
rust_decimal = { version = "1", features = ["serde-with-str"] }

# crate Cargo.toml [dependencies]
rust_decimal = { workspace = true }
```

The `serde-with-str` feature serializes `Decimal` as a JSON string, matching the Daml JSON API wire format. This applies globally to all `Decimal` fields in crates that depend on `rust_decimal` — which is correct since all Daml numeric values use string encoding.

### Validation module

New file: `crates/common/src/decimal.rs`

```rust
use rust_decimal::Decimal;
use std::fmt;
use std::str::FromStr;

const DAML_DECIMAL_SCALE: u32 = 10;

#[derive(Debug)]
pub enum DamlDecimalError {
    InvalidScale { expected: u32, actual: u32 },
    ParseError(String),
}

impl fmt::Display for DamlDecimalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DamlDecimalError::InvalidScale { expected, actual } => {
                write!(f, "expected at most {} decimal places, got {}", expected, actual)
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
    let value = Decimal::from_str(s).map_err(|e| DamlDecimalError::ParseError(e.to_string()))?;
    validate_daml_decimal(&value)?;
    Ok(value)
}
```

**Note on `scale()`:** `rust_decimal::Decimal::scale()` returns the stored scale, which includes trailing zeros. For example, `Decimal::from_str("1.10")` has `scale() == 2`. This means validation checks the *representation*, not the mathematical value. This is correct behavior — the Daml API sends canonical forms, and a value with >10 decimal digits in its string representation genuinely has too many decimal places.

Re-export from `crates/common/src/lib.rs`:

```rust
pub mod decimal;
```

### Constructing `Decimal` values in code

When creating `Decimal` literals in Rust code (e.g., in tests or hardcoded amounts):

- **Preferred:** `Decimal::from_str("0.02").unwrap()` — explicit, no precision loss
- **Also valid:** `common::decimal::parse_daml_decimal("0.02").unwrap()` — includes validation
- **Avoid:** `Decimal::from_f64(0.02)` — reintroduces the f64 precision issue we're eliminating

### Type changes

#### `ContextValue::Decimal` (`crates/common/src/transfer_factory.rs:33`)

```rust
// Before
Decimal(f64),

// After
Decimal(rust_decimal::Decimal),
```

Wire format changes from `{"tag":"AV_Decimal","value":3.14}` to `{"tag":"AV_Decimal","value":"3.14"}` — this matches what the Daml JSON API expects.

#### `Transfer::amount` (`crates/common/src/transfer.rs:8`)

```rust
// Before
pub amount: String,

// After
pub amount: rust_decimal::Decimal,
```

Serializes as `"amount": "1234.5678"` (string) due to `serde-with-str`.

#### `wallet/mining_rounds.rs` — Daml type mapping

Based on the Daml source (Splice contracts in `splice/daml`), the following fields map to specific Daml types. Fields that are `Decimal` in Daml change from `String` to `rust_decimal::Decimal`. `Int` fields, compound fee types (`FixedFee`, `RatePerRound`, `SteppedRate`), and `Microseconds` are **out of scope** — they remain unchanged.

**`OpenMiningRoundPayload`:**

| Rust field | Daml type | Change |
|------------|-----------|--------|
| `amulet_price` | `Decimal` | `String` → `Decimal` |
| `tick_duration` | `RelTime` (microseconds) | No change — keep `Microseconds` |
| `issuing_for` | `RelTime` (microseconds) | No change — keep `Microseconds` |

**`OpenMiningRoundIssuanceConfig` — all fields are `Decimal` in Daml:**

| Rust field | Daml type | Change |
|------------|-----------|--------|
| `validator_reward_percentage` | `Decimal` | `String` → `Decimal` |
| `unfeatured_app_reward_cap` | `Decimal` | `String` → `Decimal` |
| `app_reward_percentage` | `Decimal` | `String` → `Decimal` |
| `featured_app_reward_cap` | `Decimal` | `String` → `Decimal` |
| `amulet_to_issue_per_year` | `Decimal` | `String` → `Decimal` |
| `validator_reward_cap` | `Decimal` | `String` → `Decimal` |
| `opt_validator_faucet_cap` | `Optional Decimal` | See note below |

**`opt_validator_faucet_cap` serialization:** This is `Optional Decimal` in Daml. The JSON API serializes `Some value` as the decimal string and `None` as `null`. However, in practice this field is always populated with `Some`. The current Rust type is `String`. Change to `Option<Decimal>` with `#[serde(deserialize_with = ...)]` if needed to handle the optional encoding, or keep as `String` if the JSON wire format is ambiguous. **Decision: keep as `String` for now** — changing to `Option<Decimal>` requires verifying the exact JSON encoding for the `None` case, which we haven't observed in real data. Revisit when compound fee types are addressed.

**`OpenMiningRoundTransferConfigUsd`:**

| Rust field | Daml type | Change |
|------------|-----------|--------|
| `extra_featured_app_reward_amount` | `Decimal` | `String` → `Decimal` |
| `max_num_inputs` | `Int` | No change — keep `String` (serialized as string `"100"` in JSON) |
| `max_num_outputs` | `Int` | No change — keep `String` |
| `max_num_lock_holders` | `Int` | No change — keep `String` |
| `holding_fee` | `RatePerRound` (newtype over Decimal) | No change — keep `ExtendedString` |
| `lock_holder_fee` | `FixedFee` (newtype over Decimal) | No change — keep `ExtendedString` |
| `create_fee` | `FixedFee` (newtype over Decimal) | No change — keep `ExtendedString` |
| `transfer_fee` | `SteppedRate` (compound type) | No change — keep as-is |

**`OpenMiningRoundTransferFee`:**

| Rust field | Daml type | Change |
|------------|-----------|--------|
| `initial_rate` | `Decimal` | `String` → `Decimal` |

**`IssuingMiningRoundPayload`:**

| Rust field | Daml type | Change |
|------------|-----------|--------|
| `issuance_per_validator_reward_coupon` | `Decimal` | `String` → `Decimal` |
| `issuance_per_featured_app_reward_coupon` | `Decimal` | `String` → `Decimal` |
| `issuance_per_unfeatured_app_reward_coupon` | `Decimal` | `String` → `Decimal` |
| `issuance_per_sv_reward_coupon` | `Decimal` | `String` → `Decimal` |
| `opt_issuance_per_validator_faucet_coupon` | `Optional Decimal` | Keep as `String` (same rationale as `opt_validator_faucet_cap`) |

### Where validation is called

| Boundary | Location | Function | Purpose |
|----------|----------|----------|---------|
| Parsing API responses | Callers that deserialize JSON containing `Transfer` or `ContextValue` | `parse_daml_decimal(s)` | Validate numeric strings from ledger API |
| User/caller input | Code constructing `Transfer` or `ContextValue::Decimal` from external input | `validate_daml_decimal(&val)` | Reject values with >10 decimal places |
| Before ledger submission | `registry::transfer_factory::get()` — validate `Transfer.amount` and any `ContextValue::Decimal` in the request before sending | `validate_daml_decimal(&val)` | Defense in depth before HTTP POST to registry |

**Error type mapping:** `registry::transfer_factory::get()` returns `Result<..., String>`. Validation errors from `DamlDecimalError` need to be converted via `.to_string()` to match the existing error type.

Internal arithmetic between already-validated `Decimal` values does not need re-validation unless an operation could increase scale beyond 10 (e.g., multiplication).

### What doesn't change

- `ledgrpc` crate — proto definitions stay as-is (`numeric` is `string` in proto)
- Conversion between proto `string` and `Decimal` happens in `common` or `ledger` using `parse_daml_decimal`
- `wallet` compound fee types (`Rate`, `Fee`, `ExtendedString`) — structural changes deferred to a follow-up

### Test updates

#### Existing tests to update

- `crates/common/src/transfer_factory.rs` line 114: change `amount: "100.0".to_string()` to `amount: Decimal::from_str("100.0").unwrap()`
- `crates/common/src/transfer_factory.rs` line 150: change `"value":3.14` to `"value":"3.14"` in test JSON
- `crates/registry/src/transfer_factory.rs` line 105: change `amount: "0.02".to_string()` to `amount: Decimal::from_str("0.02").unwrap()`

#### New tests in `decimal.rs`

- `validate_daml_decimal` with valid values (0, 5, 10 decimal places)
- `validate_daml_decimal` with invalid values (11+ decimal places)
- `parse_daml_decimal` with valid strings
- `parse_daml_decimal` with invalid strings and excess scale

#### Existing wallet test

- `crates/wallet/src/mining_rounds.rs` `test_get_open_mining_rounds_invalid_token` (line 292): deserializes a large inline JSON blob. The `Decimal` fields in this JSON are already strings (e.g., `"0.05"`, `"20000.0"`), so `serde-with-str` deserialization should work without changing the test data. The `Int` fields remain `String` and are unchanged. Verify this test passes after the type changes.

#### New test in `transfer.rs`

- Round-trip serialization: construct a `Transfer` with a `Decimal` amount, serialize to JSON, verify `amount` is a string, deserialize back, verify equality

### Crates affected

- `common` — new `rust_decimal` dependency, new `decimal` module, type changes in `transfer.rs` and `transfer_factory.rs`
- `wallet` — new `rust_decimal` dependency, type changes in `mining_rounds.rs` (Decimal fields and Int fields per mapping above)
- `registry` — new `rust_decimal` runtime dependency (`transfer_factory::get()` takes `Transfer` containing `Decimal`); add pre-submission validation in `transfer_factory::get()`
- `ledger` — no changes needed (does not reference decimal amounts in Rust source)
- `examples` — no Rust source changes needed (README references are documentation only)

### Out of scope (follow-up)

- Wallet compound fee types: `FixedFee`, `RatePerRound`, `SteppedRate` are currently `ExtendedString`. Replacing these with proper Rust structs mirroring the Daml newtypes requires structural changes beyond a type swap.
- `Optional Decimal` fields (`opt_validator_faucet_cap`, `opt_issuance_per_validator_faucet_coupon`): need to verify the `None` case JSON encoding before changing from `String`.
- `Int` fields serialized as JSON strings (`max_num_inputs`, `max_num_outputs`, `max_num_lock_holders`): changing to `i64` requires a custom serde deserializer since the wire format is `"100"` not `100`.
- `Microseconds` / `RelTime` fields (`tick_duration`, `issuing_for`): currently `ExtendedString`, structural change needed.

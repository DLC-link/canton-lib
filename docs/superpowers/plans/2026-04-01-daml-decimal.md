# Daml Decimal Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `f64` and `String` representations of Daml `Decimal` (Numeric 10) values with `rust_decimal::Decimal` across the codebase, with boundary validation functions.

**Architecture:** Add `rust_decimal` as a workspace dependency with `serde-with-str` (serializes as JSON string). Create a validation module in the `common` crate. Change type annotations in `common`, `wallet`, and `registry` crates. All Daml Decimal fields serialize/deserialize as JSON strings — no wire format change for fields already using strings.

**Tech Stack:** Rust, `rust_decimal` crate (v1, `serde-with-str` feature), `serde`

**Spec:** `docs/superpowers/specs/2026-04-01-daml-decimal-design.md`

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `Cargo.toml` | Add `rust_decimal` to workspace dependencies |
| Modify | `crates/common/Cargo.toml` | Add `rust_decimal` dependency |
| Create | `crates/common/src/decimal.rs` | Validation functions and error type |
| Modify | `crates/common/src/lib.rs` | Re-export `decimal` module |
| Modify | `crates/common/src/transfer.rs` | Change `amount: String` → `Decimal` |
| Modify | `crates/common/src/transfer_factory.rs` | Change `Decimal(f64)` → `Decimal(Decimal)` |
| Modify | `crates/wallet/Cargo.toml` | Add `rust_decimal` dependency |
| Modify | `crates/wallet/src/mining_rounds.rs` | Change Decimal fields from `String` → `Decimal` |
| Modify | `crates/registry/Cargo.toml` | Add `rust_decimal` dependency |
| Modify | `crates/registry/src/transfer_factory.rs` | Add pre-submission validation, update test |

---

### Task 1: Add `rust_decimal` workspace dependency

**Files:**
- Modify: `Cargo.toml:26-38` (workspace dependencies)
- Modify: `crates/common/Cargo.toml:7-10` (dependencies)

- [ ] **Step 1: Add `rust_decimal` to workspace `Cargo.toml`**

In `Cargo.toml`, add to the `[workspace.dependencies]` section:

```toml
rust_decimal = { version = "1", features = ["serde-with-str"] }
```

- [ ] **Step 2: Add `rust_decimal` to `crates/common/Cargo.toml`**

In `crates/common/Cargo.toml`, add to `[dependencies]`:

```toml
rust_decimal = { workspace = true }
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p common`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/common/Cargo.toml Cargo.lock
git commit -m "feat: add rust_decimal workspace dependency"
```

---

### Task 2: Create validation module

**Files:**
- Create: `crates/common/src/decimal.rs`
- Modify: `crates/common/src/lib.rs`

- [ ] **Step 1: Write the tests for `decimal.rs`**

Create `crates/common/src/decimal.rs` with the module and tests only:

```rust
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
    todo!()
}

/// Parses a string into a Decimal and validates it has at most 10 decimal places.
pub fn parse_daml_decimal(s: &str) -> Result<Decimal, DamlDecimalError> {
    todo!()
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
```

- [ ] **Step 2: Add module to `lib.rs`**

In `crates/common/src/lib.rs`, add:

```rust
pub mod decimal;
```

The full file becomes:

```rust
pub mod accept;
pub mod consts;
pub mod decimal;
pub mod filters;
pub mod submission;
pub mod transfer;
pub mod transfer_factory;
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p common decimal`
Expected: FAIL — `todo!()` panics

- [ ] **Step 4: Implement the validation functions**

Replace the `todo!()` bodies in `crates/common/src/decimal.rs`:

```rust
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
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p common decimal`
Expected: all 8 tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/common/src/decimal.rs crates/common/src/lib.rs
git commit -m "feat: add Daml decimal validation module"
```

---

### Task 3: Change `Transfer::amount` and `ContextValue::Decimal` types

These two changes must happen together because `transfer_factory.rs` constructs `Transfer` with `amount` — changing one without the other breaks compilation.

**Files:**
- Modify: `crates/common/src/transfer.rs:8`
- Modify: `crates/common/src/transfer_factory.rs:33,114,150`

- [ ] **Step 1: Change `Transfer::amount` from `String` to `Decimal`**

In `crates/common/src/transfer.rs`, change line 8:

```rust
// Before
    pub amount: String,

// After
    pub amount: rust_decimal::Decimal,
```

- [ ] **Step 2: Change `ContextValue::Decimal` from `f64` to `Decimal`**

In `crates/common/src/transfer_factory.rs`, change line 33:

```rust
// Before
    Decimal(f64),

// After
    Decimal(rust_decimal::Decimal),
```

- [ ] **Step 3: Fix the `test_choice_arguments_serialization` test**

In `crates/common/src/transfer_factory.rs`, add the import at the top of the `tests` module (after `use super::*;`):

```rust
use std::str::FromStr;
```

Then change line 114:

```rust
// Before
                amount: "100.0".to_string(),

// After
                amount: rust_decimal::Decimal::from_str("100.0").unwrap(),
```

- [ ] **Step 4: Fix the `test_context_deserialization_all_variants` test**

In `crates/common/src/transfer_factory.rs`, change the decimal value in the test JSON at line 150:

```rust
// Before
            "decimal-field":{"tag":"AV_Decimal","value":3.14},

// After
            "decimal-field":{"tag":"AV_Decimal","value":"3.14"},
```

Also add an assertion for the decimal variant after line 163 (after the `party-field` assertion):

```rust
        assert_eq!(
            ctx.values.get("decimal-field"),
            Some(&ContextValue::Decimal(rust_decimal::Decimal::from_str("3.14").unwrap()))
        );
```

This requires adding `use std::str::FromStr;` to the test module imports if not already added in Step 3 (it's the same module, so the import from Step 3 covers this).

- [ ] **Step 5: Add round-trip serialization test for `Transfer`**

Add to the bottom of `crates/common/src/transfer.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn transfer_amount_serializes_as_string() {
        let transfer = Transfer {
            sender: "sender1".to_string(),
            receiver: "receiver1".to_string(),
            amount: Decimal::from_str("0.02").unwrap(),
            instrument_id: InstrumentId {
                admin: "admin1".to_string(),
                id: "CBTC".to_string(),
            },
            requested_at: "2024-01-01T00:00:00Z".to_string(),
            execute_before: "2024-12-31T23:59:59Z".to_string(),
            input_holding_cids: None,
            meta: None,
        };

        let json = serde_json::to_value(&transfer).unwrap();
        // amount must be a JSON string, not a number
        assert_eq!(json["amount"], serde_json::Value::String("0.02".to_string()));

        // round-trip
        let json_str = serde_json::to_string(&transfer).unwrap();
        let deserialized: Transfer = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.amount, Decimal::from_str("0.02").unwrap());
    }
}
```

- [ ] **Step 6: Run all common tests**

Run: `cargo test -p common`
Expected: all tests PASS (decimal, transfer, and transfer_factory tests)

- [ ] **Step 7: Commit**

```bash
git add crates/common/src/transfer.rs crates/common/src/transfer_factory.rs
git commit -m "feat: change Transfer::amount and ContextValue::Decimal to rust_decimal::Decimal"
```

---

### Task 4: Change wallet `mining_rounds.rs` Decimal fields

**Files:**
- Modify: `crates/wallet/Cargo.toml`
- Modify: `crates/wallet/src/mining_rounds.rs:86,107,110,113,116,119,122,134,158,200,206,212,218`

- [ ] **Step 1: Add `rust_decimal` dependency to wallet**

In `crates/wallet/Cargo.toml`, add to `[dependencies]`:

```toml
rust_decimal = { workspace = true }
```

- [ ] **Step 2: Change `OpenMiningRoundPayload::amulet_price`**

In `crates/wallet/src/mining_rounds.rs`, change line 86:

```rust
// Before
    pub amulet_price: String,

// After
    pub amulet_price: rust_decimal::Decimal,
```

- [ ] **Step 3: Change `OpenMiningRoundIssuanceConfig` Decimal fields**

In `crates/wallet/src/mining_rounds.rs`, change lines 107, 110, 113, 116, 119, 122:

```rust
// Before (lines 107, 110, 113, 116, 119, 122)
    pub validator_reward_percentage: String,
    pub unfeatured_app_reward_cap: String,
    pub app_reward_percentage: String,
    pub featured_app_reward_cap: String,
    pub amulet_to_issue_per_year: String,
    pub validator_reward_cap: String,

// After
    pub validator_reward_percentage: rust_decimal::Decimal,
    pub unfeatured_app_reward_cap: rust_decimal::Decimal,
    pub app_reward_percentage: rust_decimal::Decimal,
    pub featured_app_reward_cap: rust_decimal::Decimal,
    pub amulet_to_issue_per_year: rust_decimal::Decimal,
    pub validator_reward_cap: rust_decimal::Decimal,
```

Note: `opt_validator_faucet_cap` (line 125) stays as `String` — it's `Optional Decimal` in Daml and needs separate handling.

- [ ] **Step 4: Change `OpenMiningRoundTransferConfigUsd::extra_featured_app_reward_amount`**

In `crates/wallet/src/mining_rounds.rs`, change line 134:

```rust
// Before
    pub extra_featured_app_reward_amount: String,

// After
    pub extra_featured_app_reward_amount: rust_decimal::Decimal,
```

Note: `max_num_inputs` (137), `max_num_lock_holders` (146), `max_num_outputs` (152) stay as `String` — they're `Int` in Daml but serialized as JSON strings.

- [ ] **Step 5: Change `OpenMiningRoundTransferFee::initial_rate`**

In `crates/wallet/src/mining_rounds.rs`, change line 158:

```rust
// Before
    pub initial_rate: String,

// After
    pub initial_rate: rust_decimal::Decimal,
```

- [ ] **Step 6: Change `IssuingMiningRoundPayload` Decimal fields**

In `crates/wallet/src/mining_rounds.rs`, change lines 200, 206, 212, 218:

```rust
// Before (lines 200, 206, 212, 218)
    pub issuance_per_featured_app_reward_coupon: String,
    pub issuance_per_sv_reward_coupon: String,
    pub issuance_per_unfeatured_app_reward_coupon: String,
    pub issuance_per_validator_reward_coupon: String,

// After
    pub issuance_per_featured_app_reward_coupon: rust_decimal::Decimal,
    pub issuance_per_sv_reward_coupon: rust_decimal::Decimal,
    pub issuance_per_unfeatured_app_reward_coupon: rust_decimal::Decimal,
    pub issuance_per_validator_reward_coupon: rust_decimal::Decimal,
```

Note: `opt_issuance_per_validator_faucet_coupon` (line 197) stays as `String` — same rationale as `opt_validator_faucet_cap`.

- [ ] **Step 7: Run the existing wallet deserialization test**

Run: `cargo test -p wallet test_get_open_mining_rounds_invalid_token`
Expected: PASS — all Decimal fields in the test JSON are already strings (e.g., `"0.05"`, `"20000.0"`), so `serde-with-str` deserializes them correctly. The `Int` and `Optional Decimal` fields remain `String` and are unchanged.

- [ ] **Step 8: Commit**

```bash
git add crates/wallet/Cargo.toml crates/wallet/src/mining_rounds.rs
git commit -m "feat: change wallet mining_rounds Decimal fields from String to Decimal"
```

---

### Task 5: Add `rust_decimal` to registry and update test

The `registry` crate needs `rust_decimal` as a runtime dependency because its public function `transfer_factory::get()` takes `Params` containing `common::transfer_factory::ChoiceArguments`, which contains `common::transfer::Transfer` — and `Transfer::amount` is now `rust_decimal::Decimal`. The compiler needs `rust_decimal` to resolve this type.

**Files:**
- Modify: `crates/registry/Cargo.toml`
- Modify: `crates/registry/src/transfer_factory.rs:56,105`

- [ ] **Step 1: Add `rust_decimal` dependency to registry**

In `crates/registry/Cargo.toml`, add to `[dependencies]`:

```toml
rust_decimal = { workspace = true }
```

- [ ] **Step 2: Update the test `Transfer` construction**

In `crates/registry/src/transfer_factory.rs`, add the import in the test module (after `use std::env;`):

```rust
use std::str::FromStr;
```

Then change line 105:

```rust
// Before
                        amount: "0.02".to_string(),

// After
                        amount: rust_decimal::Decimal::from_str("0.02").unwrap(),
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p registry`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add crates/registry/Cargo.toml crates/registry/src/transfer_factory.rs
git commit -m "feat: add rust_decimal to registry, update transfer factory test"
```

---

### Task 6: Add pre-submission validation in registry

**Files:**
- Modify: `crates/registry/src/transfer_factory.rs:17-47`

- [ ] **Step 1: Add validation before HTTP POST**

In `crates/registry/src/transfer_factory.rs`, add validation at the start of the `get` function (after line 17, before constructing the URL):

```rust
pub async fn get(params: Params) -> Result<common::transfer_factory::Response, String> {
    // Validate decimal fields before submission
    common::decimal::validate_daml_decimal(&params.request.choice_arguments.transfer.amount)
        .map_err(|e| format!("Invalid transfer amount: {}", e))?;

    let client = reqwest::Client::new();
    // ... rest of the function unchanged
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p registry`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add crates/registry/src/transfer_factory.rs
git commit -m "feat: add pre-submission decimal validation in registry"
```

---

### Task 7: Full workspace build and test

- [ ] **Step 1: Run full workspace check**

Run: `cargo check --workspace`
Expected: compiles with no errors

- [ ] **Step 2: Run unit tests only (exclude integration tests)**

Run: `cargo test --workspace -- --skip test_transfer_factory --skip test_get_amulet_rules --skip test_get_open_mining_rounds --skip test_get_accept_context`

The skipped tests are integration tests that require environment variables (Keycloak credentials, API hosts) and will panic if not configured. They are:
- `registry::transfer_factory::tests::test_transfer_factory`
- `wallet::amulet_rules::tests::test_get_amulet_rules_integration`
- `wallet::mining_rounds::tests::test_get_open_mining_rounds`
- `registry::accept_context::tests::test_get_accept_context`

Expected: all non-integration tests PASS

- [ ] **Step 3: Commit any remaining fixes**

If any compilation or test issues were found and fixed, commit them:

```bash
git add -p
git commit -m "fix: resolve remaining compilation issues from decimal migration"
```

If no fixes needed, skip this step.

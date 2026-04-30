# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `ledger::submit::wait_for_transaction(Params)` — calls the flat `POST /v2/commands/submit-and-wait-for-transaction` JSON Ledger API endpoint. When `Submission::transaction_format` is unset, builds a default `TransactionFormat { transactionShape: TRANSACTION_SHAPE_LEDGER_EFFECTS, eventFormat: { verbose: true, filtersByParty: <actAs ∪ readAs> → {} } }` to preserve the behavior the deprecated tree endpoint hardcoded server-side. When the field is set, the caller's value is used verbatim and is moved to the top-level request body so it is not double-nested inside `commands`.

### Changed

- **Deprecated:** `ledger::submit::wait_for_transaction_tree` now forwards to `wait_for_transaction` and emits a deprecation warning. The underlying `submit-and-wait-for-transaction-tree` endpoint is removed in Canton 3.5.0; callers should migrate to `wait_for_transaction`. Note the response body is now the flat `JsSubmitAndWaitForTransactionResponse` shape (`transaction.events: Event[]`) rather than the tree shape (`transactionTree.eventsById`) — any downstream code that parsed the returned `String` must be updated.
- README "Submit Commands" curl example updated to the flat endpoint and shows the required `transactionFormat` block.

## [0.4.0] - 2026-04-07

### Added

- `common::decimal::DamlDecimal` newtype wrapper for Daml `Decimal` (Numeric 10) values
  - Validates ≤10 decimal places at construction time
  - Arithmetic (`+`, `-`, `*`, `/`, negation, `+=`, `-=`, `*=`, `/=`) with banker's rounding (HalfEven) on `*` and `/`, matching Daml's behavior
  - Custom serde: serializes as JSON string, deserializes with validation
  - `Display`, `FromStr`, `Sum`, `Copy`, `Eq`, `Ord`, `Hash`
  - `DamlDecimal::ZERO` constant
- `common::decimal::DamlDecimalError` error type for construction/parse failures
- `rust-version = "1.85"` in workspace manifest (required by `edition = "2024"`)

### Changed

- **Breaking:** `Transfer::amount` changed from `String` to `DamlDecimal`

  ```rust
  // Before
  amount: "0.02".to_string(),

  // After
  use common::decimal::DamlDecimal;
  amount: DamlDecimal::parse("0.02").unwrap(),
  ```

- **Breaking:** `ContextValue::Decimal` changed from `f64` to `DamlDecimal`

  ```rust
  // Before
  ContextValue::Decimal(3.14),

  // After
  ContextValue::Decimal(DamlDecimal::parse("3.14").unwrap()),
  ```

- **Breaking:** Wallet `mining_rounds` fields changed from `String` to `DamlDecimal`:
  `amulet_price`, `validator_reward_percentage`, `unfeatured_app_reward_cap`,
  `app_reward_percentage`, `featured_app_reward_cap`, `amulet_to_issue_per_year`,
  `validator_reward_cap`, `extra_featured_app_reward_amount`, `initial_rate`,
  `issuance_per_featured_app_reward_coupon`, `issuance_per_sv_reward_coupon`,
  `issuance_per_unfeatured_app_reward_coupon`, `issuance_per_validator_reward_coupon`

- **Breaking:** Wallet `mining_rounds` Optional Decimal fields changed from `String` to `Option<DamlDecimal>`:
  `opt_validator_faucet_cap`, `opt_issuance_per_validator_faucet_coupon`

- **Breaking:** `wallet` crate now depends on `common` instead of `rust_decimal` directly.
  If you were using `rust_decimal::Decimal` for wallet field types, switch to `common::decimal::DamlDecimal`.

- `rust_decimal` workspace dependency: disabled default features, keeping only `std` and `serde-with-str`

### Removed

- `common::decimal::validate_daml_decimal()` — replaced by `DamlDecimal::new()`
- `common::decimal::validate_context_value()` — no longer needed (type system enforces validity)
- `common::decimal::validate_submission()` — no longer needed (type system enforces validity)
- `common::decimal::parse_daml_decimal()` — replaced by `DamlDecimal::parse()`
- `log` dependency from `common` crate and workspace

## [0.3.1]

### Fixed

- Minor fixes

## [0.3.0]

### Changed

- **Breaking:** `common::transfer::DisclosedContract::template_id` changed from `String` to `Option<String>`

## [0.2.0]

### Changed

- **Breaking:** `common::submission::Submission` gained extra fields (use `..Default::default()`)

## [0.1.0]

### Added

- Initial release

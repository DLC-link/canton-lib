# Canton Rust Library

A Rust library for interacting with the Canton blockchain.

---

## Table of Contents

1. [Overview](#overview)
2. [Installation](#installation)
3. [Configuration](#configuration)
4. [Usage Examples](#usage-examples)
5. [API Reference](#api-reference)
6. [Direct Canton API Usage](#direct-canton-api-usage-reference)
7. [Contributing](#contributing)

---

## Overview

This library provides a Rust interface for interacting with Canton blockchain participant nodes. It handles authentication, ledger queries, and contract management.

### Crates

| Crate | Description |
|-------|-------------|
| `ledger` | Low-level Canton Ledger API client (active contracts, submissions, WebSocket streaming) |
| `keycloak` | Keycloak/OIDC authentication (password grant, client credentials, token refresh) |
| `registry` | Canton registry service integration |
| `common` | Common types for transfers and contract operations |
| `wallet` | Wallet operations (amulet rules, mining rounds) |
| `cryptography` | Cryptographic utilities (AES-256-GCM) |

### Prerequisites

Before using this library, you need:

1. **A Canton Participant Node** - Access to a Canton participant node (devnet, testnet, or mainnet)
2. **Keycloak Credentials** - Authentication credentials for your participant node
3. **A Party ID** - Your unique party identifier on the Canton network

---

## Installation

Add the crates you need to your `Cargo.toml`:

```toml
[dependencies]
ledger = { git = "ssh://git@github.com/DLC-link/canton-rs", branch = "main" }
keycloak = { git = "ssh://git@github.com/DLC-link/canton-rs", branch = "main" }
registry = { git = "ssh://git@github.com/DLC-link/canton-rs", branch = "main" }
common = { git = "ssh://git@github.com/DLC-link/canton-rs", branch = "main" }
wallet = { git = "ssh://git@github.com/DLC-link/canton-rs", branch = "main" }
```

---

## Configuration

### Setup

1. **Create environment configuration**

Copy `.env.example` to `.env` and fill in your values:

```bash
cp .env.example .env
```

2. **Configure your environment variables**

Edit `.env` with your Canton participant node details:

```bash
# Authentication
KEYCLOAK_HOST=https://keycloak.example.com
KEYCLOAK_REALM=your-realm
KEYCLOAK_CLIENT_ID=your-client-id
KEYCLOAK_USERNAME=your-username
KEYCLOAK_PASSWORD=your-password

# Canton Network
LEDGER_HOST=https://participant.example.com
PARTY_ID=your-party::1220...
```

---

## Usage Examples

### Authentication

```rust
use keycloak::login;

// Password grant authentication
let auth = login::password(login::PasswordParams {
    client_id: "your-client-id".to_string(),
    username: "your-username".to_string(),
    password: "your-password".to_string(),
    url: login::password_url("https://your-keycloak-host", "your-realm"),
}).await?;

// Use auth.access_token for subsequent API calls
```

### Query Active Contracts

```rust
use ledger::{ledger_end, websocket::active_contracts, common};

// Get ledger end offset
let ledger_end = ledger_end::get(ledger_end::Params {
    access_token: auth.access_token.clone(),
    ledger_host: "https://participant.example.com".to_string(),
}).await?;

// Query contracts by template ID
let contracts = active_contracts::get(active_contracts::Params {
    ledger_host: "https://participant.example.com".to_string(),
    party: "your-party::1220...".to_string(),
    filter: common::IdentifierFilter::TemplateIdentifierFilter(
        common::TemplateIdentifierFilter {
            template_filter: common::TemplateFilter {
                value: common::TemplateFilterValue {
                    template_id: Some("package:Module:Template".to_string()),
                    include_created_event_blob: true,
                },
            },
        },
    ),
    access_token: auth.access_token,
    ledger_end: ledger_end.offset,
}).await?;
```

### Running Examples

The `examples` crate includes ready-to-run programs:

```bash
# List contracts by template ID
cargo run -p examples --bin list_contracts -- "splice-amulet-0.1.10:Splice.Amulet:Amulet"

# Delete executed transfer contracts
cargo run -p examples --bin delete_executed_transfers
```

---

## API Reference

### `keycloak::login`

- `password(PasswordParams)` - Authenticate with username/password
- `client_credentials(ClientCredentialsParams)` - Service account authentication
- `password_url(host, realm)` - Build password grant URL

### `ledger`

- `ledger_end::get(Params)` - Get current ledger offset
- `active_contracts::get(Params)` - Query active contracts (REST)
- `websocket::active_contracts::get(Params)` - Query active contracts (WebSocket)
- `websocket::update::stream(Params)` - Stream ledger updates
- `submit::submit(Params)` - Submit commands to the ledger

### `registry`

- `transfer_factory::get(Params)` - Get transfer factory disclosures
- `accept_context::get(Params)` - Get accept choice context

### `common`

- Types for transfer operations, submissions, and contract filters

### `wallet`

- `amulet_rules` - Amulet rules queries
- `mining_rounds` - Mining round operations

---

## Direct Canton API Usage (Reference)

For teams who want to understand the underlying protocol or implement custom workflows, here's how to interact with Canton APIs directly.

### Get Ledger End

```bash
curl -X GET "$LEDGER_HOST/v2/state/ledger-end" \
  -H "Authorization: Bearer $ACCESS_TOKEN"
```

### Get Active Contracts

```bash
LEDGER_OFFSET=$(curl -X GET "$LEDGER_HOST/v2/state/ledger-end" \
  -H "Authorization: Bearer $ACCESS_TOKEN" | jq -r '.offset')

curl -X POST $LEDGER_HOST/v2/state/active-contracts \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -d '{
    "filter": {
      "filtersByParty": {
        "'$PARTY_ID'": {
          "cumulative": [{
            "identifierFilter": {
              "TemplateFilter": {
                "value": {
                  "templateId": "package:Module:Template",
                  "includeCreatedEventBlob": true
                }
              }
            }
          }]
        }
      }
    },
    "verbose": false,
    "activeAtOffset": '$LEDGER_OFFSET'
  }'
```

### Submit Commands

```bash
curl -X POST $LEDGER_HOST/v2/commands/submit-and-wait-for-transaction-tree \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "commands": [{
      "ExerciseCommand": {
        "templateId": "package:Module:Template",
        "contractId": "'$CONTRACT_ID'",
        "choice": "ChoiceName",
        "choiceArgument": {}
      }
    }],
    "commandId": "'$(uuidgen)'",
    "actAs": ["'$PARTY_ID'"]
  }'
```

---

## Contributing

Contributions are welcome! This library is designed to help developers build on Canton.

### How to Contribute

1. **Fork the repository** and create a feature branch

   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**

   - Follow Rust best practices and naming conventions
   - Keep library code free of environment variable dependencies
   - Add tests for new functionality (when applicable)

3. **Test your changes**

   ```bash
   # Build the library
   cargo build --release

   # Run clippy for linting
   cargo clippy --all-targets --all-features

   # Format code
   cargo fmt --all
   ```

4. **Submit a pull request**

### Development Guidelines

- **Library code** should accept all configuration as function parameters (no `env::var()` calls)
- **Example code** can read from environment variables to demonstrate usage

---

## License

MIT License - see [LICENSE](LICENSE) file for details

## Resources

- [Canton Documentation](https://docs.digitalasset.com/)
- [Canton Network](https://www.canton.network/)
- [Participant JSON Ledger API](https://docs.digitalasset.com/build/3.3/reference/json-api/openapi.html)
- [Participant JSON Ledger AsyncAPI](https://docs.digitalasset.com/build/3.3/reference/json-api/asyncapi.html)
- [Participant gRPC](https://docs.digitalasset.com/build/3.3/reference/lapi-proto-docs.html)

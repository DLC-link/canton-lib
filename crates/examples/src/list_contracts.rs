/// Example: List active contracts by template ID
///
/// This example demonstrates how to query active contracts from the ledger
/// filtered by a specific template ID.
///
/// Run with: cargo run -p examples --bin list_contracts -- <TEMPLATE_ID>
///
/// Required environment variables:
/// - KEYCLOAK_HOST, KEYCLOAK_REALM, KEYCLOAK_CLIENT_ID
/// - KEYCLOAK_USERNAME, KEYCLOAK_PASSWORD
/// - LEDGER_HOST, PARTY_ID
use std::env;

#[tokio::main]
async fn main() -> Result<(), String> {
    dotenvy::dotenv().ok();
    env_logger::init();

    // Get template_id from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run -p examples --bin list_contracts -- <TEMPLATE_ID>");
        eprintln!("\nExample:");
        eprintln!(
            "  cargo run -p examples --bin list_contracts -- \"splice-amulet-0.1.10:Splice.Amulet:Amulet\""
        );
        std::process::exit(1);
    }
    let template_id = &args[1];

    // Authenticate
    println!("Authenticating...");
    let login_params = keycloak::login::PasswordParams {
        client_id: env::var("KEYCLOAK_CLIENT_ID").expect("KEYCLOAK_CLIENT_ID must be set"),
        username: env::var("KEYCLOAK_USERNAME").expect("KEYCLOAK_USERNAME must be set"),
        password: env::var("KEYCLOAK_PASSWORD").expect("KEYCLOAK_PASSWORD must be set"),
        url: keycloak::login::password_url(
            &env::var("KEYCLOAK_HOST").expect("KEYCLOAK_HOST must be set"),
            &env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set"),
        ),
    };

    let auth = keycloak::login::password(login_params)
        .await
        .map_err(|e| format!("Authentication failed: {}", e))?;

    let party = env::var("PARTY_ID").expect("PARTY_ID must be set");
    let ledger_host = env::var("LEDGER_HOST").expect("LEDGER_HOST must be set");

    println!("\nQuerying contracts for template: {}", template_id);
    println!("Party: {}", party);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Get ledger end
    let ledger_end_result = ledger::ledger_end::get(ledger::ledger_end::Params {
        access_token: auth.access_token.clone(),
        ledger_host: ledger_host.clone(),
    })
    .await?;

    // Query active contracts with template filter
    let contracts =
        ledger::websocket::active_contracts::get(ledger::websocket::active_contracts::Params {
            ledger_host,
            party: party.clone(),
            filter: ledger::common::IdentifierFilter::TemplateIdentifierFilter(
                ledger::common::TemplateIdentifierFilter {
                    template_filter: ledger::common::TemplateFilter {
                        value: ledger::common::TemplateFilterValue {
                            template_id: Some(template_id.clone()),
                            include_created_event_blob: true,
                        },
                    },
                },
            ),
            access_token: auth.access_token,
            ledger_end: ledger_end_result.offset,
        })
        .await?;

    // Display results
    println!("Found {} contracts\n", contracts.len());

    for (i, contract) in contracts.iter().enumerate() {
        let contract_id = &contract.created_event.contract_id;
        let short_id = if contract_id.len() > 20 {
            format!(
                "{}...{}",
                &contract_id[..10],
                &contract_id[contract_id.len() - 10..]
            )
        } else {
            contract_id.clone()
        };

        println!("{}. Contract ID: {}", i + 1, short_id);
        println!("   Full ID: {}", contract_id);

        // Print create_argument if available
        if let Some(Some(create_arg)) = &contract.created_event.create_argument {
            let pretty = serde_json::to_string_pretty(create_arg).unwrap_or_default();
            println!("   Create Argument: {}", pretty);
        }

        println!();
    }

    // Summary
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Total contracts found: {}", contracts.len());

    Ok(())
}

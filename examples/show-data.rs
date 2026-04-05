// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,tmr_client=info,show_data=info".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let montrose = tmr_client::TmrClient::new("TMR Client Sample");
    let montrose = montrose.connect().await?;

    println!("-------- Accounts --------");
    let accounts = montrose.get_user_accounts().await?;
    for account in &accounts {
        println!(
            "Account: {} {} {}",
            account.account_id,
            account.account_number,
            account.account_name.as_deref().unwrap_or("")
        );
    }

    println!();
    println!("-------- Holdings --------");
    let holdings = montrose.get_holdings(None).await?;
    for holding in holdings {
        println!(
            "Account: {} {} {} {}",
            holding.account_id, holding.account_number, holding.account_name, holding.currency
        );
        println!(
            "  Total market value: {}\n  Available for purchase: {}\n  Total value: {}\n  Currency: {}",
            holding.summary.total_market_value,
            holding.summary.available_for_purchase,
            holding.summary.total_value,
            holding.summary.currency
        );
        let mut positions = holding.positions;
        positions.sort_by(|a, b| a.instrument_name.cmp(&b.instrument_name));
        for position in positions {
            println!("  Position: {}", position.instrument_name);
            println!(
                "    Order book ID: {}\n    Ticker: {}\n    Quantity: {}",
                position.orderbook_id, position.ticker, position.quantity,
            );
        }
    }

    Ok(())
}

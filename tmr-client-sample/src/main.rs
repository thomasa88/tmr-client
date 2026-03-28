// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

use anyhow::Context;
use tmr_client::Decimal;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let client = tmr_client::TmrClient::new();
    let client = client.connect().await?;

    // let accounts = tmr_client.get_user_accounts().await?;
    let accounts = dbg!(client.get_holdings(None).await?);

    // dbg!(tmr_client.get_holdings(None).await?);
    // let accounts = tmr_client.get_user_accounts().await?;
    // dbg!(&accounts);

    // client
    //     .create_trade_ticket(tmr_client::tools::TradeTicketArgs {
    //         side: tmr_client::tools::Side::Buy,
    //         account_id: Some(accounts.get(0).context("No accounts")?.account_id),
    //         amount_sek: Some(Decimal::new(1, 0)),
    //         // ticker: Some("SB GLOB A SEK".to_string()),
    //         orderbook_id: Some(3361), // LF GLOB
    //         ..Default::default()
    //     })
    //     .await?;

    // tmr_client.client.cancel().await?;

    Ok(())
}

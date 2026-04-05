// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

use anyhow::Context;
use tmr_client::{
    Decimal,
    tools::{TradeInstrument, TradeSide, TradeSize, TradeTicketArgs},
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,tmr_client=info,create_trade=info".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let montrose = tmr_client::TmrClient::new("TMR Client Sample");
    let montrose = montrose.connect().await?;

    let accounts = montrose.get_user_accounts().await?;
    let trade_account = accounts.get(0).context("No accounts")?;

    info!(
        "Using account {}",
        trade_account
            .account_name
            .as_deref()
            .unwrap_or(&trade_account.account_number)
    );

    info!("Creating trade ticket...");
    let trade_url = montrose
        .create_trade_ticket(TradeTicketArgs {
            side: TradeSide::Buy,
            account_id: Some(trade_account.account_id),
            size: TradeSize::Amount(Decimal::new(100, 0)),
            // instrument: TradeInstrument::Ticker("LF GLOB".to_string()),
            instrument: TradeInstrument::OrderbookId(3361),
            price: None,
        })
        .await?;
    info!("Trade ticket URL: {}", trade_url);

    Ok(())
}

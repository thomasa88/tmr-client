// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

use anyhow::Context;
use tmr_client::{Decimal, tools::{TradeInstrument, TradeSide, TradeSize, TradeTicketArgs}};
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

    let client = tmr_client::TmrClient::new("TMR Client Sample");
    let client = client.connect().await?;

    // let accounts = tmr_client.get_user_accounts().await?;
    let accounts = client.get_holdings(None).await?;
    println!("{} accounts", accounts.len());

    // dbg!(tmr_client.get_holdings(None).await?);
    // let accounts = tmr_client.get_user_accounts().await?;
    // dbg!(&accounts);

    // client
    //     .create_trade_ticket(TradeTicketArgs {
    //         side: TradeSide::Buy,
    //         account_id: Some(accounts.get(0).context("No accounts")?.account_id),
    //         size: TradeSize::Amount(Decimal::new(1, 0)),
    //         // instrument: TradeInstrument::Ticker("SB GLOB A SEK".to_string()),
    //         instrument: TradeInstrument::OrderbookId(3361),
    //         price: None,
    //     })
    //     .await?;

    // tmr_client.client.cancel().await?;

    Ok(())
}

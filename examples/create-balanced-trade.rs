// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

use std::{
    fmt::Display,
    io::{Read, Write},
};

use anyhow::{Context, ensure};
use rust_decimal::dec;
use tmr_client::{
    Decimal,
    tools::{TradeInstrument, TradeSide, TradeSize, TradeTicketArgs},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct Trade {
    ticker: String,
    percentage: Decimal,
    amount_sek: Option<Decimal>,
}

impl Display for Trade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:20} {:>3}% ({:>10.2} SEK)",
            self.ticker,
            self.percentage,
            self.amount_sek.unwrap_or(Decimal::ZERO)
        )
    }
}

fn usage() {
    println!(
        "Usage: create-balanced-trade <account name> <margin/total SEK> <ticker> <percentage> [<ticker> <percentage> ...]"
    );
    println!();
    println!(
        "Margin specifies how much cash to leave in the account after the trade, to account for fees and currency conversion costs. Specifying a negative value sets the total amount to invest, instead of the margin."
    );
    println!("");
    println!(
        "No trade will be initiated if the amount to invest is more than the available cash in the account."
    );
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,create_balanced_trade=info".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 || (args.len() > 1 && args[1] == "--help") || args.len() % 2 != 1 {
        usage();
        return Ok(());
    }
    let account_name = args.get(1).context("Missing account name")?;
    let margin_or_total: Decimal = args.get(2).context("Missing total value")?.parse()?;
    let mut trades = Vec::new();
    let mut i = 3;
    let mut total_percentage = Decimal::ZERO;
    while i + 1 < args.len() {
        let ticker = args.get(i).context("Missing ticker")?.to_string();
        let percentage: Decimal = args.get(i + 1).context("Missing percentage")?.parse()?;
        trades.push(Trade {
            ticker,
            percentage,
            amount_sek: None,
        });
        total_percentage += percentage;
        i += 2;
    }
    ensure!(
        total_percentage == dec!(100.0),
        "Total percentage must be 100%"
    );

    let montrose = tmr_client::TmrClient::new("TMR Client Sample");
    let montrose = montrose.connect().await?;

    let account = montrose
        .get_user_accounts()
        .await?
        .into_iter()
        .find(|a| a.account_name.as_ref() == Some(account_name))
        .context("Account not found")?;

    let all_holdings = montrose.get_holdings(Some(account.account_id)).await?;
    let holdings = all_holdings
        .first()
        .context("Failed to get holdings for the account")?;

    let available_cash = holdings.summary.available_for_purchase;
    let currency = &holdings.summary.currency;
    ensure!(
        currency == "SEK",
        "This example only works for SEK accounts. The trade API uses SEK. Need to implement conversion calculations for other currencies."
    );
    let amount_to_buy = if margin_or_total > Decimal::ZERO {
        if available_cash < margin_or_total {
            Decimal::ZERO
        } else {
            available_cash - margin_or_total
        }
    } else {
        -margin_or_total
    }
    .round_dp_with_strategy(2, rust_decimal::RoundingStrategy::ToNegativeInfinity);

    for trade in &mut trades {
        trade.amount_sek = Some(
            (amount_to_buy * (trade.percentage / dec!(100.0)))
                .round_dp_with_strategy(2, rust_decimal::RoundingStrategy::ToNegativeInfinity),
        );
    }

    println!(
        "Using account {} ({}) with {available_cash:.2} {currency}",
        account.account_name.as_deref().unwrap_or_default(),
        account.account_number
    );
    println!("");
    println!("Will buy the following instruments for a total of {amount_to_buy:.2} {currency}:");
    for trade in &trades {
        println!("{trade}");
    }
    println!("");
    if amount_to_buy >= available_cash {
        println!("The amount to buy is more than the available cash. No trades will be made.");
        return Ok(());
    } else if amount_to_buy == Decimal::ZERO {
        println!("The amount to buy is 0.00 SEK. No trades will be made.");
        return Ok(());
    }

    print!("Continue? (y/N) ");
    std::io::stdout().flush()?;
    let mut buf = [0; 1];
    std::io::stdin().read_exact(&mut buf)?;
    if buf != [b'y'] {
        println!("Aborting");
        return Ok(());
    }
    println!();

    println!("Creating trade tickets...");
    for trade in &trades {
        print!("{trade}: ");
        let trade_url = montrose
            .create_trade_ticket(TradeTicketArgs {
                side: TradeSide::Buy,
                account_id: Some(account.account_id),
                size: TradeSize::AmountSek(trade.amount_sek.expect("Amount should be set")),
                instrument: TradeInstrument::Ticker(trade.ticker.to_string()),
                price: None,
            })
            .await?;
        println!("{trade_url}");
    }
    println!("Done");
    Ok(())
}

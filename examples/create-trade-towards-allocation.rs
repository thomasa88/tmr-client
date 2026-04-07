// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

use std::io::{Read, Write};

use anyhow::{Context, ensure};
use rust_decimal::dec;
use tmr_client::{
    Decimal,
    tools::{TradeInstrument, TradeSide, TradeSize, TradeTicketArgs},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, PartialEq)]
struct WantedTrade {
    ticker: String,
    current_amount_sek: Decimal,
    wanted_frac: Decimal,
}

#[derive(Debug, PartialEq, Default)]
struct Trade {
    ticker: String,
    current_amount_sek: Decimal,
    // Can be division by zero if the account is empty
    current_frac: Option<Decimal>,
    wanted_amount_sek: Decimal,
    wanted_frac: Decimal,
    result_amount_sek: Decimal,
    result_frac: Decimal,
    to_buy_amount_sek: Decimal,
}

fn usage() {
    println!(
        "Usage: create-trade-towards-allocation <account name> <margin/-total SEK> <ticker> <percentage> [<ticker> <percentage> ...]"
    );
    println!();
    println!(
        "Margin specifies how much cash to leave in the account after the trade, to account for fees and currency conversion costs. Specifying a negative value sets the total amount to invest, instead of the margin."
    );
    println!();
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
    let mut wanted_trades = Vec::new();
    let mut i = 3;
    let mut total_percentage = Decimal::ZERO;
    while i + 1 < args.len() {
        let ticker = args.get(i).context("Missing ticker")?.to_string();
        let percentage: Decimal = args.get(i + 1).context("Missing percentage")?.parse()?;
        wanted_trades.push(WantedTrade {
            ticker,
            current_amount_sek: Decimal::ZERO,
            wanted_frac: percentage / dec!(100),
        });
        total_percentage += percentage;
        i += 2;
    }
    ensure!(
        total_percentage == dec!(100),
        "Total percentage must sum to 100%. It is currently {total_percentage}%.",
    );
    wanted_trades.sort_by(|a, b| a.ticker.cmp(&b.ticker));

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

    println!(
        "Using account {} ({}) with {available_cash:.2} {currency}",
        account.account_name.as_deref().unwrap_or_default(),
        account.account_number
    );

    // Only caring about allocation of the instruments that the user specified
    for position in &holdings.positions {
        if let Some(trade) = wanted_trades
            .iter_mut()
            .find(|b| b.ticker == position.ticker)
        {
            trade.current_amount_sek = position.market_value.account_currency;
        }
    }

    println!();
    println!("Will buy the following instruments for a total of {amount_to_buy:.2} {currency}:");
    println!();
    let trades = rebalance(&wanted_trades, amount_to_buy);

    let mut sums = Trade {
        ticker: "Total".to_string(),
        current_amount_sek: Decimal::ZERO,
        current_frac: Some(Decimal::ZERO),
        wanted_amount_sek: Decimal::ZERO,
        wanted_frac: Decimal::ZERO,
        result_amount_sek: Decimal::ZERO,
        result_frac: Decimal::ZERO,
        to_buy_amount_sek: Decimal::ZERO,
    };
    for trade in &trades {
        sums.current_amount_sek += trade.current_amount_sek;
        *sums.current_frac.as_mut().unwrap() += trade.current_frac.unwrap_or(Decimal::ZERO);
        sums.wanted_amount_sek += trade.wanted_amount_sek;
        sums.result_amount_sek += trade.result_amount_sek;
        sums.to_buy_amount_sek += trade.to_buy_amount_sek;
        sums.wanted_frac += trade.wanted_frac;
        sums.result_frac += trade.result_frac;
    }

    println!(
        "Ticker               | Current allocation         | Wanted allocation          | Trade                         | Resulting allocation                  "
    );
    println!(
        "---------------------|----------------------------|----------------------------|-------------------------------|---------------------------"
    );
    for trade in &trades {
        print_trade_line(trade);
    }
    println!(
        "---------------------|----------------------------|----------------------------|-------------------------------|---------------------------"
    );
    print_trade_line(&sums);
    println!();

    // Check after printing the results, so that the user can inspect them even if there's not enough cas   h
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
        if trade.to_buy_amount_sek <= Decimal::ZERO {
            continue;
        }
        print!("{:20} {:>+13.2} SEK", trade.ticker, trade.to_buy_amount_sek);
        let trade_url = montrose
            .create_trade_ticket(TradeTicketArgs {
                side: TradeSide::Buy,
                account_id: Some(account.account_id),
                size: TradeSize::AmountSek(trade.to_buy_amount_sek),
                instrument: TradeInstrument::Ticker(trade.ticker.to_string()),
                price: None,
            })
            .await?;
        println!(" {trade_url}");
    }
    println!("Done");
    Ok(())
}

fn print_trade_line(trade: &Trade) {
    println!(
        "{:20} \
            | {:>12.2} SEK ({:>6.2}%) \
            | {:>12.2} SEK ({:>6.2}%) \
            | {:>+13.2} SEK ({:>+7.2}pp) \
            | {:>12.2} SEK ({:>6.2}%)",
        trade.ticker,
        trade.current_amount_sek,
        trade
            .current_frac
            .map(|f| f * dec!(100))
            .unwrap_or_else(|| dec!(0)),
        trade.wanted_amount_sek,
        trade.wanted_frac * dec!(100),
        trade.to_buy_amount_sek,
        (trade.result_frac - trade.current_frac.unwrap_or(Decimal::ZERO)) * dec!(100),
        trade.result_amount_sek,
        trade.result_frac * dec!(100),
    );
}

fn rebalance(wanted: &[WantedTrade], amount_to_buy: Decimal) -> Vec<Trade> {
    let mut trades: Vec<Trade> = wanted
        .iter()
        .map(|w| Trade {
            ticker: w.ticker.clone(),
            current_amount_sek: w.current_amount_sek,
            wanted_frac: w.wanted_frac,
            ..Default::default()
        })
        .collect();

    let current_total_value = trades.iter().map(|t| t.current_amount_sek).sum();
    let new_total_value = current_total_value + amount_to_buy;

    // Will only buy those instruments that are under-allocated
    // Calculate how much we would ideally buy
    let mut want_to_buy_amount = Decimal::ZERO;
    for trade in &mut trades {
        // checked_div: Avoid division by zero if the account is empty
        trade.current_frac = trade.current_amount_sek.checked_div(current_total_value);
        trade.wanted_amount_sek = new_total_value * trade.wanted_frac;

        // Only buy for the under-allocated instruments
        // Start with the ideal (wanted) amount to buy
        if trade.wanted_amount_sek > trade.current_amount_sek {
            trade.to_buy_amount_sek = trade.wanted_amount_sek - trade.current_amount_sek;
            want_to_buy_amount += trade.to_buy_amount_sek;
        } else {
            trade.to_buy_amount_sek = Decimal::ZERO;
        }
    }

    // We want to buy for a certain amount to move the under-allocated
    // instruments closer towards their target values. However, we can only use
    // the cash available.
    assert!(want_to_buy_amount >= amount_to_buy);
    for trade in &mut trades {
        // Scale the amount to buy by the portion of how much the want to buy
        // value represents of the total want to buy value.
        trade.to_buy_amount_sek = (trade.to_buy_amount_sek / want_to_buy_amount) * amount_to_buy;

        trade.result_amount_sek = trade.current_amount_sek + trade.to_buy_amount_sek;
        trade.result_frac = trade.result_amount_sek / new_total_value;
    }

    trades
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::prelude::FromPrimitive;
    use tmr_client::Decimal;

    fn input_table(table: Vec<(impl Into<String>, Decimal, Decimal)>) -> Vec<WantedTrade> {
        table
            .into_iter()
            .map(|(ticker, current_amount, wanted_frac)| WantedTrade {
                ticker: ticker.into(),
                current_amount_sek: current_amount,
                wanted_frac,
            })
            .collect()
    }

    fn assert_amount(value: Decimal, expected: impl Into<Decimal>, msg: &str) {
        assert_eq!(value.round(), expected.into(), "{msg}");
    }

    fn assert_frac(value: Decimal, expected: f32, msg: &str) {
        assert_eq!(
            value.round_dp(2),
            Decimal::from_f32(expected).unwrap().round_dp(2),
            "{msg}"
        );
    }

    #[test]
    fn rebalance_from_zero() {
        let wanted = input_table(vec![
            ("A", dec!(0), dec!(0.5)),
            ("B", dec!(0), dec!(0.3)),
            ("C", dec!(0), dec!(0.2)),
        ]);
        let amount_to_buy = dec!(1000);

        let trades = rebalance(&wanted, amount_to_buy);

        assert_amount(trades[0].result_amount_sek, 500, "Result amount A");
        assert_amount(trades[1].result_amount_sek, 300, "Result amount B");
        assert_amount(trades[2].result_amount_sek, 200, "Result amount C");

        let expected_total: Decimal = trades.iter().map(|b| b.to_buy_amount_sek).sum();
        assert_eq!(expected_total, amount_to_buy);
    }

    #[test]
    fn rebalance_all_positive() {
        let wanted = input_table(vec![
            ("A", dec!(200), dec!(0.5)),
            ("B", dec!(300), dec!(0.2)),
            ("C", dec!(500), dec!(0.3)),
        ]);
        let amount_to_buy = dec!(1000);

        let trades = rebalance(&wanted, amount_to_buy);

        assert_amount(trades[0].result_amount_sek, 1000, "Result amount A");
        assert_amount(trades[1].result_amount_sek, 400, "Result amount B");
        assert_eq!(
            trades[2].result_amount_sek.round_dp(3),
            dec!(600),
            "Result amount C"
        );

        let expected_total: Decimal = trades.iter().map(|b| b.to_buy_amount_sek).sum();
        assert_eq!(expected_total, amount_to_buy);
    }

    #[test]
    fn rebalance_some_negative() {
        let wanted = input_table(vec![
            ("A", dec!(200), dec!(0.5)),
            ("B", dec!(500), dec!(0.2)),
            ("C", dec!(300), dec!(0.3)),
        ]);

        let amount_to_buy = dec!(1000);
        let trades = rebalance(&wanted, amount_to_buy);

        assert_amount(trades[0].result_amount_sek, 927, "Result amount A");
        assert_amount(trades[1].result_amount_sek, 500, "Result amount B");
        assert_amount(trades[2].result_amount_sek, 573, "Result amount C");

        let expected_total: Decimal = trades.iter().map(|b| b.to_buy_amount_sek).sum();
        assert_eq!(expected_total, amount_to_buy);
    }

    #[test]
    fn rebalance_large_diff() {
        let wanted = input_table(vec![
            ("A", dec!(600), dec!(0.3)),
            ("B", dec!(200), dec!(0.2)),
            ("C", dec!(100), dec!(0.3)),
            ("D", dec!(100), dec!(0.2)),
        ]);

        let amount_to_buy = dec!(100);
        let trades = rebalance(&wanted, amount_to_buy);
        assert_amount(trades[0].result_amount_sek, 600, "Result amount A");
        assert_amount(trades[1].result_amount_sek, 205, "Result amount B");
        assert_amount(trades[2].result_amount_sek, 162, "Result amount C");
        assert_amount(trades[3].result_amount_sek, 132, "Result amount D");

        let expected_total: Decimal = trades.iter().map(|b| b.to_buy_amount_sek).sum();
        assert_eq!(expected_total.round_dp(3), amount_to_buy);
    }

    #[test]
    fn all_fields_filled_in() {
        let wanted = input_table(vec![
            ("A", dec!(600), dec!(0.3)),
            ("B", dec!(200), dec!(0.2)),
            ("C", dec!(100), dec!(0.3)),
            ("D", dec!(100), dec!(0.2)),
        ]);

        let amount_to_buy = dec!(100);
        let trades = rebalance(&wanted, amount_to_buy);

        assert_amount(trades[1].current_amount_sek, 200, "Current amount B");
        assert_frac(trades[1].current_frac.unwrap(), 0.2, "Current fraction B");
        assert_amount(trades[1].wanted_amount_sek, 220, "Wanted amount B");
        assert_frac(trades[1].wanted_frac, 0.2, "Wanted fraction B");
        assert_amount(trades[1].to_buy_amount_sek, 5, "To buy amount B");
        assert_amount(trades[1].result_amount_sek, 205, "Result amount B");
        assert_frac(trades[1].result_frac, 0.19, "Result fraction B");
    }
}

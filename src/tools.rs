// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

use reqwest::Url;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_with::{NoneAsEmptyString, serde_as};
use uuid::Uuid;

// TODO: Generate types from tool input and output JSON schemas?

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub account_id: Uuid,
    pub account_number: String,
    pub account_name: String,
    pub currency: String,
    pub summary: AccountSummary,
    pub positions: Vec<Position>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    /// Value of the investments in the account
    pub total_market_value: Decimal,
    /// Amount available for purchase
    pub available_for_purchase: Decimal,
    /// Total value of the account. The sum of investments and cash.
    pub total_value: Decimal,
    /// Currency of the account
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub instrument_name: String,
    pub ticker: String,
    pub orderbook_id: u64,
    pub possible_orderbook_ids: Vec<u64>,
    /// Number of shares
    pub quantity: Decimal,
    /// Value of the position (quantity * price)
    pub market_value: InstrumentValue,
    pub unrealized_result: InstrumentValue,
    pub unrealized_result_percent: Decimal,
    pub instrument_currency: String,
    /// Exchange rate (instrument_currency / account_currency)
    pub fx_rate: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentValue {
    /// Value in the account currency
    pub account_currency: Decimal,
    /// Value in the instrument currency
    pub instrument_currency: Decimal,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub account_id: Uuid,
    pub account_number: String,
    /// Account name, as set by the user
    #[serde_as(as = "NoneAsEmptyString")]
    pub account_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeTicketArgs {
    /// The side of the order: Buy or Sell.
    pub side: TradeSide,

    /// Optional account ID. Use GetUserAccounts to find valid account IDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Uuid>,

    /// Optional price for the order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<Decimal>,

    /// How much of the instrument to trade
    #[serde(flatten)]
    pub size: TradeSize,

    /// The instrument to trade
    #[serde(flatten)]
    pub instrument: TradeInstrument,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TradeSize {
    /// SEK amount to trade.
    #[serde(rename = "amount")]
    AmountSek(Decimal),
    /// Number of shares to trade.
    Quantity(Decimal),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TradeInstrument {
    /// instrument name (string) to search for the instrument.
    Name(String),
    /// orderbookId (int) to identify the instrument directly.
    OrderbookId(i64),
    /// ticker (string) to identify the instrument  ticker symbol, e.g. "VOLV B".
    Ticker(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum TradeSide {
    #[default]
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTradeTicketResult {
    pub url: Url,
}

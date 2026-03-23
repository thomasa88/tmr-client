use reqwest::Url;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_with::{NoneAsEmptyString, serde_as};
use uuid::Uuid;

// TODO: Generate types from tool input and output JSON schemas?

pub type GetHoldingsResult = Vec<Account>;

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
    pub total_market_value: Decimal,
    pub available_for_purchase: Decimal,
    pub total_value: Decimal,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub instrument_name: String,
    pub ticker: String,
    pub orderbook_id: u64,
    pub possible_orderbook_ids: Vec<u64>,
    pub quantity: Decimal,
    pub market_value: CurrencyValue,
    pub unrealized_result: CurrencyValue,
    pub unrealized_result_percent: Decimal,
    pub instrument_currency: String,
    pub fx_rate: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyValue {
    pub account_currency: Decimal,
    pub instrument_currency: Decimal,
}

pub type Accounts = Vec<AccountInfo>;

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub account_id: Uuid,
    pub account_number: String,
    #[serde_as(as = "NoneAsEmptyString")]
    pub account_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TradeTicketArgs {
    /// The side of the order: Buy or Sell.
    pub side: Side,

    /// Optional account ID. Use GetUserAccounts to find valid account IDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<Uuid>,

    /// Optional SEK amount to trade. Exactly one of quantity or amount must be provided.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "amount")]
    pub amount_sek: Option<Decimal>,

    /// Optional price for the order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<Decimal>,

    /// Optional number of shares to trade. Exactly one of quantity or amount must be provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<Decimal>,

    /// Optional instrument name (string) to search for the instrument.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Optional orderbookId (int) to identify the instrument directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orderbook_id: Option<i64>,

    /// Optional ticker (string) to identify the instrument by ticker symbol, e.g. "VOLV B".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum Side {
    #[default]
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTradeTicketResult {
    pub url: Url,
}

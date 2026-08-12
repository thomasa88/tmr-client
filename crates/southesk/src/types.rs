// Copyright 2026 Thomas Axelsson
// SPDX-License-Identifier: MIT

//! Montrose API types.
//!
//! The types correspond to the types used by the Montrose MCP API. Some have
//! been slightly adapted for better ergonomics in Rust.

use derive_more::{AsRef, Display};
use reqwest::Url;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_with::{NoneAsEmptyString, serde_as};
use uuid::Uuid;

/// Selects what accounts to operate on.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AccountFilter {
    /// Use all accounts.
    All,
    /// Use the account with the given account ID.
    AccountId(Uuid),
}

/// An account number for a financial account.
///
/// Example: `1234567`
#[derive(
    Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, AsRef,
)]
pub struct AccountNumber(String);

impl AccountNumber {
    /// Creates a new account number.
    pub fn new(num: impl Into<String>) -> Result<Self, error::ParseAccountNumberError> {
        let num = num.into();
        // Taking a guess on the min and max length
        if num.is_empty() || num.len() < 6 || num.len() > 12 {
            Err(error::ParseAccountNumberError::BadLength)
        } else if !num.chars().all(|c| c.is_ascii_digit()) {
            Err(error::ParseAccountNumberError::InvalidFormat)
        } else {
            Ok(AccountNumber(num))
        }
    }
}

impl PartialEq<str> for AccountNumber {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for AccountNumber {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<AccountNumber> for str {
    fn eq(&self, other: &AccountNumber) -> bool {
        other == self
    }
}

impl PartialEq<AccountNumber> for &str {
    fn eq(&self, other: &AccountNumber) -> bool {
        other == self
    }
}

/// ISO 4217 currency code.
///
/// Examples: `SEK`, `USD`, `EUR`
#[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, AsRef)]
pub struct Currency(String);

impl Currency {
    /// Creates a new currency code.
    ///
    /// A simple format validation is performed, but the code is not checked for
    /// validity.
    pub fn new(code: impl AsRef<str>) -> Result<Self, error::ParseCurrencyError> {
        let code = code.as_ref();
        if code.len() != 3 || !code.chars().all(|c| c.is_ascii_alphabetic()) {
            Err(error::ParseCurrencyError::InvalidFormat)
        } else {
            Ok(Currency(code.to_uppercase()))
        }
    }
}

impl<'de> Deserialize<'de> for Currency {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let code = String::deserialize(deserializer)?;
        Currency::new(code).map_err(serde::de::Error::custom)
    }
}

impl PartialEq<str> for Currency {
    fn eq(&self, other: &str) -> bool {
        self.0.eq_ignore_ascii_case(other)
    }
}

impl PartialEq<&str> for Currency {
    fn eq(&self, other: &&str) -> bool {
        self.0.eq_ignore_ascii_case(other)
    }
}

impl PartialEq<Currency> for str {
    fn eq(&self, other: &Currency) -> bool {
        other == self
    }
}

impl PartialEq<Currency> for &str {
    fn eq(&self, other: &Currency) -> bool {
        other == self
    }
}

/// Account holdings, including account identifiers.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountHoldings {
    /// Unique account ID. Not visible to the user.
    ///
    /// Example: `771c4286-991c-48aa-965e-c7dd62e31735`
    pub account_id: Uuid,
    /// Account number.
    ///
    /// Example: `1234567`
    pub account_number: AccountNumber,
    /// Account name, as set by the user. It is not guaranteed to be unique, as
    /// multiple accounts can have the same name.
    #[serde_as(as = "NoneAsEmptyString")]
    pub account_name: Option<String>,
    /// Account type.
    pub account_type: AccountType,
    /// The main currency of the account.
    pub currency: Currency,
    /// Summary of the account holdings.
    pub summary: AccountSummary,
    /// List of positions (instruments) in the account.
    pub positions: Vec<Position>,
    /// List of currency positions (cash holdings) in the account.
    pub currency_positions: Vec<CurrencyPosition>,
}

/// Type of financial account.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum AccountType {
    /// Investment savings account (ISK).
    #[doc(alias = "ISK")]
    #[serde(rename = "ISK")]
    InvestmentSavings,
    /// Capital insurance (Kapitalförsäkring).
    #[doc(alias = "KF")]
    #[serde(rename = "KF")]
    CapitalInsurance,
    /// Regular brokerage account (Depå).
    #[doc(alias = "Depot")]
    #[serde(rename = "Depot")]
    Brokerage,
    /// Savings account (Sparkonto).
    #[serde(rename = "SPAR")]
    Savings,
    /// Credit account (Kreditkonto).
    #[serde(rename = "Credit")]
    Credit,
    /// Other (not yet categorized) account type.
    #[serde(untagged)]
    Other(String),
}

/// Summary of the account holdings.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    /// Value of the investments in the account, converted to the account's main
    /// currency.
    pub total_market_value: Decimal,
    /// Amount available for purchase, converted to the account's main currency.
    pub available_for_purchase: Decimal,
    /// Total value of the account. The sum of investments and cash, converted
    /// to the account's main currency.
    pub total_value: Decimal,
    /// Main currency of the account.
    pub currency: Currency,
}

/// A position in a single instrument within an account.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    /// Name of the instrument.
    pub instrument_name: String,
    /// Ticker symbol of the instrument.
    pub ticker: String,
    /// Orderbook ID of the instrument.
    pub orderbook_id: u64,
    /// Possible orderbook IDs for the instrument. This is a list of orderbook
    /// IDs that can be used to identify the instrument, e.g. in different
    /// markets.
    pub possible_orderbook_ids: Vec<u64>,
    /// Number of shares.
    pub quantity: Decimal,
    /// Value of the position (`quantity * price`).
    pub market_value: InstrumentValue,
    /// Unrealized result of the position.
    pub unrealized_result: InstrumentValue,
    /// Unrealized result as a percentage.
    pub unrealized_result_percent: Decimal,
    /// Currency of the instrument.
    pub instrument_currency: Currency,
    /// Exchange rate (`instrument_currency / account_currency`).
    pub fx_rate: Decimal,
}

/// Cash balance in a single currency within an account.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyPosition {
    /// ISO 4217 currency code (e.g. "SEK", "USD", "EUR").
    pub currency_code: Currency,
    /// Balance of the currency.
    pub balance: Decimal,
    /// Accrued interest for the currency.
    pub accrued_interest: Decimal,
    /// Amount available for purchase in the currency.
    pub available_for_purchase: Decimal,
}

/// Value of an instrument in different currencies.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentValue {
    /// Value in the account's main currency.
    pub account_currency: Decimal,
    /// Value in the instrument currency.
    pub instrument_currency: Decimal,
}

/// Various identifiers that can be used to identify an account.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountIdentifiers {
    /// Unique account ID. Not visible to the user.
    ///
    /// Example: `771c4286-991c-48aa-965e-c7dd62e31735`
    pub account_id: Uuid,
    /// Account number
    ///
    /// Example: `1234567`
    pub account_number: AccountNumber,
    /// Account name, as set by the user. It is not guaranteed to be unique, as
    /// multiple accounts can have the same name.
    #[serde_as(as = "NoneAsEmptyString")]
    pub account_name: Option<String>,
    /// Account type
    pub account_type: String,
}

/// Arguments for creating a trade ticket.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeTicketArgs {
    /// The side of the order: Buy or Sell.
    pub side: TradeSide,

    /// Optional account ID. Use
    /// [`get_user_accounts`](crate::Client::get_user_accounts) to find valid
    /// account IDs.
    pub account_id: Option<Uuid>,

    /// Optional price for the order.
    pub price: Option<Decimal>,

    /// How much of the instrument to trade.
    #[serde(flatten)]
    pub volume: TradeVolume,

    /// ISO 4217 currency code (e.g. \"SEK\", \"USD\", \"EUR\") for the amount.
    /// Only set this to [`TradeCurrency::Code`] when the user explicitly states
    /// a currency. When omitted and an [`account_id`](Self::account_id) is
    /// provided, the currency is resolved from the account's currency
    /// positions: if the account holds a cash position in the instrument's
    /// trading currency, that currency is used; otherwise the account's main
    /// currency.
    pub currency: TradeCurrency,

    /// The instrument to trade.
    #[serde(flatten)]
    pub instrument: Instrument,
}

/// Specifies how much of an instrument to trade.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TradeVolume {
    /// Amount (money) to trade. If the user explicitly specifies a currency
    /// (e.g. "10 000 SEK", "500 USD"), pass it via the
    /// [`TradeTicketArgs::currency`] parameter.
    Amount(Decimal),
    /// Number of shares to trade.
    Quantity(Decimal),
}

/// Currency to use in a trade.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum TradeCurrency {
    /// Use the account's main currency.
    #[default]
    Account,
    /// Use the currency given by the ISO 4217 code.
    ///
    /// Examples: `SEK`, `USD`, `EUR`.
    Code(Currency),
}

impl Serialize for TradeCurrency {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // From the create_trade_ticket tool documentation:
        // "If the user explicitly specifies a currency
        // (e.g. "10 000 SEK", "500 USD"), pass it via the currency parameter;
        // otherwise leave currency unset and the account's main currency will
        // be used."
        match self {
            Self::Account => serializer.serialize_none(),
            Self::Code(code) => serializer.serialize_some(code),
        }
    }
}

impl<'de> Deserialize<'de> for TradeCurrency {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_option(TradeCurrencyVisitor)
    }
}

struct TradeCurrencyVisitor;

impl<'de> serde::de::Visitor<'de> for TradeCurrencyVisitor {
    type Value = TradeCurrency;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("an ISO 4217 code of length 3")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(TradeCurrency::Account)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let code = Currency::deserialize(deserializer)?;
        Ok(TradeCurrency::Code(code))
    }
}

/// Specifies the instrument to trade.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Instrument {
    /// Optional instrument name (string) to search for the instrument. This is
    /// a convenience identifier and may be ambiguous; use [`search_instruments`](crate::Client::search_instruments) to
    /// find the correct orderbookId when needed.
    Name(String),
    /// Optional orderbookId (int) to identify the instrument directly. This is the safest identifier and should be preferred when known or after using [`search_instruments`](crate::Client::search_instruments).
    OrderbookId(u64),
    /// Optional ticker (string) to identify the instrument by ticker symbol,
    /// e.g. \"VOLV B\". This is a convenience identifier and may be ambiguous;
    /// use [`search_instruments`](crate::Client::search_instruments) to find the correct orderbookId when needed.
    Ticker(String),
}

/// Whether to buy or sell an instrument.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub enum TradeSide {
    /// Buy the instrument.
    #[default]
    Buy,
    /// Sell the instrument.
    Sell,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateTradeTicketResult {
    pub url: Url,
}

/// Various identifiers that can be used to identify a trade instrument.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentIdentifiers {
    /// Instrument name.
    pub name: String,
    /// Instrument order book ID.
    pub orderbook_id: u64,
    /// Instrument ticker.
    pub ticker: String,
}

/// Summary info about an instrument watchlist.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistInfo {
    /// Watchlist ID.
    pub list_id: u64,
    /// Watchlist name.
    pub name: String,
    /// Number of instruments in the watchlist.
    pub orderbook_count: u64,
}

/// An instrument watchlist.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Watchlist {
    /// Watchlist ID.
    pub list_id: u64,
    /// Watchlist name.
    pub name: String,
    /// Instruments in the watchlist.
    #[doc(alias = "instruments")]
    pub items: Vec<InstrumentIdentifiers>,
}

/// Result of adding or removing instruments from a watchlist.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifyWatchlistResult {
    /// Watchlist ID.
    pub list_id: u64,
    /// This is the affected orderbook IDs for some operations and the IDs in
    /// the call for others (2026-06-11).
    pub orderbook_ids: Vec<u64>,
}

/// Errors related to the MCP types.
pub mod error {
    /// Errors that can occur when parsing an account number.
    #[derive(Debug, thiserror::Error, PartialEq, Eq)]
    pub enum ParseAccountNumberError {
        #[error("invalid account number format")]
        InvalidFormat,
        #[error("account number has a bad length")]
        BadLength,
    }

    /// Errors that can occur when parsing a currency code.
    #[derive(Debug, thiserror::Error, PartialEq, Eq)]
    pub enum ParseCurrencyError {
        #[error("invalid currency code")]
        InvalidFormat,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trade_currency_serialization() {
        assert_eq!(
            serde_json::to_string(&TradeCurrency::Account).unwrap(),
            "null"
        );
        assert_eq!(
            serde_json::from_str::<TradeCurrency>("null").unwrap(),
            TradeCurrency::Account
        );

        assert_eq!(
            serde_json::to_string(&TradeCurrency::Code(Currency::new("SEK").unwrap())).unwrap(),
            "\"SEK\""
        );
        assert_eq!(
            serde_json::from_str::<TradeCurrency>("\"USD\"").unwrap(),
            TradeCurrency::Code(Currency::new("USD").unwrap())
        );
        assert!(serde_json::from_str::<TradeCurrency>("\"WRONG_LEN\"").is_err());
        assert!(serde_json::from_str::<TradeCurrency>("\"\"").is_err());
    }

    #[test]
    fn account_number_parsing() {
        assert_eq!(
            AccountNumber::new("1234567").unwrap(),
            AccountNumber("1234567".to_string())
        );
        assert_eq!(
            AccountNumber::new(""),
            Err(error::ParseAccountNumberError::BadLength)
        );
        assert_eq!(
            AccountNumber::new("283333a"),
            Err(error::ParseAccountNumberError::InvalidFormat)
        );
        assert_eq!(
            AccountNumber::new("2833333939393939392222222"),
            Err(error::ParseAccountNumberError::BadLength)
        );
    }

    #[test]
    fn account_number_to_string() {
        assert_eq!(AccountNumber("1111233".to_string()).to_string(), "1111233");
    }

    #[test]
    fn account_number_eq() {
        let account_number = AccountNumber("1111233".to_string());
        // Test symmetry of str == AccountNumber
        assert_eq!(account_number, "1111233");
        assert_eq!("1111233", account_number);

        assert_eq!(&account_number, "1111233");
        assert_eq!("1111233", &account_number);

        assert_eq!(&account_number, &account_number);
        assert_eq!(account_number, account_number);
    }

    #[test]
    fn currency_parsing() {
        assert_eq!(Currency::new("USD").unwrap(), Currency("USD".to_string()));
        assert_eq!(Currency::new("sek").unwrap(), Currency("SEK".to_string()));
        assert_eq!(
            Currency::new(""),
            Err(error::ParseCurrencyError::InvalidFormat)
        );
        assert_eq!(
            Currency::new("USDOLLAR"),
            Err(error::ParseCurrencyError::InvalidFormat)
        );
        assert_eq!(
            Currency::new("US2"),
            Err(error::ParseCurrencyError::InvalidFormat)
        );
    }

    #[test]
    fn currency_to_string() {
        assert_eq!(Currency("USD".to_string()).to_string(), "USD");
    }

    #[test]
    fn currency_deserialization() {
        let json = "\"usd\"";
        let currency: Currency = serde_json::from_str(json).unwrap();
        assert_eq!(currency, Currency("USD".to_string()));

        let invalid_json = "\"US2\"";
        assert!(serde_json::from_str::<Currency>(invalid_json).is_err());
    }

    #[test]
    fn currency_eq() {
        let currency = Currency("USD".to_string());
        // Test symmetry of str == Currency
        assert_eq!(currency, "USD");
        assert_eq!(currency, "usd");
        assert_eq!("USD", currency);
        assert_eq!("usd", currency);

        assert_eq!(&currency, "USD");
        assert_eq!(&currency, "usd");
        assert_eq!("USD", &currency);
        assert_eq!("usd", &currency);

        assert_eq!(&currency, &currency);
        assert_eq!(currency, currency);
    }
}

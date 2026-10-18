//! The DeepSeek balance endpoint: request, response model and env overrides.
//!
//! The network layer is deliberately separate from the UI: [`fetch`] is a plain
//! blocking call, so the window host can run it on a worker thread while the UI
//! keeps painting, and the headless UI tests never touch the network.

use deepseek_util::currency::symbol;
use serde::Deserialize;

use crate::keyring::{self, Spec};
use crate::net;

/// Balance endpoint used when `DEEPSEEK_BALANCE_URL` is not set.
const DEFAULT_BALANCE_URL: &str = "https://api.deepseek.com/user/balance";

/// The error [`resolve_api_key`] reports when no key is configured anywhere.
pub const MISSING_KEY_MESSAGE: &str =
    "未配置 DEEPSEEK_API_KEY 环境变量（写入 ~/.zshrc 后点刷新即可，无需重启）";

/// A balance reply: availability plus one entry per currency.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Balance {
    pub is_available: bool,
    pub balance_infos: Vec<BalanceInfo>,
}

/// Balance for a single currency, as returned by the API (amounts are strings).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BalanceInfo {
    pub currency: String,
    pub total_balance: String,
    pub granted_balance: String,
    pub topped_up_balance: String,
}

impl Balance {
    /// Parses a response body.
    pub fn parse(body: &str) -> Result<Self, String> {
        serde_json::from_str(body).map_err(|error| format!("解析响应失败: {error}"))
    }

    /// A single line for the menu bar, e.g. `¥-0.19`.
    ///
    /// Only the first currency is shown: the status bar is a handful of
    /// characters wide, and the panel next to it carries the full breakdown.
    pub fn headline(&self) -> String {
        match self.balance_infos.first() {
            Some(info) => format!("{}{}", symbol(&info.currency), info.total_balance),
            None => "无余额信息".to_string(),
        }
    }
}

/// Balance endpoint, overridable with `DEEPSEEK_BALANCE_URL`.
pub fn endpoint() -> String {
    std::env::var("DEEPSEEK_BALANCE_URL").unwrap_or_else(|_| DEFAULT_BALANCE_URL.to_string())
}

/// How the key is found: the process environment, then the user's login shell
/// (see [`crate::keyring`]).
fn key_spec() -> Spec<'static> {
    Spec {
        env_var: "DEEPSEEK_API_KEY",
        marker: "__IGUI_KEY__",
        missing: MISSING_KEY_MESSAGE,
    }
}

/// The key to fetch with, or the "未配置" error the UI shows verbatim.
pub fn resolve_api_key() -> Result<String, String> {
    keyring::resolve(&key_spec(), None)
}

/// Queries the balance endpoint. Blocking; call it off the UI thread.
pub fn fetch(endpoint: &str, api_key: &str) -> Result<Balance, String> {
    let body = net::get(endpoint, api_key).map_err(net::Error::message)?;
    Balance::parse(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "is_available": true,
        "balance_infos": [
            {
                "currency": "CNY",
                "total_balance": "110.00",
                "granted_balance": "10.00",
                "topped_up_balance": "100.00"
            }
        ]
    }"#;

    #[test]
    fn parses_a_balance_reply() {
        let balance = Balance::parse(SAMPLE).expect("valid reply");
        assert!(balance.is_available);
        assert_eq!(balance.balance_infos.len(), 1);
        let info = &balance.balance_infos[0];
        assert_eq!(info.currency, "CNY");
        assert_eq!(info.total_balance, "110.00");
        assert_eq!(info.granted_balance, "10.00");
        assert_eq!(info.topped_up_balance, "100.00");
    }

    #[test]
    fn reports_a_bad_reply() {
        let error = Balance::parse("not json").expect_err("invalid body");
        assert!(error.contains("解析响应失败"), "{error}");
    }

    /// The headline is the symbol plus the amount: a known currency (the first
    /// entry only), an unknown code, and an empty reply all have an answer.
    #[test]
    fn the_headline_is_symbol_plus_amount() {
        let balance = Balance::parse(SAMPLE).expect("valid reply");
        assert_eq!(balance.headline(), "¥110.00");

        let mut unknown = balance.clone();
        unknown.balance_infos[0].currency = "JPY".to_string();
        assert_eq!(unknown.headline(), "JPY 110.00");

        let empty = Balance {
            is_available: false,
            balance_infos: Vec::new(),
        };
        assert_eq!(empty.headline(), "无余额信息");
    }

    #[test]
    fn the_missing_key_message_names_the_variable() {
        assert!(MISSING_KEY_MESSAGE.contains("DEEPSEEK_API_KEY"));
    }
}

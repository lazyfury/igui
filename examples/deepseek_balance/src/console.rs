//! The OpenCode Console budget endpoint: request, response model and key lookup.
//!
//! The Console API exposes each workspace member's effective monthly budget:
//! `GET {CONSOLE_URL}/api/v1/budgets/members` (bearer service-account key with
//! `All` permission) answers a JSON array, and every member carries a
//! `limit_micro_cents`, a `spent_micro_cents`, an `exceeded` flag and the
//! `resets_at` stamp. Money is a decimal string in microcents (1e8 per dollar).
//!
//! Unlike Go's *percent* windows, this is a real dollar budget, so the view
//! shows the remainder as currency next to the Go quota. A workspace has one
//! member per human (plus service accounts); a personal one has a single entry,
//! so the first member is shown. Which member is "us" is not part of the
//! unauthenticated reply, so the email is shown to make a wrong pick obvious.
//!
//! Like [`crate::api`] and [`crate::go`], the network call is a plain blocking
//! function so the host can run it on a worker thread; the headless UI tests
//! never touch it.

use std::time::Duration;

use deepseek_util::currency;
use deepseek_util::time::parse_rfc3339_utc;
use serde::Deserialize;

use crate::keyring::{self, Spec};
use crate::net;

/// Console base URL used when `OPENCODE_CONSOLE_URL` is not set.
const DEFAULT_CONSOLE_URL: &str = "https://opencode.ai/console";

/// The members-and-budgets path under the Console base URL.
const MEMBERS_PATH: &str = "/api/v1/budgets/members";

/// The error [`resolve_api_key`] reports when no key is configured anywhere.
pub const MISSING_KEY_MESSAGE: &str =
    "未配置 OpenCode 控制台密钥（需 oc_sk_… 且权限为 All，见本页提示）";

/// One member's effective monthly budget, as returned by the Console API.
///
/// Every money field is a decimal string; `limit_micro_cents` (and `source`) are
/// null when the member is unlimited.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MemberBudget {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub limit_micro_cents: Option<String>,
    #[serde(default)]
    pub spent_micro_cents: Option<String>,
    #[serde(default)]
    pub exceeded: bool,
    /// RFC 3339 instant the budget resets, as returned by the API.
    #[serde(default)]
    pub resets_at: Option<String>,
    /// `"custom"` for a member override, `"default"` for the workspace value.
    #[serde(default)]
    pub source: Option<String>,
}

impl MemberBudget {
    /// The monthly limit as currency, or `不限` when the member is unlimited.
    pub fn limit_label(&self) -> String {
        match self.limit_micro_cents() {
            Some(micro) => currency::usd(micro),
            None => "不限".to_string(),
        }
    }

    /// What has been spent this month, or `—` when the reply omitted it.
    pub fn spent_label(&self) -> String {
        match self.spent_micro_cents() {
            Some(micro) => currency::usd(micro),
            None => "—".to_string(),
        }
    }

    /// The limit left this month: the difference, `已超支 $x` when it is gone
    /// past zero, or `不限` for an unlimited member.
    pub fn remaining_label(&self) -> String {
        match (self.limit_micro_cents(), self.spent_micro_cents()) {
            (Some(limit), Some(spent)) => {
                let left = limit - spent;
                if left < 0 {
                    format!("已超支 {}", currency::usd(-left))
                } else {
                    currency::usd(left)
                }
            }
            (None, _) => "不限".to_string(),
            (Some(limit), None) => currency::usd(limit),
        }
    }

    /// Time until `resets_at` relative to `now_epoch`, if the stamp parses and is
    /// still ahead. `None` means "no usable reset time".
    pub fn reset_in(&self, now_epoch: i64) -> Option<Duration> {
        let at = parse_rfc3339_utc(self.resets_at.as_deref()?)?;
        let seconds = at - now_epoch;
        (seconds > 0).then(|| Duration::from_secs(seconds as u64))
    }

    /// A compact line for the menu bar and the badge, e.g. `$62.75`.
    pub fn headline(&self) -> String {
        self.remaining_label()
    }

    fn limit_micro_cents(&self) -> Option<i128> {
        self.limit_micro_cents
            .as_deref()
            .and_then(currency::parse_micro_cents)
    }

    fn spent_micro_cents(&self) -> Option<i128> {
        self.spent_micro_cents
            .as_deref()
            .and_then(currency::parse_micro_cents)
    }
}

/// Parses a members reply, taking the first member's budget.
///
/// The endpoint answers an array; a workspace's humans come in email order, and
/// a personal one has a single entry. An empty list is an error rather than a
/// blank card.
pub fn parse(body: &str) -> Result<MemberBudget, String> {
    let members: Vec<MemberBudget> =
        serde_json::from_str(body).map_err(|error| format!("解析响应失败: {error}"))?;
    members
        .into_iter()
        .next()
        .ok_or_else(|| "控制台未返回成员预算".to_string())
}

/// Console base URL, overridable with `OPENCODE_CONSOLE_URL`.
pub fn endpoint() -> String {
    std::env::var("OPENCODE_CONSOLE_URL").unwrap_or_else(|_| DEFAULT_CONSOLE_URL.to_string())
}

/// The key to fetch with: the process environment, then the user's login shell.
///
/// The Console key is a service-account key (`oc_sk_…` with `All` permission),
/// not the Go key from `auth.json`, so it has its own variable.
pub fn resolve_api_key() -> Result<String, String> {
    keyring::resolve(
        &Spec {
            env_var: "OPENCODE_CONSOLE_API_KEY",
            marker: "__IGUI_CONSOLE_KEY__",
            missing: MISSING_KEY_MESSAGE,
        },
        None,
    )
}

/// Queries the members-and-budgets endpoint. Blocking; call it off the UI thread.
pub fn fetch(endpoint: &str, api_key: &str) -> Result<MemberBudget, String> {
    let url = format!("{}{MEMBERS_PATH}", endpoint.trim_end_matches('/'));
    let body = net::get(&url, api_key).map_err(net::Error::message)?;
    parse(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A members reply with one custom budget and one inherited/unlimited member.
    const SAMPLE: &str = r#"[
        {
            "user_id": "user_1",
            "email": "alice@example.com",
            "limit_micro_cents": "7525000000",
            "spent_micro_cents": "1250000000",
            "exceeded": false,
            "resets_at": "2026-10-01T00:00:00.000Z",
            "source": "custom",
            "updated_at": "2026-09-03T12:00:00.000Z"
        },
        {
            "user_id": "user_2",
            "email": "bob@example.com",
            "limit_micro_cents": null,
            "spent_micro_cents": "0",
            "exceeded": false,
            "resets_at": "2026-10-01T00:00:00.000Z",
            "source": "default",
            "updated_at": null
        }
    ]"#;

    #[test]
    fn parses_a_members_reply_as_the_first_member() {
        let member = parse(SAMPLE).expect("valid reply");
        assert_eq!(member.email, "alice@example.com");
        assert!(!member.exceeded);
        assert_eq!(member.source.as_deref(), Some("custom"));
    }

    #[test]
    fn reports_a_bad_or_empty_reply() {
        assert!(parse("not json")
            .expect_err("invalid body")
            .contains("解析响应失败"));
        assert!(parse("[]")
            .expect_err("empty list")
            .contains("未返回成员预算"));
    }

    /// Money renders as dollars, an unlimited member says so, and an overspent
    /// one shows the overrun instead of a negative.
    #[test]
    fn the_budget_renders_as_dollars() {
        let member = parse(SAMPLE).expect("valid reply");
        assert_eq!(member.limit_label(), "$75.25");
        assert_eq!(member.spent_label(), "$12.50");
        assert_eq!(member.remaining_label(), "$62.75");

        let unlimited = MemberBudget {
            limit_micro_cents: None,
            spent_micro_cents: Some("0".to_string()),
            ..member.clone()
        };
        assert_eq!(unlimited.limit_label(), "不限");
        assert_eq!(unlimited.remaining_label(), "不限");

        let over = MemberBudget {
            limit_micro_cents: Some("1000000000".to_string()),
            spent_micro_cents: Some("1500000000".to_string()),
            exceeded: true,
            ..member.clone()
        };
        assert_eq!(over.remaining_label(), "已超支 $5.00");
    }

    /// A missing spend is `—`, not a made-up zero.
    #[test]
    fn a_missing_spend_is_a_placeholder() {
        let member = MemberBudget {
            limit_micro_cents: Some("1000000000".to_string()),
            spent_micro_cents: None,
            exceeded: false,
            email: String::new(),
            resets_at: None,
            source: None,
        };
        assert_eq!(member.spent_label(), "—");
        assert_eq!(member.remaining_label(), "$10.00");
    }

    #[test]
    fn reset_in_is_the_gap_to_the_stamp() {
        let member = parse(SAMPLE).expect("valid reply");
        let before = parse_rfc3339_utc("2026-09-30T23:00:00Z").unwrap();
        assert_eq!(member.reset_in(before), Some(Duration::from_secs(3_600)));
        let after = parse_rfc3339_utc("2026-10-01T01:00:00Z").unwrap();
        assert_eq!(member.reset_in(after), None);
    }

    #[test]
    fn the_missing_key_message_names_the_variable() {
        assert!(MISSING_KEY_MESSAGE.contains("oc_sk_"));
    }
}

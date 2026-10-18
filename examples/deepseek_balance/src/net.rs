//! Shared blocking HTTP for the data sources: one configured agent and the
//! response-to-error mapping.
//!
//! The agent exists so every request carries the same timeouts. `ureq` defaults
//! to a connect timeout only, and a read that never returns would leave the
//! worker thread — and the view's `Busy` state — stuck forever. Building it once
//! also pools connections across refreshes and both sources.
//!
//! Like the rest of the data layer, this is just functions: no backend, no UI.

use std::sync::OnceLock;
use std::time::Duration;

/// How long to wait for the TCP/TLS connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Overall ceiling for a request, response body included.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Sent on every request; some gateways reject or throttle unknown agents.
const USER_AGENT: &str = concat!("deepseek_balance/", env!("CARGO_PKG_VERSION"));

/// Longest error detail shown verbatim; longer bodies are cut with `…` so one
/// pathological reply cannot flood the error line.
const MAX_ERROR_DETAIL_CHARS: usize = 160;

/// A failed HTTP request, before it is turned into the message the UI shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The request never produced a response.
    Transport(String),
    /// A response arrived but its body could not be read.
    Body(String),
    /// A non-2xx reply; `detail` is one line taken from the body.
    Http { status: u16, detail: String },
}

impl Error {
    /// The user-facing message, preserving the wording the UI has always shown.
    pub fn message(self) -> String {
        match self {
            Self::Transport(error) => format!("请求失败: {error}"),
            Self::Body(error) => format!("读取响应失败: {error}"),
            Self::Http { status, detail } => format!("HTTP {status}: {detail}"),
        }
    }
}

/// The shared agent: built on first use, reused for every request.
fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout_connect(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .user_agent(USER_AGENT)
            .build()
    })
}

/// GETs `url` with a bearer `api_key` and returns the response body.
pub fn get(url: &str, api_key: &str) -> Result<String, Error> {
    match agent()
        .get(url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Accept", "application/json")
        .call()
    {
        Ok(response) => response
            .into_string()
            .map_err(|error| Error::Body(error.to_string())),
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            Err(Error::Http {
                status,
                detail: error_detail(&body),
            })
        }
        Err(error) => Err(Error::Transport(error.to_string())),
    }
}

/// One line of error detail from a response body.
///
/// An API failure carries its reason in one of two shapes: inference/Go use
/// `error.message`, while the Console API uses a top-level `_tag`. Showing the
/// body verbatim would put a dense JSON line on screen, so the reason is pulled
/// out when present; anything else is shown trimmed. Either way the result is
/// capped to [`MAX_ERROR_DETAIL_CHARS`].
fn error_detail(body: &str) -> String {
    let detail = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(|message| message.as_str())
                .or_else(|| value.get("_tag").and_then(|tag| tag.as_str()))
                .map(str::trim)
                .filter(|message| !message.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.trim().to_string());
    truncate_chars(&detail, MAX_ERROR_DETAIL_CHARS)
}

fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(max).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The agent must carry a request timeout: without one a stalled read hangs
    /// the worker (and the view's busy state) forever.
    #[test]
    fn the_shared_agent_sets_a_timeout_and_user_agent() {
        let debug = format!("{:?}", agent());
        assert!(debug.contains("timeout: Some"), "{debug}");
        assert!(debug.contains("deepseek_balance/"), "{debug}");
    }

    /// A wrong key returns `{"error":{"message":...}}`; the user sees the
    /// reason, not the JSON envelope.
    #[test]
    fn an_api_error_body_shows_its_message_not_the_json() {
        let body = r#"{"error":{"message":"Authentication Fails (no such user)","type":"authentication_error","code":"invalid_request_error"}}"#;
        assert_eq!(error_detail(body), "Authentication Fails (no such user)");
    }

    #[test]
    fn a_non_json_error_body_shows_itself_trimmed() {
        assert_eq!(error_detail("  gateway timeout  "), "gateway timeout");
    }

    /// The Console API names its error in `_tag`; show the reason, not the JSON.
    #[test]
    fn a_tagged_error_body_shows_the_tag() {
        assert_eq!(error_detail(r#"{"_tag":"Forbidden"}"#), "Forbidden");
        assert_eq!(error_detail(r#"{"_tag":"Unauthorized"}"#), "Unauthorized");
    }

    /// JSON without an `error.message` (or with a blank one) falls back to the
    /// body instead of showing nothing.
    #[test]
    fn json_without_an_error_message_falls_back_to_the_body() {
        assert_eq!(error_detail(r#"{"ok":false}"#), r#"{"ok":false}"#);
        assert_eq!(
            error_detail(r#"{"error":{"message":"   "}}"#),
            r#"{"error":{"message":"   "}}"#
        );
    }

    #[test]
    fn an_overlong_detail_is_truncated() {
        let body = "x".repeat(500);
        let detail = error_detail(&body);
        assert_eq!(detail.chars().count(), MAX_ERROR_DETAIL_CHARS + 1);
        assert!(detail.ends_with('…'), "{detail}");
    }

    /// The rendered message matches the wording the UI has always shown.
    #[test]
    fn errors_render_as_before() {
        assert_eq!(
            Error::Transport("dns".into()).message(),
            "请求失败: dns".to_string()
        );
        assert_eq!(
            Error::Body("bad chunk".into()).message(),
            "读取响应失败: bad chunk".to_string()
        );
        assert_eq!(
            Error::Http {
                status: 401,
                detail: "nope".into(),
            }
            .message(),
            "HTTP 401: nope".to_string()
        );
    }
}

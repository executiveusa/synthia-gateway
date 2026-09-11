//! Provider failure classification.
//!
//! Every upstream error is turned into a [`FailureKind`] so the router can
//! decide whether trying the next entry in the fallback chain is worthwhile
//! (rate limits, daily caps, 5xx, transport errors) or pointless (auth
//! failures, malformed requests).

use std::fmt;

/// What went wrong with an upstream provider call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// Provider is configured wrong or not configured at all — another chain
    /// entry may still work.
    Unavailable,
    /// 429 where the body says a *daily* quota is exhausted (e.g. Groq's
    /// "tokens per day (TPD)"). Retrying the same provider is useless until
    /// tomorrow; fall back immediately.
    RateLimitDaily,
    /// 429 without a daily-cap marker — per-minute/per-second limit.
    RateLimitTransient,
    /// Upstream 5xx.
    Server,
    /// DNS/connect/TLS/timeout — never reached the provider.
    Transport,
    /// 401/403 — bad or revoked key. Fail-closed: the chain stops here
    /// rather than silently serving from another provider (see
    /// [`FailureKind::should_fallback`]).
    Auth,
    /// 400/404/422 — the request itself is bad; every provider would reject it.
    BadRequest,
    /// Anything we could not classify.
    Unknown,
}

impl FailureKind {
    /// Should the router walk on to the next fallback-chain entry?
    ///
    /// Auth failures (401/403) are deliberately fail-closed: the provider
    /// rejected our credential, which means a misconfigured or revoked key.
    /// Silently serving the request from a different provider would mask a
    /// broken lane and muddy per-provider consent/spend expectations, so the
    /// chain stops and the error surfaces loudly instead.
    pub fn should_fallback(&self) -> bool {
        matches!(
            self,
            FailureKind::Unavailable
                | FailureKind::RateLimitDaily
                | FailureKind::RateLimitTransient
                | FailureKind::Server
                | FailureKind::Transport
        )
    }

    /// Should this failure count against the provider's circuit breaker?
    pub fn trips_circuit(&self) -> bool {
        !matches!(self, FailureKind::BadRequest)
    }
}

impl fmt::Display for FailureKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            FailureKind::Unavailable => "unavailable",
            FailureKind::RateLimitDaily => "rate_limit_daily",
            FailureKind::RateLimitTransient => "rate_limit_transient",
            FailureKind::Server => "server_error",
            FailureKind::Transport => "transport_error",
            FailureKind::Auth => "auth_error",
            FailureKind::BadRequest => "bad_request",
            FailureKind::Unknown => "unknown",
        };
        f.write_str(s)
    }
}

/// Structured upstream failure: HTTP status (when we got a response at all)
/// plus a truncated copy of the provider's error body.
#[derive(Debug)]
pub struct ProviderFailure {
    pub status: Option<u16>,
    pub body: String,
}

impl fmt::Display for ProviderFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.status {
            Some(s) => write!(f, "upstream HTTP {}: {}", s, self.body),
            None => write!(f, "upstream failure: {}", self.body),
        }
    }
}

impl std::error::Error for ProviderFailure {}

/// Build an anyhow error carrying a [`ProviderFailure`] so the routing layer
/// can downcast and classify it. The body is truncated so provider responses
/// can never bloat logs or leak oversized payloads into our own errors.
pub fn provider_failure(status: u16, body: impl Into<String>) -> anyhow::Error {
    let mut body = body.into();
    if body.len() > 512 {
        // String::truncate panics off a char boundary; walk back to one.
        let mut end = 512;
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        body.truncate(end);
        body.push_str("…[truncated]");
    }
    anyhow::Error::new(ProviderFailure {
        status: Some(status),
        body,
    })
}

/// Classify an anyhow error coming out of a provider adapter.
pub fn classify_error(err: &anyhow::Error) -> FailureKind {
    if let Some(pf) = err.downcast_ref::<ProviderFailure>() {
        return classify_status(pf.status, &pf.body);
    }
    if err.downcast_ref::<reqwest::Error>().is_some() {
        return FailureKind::Transport;
    }
    FailureKind::Unknown
}

/// Classify by HTTP status, sniffing daily-quota language out of 429 bodies.
pub fn classify_status(status: Option<u16>, body: &str) -> FailureKind {
    match status {
        Some(429) => {
            if is_daily_cap_body(body) {
                FailureKind::RateLimitDaily
            } else {
                FailureKind::RateLimitTransient
            }
        }
        Some(401) | Some(403) => FailureKind::Auth,
        Some(400) | Some(404) | Some(422) => FailureKind::BadRequest,
        Some(s) if (500..=599).contains(&s) => FailureKind::Server,
        Some(_) => FailureKind::Unknown,
        None => FailureKind::Transport,
    }
}

/// Daily-quota markers seen in the wild: Groq's "tokens per day (TPD)" and
/// "Requested ... Please try again in 24h", generic "requests per day" /
/// "daily quota" phrasings.
fn is_daily_cap_body(body: &str) -> bool {
    let b = body.to_ascii_lowercase();
    (b.contains("tokens per day")
        || b.contains("(tpd")
        || b.contains("tpd:")
        || b.contains("requests per day")
        || b.contains("(rpd")
        || b.contains("daily quota")
        || b.contains("daily limit"))
        && (b.contains("rate_limit") || b.contains("limit") || b.contains("quota"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groq_tpd_body_is_daily_cap() {
        let body = r#"{"error":{"message":"Rate limit reached for model `openai/gpt-oss-120b` in organization `org_123` on tokens per day (TPD): Limit 200000, Used 196639, Requested 6716. Please try again in 24m9s.","type":"tokens","code":"rate_limit_exceeded"}}"#;
        assert_eq!(
            classify_status(Some(429), body),
            FailureKind::RateLimitDaily
        );
    }

    #[test]
    fn per_minute_429_is_transient() {
        let body = r#"{"error":{"message":"Rate limit reached for model `openai/gpt-oss-120b` on tokens per minute (TPM): Limit 8000, Used 7900, Requested 500. Please try again in 5s.","type":"tokens","code":"rate_limit_exceeded"}}"#;
        assert_eq!(
            classify_status(Some(429), body),
            FailureKind::RateLimitTransient
        );
    }

    #[test]
    fn auth_and_bad_request_do_not_fallback_to_next_model_but_classify_right() {
        assert_eq!(classify_status(Some(401), "{}"), FailureKind::Auth);
        assert_eq!(classify_status(Some(403), "{}"), FailureKind::Auth);
        assert_eq!(classify_status(Some(400), "{}"), FailureKind::BadRequest);
        assert!(!FailureKind::BadRequest.should_fallback());
        assert!(!FailureKind::Auth.should_fallback(), "auth is fail-closed");
        assert!(FailureKind::Auth.trips_circuit(), "auth still trips the circuit");
        assert!(FailureKind::RateLimitDaily.should_fallback());
        assert!(FailureKind::Server.should_fallback());
        assert!(FailureKind::Transport.should_fallback());
    }

    #[test]
    fn transport_and_unknown() {
        assert_eq!(classify_status(None, "dns"), FailureKind::Transport);
        assert_eq!(classify_status(Some(418), ""), FailureKind::Unknown);
    }

    #[test]
    fn truncation_is_utf8_safe_on_multibyte_bodies() {
        // 511 ASCII bytes + one 4-byte emoji straddling the 512 cut.
        let mut body = "a".repeat(511);
        body.push('😀');
        body.push_str(&"b".repeat(600));
        assert!(body.len() > 512);
        let err = provider_failure(500, body);
        let shown = err.to_string();
        // No panic, cut landed on a char boundary, marker appended.
        assert!(shown.contains("…[truncated]"));
        assert!(!shown.contains('\u{fffd}'));
        let pf = err.downcast_ref::<ProviderFailure>().unwrap();
        assert!(pf.body.len() <= 512 + "…[truncated]".len());
        // The emoji sat across the boundary, so it must have been dropped.
        assert!(!pf.body.contains('😀'));
    }

    #[test]
    fn short_multibyte_bodies_pass_through_untouched() {
        let body = "fout: é😀".to_string();
        let err = provider_failure(429, body.clone());
        let pf = err.downcast_ref::<ProviderFailure>().unwrap();
        assert_eq!(pf.body, body);
    }
}

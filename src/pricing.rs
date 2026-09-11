//! Cost estimation for the spend ledger and daily budget circuit breaker.
//!
//! Two rules keep this honest:
//!  1. A model is $0 only when it is *verified free* — it appears in the
//!     explicit free lists below (checked against the providers' own docs).
//!  2. Anything unknown falls back to a small but non-zero estimate, so a
//!     paid model can never masquerade as free and silently dodge the budget.

/// Per-token rates (USD) for known paid models. Rough on purpose — this is a
/// budget tripwire, not billing.
fn paid_rates(provider: &str, model: &str) -> Option<(f64, f64)> {
    let rates = match (provider, model) {
        ("anthropic", m) if m.contains("opus") => (0.000015, 0.000075),
        ("anthropic", m) if m.contains("sonnet") => (0.000003, 0.000015),
        ("anthropic", m) if m.contains("haiku") => (0.00000025, 0.00000125),
        ("openai", m) if m.contains("gpt-4o") && !m.contains("mini") => (0.0000025, 0.00001),
        ("openai", m) if m.contains("mini") => (0.00000015, 0.0000006),
        ("gemini", m) if m.contains("1.5-pro") || m.contains("2.5-pro") => (0.00000125, 0.000005),
        ("gemini", m) if m.contains("flash") => (0.000000075, 0.0000003),
        ("nvidia", _) => (0.000000235, 0.000000235),
        ("inception", _) => (0.00000025, 0.000001),
        _ => return None,
    };
    Some(rates)
}

/// Is this exact (provider, model) pair verified free to call?
pub fn is_free(provider: &str, model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    match provider {
        // Local inference — no meter at all.
        "ollama" => true,
        // Groq free tier (console.groq.com/docs/rate-limits): gpt-oss,
        // compound, llama, gemma, qwen and whisper models are $0 within the
        // org-level rate limits.
        "groq" => {
            m.contains("gpt-oss")
                || m.contains("compound")
                || m.starts_with("llama")
                || m.contains("llama")
                || m.starts_with("gemma")
                || m.contains("gemma")
                || m.contains("qwen")
                || m.contains("whisper")
        }
        // Cloudflare Workers AI free allocation (10k neurons/day on the free
        // plan) — this lane is only wired as a free lane.
        "cloudflare" => true,
        // OpenRouter free variants carry an explicit ":free" suffix, or the
        // "openrouter/free" auto-router. Everything else on OpenRouter bills
        // against credits.
        "openrouter" => m.ends_with(":free") || m == "openrouter/free",
        _ => false,
    }
}

/// Conservative non-zero default so unknown (potentially paid) models still
/// count against the daily budget.
const DEFAULT_INPUT_RATE: f64 = 0.000001;
const DEFAULT_OUTPUT_RATE: f64 = 0.000002;

pub fn estimate_cost(provider: &str, model: &str, input_tokens: i64, output_tokens: i64) -> f64 {
    if is_free(provider, model) {
        return 0.0;
    }
    let (input_rate, output_rate) =
        paid_rates(provider, model).unwrap_or((DEFAULT_INPUT_RATE, DEFAULT_OUTPUT_RATE));
    (input_tokens as f64 * input_rate) + (output_tokens as f64 * output_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_free_models_cost_zero() {
        assert_eq!(estimate_cost("groq", "openai/gpt-oss-120b", 10_000, 10_000), 0.0);
        assert_eq!(estimate_cost("groq", "groq/compound", 10_000, 10_000), 0.0);
        assert_eq!(estimate_cost("groq", "groq/compound-mini", 10_000, 10_000), 0.0);
        assert_eq!(estimate_cost("groq", "llama-3.3-70b-versatile", 10_000, 10_000), 0.0);
        assert_eq!(estimate_cost("groq", "whisper-large-v3-turbo", 10_000, 10_000), 0.0);
        assert_eq!(estimate_cost("cloudflare", "@cf/meta/llama-3.3-70b-instruct-fp8-fast", 10_000, 10_000), 0.0);
        assert_eq!(estimate_cost("ollama", "llama3.2", 10_000, 10_000), 0.0);
        assert_eq!(estimate_cost("openrouter", "meta-llama/llama-3.3-70b-instruct:free", 10_000, 10_000), 0.0);
        assert_eq!(estimate_cost("openrouter", "openrouter/free", 10_000, 10_000), 0.0);
    }

    #[test]
    fn paid_models_are_never_masked_as_free() {
        assert!(estimate_cost("anthropic", "claude-sonnet-4-20250514", 1_000, 1_000) > 0.0);
        assert!(estimate_cost("anthropic", "claude-opus-4-20250514", 1_000, 1_000) > 0.0);
        assert!(estimate_cost("openai", "gpt-4o", 1_000, 1_000) > 0.0);
        assert!(estimate_cost("openai", "gpt-4o-mini", 1_000, 1_000) > 0.0);
        assert!(estimate_cost("openrouter", "openai/gpt-4o", 1_000, 1_000) > 0.0);
        assert!(estimate_cost("inception", "mercury-2", 1_000, 1_000) > 0.0);
        // Unknown provider+model: conservative non-zero, never free.
        assert!(estimate_cost("some-new-provider", "some-new-model", 1_000, 1_000) > 0.0);
    }
}

//! Per-provider circuit breaker.
//!
//! Real traffic flips the breaker: `failure_threshold` consecutive failures
//! open the circuit, the provider is skipped while open, and after
//! `reset_seconds` a single trial request is allowed through (half-open).
//! State lives in memory and is mirrored to the `provider_status` table by
//! `AppState`, so a restart restores the last known health picture.

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    /// Healthy; `failures` counts consecutive failures so far.
    Closed { failures: u32 },
    /// Failing; requests blocked until `since + reset` elapses.
    Open { since: Instant },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CircuitSnapshot {
    pub provider: String,
    pub open: bool,
    pub consecutive_failures: u32,
}

pub struct CircuitBreaker {
    threshold: u32,
    reset: Duration,
    states: HashMap<String, CircuitState>,
}

impl CircuitBreaker {
    pub fn new(threshold: u32, reset_seconds: u64) -> Self {
        Self {
            threshold: threshold.max(1),
            reset: Duration::from_secs(reset_seconds),
            states: HashMap::new(),
        }
    }

    /// Should a request to this provider be allowed right now?
    /// An open circuit whose reset window has elapsed allows one trial
    /// (half-open); the breaker stays Open until the trial succeeds.
    pub fn allows(&self, provider: &str) -> bool {
        match self.states.get(provider) {
            Some(CircuitState::Open { since }) => since.elapsed() >= self.reset,
            _ => true,
        }
    }

    pub fn is_open(&self, provider: &str) -> bool {
        matches!(self.states.get(provider), Some(CircuitState::Open { .. }))
    }

    pub fn record_success(&mut self, provider: &str) {
        self.states
            .insert(provider.to_string(), CircuitState::Closed { failures: 0 });
    }

    pub fn record_failure(&mut self, provider: &str) {
        let failures = match self.states.get(provider) {
            Some(CircuitState::Closed { failures }) => failures + 1,
            // Already open (or half-open trial failed): re-open the timer.
            Some(CircuitState::Open { .. }) => self.threshold,
            None => 1,
        };
        let state = if failures >= self.threshold {
            CircuitState::Open {
                since: Instant::now(),
            }
        } else {
            CircuitState::Closed { failures }
        };
        self.states.insert(provider.to_string(), state);
    }

    /// Rehydrate from the provider_status table on boot.
    pub fn seed(&mut self, provider: &str, open: bool, failures: u32) {
        let state = if open {
            CircuitState::Open {
                since: Instant::now(),
            }
        } else {
            CircuitState::Closed { failures }
        };
        self.states.insert(provider.to_string(), state);
    }

    pub fn snapshot(&self) -> Vec<CircuitSnapshot> {
        let mut out: Vec<CircuitSnapshot> = self
            .states
            .iter()
            .map(|(provider, state)| CircuitSnapshot {
                provider: provider.clone(),
                open: matches!(state, CircuitState::Open { .. }),
                consecutive_failures: match state {
                    CircuitState::Closed { failures } => *failures,
                    CircuitState::Open { .. } => self.threshold,
                },
            })
            .collect();
        out.sort_by(|a, b| a.provider.cmp(&b.provider));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_after_threshold_and_blocks() {
        let mut cb = CircuitBreaker::new(3, 60);
        assert!(cb.allows("groq"));
        cb.record_failure("groq");
        cb.record_failure("groq");
        assert!(cb.allows("groq"), "still closed below threshold");
        assert!(!cb.is_open("groq"));
        cb.record_failure("groq");
        assert!(cb.is_open("groq"));
        assert!(!cb.allows("groq"));
        assert!(cb.allows("gemini"), "other providers unaffected");
    }

    #[test]
    fn success_resets() {
        let mut cb = CircuitBreaker::new(2, 60);
        cb.record_failure("groq");
        cb.record_failure("groq");
        assert!(!cb.allows("groq"));
        // half-open trial allowed after reset window
        let mut cb2 = CircuitBreaker::new(2, 0);
        cb2.record_failure("groq");
        cb2.record_failure("groq");
        assert!(cb2.allows("groq"), "zero reset = immediate half-open");
        cb2.record_success("groq");
        assert!(!cb2.is_open("groq"));
        let _ = cb;
    }

    #[test]
    fn failed_half_open_trial_reopens() {
        let mut cb = CircuitBreaker::new(2, 0);
        cb.record_failure("groq");
        cb.record_failure("groq");
        assert!(cb.allows("groq"));
        cb.record_failure("groq");
        assert!(cb.is_open("groq"));
    }

    #[test]
    fn snapshot_reports_state() {
        let mut cb = CircuitBreaker::new(1, 60);
        cb.record_failure("zai");
        let snap = cb.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].provider, "zai");
        assert!(snap[0].open);
    }
}

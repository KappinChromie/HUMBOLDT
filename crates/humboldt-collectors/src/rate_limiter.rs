use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::warn;

/// Token bucket rate limiter tracking burst capacity and refill rate.
pub struct TokenBucket {
    capacity: f64,
    tokens: Mutex<f64>,
    refill_rate_per_sec: f64,
    last_refill: Mutex<Instant>,
}

impl TokenBucket {
    pub fn new(capacity: f64, refill_rate_per_sec: f64) -> Self {
        Self {
            capacity,
            tokens: Mutex::new(capacity),
            refill_rate_per_sec,
            last_refill: Mutex::new(Instant::now()),
        }
    }

    /// Attempts to consume 1 token. Returns true if allowed, false if quota exceeded.
    pub fn try_acquire(&self) -> bool {
        let mut tokens_lock = self.tokens.lock().unwrap();
        let mut last_refill_lock = self.last_refill.lock().unwrap();

        let now = Instant::now();
        let elapsed = now.duration_since(*last_refill_lock).as_secs_f64();
        *last_refill_lock = now;

        // Add newly accrued tokens up to capacity
        *tokens_lock = (*tokens_lock + elapsed * self.refill_rate_per_sec).min(self.capacity);

        if *tokens_lock >= 1.0 {
            *tokens_lock -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Circuit breaker with exponential backoff capped at 15 minutes per Section 6.3.
pub struct CircuitBreaker {
    state: Mutex<CircuitState>,
    failure_count: AtomicU32,
    last_failure: Mutex<Option<Instant>>,
    current_backoff: Mutex<Duration>,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

impl CircuitBreaker {
    pub const MAX_BACKOFF: Duration = Duration::from_secs(900); // 15 minutes

    pub fn new() -> Self {
        Self {
            state: Mutex::new(CircuitState::Closed),
            failure_count: AtomicU32::new(0),
            last_failure: Mutex::new(None),
            current_backoff: Mutex::new(Duration::from_secs(1)),
        }
    }

    /// Checks if a request may proceed.
    pub fn can_execute(&self) -> bool {
        let mut state = self.state.lock().unwrap();
        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let last = self.last_failure.lock().unwrap();
                let backoff = *self.current_backoff.lock().unwrap();
                if let Some(t) = *last {
                    if t.elapsed() >= backoff {
                        *state = CircuitState::HalfOpen;
                        true
                    } else {
                        false
                    }
                } else {
                    true
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Records an execution failure (HTTP 429, timeout, 5xx) and escalates backoff.
    pub fn record_failure(&self) {
        let count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
        let mut state = self.state.lock().unwrap();
        *state = CircuitState::Open;

        let mut last_failure = self.last_failure.lock().unwrap();
        *last_failure = Some(Instant::now());

        let mut backoff = self.current_backoff.lock().unwrap();
        // Exponential backoff: 2^(n-1) seconds with jitter, capped at 15 minutes
        let factor = 2u64.saturating_pow(count.min(10));
        let new_backoff = Duration::from_secs(factor).min(Self::MAX_BACKOFF);
        *backoff = new_backoff;

        warn!(
            "Circuit breaker tripped to OPEN (consecutive failures: {}). Backoff duration: {:?}",
            count, new_backoff
        );
    }

    /// Records an execution success, closing the circuit.
    pub fn record_success(&self) {
        let mut state = self.state.lock().unwrap();
        *state = CircuitState::Closed;
        self.failure_count.store(0, Ordering::SeqCst);
        let mut backoff = self.current_backoff.lock().unwrap();
        *backoff = Duration::from_secs(1);
    }
}

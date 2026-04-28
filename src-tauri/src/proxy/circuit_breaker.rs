use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Simple circuit breaker for API entries.
/// State is kept in memory only (not persisted).
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    consecutive_failures: Arc<AtomicU32>,
    last_opened_at: Arc<RwLock<Option<Instant>>>,
    recovery_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new(recovery_secs: u64) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            consecutive_failures: Arc::new(AtomicU32::new(0)),
            last_opened_at: Arc::new(RwLock::new(None)),
            recovery_secs,
        }
    }

    pub fn is_available(&self) -> bool {
        let state = match self.state.try_read() {
            Ok(s) => s,
            Err(_) => return false, // Lock contention → treat as unavailable
        };
        match *state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => true,
            CircuitState::Open => {
                drop(state);
                let last_open = match self.last_opened_at.try_read() {
                    Ok(guard) => guard,
                    Err(_) => return false,
                };
                if let Some(opened_at) = *last_open {
                    if opened_at.elapsed().as_secs() >= self.recovery_secs {
                        drop(last_open);
                        if let Ok(mut s) = self.state.try_write() {
                            *s = CircuitState::HalfOpen;
                        }
                        return true;
                    }
                }
                false
            }
        }
    }

    pub fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        if let Ok(mut state) = self.state.try_write() {
            *state = CircuitState::Closed;
        }
    }

    pub fn record_failure(&self, threshold: u32) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
        if failures >= threshold {
            if let Ok(mut state) = self.state.try_write() {
                *state = CircuitState::Open;
            }
            if let Ok(mut last_opened) = self.last_opened_at.try_write() {
                *last_opened = Some(Instant::now());
            }
        }
    }

    pub fn get_state(&self) -> CircuitState {
        self.state
            .try_read()
            .map(|s| *s)
            .unwrap_or(CircuitState::Closed)
    }

    /// Update recovery_secs (e.g. when user changes the setting at runtime).
    pub fn set_recovery_secs(&mut self, secs: u64) {
        self.recovery_secs = secs;
    }
}

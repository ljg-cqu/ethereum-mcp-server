/// Circuit breaker implementation for external service reliability
/// Prevents cascade failures when RPC endpoints become unreliable
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,   // Normal operation
    Open,     // Circuit is open, failing fast
    HalfOpen, // Testing if service has recovered
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: usize,   // Number of failures before opening
    pub timeout_duration: Duration, // How long to stay open
    pub success_threshold: usize,   // Successes needed to close from half-open
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout_duration: Duration::from_secs(30),
            success_threshold: 3,
        }
    }
}

/// Circuit breaker for protecting against failing external services
#[derive(Debug)]
pub struct CircuitBreaker {
    state: std::sync::RwLock<CircuitState>,
    failure_count: AtomicUsize,
    success_count: AtomicUsize,
    last_failure_time: AtomicU64,
    config: CircuitBreakerConfig,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

impl CircuitBreaker {
    /// Create a new circuit breaker with default configuration
    pub fn new() -> Self {
        Self::with_config(CircuitBreakerConfig::default())
    }

    /// Create a new circuit breaker with custom configuration
    pub fn with_config(config: CircuitBreakerConfig) -> Self {
        Self {
            state: std::sync::RwLock::new(CircuitState::Closed),
            failure_count: AtomicUsize::new(0),
            success_count: AtomicUsize::new(0),
            last_failure_time: AtomicU64::new(0),
            config,
        }
    }

    /// Execute an operation through the circuit breaker
    pub async fn call<F, Fut, T, E>(&self, operation: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        // Check if circuit should be closed
        self.update_state();

        let current_state = {
            let state = self.state.read().unwrap();
            state.clone()
        };

        match current_state {
            CircuitState::Open => {
                debug!("Circuit breaker is open, failing fast");
                Err(CircuitBreakerError::CircuitOpen)
            }
            CircuitState::Closed | CircuitState::HalfOpen => match operation().await {
                Ok(result) => {
                    self.on_success();
                    Ok(result)
                }
                Err(error) => {
                    self.on_failure();
                    Err(CircuitBreakerError::OperationFailed(error))
                }
            },
        }
    }

    /// Get current circuit breaker state
    pub fn state(&self) -> CircuitState {
        let state = self.state.read().unwrap();
        state.clone()
    }

    /// Get failure count
    pub fn failure_count(&self) -> usize {
        self.failure_count.load(Ordering::Relaxed)
    }

    /// Record a successful operation
    fn on_success(&self) {
        let current_state = {
            let state = self.state.read().unwrap();
            state.clone()
        };

        match current_state {
            CircuitState::HalfOpen => {
                let success_count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
                if success_count >= self.config.success_threshold {
                    self.close_circuit();
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::Open => {
                // Should not happen, but reset counts
                self.failure_count.store(0, Ordering::Relaxed);
                self.success_count.store(0, Ordering::Relaxed);
            }
        }
    }

    /// Record a failed operation
    fn on_failure(&self) {
        let failure_count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;

        // Record timestamp of failure
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_failure_time.store(now, Ordering::Relaxed);

        let current_state = {
            let state = self.state.read().unwrap();
            state.clone()
        };

        match current_state {
            CircuitState::Closed => {
                if failure_count >= self.config.failure_threshold {
                    self.open_circuit();
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open state reopens the circuit
                self.open_circuit();
            }
            CircuitState::Open => {
                // Already open, nothing to do
            }
        }
    }

    /// Open the circuit (start failing fast)
    fn open_circuit(&self) {
        {
            let mut state = self.state.write().unwrap();
            *state = CircuitState::Open;
        }
        self.success_count.store(0, Ordering::Relaxed);
        warn!(
            failure_count = self.failure_count.load(Ordering::Relaxed),
            "Circuit breaker opened due to failures"
        );
    }

    /// Close the circuit (normal operation)
    fn close_circuit(&self) {
        {
            let mut state = self.state.write().unwrap();
            *state = CircuitState::Closed;
        }
        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        debug!("Circuit breaker closed, normal operation resumed");
    }

    /// Transition to half-open state
    fn half_open_circuit(&self) {
        {
            let mut state = self.state.write().unwrap();
            *state = CircuitState::HalfOpen;
        }
        self.success_count.store(0, Ordering::Relaxed);
        debug!("Circuit breaker transitioned to half-open state");
    }

    /// Update circuit state based on timeout
    fn update_state(&self) {
        let current_state = {
            let state = self.state.read().unwrap();
            state.clone()
        };

        if current_state == CircuitState::Open {
            let last_failure = self.last_failure_time.load(Ordering::Relaxed);
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if now - last_failure >= self.config.timeout_duration.as_secs() {
                self.half_open_circuit();
            }
        }
    }
}

/// Circuit breaker error types
#[derive(Debug)]
pub enum CircuitBreakerError<E> {
    CircuitOpen,
    OperationFailed(E),
}

impl<E: std::fmt::Display> std::fmt::Display for CircuitBreakerError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitBreakerError::CircuitOpen => write!(f, "Circuit breaker is open"),
            CircuitBreakerError::OperationFailed(e) => write!(f, "Operation failed: {}", e),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CircuitBreakerError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CircuitBreakerError::CircuitOpen => None,
            CircuitBreakerError::OperationFailed(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_circuit_breaker_normal_operation() {
        let breaker = CircuitBreaker::new();

        // Successful operation should work
        let result = breaker.call(|| async { Ok::<i32, String>(42) }).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_on_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout_duration: Duration::from_secs(60), // Long timeout to prevent auto-recovery during test
            success_threshold: 1,
        };
        let breaker = CircuitBreaker::with_config(config);

        // First failure
        let result = breaker
            .call(|| async { Err::<i32, String>("error".to_string()) })
            .await;
        assert!(result.is_err());
        assert_eq!(breaker.state(), CircuitState::Closed);

        // Second failure should open circuit
        let result = breaker
            .call(|| async { Err::<i32, String>("error".to_string()) })
            .await;
        assert!(result.is_err());

        // Circuit should now be open
        assert_eq!(breaker.state(), CircuitState::Open);

        // Next call should fail fast (circuit is open)
        let result = breaker.call(|| async { Ok::<i32, String>(42) }).await;
        assert!(matches!(result, Err(CircuitBreakerError::CircuitOpen)));

        // Circuit should still be open
        assert_eq!(breaker.state(), CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_recovery() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            timeout_duration: Duration::from_millis(50),
            success_threshold: 1,
        };
        let breaker = CircuitBreaker::with_config(config);

        // Cause failure to open circuit
        let result = breaker
            .call(|| async { Err::<i32, String>("error".to_string()) })
            .await;
        assert!(result.is_err());
        assert_eq!(breaker.state(), CircuitState::Open);

        // Wait for timeout
        sleep(Duration::from_millis(60)).await;

        // Next call should transition to half-open
        let result = breaker.call(|| async { Ok::<i32, String>(42) }).await;
        assert!(result.is_ok());
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_state_transitions() {
        let breaker = CircuitBreaker::new();
        assert_eq!(breaker.state(), CircuitState::Closed);
        assert_eq!(breaker.failure_count(), 0);

        // Test failure recording
        breaker.on_failure();
        assert_eq!(breaker.failure_count(), 1);
        assert_eq!(breaker.state(), CircuitState::Closed);
    }
}

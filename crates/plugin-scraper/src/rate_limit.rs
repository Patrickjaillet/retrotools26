use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Enforces a minimum delay between calls and retries transient failures
/// with a linearly increasing backoff. The sleep function is injectable so
/// tests can verify call counts/timing without a real multi-second test.
pub struct RateLimiter {
    min_interval: Duration,
    max_retries: u32,
    last_call: Mutex<Option<Instant>>,
    sleep_fn: Box<dyn Fn(Duration) + Send + Sync>,
}

impl RateLimiter {
    pub fn new(min_interval: Duration, max_retries: u32) -> Self {
        Self {
            min_interval,
            max_retries,
            last_call: Mutex::new(None),
            sleep_fn: Box::new(std::thread::sleep),
        }
    }

    #[cfg(test)]
    pub fn with_sleep_fn(min_interval: Duration, max_retries: u32, sleep_fn: impl Fn(Duration) + Send + Sync + 'static) -> Self {
        Self {
            min_interval,
            max_retries,
            last_call: Mutex::new(None),
            sleep_fn: Box::new(sleep_fn),
        }
    }

    fn wait_for_slot(&self) {
        let mut last_call = self.last_call.lock().expect("rate limiter mutex poisoned");
        if let Some(previous) = *last_call {
            let elapsed = previous.elapsed();
            if elapsed < self.min_interval {
                (self.sleep_fn)(self.min_interval - elapsed);
            }
        }
        *last_call = Some(Instant::now());
    }

    /// Runs `call`, retrying up to `max_retries` times (with linearly
    /// increasing backoff) when it returns `Err(true)` (a transient
    /// failure worth retrying — e.g. a quota/5xx response), and always
    /// respecting the minimum interval between attempts. `Err(false)`
    /// (a permanent failure, e.g. bad credentials) is returned immediately.
    pub fn run<T>(&self, mut call: impl FnMut() -> Result<T, (String, bool)>) -> Result<T, String> {
        let mut attempt = 0;
        loop {
            self.wait_for_slot();
            match call() {
                Ok(value) => return Ok(value),
                Err((message, transient)) => {
                    if !transient || attempt >= self.max_retries {
                        return Err(message);
                    }
                    attempt += 1;
                    (self.sleep_fn)(self.min_interval * attempt);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn retries_transient_failures_up_to_the_limit_then_gives_up() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = calls.clone();
        let limiter = RateLimiter::with_sleep_fn(Duration::from_millis(1), 2, |_| {});
        let result: Result<(), String> = limiter.run(|| {
            calls_clone.fetch_add(1, Ordering::SeqCst);
            Err(("still failing".to_string(), true))
        });
        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 3, "1 initial attempt + 2 retries");
    }

    #[test]
    fn stops_retrying_immediately_on_a_permanent_failure() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = calls.clone();
        let limiter = RateLimiter::with_sleep_fn(Duration::from_millis(1), 5, |_| {});
        let result: Result<(), String> = limiter.run(|| {
            calls_clone.fetch_add(1, Ordering::SeqCst);
            Err(("bad credentials".to_string(), false))
        });
        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn succeeds_without_retrying_when_the_call_works_first_try() {
        let limiter = RateLimiter::with_sleep_fn(Duration::from_millis(1), 3, |_| {});
        let result = limiter.run(|| Ok::<_, (String, bool)>(42));
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn enforces_the_minimum_interval_between_calls() {
        let sleeps = Arc::new(AtomicUsize::new(0));
        let sleeps_clone = sleeps.clone();
        let limiter = RateLimiter::with_sleep_fn(Duration::from_millis(50), 0, move |_| {
            sleeps_clone.fetch_add(1, Ordering::SeqCst);
        });
        let _ = limiter.run(|| Ok::<_, (String, bool)>(()));
        let _ = limiter.run(|| Ok::<_, (String, bool)>(()));
        assert_eq!(sleeps.load(Ordering::SeqCst), 1, "second call should wait, first should not");
    }
}

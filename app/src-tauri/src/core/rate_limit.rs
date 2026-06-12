use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tracing::{debug, warn};

#[derive(Debug)]
struct RateLimitState {
    window_started_at: Instant,
    used_weight: u32,
    consecutive_429: u32,
    circuit_open_until: Option<Instant>,
    last_pause_log_until: Option<Instant>,
}

#[derive(Clone, Debug)]
pub struct BinanceRateLimiter {
    state: Arc<Mutex<RateLimitState>>,
    semaphore: Arc<Semaphore>,
    max_weight_per_minute: u32,
    safety_weight_per_minute: u32,
}

impl BinanceRateLimiter {
    pub fn new(max_weight_per_minute: u32, safety_ratio: f64, max_concurrency: usize) -> Self {
        let safety_weight_per_minute = ((max_weight_per_minute as f64) * safety_ratio)
            .round()
            .clamp(1.0, max_weight_per_minute as f64) as u32;

        Self {
            state: Arc::new(Mutex::new(RateLimitState {
                window_started_at: Instant::now(),
                used_weight: 0,
                consecutive_429: 0,
                circuit_open_until: None,
                last_pause_log_until: None,
            })),
            semaphore: Arc::new(Semaphore::new(max_concurrency.max(1))),
            max_weight_per_minute,
            safety_weight_per_minute,
        }
    }

    pub async fn acquire(&self, endpoint: &str, weight: u32) -> OwnedSemaphorePermit {
        loop {
            let sleep_for = {
                let mut state = self.state.lock().await;
                let now = Instant::now();

                if let Some(until) = state.circuit_open_until {
                    if until > now {
                        Some(until - now)
                    } else {
                        state.circuit_open_until = None;
                        None
                    }
                } else if now.duration_since(state.window_started_at) >= Duration::from_secs(60) {
                    state.window_started_at = now;
                    state.used_weight = 0;
                    None
                } else if state.used_weight.saturating_add(weight) > self.safety_weight_per_minute {
                    let window_remaining =
                        Duration::from_secs(60) - now.duration_since(state.window_started_at);
                    // Jitter 0-200ms: trải đều burst sau khi window reset.
                    // Dùng subsec_nanos() làm pseudo-random seed — không cần crate rand,
                    // mỗi task gọi acquire() ở nanosecond khác nhau nên jitter khác nhau.
                    let jitter_ms = (std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .subsec_nanos()
                        % 200) as u64;
                    let duration = window_remaining + Duration::from_millis(jitter_ms);
                    let should_warn = state
                        .last_pause_log_until
                        .map(|until| until <= now)
                        .unwrap_or(true);
                    if should_warn {
                        warn!(
                            endpoint,
                            weight,
                            used_weight = state.used_weight,
                            safety_weight = self.safety_weight_per_minute,
                            sleep_ms = duration.as_millis(),
                            "Binance REST limiter pausing request queue"
                        );
                        state.last_pause_log_until = Some(now + window_remaining);
                    } else {
                        debug!(
                            endpoint,
                            weight,
                            used_weight = state.used_weight,
                            safety_weight = self.safety_weight_per_minute,
                            sleep_ms = duration.as_millis(),
                            "Binance REST request waiting for next weight window"
                        );
                    }
                    Some(duration)
                } else {
                    state.used_weight = state.used_weight.saturating_add(weight);
                    debug!(
                        endpoint,
                        weight,
                        used_weight = state.used_weight,
                        safety_weight = self.safety_weight_per_minute,
                        "Binance REST capacity reserved"
                    );
                    None
                }
            };

            if let Some(duration) = sleep_for {
                tokio::time::sleep(duration).await;
                continue;
            }

            return self
                .semaphore
                .clone()
                .acquire_owned()
                .await
                .expect("rate limiter semaphore closed");
        }
    }

    pub async fn observe_headers(&self, used_weight: Option<u32>) {
        if let Some(remote_used) = used_weight {
            let mut state = self.state.lock().await;
            state.used_weight = state.used_weight.max(remote_used);

            if remote_used >= self.safety_weight_per_minute {
                let now = Instant::now();
                let elapsed = now.duration_since(state.window_started_at);
                let pause = Duration::from_secs(60).saturating_sub(elapsed);
                state.circuit_open_until = Some(now + pause);
                warn!(
                    remote_used,
                    max_weight = self.max_weight_per_minute,
                    safety_weight = self.safety_weight_per_minute,
                    pause_ms = pause.as_millis(),
                    "Binance REST used weight reached safety threshold"
                );
            }
        }
    }

    pub async fn observe_status(&self, status: reqwest::StatusCode) {
        let mut state = self.state.lock().await;
        match status.as_u16() {
            429 => {
                state.consecutive_429 = state.consecutive_429.saturating_add(1);
                let backoff_secs = (2_u64.pow(state.consecutive_429.min(5))).max(30);
                state.circuit_open_until = Some(Instant::now() + Duration::from_secs(backoff_secs));
                warn!(
                    backoff_secs,
                    "Binance REST returned 429; opening throttle circuit"
                );
            }
            418 => {
                state.consecutive_429 = state.consecutive_429.saturating_add(1);
                state.circuit_open_until = Some(Instant::now() + Duration::from_secs(15 * 60));
                warn!("Binance REST returned 418; pausing REST calls for 15 minutes");
            }
            403 => {
                state.circuit_open_until = Some(Instant::now() + Duration::from_secs(5 * 60));
                warn!("Binance REST returned 403; pausing REST calls for 5 minutes");
            }
            200..=299 => {
                state.consecutive_429 = 0;
            }
            503 => {
                state.circuit_open_until = Some(Instant::now() + Duration::from_secs(5));
                warn!("Binance REST returned 503; short backoff");
            }
            _ => {}
        }
    }
}

impl Default for BinanceRateLimiter {
    fn default() -> Self {
        Self::new(2400, 0.65, 8)
    }
}

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use time::OffsetDateTime;

pub trait Clock: Send + Sync {
    fn utc_now(&self) -> OffsetDateTime;
    fn monotonic_now(&self) -> Duration;
}

pub struct SystemClock {
    started: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn utc_now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    fn monotonic_now(&self) -> Duration {
        self.started.elapsed()
    }
}

#[derive(Clone)]
pub struct FixedClock {
    inner: Arc<Mutex<FixedTime>>,
}

struct FixedTime {
    utc: OffsetDateTime,
    monotonic: Duration,
}

impl FixedClock {
    pub fn new(utc: OffsetDateTime) -> Self {
        Self {
            inner: Arc::new(Mutex::new(FixedTime {
                utc,
                monotonic: Duration::ZERO,
            })),
        }
    }

    pub fn advance(&self, duration: Duration) {
        let mut inner = self.inner.lock().expect("fixed clock poisoned");
        inner.monotonic += duration;
        inner.utc += time::Duration::try_from(duration).expect("duration fits time::Duration");
    }

    pub fn set_utc(&self, utc: OffsetDateTime) {
        self.inner.lock().expect("fixed clock poisoned").utc = utc;
    }
}

impl Clock for FixedClock {
    fn utc_now(&self) -> OffsetDateTime {
        self.inner.lock().expect("fixed clock poisoned").utc
    }

    fn monotonic_now(&self) -> Duration {
        self.inner.lock().expect("fixed clock poisoned").monotonic
    }
}

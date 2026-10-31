use std::time::{Duration, Instant};

/// A timeout is a point in time after which an analysis should give up and return "unknown".
pub trait Timeout {
    fn into_instant(self) -> Instant;
}

impl Timeout for Instant {
    fn into_instant(self) -> Instant {
        self
    }
}

impl Timeout for Duration {
    fn into_instant(self) -> Instant {
        Instant::now() + self
    }
}
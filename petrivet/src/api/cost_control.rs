use std::time::{Duration, Instant};

/// An absolute or relative timeout for a procedure.
/// `petrivet` will stop the procedure when the timeout is reached
/// (on a best-effort basis, since some procedures may not be interruptible).
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

/// A procedure complexity control mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effort {
    /// Only efficient (polynomial-time) procedures will be used;
    /// if none are available, the analysis will return `None`.
    Efficient,
    /// Use any available procedure, even if it is exponential-time.
    Any,
}

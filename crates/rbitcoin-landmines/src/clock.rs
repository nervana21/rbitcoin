//! Pre-qday / post-qday activation clock (stub).

/// Caller-supplied chain time or height for phase checks.
pub trait ActivationClock {
    /// True once post-qday rules are active.
    fn is_post_qday(&self) -> bool;

    /// Monotonic time or height used for challenge timeouts.
    fn now(&self) -> u64;
}

/// Test / sim clock. Not consensus wall time.
#[derive(Debug, Clone, Copy)]
pub struct StubClock {
    /// Whether post-qday rules apply.
    pub post_qday: bool,
    /// Current height-or-time unit.
    pub now: u64,
}

impl ActivationClock for StubClock {
    fn is_post_qday(&self) -> bool {
        self.post_qday
    }

    fn now(&self) -> u64 {
        self.now
    }
}

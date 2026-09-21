use anyhow::{ensure, Result};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub(super) struct Deadline(Instant);

impl Deadline {
    pub fn from_stored(deadline: &str) -> Result<Self> {
        let deadline = chrono::DateTime::parse_from_rfc3339(deadline)?;
        let remaining = deadline
            .signed_duration_since(chrono::Utc::now())
            .to_std()
            .map_err(|_| anyhow::anyhow!("continuation_budget_exceeded: stored deadline"))?;
        ensure!(
            !remaining.is_zero(),
            "continuation_budget_exceeded: stored deadline"
        );
        Ok(Self::after(remaining.min(Duration::from_secs(15 * 60))))
    }

    pub fn after(duration: Duration) -> Self {
        Self(Instant::now() + duration)
    }

    pub fn within(self, duration: Duration) -> Self {
        Self(self.0.min(Instant::now() + duration))
    }

    pub fn remaining(self) -> Result<Duration> {
        let remaining = self.0.saturating_duration_since(Instant::now());
        ensure!(
            !remaining.is_zero(),
            "continuation_budget_exceeded: deadline"
        );
        Ok(remaining)
    }

    pub fn check(self) -> Result<()> {
        self.remaining().map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_deadline_does_not_restart_fifteen_minute_budget() {
        let persisted = (chrono::Utc::now() + chrono::Duration::seconds(120)).to_rfc3339();
        let remaining = Deadline::from_stored(&persisted)
            .unwrap()
            .remaining()
            .unwrap();
        assert!(remaining <= Duration::from_secs(120));
        assert!(Deadline::from_stored("2020-01-01T00:00:00Z").is_err());
    }
}

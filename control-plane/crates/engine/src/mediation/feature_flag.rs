//! P017 Phase B: Feature flag for lead mediation.
//!
//! `p017_phase_b_lead_mediation_enabled` defaults to off.
//! Enablement requires Phase 0 green, B1 merged, and pre-dogfood gate passed.

use std::sync::atomic::{AtomicBool, Ordering};

static PHASE_B_MEDIATION_ENABLED: AtomicBool = AtomicBool::new(false);

/// Check whether Phase B lead mediation is enabled.
pub fn is_phase_b_mediation_enabled() -> bool {
    // Also check env var for runtime override
    if std::env::var("P017_PHASE_B_LEAD_MEDIATION_ENABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
    {
        return true;
    }
    PHASE_B_MEDIATION_ENABLED.load(Ordering::Relaxed)
}

/// Enable Phase B lead mediation at runtime. Called after pre-dogfood gate passes.
pub fn enable_phase_b_mediation() {
    PHASE_B_MEDIATION_ENABLED.store(true, Ordering::Relaxed);
}

/// Disable Phase B lead mediation at runtime.
pub fn disable_phase_b_mediation() {
    PHASE_B_MEDIATION_ENABLED.store(false, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_disabled() {
        // Reset to known state
        disable_phase_b_mediation();
        assert!(!is_phase_b_mediation_enabled());
    }

    #[test]
    fn can_enable_and_disable() {
        // Use the atomic directly to avoid env var interference
        PHASE_B_MEDIATION_ENABLED.store(true, Ordering::Relaxed);
        assert!(PHASE_B_MEDIATION_ENABLED.load(Ordering::Relaxed));
        PHASE_B_MEDIATION_ENABLED.store(false, Ordering::Relaxed);
        assert!(!PHASE_B_MEDIATION_ENABLED.load(Ordering::Relaxed));
    }
}

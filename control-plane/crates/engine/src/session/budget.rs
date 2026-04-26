#[derive(Clone, Debug, PartialEq)]
pub struct BudgetSignals {
    pub turn_count: i64,
    pub estimated_input_tokens: i64,
    pub cumulative_prompt_tokens: i64,
    pub cumulative_cost_cents: i64,
    pub idle_age_seconds: f64,
    pub transcript_growth_ratio: Option<f64>,
    pub cached_token_share: Option<f64>,
    pub normalized_savings_versus_fresh: Option<f64>,
    pub effective_prompt_size_fraction: Option<f64>,
    pub compaction_churn_count: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BudgetConfig {
    pub max_turns: i64,
    pub max_estimated_input_tokens: i64,
    pub max_cumulative_prompt_tokens: i64,
    pub max_cumulative_cost_cents: i64,
    pub max_idle_age_seconds: f64,
    pub max_transcript_growth_ratio: f64,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            max_turns: 20,
            max_estimated_input_tokens: 128_000,
            max_cumulative_prompt_tokens: 1_000_000,
            max_cumulative_cost_cents: 500,
            max_idle_age_seconds: 14_400.0,
            max_transcript_growth_ratio: 2.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BudgetDecision {
    ContinueReuse,
    Compact { reason: String },
    Invalidate { reason: String },
}

pub fn evaluate(signals: &BudgetSignals, config: &BudgetConfig) -> BudgetDecision {
    if signals.turn_count >= config.max_turns {
        return BudgetDecision::Compact {
            reason: format!(
                "turn_count {} exceeded {}",
                signals.turn_count, config.max_turns
            ),
        };
    }
    if signals.estimated_input_tokens >= config.max_estimated_input_tokens {
        return BudgetDecision::Compact {
            reason: format!(
                "estimated_input_tokens {} exceeded {}",
                signals.estimated_input_tokens, config.max_estimated_input_tokens
            ),
        };
    }
    if signals.cumulative_prompt_tokens >= config.max_cumulative_prompt_tokens {
        return BudgetDecision::Invalidate {
            reason: format!(
                "cumulative_prompt_tokens {} exceeded {}",
                signals.cumulative_prompt_tokens, config.max_cumulative_prompt_tokens
            ),
        };
    }
    if signals.cumulative_cost_cents >= config.max_cumulative_cost_cents {
        return BudgetDecision::Invalidate {
            reason: format!(
                "cumulative_cost_cents {} exceeded {}",
                signals.cumulative_cost_cents, config.max_cumulative_cost_cents
            ),
        };
    }
    if signals.idle_age_seconds >= config.max_idle_age_seconds {
        return BudgetDecision::Invalidate {
            reason: format!(
                "idle_age_seconds {} exceeded {}",
                signals.idle_age_seconds, config.max_idle_age_seconds
            ),
        };
    }

    if signals.effective_prompt_size_fraction.unwrap_or(0.0) > 0.5 {
        return BudgetDecision::Compact {
            reason: "effective_prompt_size_fraction exceeded 0.5".into(),
        };
    }
    if signals.cached_token_share.unwrap_or(1.0) < 0.2 && signals.estimated_input_tokens > 50_000 {
        return BudgetDecision::Compact {
            reason: "low_cached_token_share_with_large_prompt".into(),
        };
    }
    if signals.transcript_growth_ratio.unwrap_or(1.0) > config.max_transcript_growth_ratio {
        return BudgetDecision::Compact {
            reason: format!(
                "transcript_growth_ratio {} exceeded {}",
                signals.transcript_growth_ratio.unwrap_or_default(),
                config.max_transcript_growth_ratio
            ),
        };
    }
    if signals.normalized_savings_versus_fresh.unwrap_or(0.0) < -0.05 {
        return BudgetDecision::Invalidate {
            reason: "normalized_savings_versus_fresh below -0.05".into(),
        };
    }
    if signals.compaction_churn_count >= 3 {
        return BudgetDecision::Invalidate {
            reason: "compaction_churn_count exceeded".into(),
        };
    }

    BudgetDecision::ContinueReuse
}

#[cfg(test)]
mod tests {
    use super::{evaluate, BudgetConfig, BudgetDecision, BudgetSignals};

    fn default_config() -> BudgetConfig {
        BudgetConfig::default()
    }

    fn base_signals() -> BudgetSignals {
        BudgetSignals {
            turn_count: 1,
            estimated_input_tokens: 8_000,
            cumulative_prompt_tokens: 40_000,
            cumulative_cost_cents: 25,
            idle_age_seconds: 30.0,
            transcript_growth_ratio: Some(1.1),
            cached_token_share: Some(0.6),
            normalized_savings_versus_fresh: Some(0.10),
            effective_prompt_size_fraction: Some(0.2),
            compaction_churn_count: 0,
        }
    }

    #[test]
    fn compacts_when_turn_budget_is_exhausted() {
        assert!(matches!(
            evaluate(
                &BudgetSignals {
                    turn_count: 20,
                    ..base_signals()
                },
                &default_config()
            ),
            BudgetDecision::Compact { .. }
        ));
    }

    #[test]
    fn invalidates_when_cumulative_cost_budget_is_exhausted() {
        assert!(matches!(
            evaluate(
                &BudgetSignals {
                    cumulative_cost_cents: 500,
                    ..base_signals()
                },
                &default_config()
            ),
            BudgetDecision::Invalidate { .. }
        ));
    }

    #[test]
    fn invalidates_when_reuse_is_economically_worse_than_fresh() {
        assert!(matches!(
            evaluate(
                &BudgetSignals {
                    normalized_savings_versus_fresh: Some(-0.10),
                    ..base_signals()
                },
                &default_config()
            ),
            BudgetDecision::Invalidate { .. }
        ));
    }

    #[test]
    fn compacts_when_transcript_growth_ratio_exceeds_threshold() {
        assert!(matches!(
            evaluate(
                &BudgetSignals {
                    transcript_growth_ratio: Some(2.5),
                    ..base_signals()
                },
                &default_config()
            ),
            BudgetDecision::Compact { .. }
        ));
    }

    #[test]
    fn continues_when_all_budget_signals_have_headroom() {
        assert_eq!(
            evaluate(&base_signals(), &default_config()),
            BudgetDecision::ContinueReuse
        );
    }
}

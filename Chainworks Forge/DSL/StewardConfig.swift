import Foundation

nonisolated struct StewardConfig: Codable, Hashable, Sendable {
    let schemaVersion: Int
    let windows: WindowConfig
    let thresholds: [String: ThresholdEntry]
    let triggers: TriggerConfig
    let contextStrategyProfiles: [String: StewardContextStrategyProfile]

    enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case windows, thresholds, triggers
        case contextStrategyProfiles = "context_strategy_profiles"
    }
}

nonisolated struct WindowConfig: Codable, Hashable, Sendable {
    let observationWindowSize: Int
    let baselineWindowSize: Int
    let minimumWindowSize: Int
    let maximumWindowAgeDays: Int

    enum CodingKeys: String, CodingKey {
        case observationWindowSize = "observation_window_size"
        case baselineWindowSize = "baseline_window_size"
        case minimumWindowSize = "minimum_window_size"
        case maximumWindowAgeDays = "maximum_window_age_days"
    }
}

nonisolated struct ThresholdEntry: Codable, Hashable, Sendable {
    let method: String
    let trigger: Double
}

nonisolated struct TriggerConfig: Codable, Hashable, Sendable {
    let postRunHook: PostRunHookConfig
    let onConfigChange: OnConfigChangeConfig
    let schedule: ScheduleConfig

    enum CodingKeys: String, CodingKey {
        case postRunHook = "post_run_hook"
        case onConfigChange = "on_config_change"
        case schedule
    }
}

nonisolated struct PostRunHookConfig: Codable, Hashable, Sendable {
    let enabled: Bool
    let runInterval: Int

    enum CodingKeys: String, CodingKey {
        case enabled
        case runInterval = "run_interval"
    }
}

nonisolated struct OnConfigChangeConfig: Codable, Hashable, Sendable {
    let enabled: Bool
}

nonisolated struct ScheduleConfig: Codable, Hashable, Sendable {
    let enabled: Bool
    let cron: String
}

// MARK: - Context Strategy Profiles

nonisolated enum StewardHandoffMode: String, Codable, Sendable {
    case selective
    case fullForward = "full_forward"
    case full
}

nonisolated enum StewardContextContinuityMode: String, Codable, Sendable {
    case familyWithinRun = "family_within_run"
    case none
}

nonisolated struct StewardContextStrategyProfile: Codable, Hashable, Sendable {
    let defaultHandoffMode: StewardHandoffMode
    let defaultModelTier: String?
    let escalationModelTier: String?
    let agents: [String: StewardContextStrategyAgentProfile]

    enum CodingKeys: String, CodingKey {
        case defaultHandoffMode = "default_handoff_mode"
        case defaultModelTier = "default_model_tier"
        case escalationModelTier = "escalation_model_tier"
        case agents
    }
}

nonisolated struct StewardContextStrategyAgentProfile: Codable, Hashable, Sendable {
    let continuityMode: StewardContextContinuityMode?
    let handoffPolicy: StewardContextHandoffPolicy?

    enum CodingKeys: String, CodingKey {
        case continuityMode = "continuity_mode"
        case handoffPolicy = "handoff_policy"
    }
}

nonisolated struct StewardContextHandoffPolicy: Codable, Hashable, Sendable {
    let mandatory: [String]
    let summarized: [String]
    let lazy: [String]

    enum CodingKeys: String, CodingKey {
        case mandatory
        case summarized
        case lazy
    }

    init(
        mandatory: [String] = [],
        summarized: [String] = [],
        lazy: [String] = []
    ) {
        self.mandatory = mandatory
        self.summarized = summarized
        self.lazy = lazy
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.mandatory = try container.decodeIfPresent([String].self, forKey: .mandatory) ?? []
        self.summarized = try container.decodeIfPresent([String].self, forKey: .summarized) ?? []
        self.lazy = try container.decodeIfPresent([String].self, forKey: .lazy) ?? []
    }
}

// MARK: - Defaults

extension StewardConfig {
    static let defaultConfig = StewardConfig(
        schemaVersion: 1,
        windows: WindowConfig(
            observationWindowSize: 20,
            baselineWindowSize: 20,
            minimumWindowSize: 5,
            maximumWindowAgeDays: 90
        ),
        thresholds: [
            "timing": ThresholdEntry(method: "median_percentage", trigger: 0.30),
            "rework": ThresholdEntry(method: "mean_percentage", trigger: 0.50),
            "quality": ThresholdEntry(method: "ratio", trigger: 2.0),
            "cost": ThresholdEntry(method: "median_percentage", trigger: 0.25),
            "stability": ThresholdEntry(method: "ratio", trigger: 2.0),
        ],
        triggers: TriggerConfig(
            postRunHook: PostRunHookConfig(enabled: true, runInterval: 1),
            onConfigChange: OnConfigChangeConfig(enabled: true),
            schedule: ScheduleConfig(enabled: false, cron: "0 8 * * 1")
        ),
        contextStrategyProfiles: [
            "current_mixed_baseline": StewardContextStrategyProfile(
                defaultHandoffMode: .fullForward,
                defaultModelTier: nil,
                escalationModelTier: nil,
                agents: [
                    "*": StewardContextStrategyAgentProfile(
                        continuityMode: nil,
                        handoffPolicy: StewardContextHandoffPolicy(
                            mandatory: ["current_task_description"],
                            summarized: [],
                            lazy: ["*"]
                        )
                    )
                ]
            ),
            "manual_like_long_continuity": StewardContextStrategyProfile(
                defaultHandoffMode: .fullForward,
                defaultModelTier: nil,
                escalationModelTier: nil,
                agents: [
                    "lead_orchestrator": StewardContextStrategyAgentProfile(
                        continuityMode: .familyWithinRun,
                        handoffPolicy: nil
                    ),
                    "proposal_writer": StewardContextStrategyAgentProfile(
                        continuityMode: .familyWithinRun,
                        handoffPolicy: nil
                    ),
                    "*": StewardContextStrategyAgentProfile(
                        continuityMode: StewardContextContinuityMode.none,
                        handoffPolicy: StewardContextHandoffPolicy(
                            mandatory: ["current_task_description"],
                            summarized: [],
                            lazy: ["*"]
                        )
                    )
                ]
            ),
            "selective_compression_and_escalation": StewardContextStrategyProfile(
                defaultHandoffMode: .selective,
                defaultModelTier: "fast",
                escalationModelTier: "frontier",
                agents: [
                    "proposal_writer": StewardContextStrategyAgentProfile(
                        continuityMode: nil,
                        handoffPolicy: StewardContextHandoffPolicy(
                            mandatory: [
                                "idea_brief",
                                "proposal_current",
                                "review_corpus_bundle",
                                "proposal_review_po",
                                "proposal_review_ux",
                                "proposal_review_ui",
                                "proposal_review_architect",
                                "proposal_review_summary",
                                "score_lift_backlog",
                                "proposal_fact_digest"
                            ],
                            summarized: [],
                            lazy: ["security_audit_raw"]
                        )
                    ),
                    "*": StewardContextStrategyAgentProfile(
                        continuityMode: nil,
                        handoffPolicy: StewardContextHandoffPolicy(
                            mandatory: ["current_task_description"],
                            summarized: [],
                            lazy: ["*"]
                        )
                    )
                ]
            ),
            "fresh_control": StewardContextStrategyProfile(
                defaultHandoffMode: .fullForward,
                defaultModelTier: "frontier",
                escalationModelTier: nil,
                agents: [
                    "*": StewardContextStrategyAgentProfile(
                        continuityMode: StewardContextContinuityMode.none,
                        handoffPolicy: StewardContextHandoffPolicy(
                            mandatory: ["current_task_description"],
                            summarized: [],
                            lazy: ["*"]
                        )
                    )
                ]
            )
        ]
    )
}

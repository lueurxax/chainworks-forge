import Foundation

struct StewardConfig: Codable, Hashable, Sendable {
    let schemaVersion: Int
    let windows: WindowConfig
    let thresholds: [String: ThresholdEntry]
    let triggers: TriggerConfig

    enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case windows, thresholds, triggers
    }
}

struct WindowConfig: Codable, Hashable, Sendable {
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

struct ThresholdEntry: Codable, Hashable, Sendable {
    let method: String
    let trigger: Double
}

struct TriggerConfig: Codable, Hashable, Sendable {
    let postRunHook: PostRunHookConfig
    let onConfigChange: OnConfigChangeConfig
    let schedule: ScheduleConfig

    enum CodingKeys: String, CodingKey {
        case postRunHook = "post_run_hook"
        case onConfigChange = "on_config_change"
        case schedule
    }
}

struct PostRunHookConfig: Codable, Hashable, Sendable {
    let enabled: Bool
    let runInterval: Int

    enum CodingKeys: String, CodingKey {
        case enabled
        case runInterval = "run_interval"
    }
}

struct OnConfigChangeConfig: Codable, Hashable, Sendable {
    let enabled: Bool
}

struct ScheduleConfig: Codable, Hashable, Sendable {
    let enabled: Bool
    let cron: String
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
            postRunHook: PostRunHookConfig(enabled: false, runInterval: 5),
            onConfigChange: OnConfigChangeConfig(enabled: true),
            schedule: ScheduleConfig(enabled: false, cron: "0 8 * * 1")
        )
    )
}

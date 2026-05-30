pub const IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID: &str = "implementation_self_assessment_v2";
pub const IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH: &str =
    "implementation/self-assessment.json";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractParseContext {
    pub run_id: String,
    pub run_age: Option<String>,
    pub declared_contract_id: Option<String>,
    pub canonical_artifact_path: String,
    pub raw_artifact_path: Option<String>,
    pub source_generation_id: Option<String>,
    pub artifact_created_at: Option<DateTime<Utc>>,
    pub v2_generation_seen_for_run: bool,
    pub legacy_v1_generation_available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationSelfAssessmentV2 {
    pub implementation_complete: bool,
    pub verification_green: bool,
    pub remaining_code_tasks: Vec<RemainingCodeTask>,
    pub handoff_tasks: Vec<HandoffTask>,
    pub known_risks: Vec<String>,
    pub tests_run: Vec<String>,
    pub docs_impacted: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemainingCodeTask {
    pub summary: String,
    pub owner: String,
    pub blocking: bool,
    pub evidence: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandoffTask {
    pub summary: String,
    pub owner_class: HandoffOwnerClass,
    pub target_stage: String,
    pub blocking_review: bool,
    pub evidence: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandoffOwnerClass {
    Docs,
    ManualEvidence,
    Release,
    Ops,
    Product,
    HumanOperator,
    Unknown,
}

impl HandoffOwnerClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            HandoffOwnerClass::Docs => "docs",
            HandoffOwnerClass::ManualEvidence => "manual_evidence",
            HandoffOwnerClass::Release => "release",
            HandoffOwnerClass::Ops => "ops",
            HandoffOwnerClass::Product => "product",
            HandoffOwnerClass::HumanOperator => "human_operator",
            HandoffOwnerClass::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for HandoffOwnerClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for HandoffOwnerClass {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "docs" => Ok(HandoffOwnerClass::Docs),
            "manual_evidence" => Ok(HandoffOwnerClass::ManualEvidence),
            "release" => Ok(HandoffOwnerClass::Release),
            "ops" => Ok(HandoffOwnerClass::Ops),
            "product" => Ok(HandoffOwnerClass::Product),
            "human_operator" => Ok(HandoffOwnerClass::HumanOperator),
            "unknown" => Ok(HandoffOwnerClass::Unknown),
            other => Err(format!("unknown handoff owner_class: {other}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImplementationSelfAssessmentStatus {
    Invalid,
    NeedsCodeFixes,
    Blocked,
    HandoffRequired,
    Complete,
    Unknown,
}

impl ImplementationSelfAssessmentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImplementationSelfAssessmentStatus::Invalid => "invalid",
            ImplementationSelfAssessmentStatus::NeedsCodeFixes => "needs_code_fixes",
            ImplementationSelfAssessmentStatus::Blocked => "blocked",
            ImplementationSelfAssessmentStatus::HandoffRequired => "handoff_required",
            ImplementationSelfAssessmentStatus::Complete => "complete",
            ImplementationSelfAssessmentStatus::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for ImplementationSelfAssessmentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ImplementationSelfAssessmentStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "invalid" => Ok(ImplementationSelfAssessmentStatus::Invalid),
            "needs_code_fixes" => Ok(ImplementationSelfAssessmentStatus::NeedsCodeFixes),
            "blocked" => Ok(ImplementationSelfAssessmentStatus::Blocked),
            "handoff_required" => Ok(ImplementationSelfAssessmentStatus::HandoffRequired),
            "complete" => Ok(ImplementationSelfAssessmentStatus::Complete),
            "unknown" => Ok(ImplementationSelfAssessmentStatus::Unknown),
            other => Err(format!(
                "unknown implementation self-assessment status: {other}"
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub code: String,
    pub message: String,
    pub pointer: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetStageSummary {
    pub target_stage: String,
    pub count: usize,
    pub blocking_review_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemainingCodeTaskSummary {
    pub summary: String,
    pub owner: String,
    pub blocking: bool,
    pub evidence: String,
    pub source_pointer: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandoffTaskSummary {
    pub summary: String,
    pub owner_class: HandoffOwnerClass,
    pub target_stage: String,
    pub blocking_review: bool,
    pub evidence: String,
    pub source_pointer: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationSelfAssessmentSummary {
    pub contract_id: String,
    pub artifact_path: String,
    pub status: ImplementationSelfAssessmentStatus,
    pub implementation_complete: Option<bool>,
    pub verification_green: Option<bool>,
    pub remaining_code_task_count: Option<usize>,
    pub blocking_remaining_code_task_count: Option<usize>,
    pub handoff_task_count: Option<usize>,
    pub blocking_review_handoff_task_count: Option<usize>,
    pub owner_class_counts: BTreeMap<String, usize>,
    pub target_stage_summaries: Vec<TargetStageSummary>,
    pub remaining_code_tasks: Vec<RemainingCodeTaskSummary>,
    pub handoff_tasks: Vec<HandoffTaskSummary>,
    pub known_risks: Vec<String>,
    pub tests_run: Vec<String>,
    pub docs_impacted: Vec<String>,
    pub validation_errors: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
    pub raw_artifact_available: bool,
}

pub fn parse_implementation_self_assessment_v2(
    raw_json: &Value,
    context: ContractParseContext,
) -> ImplementationSelfAssessmentSummary {
    let artifact_path = if context.canonical_artifact_path.trim().is_empty() {
        IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.to_string()
    } else {
        context.canonical_artifact_path.clone()
    };
    let mut builder = SummaryBuilder::new(artifact_path, !raw_json.is_null());

    if raw_json.is_null() {
        builder.status = ImplementationSelfAssessmentStatus::Unknown;
        return builder.finish();
    }

    let declared_v2 = context.declared_contract_id.as_deref()
        == Some(IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID);
    let looks_v2 = looks_like_v2(raw_json);

    if looks_v2 && !declared_v2 {
        builder.error(
            "raw_only_v2_artifact",
            "v2-shaped artifact is missing the implementation_self_assessment_v2 contract declaration",
            "",
        );
    }

    if declared_v2 || looks_v2 {
        parse_v2(raw_json, &mut builder);
    } else if let Some(seemingly_complete) = raw_json
        .as_object()
        .and_then(|object| object.get("seemingly_complete"))
        .and_then(Value::as_bool)
    {
        builder.implementation_complete = Some(seemingly_complete);
        builder.v1_dimensions_unavailable = true;
        builder.warn(
            "legacy_v1_compatibility",
            "mapped legacy seemingly_complete to implementation_complete for compatibility",
            "/seemingly_complete",
        );
        builder.warn(
            "legacy_v1_dimensions_unavailable",
            "legacy implementation_self_assessment cannot express handoff, blocking-code-task, or blocked-verification dimensions",
            "",
        );
    } else {
        builder.status = ImplementationSelfAssessmentStatus::Unknown;
        return builder.finish();
    }

    builder.derive_status();
    builder.finish()
}

pub fn invalid_implementation_self_assessment_summary(
    context: ContractParseContext,
    code: impl Into<String>,
    message: impl Into<String>,
    pointer: impl Into<String>,
) -> ImplementationSelfAssessmentSummary {
    let artifact_path = if context.canonical_artifact_path.trim().is_empty() {
        IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.to_string()
    } else {
        context.canonical_artifact_path
    };
    let mut builder = SummaryBuilder::new(artifact_path, true);
    builder.status = ImplementationSelfAssessmentStatus::Invalid;
    builder.error(code, message, pointer);
    builder.finish()
}

fn parse_v2(raw_json: &Value, builder: &mut SummaryBuilder) {
    let Some(object) = raw_json.as_object() else {
        builder.error("expected_object", "artifact must be a JSON object", "");
        return;
    };

    let implementation_complete = required_bool(object, "implementation_complete", builder);
    let verification_green = required_bool(object, "verification_green", builder);
    let remaining_code_tasks = required_remaining_code_tasks(object, builder);
    let handoff_tasks = required_handoff_tasks(object, builder);
    let known_risks = required_display_array(object, "known_risks", builder);
    let tests_run = required_display_array(object, "tests_run", builder);
    let docs_impacted = required_display_array(object, "docs_impacted", builder);

    builder.implementation_complete = implementation_complete;
    builder.verification_green = verification_green;
    builder.remaining_code_tasks = remaining_code_tasks;
    builder.handoff_tasks = handoff_tasks;
    builder.known_risks = known_risks;
    builder.tests_run = tests_run;
    builder.docs_impacted = docs_impacted;
    builder.add_suspicious_classification_warnings();
}

fn required_bool(
    object: &serde_json::Map<String, Value>,
    field: &str,
    builder: &mut SummaryBuilder,
) -> Option<bool> {
    match object.get(field) {
        Some(Value::Bool(value)) => Some(*value),
        Some(_) => {
            builder.error(
                "expected_bool",
                format!("required field {field} must be a boolean"),
                format!("/{field}"),
            );
            None
        }
        None => {
            builder.error(
                "missing_required_field",
                format!("required field {field} is missing"),
                format!("/{field}"),
            );
            None
        }
    }
}

fn required_remaining_code_tasks(
    object: &serde_json::Map<String, Value>,
    builder: &mut SummaryBuilder,
) -> Vec<RemainingCodeTaskSummary> {
    let Some(value) = object.get("remaining_code_tasks") else {
        builder.error(
            "missing_required_field",
            "required field remaining_code_tasks is missing",
            "/remaining_code_tasks",
        );
        return Vec::new();
    };

    let Some(items) = value.as_array() else {
        builder.error(
            "expected_array",
            "remaining_code_tasks must be an array",
            "/remaining_code_tasks",
        );
        return Vec::new();
    };

    let mut tasks = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let pointer = format!("/remaining_code_tasks/{index}");
        let Some(task_object) = item.as_object() else {
            builder.error(
                "expected_object",
                "remaining_code_tasks entries must be objects",
                &pointer,
            );
            continue;
        };

        let summary = required_non_empty_string(task_object, "summary", &pointer, builder);
        let owner = required_non_empty_string(task_object, "owner", &pointer, builder);
        let blocking = required_nested_bool(task_object, "blocking", &pointer, builder);
        let evidence = required_non_empty_string(task_object, "evidence", &pointer, builder);

        if let (Some(summary), Some(owner), Some(blocking), Some(evidence)) =
            (summary, owner, blocking, evidence)
        {
            tasks.push(RemainingCodeTaskSummary {
                summary,
                owner,
                blocking,
                evidence,
                source_pointer: pointer,
            });
        }
    }
    tasks
}

fn required_handoff_tasks(
    object: &serde_json::Map<String, Value>,
    builder: &mut SummaryBuilder,
) -> Vec<HandoffTaskSummary> {
    let Some(value) = object.get("handoff_tasks") else {
        builder.error(
            "missing_required_field",
            "required field handoff_tasks is missing",
            "/handoff_tasks",
        );
        return Vec::new();
    };

    let Some(items) = value.as_array() else {
        builder.error(
            "expected_array",
            "handoff_tasks must be an array",
            "/handoff_tasks",
        );
        return Vec::new();
    };

    let mut tasks = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let pointer = format!("/handoff_tasks/{index}");
        let Some(task_object) = item.as_object() else {
            builder.error(
                "expected_object",
                "handoff_tasks entries must be objects",
                &pointer,
            );
            continue;
        };

        let summary = required_non_empty_string(task_object, "summary", &pointer, builder);
        let owner_class = required_owner_class(task_object, &pointer, builder);
        let target_stage =
            required_non_empty_string(task_object, "target_stage", &pointer, builder);
        let blocking_review =
            required_nested_bool(task_object, "blocking_review", &pointer, builder);
        let evidence = required_non_empty_string(task_object, "evidence", &pointer, builder);

        if let (
            Some(summary),
            Some(owner_class),
            Some(target_stage),
            Some(blocking_review),
            Some(evidence),
        ) = (
            summary,
            owner_class,
            target_stage,
            blocking_review,
            evidence,
        ) {
            tasks.push(HandoffTaskSummary {
                summary,
                owner_class,
                target_stage,
                blocking_review,
                evidence,
                source_pointer: pointer,
            });
        }
    }
    tasks
}

fn required_display_array(
    object: &serde_json::Map<String, Value>,
    field: &str,
    builder: &mut SummaryBuilder,
) -> Vec<String> {
    let Some(value) = object.get(field) else {
        builder.error(
            "missing_required_field",
            format!("required field {field} is missing"),
            format!("/{field}"),
        );
        return Vec::new();
    };

    let Some(items) = value.as_array() else {
        builder.error(
            "expected_array",
            format!("{field} must be an array"),
            format!("/{field}"),
        );
        return Vec::new();
    };

    items.iter().filter_map(display_safe_value).collect()
}

fn required_non_empty_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
    parent_pointer: &str,
    builder: &mut SummaryBuilder,
) -> Option<String> {
    let pointer = format!("{parent_pointer}/{field}");
    match object.get(field) {
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value.trim().to_string()),
        Some(Value::String(_)) => {
            builder.error(
                "empty_required_string",
                format!("required field {field} must not be empty"),
                pointer,
            );
            None
        }
        Some(_) => {
            builder.error(
                "expected_string",
                format!("required field {field} must be a string"),
                pointer,
            );
            None
        }
        None => {
            builder.error(
                "missing_required_field",
                format!("required field {field} is missing"),
                pointer,
            );
            None
        }
    }
}

fn required_nested_bool(
    object: &serde_json::Map<String, Value>,
    field: &str,
    parent_pointer: &str,
    builder: &mut SummaryBuilder,
) -> Option<bool> {
    let pointer = format!("{parent_pointer}/{field}");
    match object.get(field) {
        Some(Value::Bool(value)) => Some(*value),
        Some(_) => {
            builder.error(
                "expected_bool",
                format!("required field {field} must be a boolean"),
                pointer,
            );
            None
        }
        None => {
            builder.error(
                "missing_required_field",
                format!("required field {field} is missing"),
                pointer,
            );
            None
        }
    }
}

fn required_owner_class(
    object: &serde_json::Map<String, Value>,
    parent_pointer: &str,
    builder: &mut SummaryBuilder,
) -> Option<HandoffOwnerClass> {
    let pointer = format!("{parent_pointer}/owner_class");
    match object.get("owner_class") {
        Some(Value::String(value)) => match value.parse() {
            Ok(owner_class) => Some(owner_class),
            Err(message) => {
                builder.error("invalid_owner_class", message, pointer);
                None
            }
        },
        Some(_) => {
            builder.error(
                "expected_string",
                "required field owner_class must be a string",
                pointer,
            );
            None
        }
        None => {
            builder.error(
                "missing_required_field",
                "required field owner_class is missing",
                pointer,
            );
            None
        }
    }
}

fn looks_like_v2(raw_json: &Value) -> bool {
    raw_json.as_object().map_or(false, |object| {
        [
            "implementation_complete",
            "verification_green",
            "remaining_code_tasks",
            "handoff_tasks",
            "known_risks",
            "tests_run",
            "docs_impacted",
        ]
        .iter()
        .any(|field| object.contains_key(*field))
    })
}

fn display_safe_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Null => None,
        _ => None,
    }
}

fn is_vague_evidence(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized.len() < 6
        || matches!(
            normalized.as_str(),
            "n/a" | "none" | "todo" | "tbd" | "unknown" | "not sure"
        )
}

fn is_code_writer_owner(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "code_writer" | "code-writer" | "code writer"
    )
}

#[derive(Debug)]
struct SummaryBuilder {
    artifact_path: String,
    status: ImplementationSelfAssessmentStatus,
    implementation_complete: Option<bool>,
    verification_green: Option<bool>,
    remaining_code_tasks: Vec<RemainingCodeTaskSummary>,
    handoff_tasks: Vec<HandoffTaskSummary>,
    known_risks: Vec<String>,
    tests_run: Vec<String>,
    docs_impacted: Vec<String>,
    validation_errors: Vec<ValidationIssue>,
    warnings: Vec<ValidationIssue>,
    raw_artifact_available: bool,
    v1_dimensions_unavailable: bool,
}

impl SummaryBuilder {
    fn new(artifact_path: String, raw_artifact_available: bool) -> Self {
        Self {
            artifact_path,
            status: ImplementationSelfAssessmentStatus::Unknown,
            implementation_complete: None,
            verification_green: None,
            remaining_code_tasks: Vec::new(),
            handoff_tasks: Vec::new(),
            known_risks: Vec::new(),
            tests_run: Vec::new(),
            docs_impacted: Vec::new(),
            validation_errors: Vec::new(),
            warnings: Vec::new(),
            raw_artifact_available,
            v1_dimensions_unavailable: false,
        }
    }

    fn error(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        pointer: impl Into<String>,
    ) {
        self.validation_errors.push(ValidationIssue {
            code: code.into(),
            message: message.into(),
            pointer: pointer.into(),
        });
    }

    fn warn(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        pointer: impl Into<String>,
    ) {
        self.warnings.push(ValidationIssue {
            code: code.into(),
            message: message.into(),
            pointer: pointer.into(),
        });
    }

    fn add_suspicious_classification_warnings(&mut self) {
        if !self.handoff_tasks.is_empty()
            && !self.remaining_code_tasks.is_empty()
            && self.remaining_code_tasks.iter().all(|task| !task.blocking)
        {
            self.warn(
                "suspicious_nonblocking_code_tasks_with_handoff",
                "handoff tasks are present while all remaining code tasks are non-blocking",
                "/remaining_code_tasks",
            );
        }

        let unknown_count = self
            .handoff_tasks
            .iter()
            .filter(|task| task.owner_class == HandoffOwnerClass::Unknown)
            .count();
        if unknown_count > 1 {
            self.warn(
                "multiple_unknown_owner_class",
                "multiple handoff tasks use owner_class unknown and require human-operator triage",
                "/handoff_tasks",
            );
        }

        let code_evidence: Vec<(String, String)> = self
            .remaining_code_tasks
            .iter()
            .map(|task| (task.source_pointer.clone(), task.evidence.clone()))
            .collect();
        for (pointer, evidence) in code_evidence {
            if is_vague_evidence(&evidence) {
                self.warn(
                    "vague_evidence",
                    "remaining code task evidence is too vague for downstream triage",
                    format!("{pointer}/evidence"),
                );
            }
        }

        let handoff_evidence: Vec<(String, String)> = self
            .handoff_tasks
            .iter()
            .map(|task| (task.source_pointer.clone(), task.evidence.clone()))
            .collect();
        for (pointer, evidence) in handoff_evidence {
            if is_vague_evidence(&evidence) {
                self.warn(
                    "vague_evidence",
                    "handoff task evidence is too vague for downstream triage",
                    format!("{pointer}/evidence"),
                );
            }
        }
    }

    fn derive_status(&mut self) {
        if !self.validation_errors.is_empty() {
            self.status = ImplementationSelfAssessmentStatus::Invalid;
            return;
        }

        let blocking_code_tasks = self
            .remaining_code_tasks
            .iter()
            .filter(|task| task.blocking)
            .count();

        let has_nonblocking_code_tail =
            !self.remaining_code_tasks.is_empty() && blocking_code_tasks == 0;
        let has_red_nonblocking_code_writer_tail = self.verification_green == Some(false)
            && self
                .remaining_code_tasks
                .iter()
                .any(|task| !task.blocking && is_code_writer_owner(&task.owner));

        if blocking_code_tasks > 0 {
            self.status = ImplementationSelfAssessmentStatus::NeedsCodeFixes;
        } else if has_red_nonblocking_code_writer_tail {
            self.status = ImplementationSelfAssessmentStatus::NeedsCodeFixes;
        } else if self.implementation_complete == Some(true)
            && blocking_code_tasks == 0
            && self.verification_green == Some(false)
        {
            self.status = ImplementationSelfAssessmentStatus::Blocked;
        } else if self.verification_green == Some(false) {
            self.status = ImplementationSelfAssessmentStatus::Blocked;
        } else if self.verification_green == Some(true)
            && (!self.handoff_tasks.is_empty() || has_nonblocking_code_tail)
        {
            self.status = ImplementationSelfAssessmentStatus::HandoffRequired;
        } else if self.implementation_complete == Some(true)
            && self.verification_green == Some(true)
            && blocking_code_tasks == 0
        {
            self.status = ImplementationSelfAssessmentStatus::Complete;
        } else if self.implementation_complete == Some(false) {
            self.status = ImplementationSelfAssessmentStatus::NeedsCodeFixes;
        } else {
            self.status = ImplementationSelfAssessmentStatus::Unknown;
        }
    }

    fn finish(self) -> ImplementationSelfAssessmentSummary {
        let mut owner_class_counts = BTreeMap::new();
        let mut target_stage_counts: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for task in &self.handoff_tasks {
            *owner_class_counts
                .entry(task.owner_class.as_str().to_string())
                .or_insert(0) += 1;
            let stage = target_stage_counts
                .entry(task.target_stage.clone())
                .or_insert((0, 0));
            stage.0 += 1;
            if task.blocking_review {
                stage.1 += 1;
            }
        }

        let target_stage_summaries = target_stage_counts
            .into_iter()
            .map(
                |(target_stage, (count, blocking_review_count))| TargetStageSummary {
                    target_stage,
                    count,
                    blocking_review_count,
                },
            )
            .collect();

        let dimensions_available = !self.v1_dimensions_unavailable;
        let blocking_remaining_code_task_count = dimensions_available.then(|| {
            self.remaining_code_tasks
                .iter()
                .filter(|task| task.blocking)
                .count()
        });
        let blocking_review_handoff_task_count = dimensions_available.then(|| {
            self.handoff_tasks
                .iter()
                .filter(|task| task.blocking_review)
                .count()
        });

        ImplementationSelfAssessmentSummary {
            contract_id: IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.to_string(),
            artifact_path: self.artifact_path,
            status: self.status,
            implementation_complete: self.implementation_complete,
            verification_green: self.verification_green,
            remaining_code_task_count: dimensions_available
                .then_some(self.remaining_code_tasks.len()),
            blocking_remaining_code_task_count,
            handoff_task_count: dimensions_available.then_some(self.handoff_tasks.len()),
            blocking_review_handoff_task_count,
            owner_class_counts,
            target_stage_summaries,
            remaining_code_tasks: self.remaining_code_tasks,
            handoff_tasks: self.handoff_tasks,
            known_risks: self.known_risks,
            tests_run: self.tests_run,
            docs_impacted: self.docs_impacted,
            validation_errors: self.validation_errors,
            warnings: self.warnings,
            raw_artifact_available: self.raw_artifact_available,
        }
    }
}

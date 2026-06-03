use crate::config::Timestamp;
use serde::Serialize;

/// Result of an enqueue mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnqueueResult {
    pub job_id: String,
    pub state: JobState,
    pub status: EnqueueStatus,
}

/// Batch enqueue response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BatchEnqueueResult {
    pub items: Vec<EnqueueResult>,
}

/// Enqueue status classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnqueueStatus {
    Created,
    Unchanged,
    Updated,
}

/// Result of a reschedule mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RescheduleResult {
    pub job_id: String,
    pub state: JobState,
    pub schedule_at: Option<Timestamp>,
}

/// Worker handler result alias.
pub type HandlerResult = std::result::Result<JobOutcome, JobError>;

/// Retryable handler error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JobError {
    #[error("[redloop/types] retryable handler failure: {message}")]
    Retryable { message: String },
}

/// Worker outcome for a leased job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobOutcome {
    Complete,
    Reschedule { schedule_at: Option<Timestamp> },
    Fail { message: String },
}

/// Queue state for a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Ready,
    Scheduled,
    Leased,
    Failed,
}

/// Full job record for operator inspection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JobRecord {
    pub job_id: String,
    pub state: JobState,
    pub ready_at: Option<Timestamp>,
    pub schedule_at: Option<Timestamp>,
    pub failure_count: u32,
    pub leased_by: Option<LeaseHolder>,
    pub failed_at: Option<Timestamp>,
}

/// Leased worker metadata for operator queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LeaseHolder {
    pub worker_id: String,
    pub lease_token: String,
    pub leased_until: Timestamp,
}

/// Snapshot queue counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueueCounts {
    pub ready_count: u64,
    pub scheduled_due_count: u64,
    pub scheduled_future_count: u64,
    pub leased_count: u64,
    pub failed_count: u64,
}

/// Failed-job listing query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedJobsQuery {
    pub cursor: Option<String>,
    pub limit: usize,
}

/// Failed-job page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FailedJobsPage {
    pub jobs: Vec<JobRecord>,
    pub next_cursor: Option<String>,
}

/// Worker inspection record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkerInfo {
    pub worker_id: String,
    pub concurrency: usize,
    pub leased_count: u64,
    pub last_heartbeat_at: Timestamp,
}

/// Operator selector for failed-job purge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailedSelector {
    JobId(String),
    OlderThan(Timestamp),
    All,
}

/// Force-fail operator request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForceFailRequest {
    pub job_id: String,
    pub message: String,
}

impl JobState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            JobState::Ready => "ready",
            JobState::Scheduled => "scheduled",
            JobState::Leased => "leased",
            JobState::Failed => "failed",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "ready" => Some(JobState::Ready),
            "scheduled" => Some(JobState::Scheduled),
            "leased" => Some(JobState::Leased),
            "failed" => Some(JobState::Failed),
            _ => None,
        }
    }
}

impl std::fmt::Display for JobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl EnqueueStatus {
    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "created" => Some(EnqueueStatus::Created),
            "unchanged" => Some(EnqueueStatus::Unchanged),
            "updated" => Some(EnqueueStatus::Updated),
            _ => None,
        }
    }
}

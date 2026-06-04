use crate::config::{RetryPolicy, Timestamp, WorkerConfig};
use crate::error::Result;
use crate::types::{
    EnqueueResult, FailedJobsPage, FailedJobsQuery, FailedSelector, ForceFailRequest, JobRecord,
    QueueCounts, RescheduleResult, WorkerInfo,
};
use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReplaceMode {
    Always,
    IfEarlier,
    IfLater,
}

#[derive(Debug, Clone)]
pub struct EnqueueRequest {
    pub(crate) job_id: String,
    pub(crate) schedule_at: Option<Timestamp>,
    pub(crate) replace_mode: ReplaceMode,
    pub(crate) allow_existing_current_run: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ReservedJob {
    pub job_id: String,
    pub lease_token: String,
}

#[derive(Debug, Clone)]
pub struct ReserveBatch {
    pub(crate) jobs: Vec<ReservedJob>,
    pub(crate) next_schedule_at: Option<Timestamp>,
}

#[derive(Debug, Clone)]
pub(crate) enum FailureAction {
    Retryable,
    Terminal,
}

#[derive(Debug, Clone)]
pub struct FailureRequest {
    pub(crate) worker_id: String,
    pub(crate) job_id: String,
    pub(crate) lease_token: String,
    pub(crate) action: FailureAction,
    pub(crate) retry_policy: RetryPolicy,
    pub(crate) now: Timestamp,
}

#[cfg_attr(test, unimock::unimock(api = QueueStoreMock))]
#[async_trait]
pub(crate) trait QueueStore: Send + Sync {
    async fn remember_namespace(&self, namespace: &str) -> Result<()>;
    async fn list_namespaces(&self) -> Result<Vec<String>>;
    async fn enqueue(&self, namespace: &str, request: EnqueueRequest) -> Result<EnqueueResult>;
    async fn reschedule(
        &self,
        namespace: &str,
        job_id: &str,
        schedule_at: Option<Timestamp>,
    ) -> Result<RescheduleResult>;
    async fn get_job(&self, namespace: &str, job_id: &str) -> Result<Option<JobRecord>>;
    async fn counts(&self, namespace: &str, now: Timestamp) -> Result<QueueCounts>;
    async fn list_failed(&self, namespace: &str, query: FailedJobsQuery) -> Result<FailedJobsPage>;
    async fn list_workers(&self, namespace: &str, now: Timestamp) -> Result<Vec<WorkerInfo>>;
    async fn cancel(&self, namespace: &str, job_id: &str) -> Result<()>;
    async fn force_ack(&self, namespace: &str, job_id: &str) -> Result<()>;
    async fn force_fail(
        &self,
        namespace: &str,
        request: ForceFailRequest,
        now: Timestamp,
    ) -> Result<()>;
    async fn requeue(&self, namespace: &str, job_id: &str, now: Timestamp) -> Result<()>;
    async fn retry_now(&self, namespace: &str, job_id: &str, now: Timestamp) -> Result<()>;
    async fn force_retry(&self, namespace: &str, job_id: &str, now: Timestamp) -> Result<()>;
    async fn purge_failed(&self, namespace: &str, selector: FailedSelector) -> Result<u64>;
    async fn reserve(
        &self,
        namespace: &str,
        config: &WorkerConfig,
        available_capacity: usize,
        now: Timestamp,
        lease_tokens: Vec<String>,
    ) -> Result<ReserveBatch>;
    async fn heartbeat(
        &self,
        namespace: &str,
        worker_id: &str,
        job_id: &str,
        lease_token: &str,
        now: Timestamp,
        lease_duration: std::time::Duration,
    ) -> Result<()>;
    async fn ack(
        &self,
        namespace: &str,
        worker_id: &str,
        job_id: &str,
        lease_token: &str,
    ) -> Result<()>;
    async fn complete_and_reschedule(
        &self,
        namespace: &str,
        worker_id: &str,
        job_id: &str,
        lease_token: &str,
        schedule_at: Option<Timestamp>,
        now: Timestamp,
    ) -> Result<()>;
    async fn fail_or_retry(&self, namespace: &str, request: FailureRequest) -> Result<u32>;
    async fn reap_expired(&self, namespace: &str, now: Timestamp, limit: usize) -> Result<u64>;
}

use std::sync::Arc;

use async_trait::async_trait;

use crate::Result;
use crate::builder::BatchEnqueueItem;
use crate::config::{Timestamp, WorkerConfig};
use crate::types::{
    BatchEnqueueResult, EnqueueResult, FailedJobsPage, FailedJobsQuery, FailedSelector,
    ForceFailRequest, JobRecord, QueueCounts, RescheduleResult, WorkerInfo,
};
use crate::worker::handler::DynRedloopWorkerRuntime;

pub type DynRedloopClient = Arc<dyn RedloopClient>;
pub type DynRedloopNamespace = Arc<dyn RedloopNamespace>;
pub type DynRedloopEnqueueBuilder = Arc<dyn RedloopEnqueueBuilder>;
pub type DynRedloopScheduledBuilder = Arc<dyn RedloopScheduledBuilder>;

#[async_trait]
pub trait RedloopClient: Send + Sync {
    fn namespace(&self, namespace: String) -> DynRedloopNamespace;
    async fn list_namespaces(&self) -> Result<Vec<String>>;
}

#[async_trait]
pub trait RedloopNamespace: Send + Sync {
    fn job(&self, job_id: &str) -> DynRedloopEnqueueBuilder;
    async fn enqueue_batch(&self, requests: Vec<BatchEnqueueItem>) -> Result<BatchEnqueueResult>;
    async fn reschedule(
        &self,
        job_id: &str,
        schedule_at: Option<Timestamp>,
    ) -> Result<RescheduleResult>;
    async fn get_job(&self, job_id: &str) -> Result<Option<JobRecord>>;
    async fn counts(&self) -> Result<QueueCounts>;
    async fn list_failed(&self, query: FailedJobsQuery) -> Result<FailedJobsPage>;
    async fn list_workers(&self) -> Result<Vec<WorkerInfo>>;
    async fn cancel(&self, job_id: &str) -> Result<()>;
    async fn force_ack(&self, job_id: &str) -> Result<()>;
    async fn force_fail(&self, request: ForceFailRequest) -> Result<()>;
    async fn requeue(&self, job_id: &str) -> Result<()>;
    async fn retry_now(&self, job_id: &str) -> Result<()>;
    async fn force_retry(&self, job_id: &str) -> Result<()>;
    async fn purge_failed(&self, selector: FailedSelector) -> Result<u64>;
    fn worker(&self, config: WorkerConfig) -> DynRedloopWorkerRuntime;
}

#[async_trait]
pub trait RedloopEnqueueBuilder: Send + Sync {
    fn schedule_at(&self, schedule_at: Timestamp) -> DynRedloopScheduledBuilder;
    fn to_batch_item(&self) -> BatchEnqueueItem;
    async fn execute(&self) -> Result<EnqueueResult>;
}

#[async_trait]
pub trait RedloopScheduledBuilder: Send + Sync {
    fn replace_if_earlier(&self) -> DynRedloopScheduledBuilder;
    fn replace_if_later(&self) -> DynRedloopScheduledBuilder;
    fn to_batch_item(&self) -> BatchEnqueueItem;
    async fn execute(&self) -> Result<EnqueueResult>;
}

//! Namespace handle construction and trait adapters.

use std::sync::Arc;

use crate::clock::{SystemClock, TokioWorkerCoordinator, WorkerCoordinator};
use crate::config::{ConnectConfig, Timestamp, WorkerConfig};
use crate::contract::{DynRedloopNamespace, RedloopClient, RedloopNamespace};
use crate::error::Result;
use crate::redis_store::RedisStore;
use crate::types::{
    BatchEnqueueResult, FailedJobsPage, FailedJobsQuery, FailedSelector, ForceFailRequest,
    JobRecord, QueueCounts, RescheduleResult, WorkerInfo,
};
use crate::worker::handler::DynRedloopWorkerRuntime;

use super::Redloop;

impl Redloop {
    /// Connects to Redis and constructs a Redloop client.
    pub async fn connect(config: ConnectConfig) -> Result<Self> {
        config.validate()?;

        let clock = Arc::new(SystemClock);
        let coordinator: Arc<dyn WorkerCoordinator> = Arc::new(TokioWorkerCoordinator);
        let store =
            Arc::new(RedisStore::connect(config).await?) as Arc<dyn crate::store::QueueStore>;

        Ok(Self {
            store,
            clock,
            coordinator,
        })
    }

    /// Returns a namespace handle.
    pub fn namespace(&self, namespace: impl Into<String>) -> DynRedloopNamespace {
        Arc::new(self.namespace_handle(namespace.into()))
    }

    pub(crate) fn namespace_handle(&self, namespace: String) -> super::Namespace {
        super::Namespace {
            store: Arc::clone(&self.store),
            clock: Arc::clone(&self.clock),
            coordinator: Arc::clone(&self.coordinator),
            namespace,
        }
    }

    /// Lists known namespaces.
    pub async fn list_namespaces(&self) -> Result<Vec<String>> {
        self.store.list_namespaces().await
    }
}

#[async_trait::async_trait]
impl RedloopClient for Redloop {
    fn namespace(&self, namespace: String) -> DynRedloopNamespace {
        Redloop::namespace(self, namespace)
    }

    async fn list_namespaces(&self) -> Result<Vec<String>> {
        Redloop::list_namespaces(self).await
    }
}

#[async_trait::async_trait]
impl RedloopNamespace for super::Namespace {
    fn job(&self, job_id: &str) -> crate::contract::DynRedloopEnqueueBuilder {
        super::Namespace::job(self, job_id)
    }

    async fn enqueue_batch(
        &self,
        requests: Vec<crate::builder::BatchEnqueueItem>,
    ) -> Result<BatchEnqueueResult> {
        super::Namespace::enqueue_batch(self, requests).await
    }

    async fn reschedule(
        &self,
        job_id: &str,
        schedule_at: Option<Timestamp>,
    ) -> Result<RescheduleResult> {
        super::Namespace::reschedule(self, job_id, schedule_at).await
    }

    async fn get_job(&self, job_id: &str) -> Result<Option<JobRecord>> {
        super::Namespace::get_job(self, job_id).await
    }

    async fn counts(&self) -> Result<QueueCounts> {
        super::Namespace::counts(self).await
    }

    async fn list_failed(&self, query: FailedJobsQuery) -> Result<FailedJobsPage> {
        super::Namespace::list_failed(self, query).await
    }

    async fn list_workers(&self) -> Result<Vec<WorkerInfo>> {
        super::Namespace::list_workers(self).await
    }

    async fn cancel(&self, job_id: &str) -> Result<()> {
        super::Namespace::cancel(self, job_id).await
    }

    async fn force_ack(&self, job_id: &str) -> Result<()> {
        super::Namespace::force_ack(self, job_id).await
    }

    async fn force_fail(&self, request: ForceFailRequest) -> Result<()> {
        super::Namespace::force_fail(self, request).await
    }

    async fn requeue(&self, job_id: &str) -> Result<()> {
        super::Namespace::requeue(self, job_id).await
    }

    async fn retry_now(&self, job_id: &str) -> Result<()> {
        super::Namespace::retry_now(self, job_id).await
    }

    async fn force_retry(&self, job_id: &str) -> Result<()> {
        super::Namespace::force_retry(self, job_id).await
    }

    async fn purge_failed(&self, selector: FailedSelector) -> Result<u64> {
        super::Namespace::purge_failed(self, selector).await
    }

    fn worker(&self, config: WorkerConfig) -> DynRedloopWorkerRuntime {
        super::Namespace::worker(self, config)
    }
}

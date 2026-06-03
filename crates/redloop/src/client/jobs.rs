//! Job-oriented namespace operations.

use std::sync::Arc;

use crate::builder::{BatchEnqueueItem, JobBuilder};
use crate::config::Timestamp;
use crate::contract::DynRedloopEnqueueBuilder;
use crate::error::{Error, Result};
use crate::store::EnqueueRequest;
use crate::types::{
    BatchEnqueueResult, EnqueueResult, FailedSelector, ForceFailRequest, JobRecord,
    RescheduleResult,
};

use super::Namespace;

impl Namespace {
    /// Starts a fluent job builder.
    pub fn job(&self, job_id: impl AsRef<str>) -> DynRedloopEnqueueBuilder {
        Arc::new(self.job_builder(job_id.as_ref().to_owned()))
    }

    pub(crate) fn job_builder(&self, job_id: String) -> JobBuilder {
        JobBuilder::new(self.clone(), job_id)
    }

    /// Enqueues a batch of builder-produced items.
    pub async fn enqueue_batch(
        &self,
        requests: Vec<BatchEnqueueItem>,
    ) -> Result<BatchEnqueueResult> {
        let mut results = Vec::with_capacity(requests.len());
        for request in requests {
            results.push(self.enqueue_request(request.request).await?);
        }
        Ok(BatchEnqueueResult { items: results })
    }

    pub(crate) async fn enqueue_request(&self, request: EnqueueRequest) -> Result<EnqueueResult> {
        validate_job_id(&request.job_id)?;
        let result = self.store.enqueue(&self.namespace, request).await?;
        self.remember_namespace().await;
        Ok(result)
    }

    /// Reschedules a waiting job.
    pub async fn reschedule(
        &self,
        job_id: &str,
        schedule_at: Option<Timestamp>,
    ) -> Result<RescheduleResult> {
        validate_job_id(job_id)?;
        let result = self
            .store
            .reschedule(&self.namespace, job_id, schedule_at)
            .await?;
        self.remember_namespace().await;
        Ok(result)
    }

    /// Loads one job by ID.
    pub async fn get_job(&self, job_id: &str) -> Result<Option<JobRecord>> {
        validate_job_id(job_id)?;
        self.store.get_job(&self.namespace, job_id).await
    }

    /// Cancels a scheduled or failed job.
    pub async fn cancel(&self, job_id: &str) -> Result<()> {
        validate_job_id(job_id)?;
        self.store.cancel(&self.namespace, job_id).await
    }

    /// Force-acknowledges a leased job.
    pub async fn force_ack(&self, job_id: &str) -> Result<()> {
        validate_job_id(job_id)?;
        self.store.force_ack(&self.namespace, job_id).await
    }

    /// Force-fails a leased job.
    pub async fn force_fail(&self, request: ForceFailRequest) -> Result<()> {
        validate_job_id(&request.job_id)?;
        self.store
            .force_fail(&self.namespace, request, self.clock.now())
            .await
    }

    /// Moves a scheduled or failed job into ready.
    pub async fn requeue(&self, job_id: &str) -> Result<()> {
        validate_job_id(job_id)?;
        self.store
            .requeue(&self.namespace, job_id, self.clock.now())
            .await
    }

    /// Retries a failed or scheduled job immediately.
    pub async fn retry_now(&self, job_id: &str) -> Result<()> {
        validate_job_id(job_id)?;
        self.store
            .retry_now(&self.namespace, job_id, self.clock.now())
            .await
    }

    /// Reactivates a failed job immediately.
    pub async fn force_retry(&self, job_id: &str) -> Result<()> {
        validate_job_id(job_id)?;
        self.store
            .force_retry(&self.namespace, job_id, self.clock.now())
            .await
    }

    /// Purges failed jobs matching the selector.
    pub async fn purge_failed(&self, selector: FailedSelector) -> Result<u64> {
        self.store.purge_failed(&self.namespace, selector).await
    }
}

fn validate_job_id(job_id: &str) -> Result<()> {
    let length = job_id.len();
    if length > 256 {
        return Err(Error::JobIdTooLong { length });
    }
    Ok(())
}

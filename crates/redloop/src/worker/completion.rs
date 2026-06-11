//! Worker completion outcome application.

use crate::clock::Clock;
use crate::config::WorkerConfig;
use crate::error::Result;
use crate::store::{FailureAction, FailureRequest, QueueStore};
use crate::types::JobOutcome;

use super::leases::CompletedLease;

pub(crate) async fn resolve_completed(
    namespace: &str,
    store: &dyn QueueStore,
    clock: &dyn Clock,
    config: &WorkerConfig,
    completed: &CompletedLease,
) -> Result<()> {
    match &completed.result {
        Ok(JobOutcome::Complete) => {
            store
                .ack(
                    namespace,
                    &config.worker_id,
                    &completed.job_id,
                    &completed.lease_token,
                )
                .await
        }
        Ok(JobOutcome::Reschedule { schedule_at }) => {
            store
                .complete_and_reschedule(
                    namespace,
                    &config.worker_id,
                    &completed.job_id,
                    &completed.lease_token,
                    schedule_at.to_owned(),
                    clock.now(),
                )
                .await
        }
        Ok(JobOutcome::Fail { .. }) => store
            .fail_or_retry(
                namespace,
                FailureRequest {
                    worker_id: config.worker_id.clone(),
                    job_id: completed.job_id.clone(),
                    lease_token: completed.lease_token.clone(),
                    action: FailureAction::Terminal,
                    retry_policy: config.retry_policy.clone(),
                    now: clock.now(),
                },
            )
            .await
            .map(|_| ()),
        Err(message) => {
            store
                .fail_or_retry(
                    namespace,
                    FailureRequest {
                        worker_id: config.worker_id.clone(),
                        job_id: completed.job_id.clone(),
                        lease_token: completed.lease_token.clone(),
                        action: FailureAction::Retryable,
                        retry_policy: config.retry_policy.clone(),
                        now: clock.now(),
                    },
                )
                .await?;
            tracing::debug!(job_id = %completed.job_id, error = %message, "worker handled retryable failure");
            Ok(())
        }
    }
}

//! Worker runtime loop and queue interactions.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::task::{JoinError, JoinSet};

use crate::clock::{Clock, WorkerCoordinator};
use crate::config::{Timestamp, WorkerConfig};
use crate::error::{Error, Result};
use crate::store::{FailureAction, FailureRequest, QueueStore, ReservedJob};
use crate::types::JobOutcome;

use super::handler::{DynJobHandler, RedloopWorkerRuntime, RuntimeHandler, TraitHandlerAdapter};
use super::leases::{
    ActiveLease, CompletedLease, new_lease_tokens, next_poll_delay, sleep_duration,
};

/// Worker handle for one namespace.
pub(crate) struct Worker {
    namespace: String,
    store: Arc<dyn QueueStore>,
    clock: Arc<dyn Clock>,
    coordinator: Arc<dyn WorkerCoordinator>,
    config: WorkerConfig,
}

#[async_trait]
impl RedloopWorkerRuntime for Worker {
    async fn run(&self, handler: DynJobHandler) -> Result<()> {
        Worker::run(self, handler).await
    }
}

fn retryable_reserve_error(error: &Error) -> bool {
    match error {
        Error::CommandTimedOut { operation, .. } => *operation == "reserve",
        Error::Redis { operation, source } => *operation == "reserve" && source.is_timeout(),
        _ => false,
    }
}

#[cfg(test)]
#[path = "_tests_/run/mod.rs"]
mod run_tests;

impl Worker {
    pub(crate) fn new(
        namespace: String,
        store: Arc<dyn QueueStore>,
        clock: Arc<dyn Clock>,
        coordinator: Arc<dyn WorkerCoordinator>,
        config: WorkerConfig,
    ) -> Self {
        Self {
            namespace,
            store,
            clock,
            coordinator,
            config,
        }
    }

    /// Runs a trait-object handler until cancelled or a fatal runtime error occurs.
    pub async fn run(&self, handler: DynJobHandler) -> Result<()> {
        self.run_internal(Arc::new(TraitHandlerAdapter { inner: handler }))
            .await
    }

    async fn run_internal(&self, handler: Arc<dyn RuntimeHandler>) -> Result<()> {
        self.config.validate()?;

        let mut heartbeat_tick = tokio::time::interval(self.config.heartbeat_interval);
        let mut reap_tick = tokio::time::interval(self.config.reap_interval);
        let mut join_set = JoinSet::new();
        let mut active = HashMap::<String, ActiveLease>::new();
        let mut poll_delay = self.config.poll_interval_min;
        let mut next_schedule_at: Option<Timestamp> = None;
        let mut reserve_sleep: Option<(Duration, bool)> = None;

        loop {
            self.drain_completed_jobs(&mut join_set, &mut active)
                .await?;

            let capacity = self.config.concurrency.saturating_sub(active.len());
            let now = self.clock.now();
            let sleep_for = reserve_sleep
                .map(|(duration, _)| duration)
                .unwrap_or_else(|| sleep_duration(now, poll_delay, next_schedule_at));

            tokio::select! {
                biased;
                join_result = join_set.join_next(), if !join_set.is_empty() => {
                    if let Some(result) = join_result {
                        self.resolve_join_result(result, &mut active).await?;
                    }
                }
                _ = heartbeat_tick.tick(), if !active.is_empty() => {
                    self.heartbeat_active(&active).await?;
                }
                _ = reap_tick.tick() => {
                    let _ = self.store.reap_expired(&self.namespace, self.clock.now(), self.config.concurrency).await?;
                }
                reserved = self.store.reserve(
                    &self.namespace,
                    &self.config,
                    capacity,
                    self.clock.now(),
                    new_lease_tokens(capacity),
                ), if capacity > 0 && reserve_sleep.is_none() => {
                    match reserved {
                        Ok(reserved) => {
                            next_schedule_at = reserved.next_schedule_at;
                            if !reserved.jobs.is_empty() {
                                poll_delay = self.config.poll_interval_min;
                                for lease in reserved.jobs {
                                    self.spawn_job(&mut join_set, &mut active, Arc::clone(&handler), lease);
                                }
                                continue;
                            }
                            let now = self.clock.now();
                            reserve_sleep = Some((sleep_duration(now, poll_delay, next_schedule_at), true));
                        }
                        Err(error) if retryable_reserve_error(&error) => {
                            tracing::warn!(
                                error = %error,
                                "worker reserve timed out; backing off before retry"
                            );
                            reserve_sleep = Some((poll_delay, false));
                            poll_delay = next_poll_delay(poll_delay, self.config.poll_interval_max);
                        }
                        Err(error) => return Err(error),
                    }
                }
                _ = self.coordinator.sleep(sleep_for), if capacity > 0 && reserve_sleep.is_some() => {
                    if let Some((_, advance_poll_delay)) = reserve_sleep.take()
                        && advance_poll_delay
                    {
                        poll_delay = next_poll_delay(poll_delay, self.config.poll_interval_max);
                    }
                }
            }
        }
    }

    /// Resolves finished jobs before polling Redis so reserve backoff cannot delay acknowledgements.
    async fn drain_completed_jobs(
        &self,
        join_set: &mut JoinSet<CompletedLease>,
        active: &mut HashMap<String, ActiveLease>,
    ) -> Result<()> {
        while let Some(result) = join_set.try_join_next() {
            self.resolve_join_result(result, active).await?;
        }
        Ok(())
    }

    async fn resolve_join_result(
        &self,
        join_result: std::result::Result<CompletedLease, JoinError>,
        active: &mut HashMap<String, ActiveLease>,
    ) -> Result<()> {
        let completed = join_result.map_err(|source| Error::WorkerTaskJoin { source })?;
        active.remove(&completed.job_id);
        self.resolve_completed(completed).await
    }

    fn spawn_job(
        &self,
        join_set: &mut JoinSet<CompletedLease>,
        active: &mut HashMap<String, ActiveLease>,
        handler: Arc<dyn RuntimeHandler>,
        lease: ReservedJob,
    ) {
        let job_id = lease.job_id.clone();
        active.insert(
            job_id.clone(),
            ActiveLease {
                job_id: lease.job_id.clone(),
                lease_token: lease.lease_token.clone(),
            },
        );

        join_set.spawn(async move {
            let result = handler.handle(lease.job_id.clone()).await;
            CompletedLease {
                job_id: lease.job_id,
                lease_token: lease.lease_token,
                result,
            }
        });
    }

    async fn heartbeat_active(&self, active: &HashMap<String, ActiveLease>) -> Result<()> {
        let now = self.clock.now();
        for lease in active.values() {
            self.store
                .heartbeat(
                    &self.namespace,
                    &self.config.worker_id,
                    &lease.job_id,
                    &lease.lease_token,
                    now,
                    self.config.lease_duration,
                )
                .await?;
        }
        Ok(())
    }

    async fn resolve_completed(&self, completed: CompletedLease) -> Result<()> {
        match completed.result {
            Ok(JobOutcome::Complete) => {
                self.store
                    .ack(
                        &self.namespace,
                        &self.config.worker_id,
                        &completed.job_id,
                        &completed.lease_token,
                    )
                    .await
            }
            Ok(JobOutcome::Reschedule { schedule_at }) => {
                self.store
                    .complete_and_reschedule(
                        &self.namespace,
                        &self.config.worker_id,
                        &completed.job_id,
                        &completed.lease_token,
                        schedule_at,
                        self.clock.now(),
                    )
                    .await
            }
            Ok(JobOutcome::Fail { .. }) => self
                .store
                .fail_or_retry(
                    &self.namespace,
                    FailureRequest {
                        worker_id: self.config.worker_id.clone(),
                        job_id: completed.job_id.clone(),
                        lease_token: completed.lease_token.clone(),
                        action: FailureAction::Terminal,
                        retry_policy: self.config.retry_policy.clone(),
                        now: self.clock.now(),
                    },
                )
                .await
                .map(|_| ()),
            Err(message) => {
                let now = self.clock.now();
                self.store
                    .fail_or_retry(
                        &self.namespace,
                        FailureRequest {
                            worker_id: self.config.worker_id.clone(),
                            job_id: completed.job_id.clone(),
                            lease_token: completed.lease_token.clone(),
                            action: FailureAction::Retryable,
                            retry_policy: self.config.retry_policy.clone(),
                            now,
                        },
                    )
                    .await?;
                tracing::debug!(job_id = %completed.job_id, error = %message, "worker handled retryable failure");
                Ok(())
            }
        }
    }
}

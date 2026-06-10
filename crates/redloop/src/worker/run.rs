//! Worker runtime loop and queue interactions.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::task::{JoinError, JoinSet};

use crate::clock::{Clock, WorkerCoordinator};
use crate::config::{Timestamp, WorkerConfig};
use crate::error::{Error, Result};
use crate::store::{FailureAction, FailureRequest, QueueStore};
use crate::types::JobOutcome;

use super::handler::{DynJobHandler, RedloopWorkerRuntime, RuntimeHandler, TraitHandlerAdapter};
use super::leases::{
    ActiveLease, CompletedLease, new_lease_tokens, next_poll_delay, sleep_duration,
};
use super::recovery::{
    recoverable_runtime_result, runtime_backoff_deadline, runtime_backoff_ready,
    runtime_backoff_remaining,
};
use super::spawning::spawn_job;

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
        let mut pending_completed = HashMap::<String, CompletedLease>::new();
        let mut poll_delay = self.config.poll_interval_min;
        let mut next_schedule_at: Option<Timestamp> = None;
        let mut reserve_sleep: Option<(Duration, bool)> = None;
        let mut runtime_backoff_until = None;

        loop {
            self.drain_joined_jobs(&mut join_set, &mut pending_completed)?;
            if runtime_backoff_ready(runtime_backoff_until) {
                runtime_backoff_until = None;
            }
            if runtime_backoff_until.is_none()
                && self
                    .resolve_pending_completed(&mut active, &mut pending_completed)
                    .await?
            {
                runtime_backoff_until =
                    Some(runtime_backoff_deadline(self.config.poll_interval_min));
            }

            let capacity = self.config.concurrency.saturating_sub(active.len());
            let now = self.clock.now();
            let sleep_for = reserve_sleep
                .map(|(duration, _)| duration)
                .unwrap_or_else(|| sleep_duration(now, poll_delay, next_schedule_at));
            let runtime_sleep_for = runtime_backoff_remaining(runtime_backoff_until);

            tokio::select! {
                biased;
                join_result = join_set.join_next(), if !join_set.is_empty() => {
                    if let Some(result) = join_result {
                        self.record_join_result(result, &mut pending_completed)?;
                    }
                }
                _ = heartbeat_tick.tick(), if !active.is_empty() => {
                    if recoverable_runtime_result("heartbeat", self.heartbeat_active(&active).await)? {
                        runtime_backoff_until = Some(runtime_backoff_deadline(self.config.poll_interval_min));
                    }
                }
                _ = reap_tick.tick(), if runtime_backoff_until.is_none() => {
                    let result = self
                        .store
                        .reap_expired(&self.namespace, self.clock.now(), self.config.concurrency)
                        .await
                        .map(|_| ());
                    if recoverable_runtime_result("reap_expired", result)? {
                        runtime_backoff_until = Some(runtime_backoff_deadline(self.config.poll_interval_min));
                    }
                }
                _ = self.coordinator.sleep(runtime_sleep_for), if runtime_backoff_until.is_some() => {
                    runtime_backoff_until = None;
                }
                reserved = self.store.reserve(
                    &self.namespace,
                    &self.config,
                    capacity,
                    self.clock.now(),
                    new_lease_tokens(capacity),
                ), if capacity > 0 && reserve_sleep.is_none() && runtime_backoff_until.is_none() => {
                    match reserved {
                        Ok(reserved) => {
                            next_schedule_at = reserved.next_schedule_at;
                            if !reserved.jobs.is_empty() {
                                poll_delay = self.config.poll_interval_min;
                                for lease in reserved.jobs {
                                    spawn_job(&mut join_set, &mut active, Arc::clone(&handler), lease);
                                }
                                continue;
                            }
                            let now = self.clock.now();
                            reserve_sleep = Some((sleep_duration(now, poll_delay, next_schedule_at), true));
                        }
                        Err(error) if error.is_recoverable_worker_runtime() => {
                            tracing::warn!(
                                error = %error,
                                "worker reserve hit recoverable Redis error; backing off before retry"
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

    /// Records finished jobs before polling Redis so reserve backoff cannot delay acknowledgements.
    fn drain_joined_jobs(
        &self,
        join_set: &mut JoinSet<CompletedLease>,
        pending_completed: &mut HashMap<String, CompletedLease>,
    ) -> Result<()> {
        while let Some(result) = join_set.try_join_next() {
            self.record_join_result(result, pending_completed)?;
        }
        Ok(())
    }

    fn record_join_result(
        &self,
        join_result: std::result::Result<CompletedLease, JoinError>,
        pending_completed: &mut HashMap<String, CompletedLease>,
    ) -> Result<()> {
        let completed = join_result.map_err(|source| Error::WorkerTaskJoin { source })?;
        pending_completed.insert(completed.job_id.clone(), completed);
        Ok(())
    }

    async fn resolve_pending_completed(
        &self,
        active: &mut HashMap<String, ActiveLease>,
        pending_completed: &mut HashMap<String, CompletedLease>,
    ) -> Result<bool> {
        let mut should_back_off = false;
        let job_ids = pending_completed.keys().cloned().collect::<Vec<_>>();
        for job_id in job_ids {
            let Some(completed) = pending_completed.remove(&job_id) else {
                continue;
            };
            match self.resolve_completed(&completed).await {
                Ok(()) => {
                    active.remove(&completed.job_id);
                }
                Err(error) if error.is_recoverable_worker_runtime() => {
                    tracing::warn!(
                        job_id = %completed.job_id,
                        error = %error,
                        "worker completion resolution hit recoverable Redis error; keeping lease active and retrying"
                    );
                    pending_completed.insert(completed.job_id.clone(), completed);
                    should_back_off = true;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(should_back_off)
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

    async fn resolve_completed(&self, completed: &CompletedLease) -> Result<()> {
        match &completed.result {
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
                        schedule_at.to_owned(),
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

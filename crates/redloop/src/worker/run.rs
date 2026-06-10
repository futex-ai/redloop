//! Worker runtime loop and queue interactions.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::task::{JoinError, JoinSet};

use crate::clock::{Clock, WorkerCoordinator};
use crate::config::{Timestamp, WorkerConfig};
use crate::error::{Error, Result};
use crate::store::QueueStore;

use super::completion::resolve_completed;
use super::handler::{DynJobHandler, RedloopWorkerRuntime, RuntimeHandler, TraitHandlerAdapter};
use super::heartbeat::heartbeat_active;
use super::leases::{
    ActiveLease, CompletedLease, LeaseAttemptKey, new_lease_tokens, next_poll_delay, sleep_duration,
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
        let mut active = HashMap::<LeaseAttemptKey, ActiveLease>::new();
        let mut pending_completed = HashMap::<LeaseAttemptKey, CompletedLease>::new();
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
                    let result = heartbeat_active(
                        &self.namespace,
                        self.store.as_ref(),
                        self.clock.as_ref(),
                        &self.config,
                        &mut active,
                    )
                    .await;
                    if recoverable_runtime_result("heartbeat", result)? {
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
        pending_completed: &mut HashMap<LeaseAttemptKey, CompletedLease>,
    ) -> Result<()> {
        while let Some(result) = join_set.try_join_next() {
            self.record_join_result(result, pending_completed)?;
        }
        Ok(())
    }

    fn record_join_result(
        &self,
        join_result: std::result::Result<CompletedLease, JoinError>,
        pending_completed: &mut HashMap<LeaseAttemptKey, CompletedLease>,
    ) -> Result<()> {
        let completed = join_result.map_err(|source| Error::WorkerTaskJoin { source })?;
        pending_completed.insert(completed.attempt_key(), completed);
        Ok(())
    }

    async fn resolve_pending_completed(
        &self,
        active: &mut HashMap<LeaseAttemptKey, ActiveLease>,
        pending_completed: &mut HashMap<LeaseAttemptKey, CompletedLease>,
    ) -> Result<bool> {
        let attempt_keys = pending_completed.keys().cloned().collect::<Vec<_>>();
        for attempt_key in attempt_keys {
            let Some(completed) = pending_completed.remove(&attempt_key) else {
                continue;
            };
            let completed_key = completed.attempt_key();
            match resolve_completed(
                &self.namespace,
                self.store.as_ref(),
                self.clock.as_ref(),
                &self.config,
                &completed,
            )
            .await
            {
                Ok(()) => {
                    active.remove(&completed_key);
                }
                Err(Error::LeaseMismatch { job_id }) => {
                    tracing::warn!(
                        job_id = %completed.job_id,
                        lease_mismatch_job_id = %job_id,
                        "worker completion lost lease; dropping local lease attempt"
                    );
                    active.remove(&completed_key);
                }
                Err(error) if error.is_recoverable_worker_runtime() => {
                    tracing::warn!(
                        job_id = %completed.job_id,
                        error = %error,
                        "worker completion resolution hit recoverable Redis error; keeping lease active and retrying"
                    );
                    pending_completed.insert(completed_key, completed);
                    return Ok(true);
                }
                Err(error) => return Err(error),
            }
        }
        Ok(false)
    }
}

use crate::client::Namespace;
use crate::config::Timestamp;
use crate::contract::{DynRedloopScheduledBuilder, RedloopEnqueueBuilder, RedloopScheduledBuilder};
use crate::error::Result;
use crate::store::{EnqueueRequest, ReplaceMode};
use crate::types::EnqueueResult;
use async_trait::async_trait;
use std::sync::Arc;

/// Batch enqueue item built from the fluent job API.
#[derive(Debug, Clone)]
pub struct BatchEnqueueItem {
    pub(crate) request: EnqueueRequest,
}

/// Immediate job builder.
#[derive(Clone)]
pub(crate) struct JobBuilder {
    namespace: Namespace,
    job_id: String,
}

/// Scheduled job builder with replacement controls.
#[derive(Clone)]
pub(crate) struct ScheduledJobBuilder {
    namespace: Namespace,
    job_id: String,
    schedule_at: Timestamp,
    replace_mode: ReplaceMode,
}

impl JobBuilder {
    pub(crate) fn new(namespace: Namespace, job_id: String) -> Self {
        Self { namespace, job_id }
    }

    /// Moves the builder into the scheduled path.
    pub fn schedule_at(self, schedule_at: Timestamp) -> ScheduledJobBuilder {
        ScheduledJobBuilder {
            namespace: self.namespace,
            job_id: self.job_id,
            schedule_at,
            replace_mode: ReplaceMode::Always,
        }
    }

    /// Converts the builder into a batch item.
    pub fn into_batch_item(self) -> BatchEnqueueItem {
        BatchEnqueueItem {
            request: EnqueueRequest {
                job_id: self.job_id,
                schedule_at: None,
                replace_mode: ReplaceMode::Always,
                allow_existing_current_run: true,
            },
        }
    }

    /// Executes the builder as an immediate enqueue.
    pub async fn execute(self) -> Result<EnqueueResult> {
        self.namespace
            .enqueue_request(EnqueueRequest {
                job_id: self.job_id,
                schedule_at: None,
                replace_mode: ReplaceMode::Always,
                allow_existing_current_run: true,
            })
            .await
    }
}

#[async_trait]
impl RedloopEnqueueBuilder for JobBuilder {
    fn schedule_at(&self, schedule_at: Timestamp) -> DynRedloopScheduledBuilder {
        Arc::new(self.clone().schedule_at(schedule_at))
    }

    fn to_batch_item(&self) -> BatchEnqueueItem {
        self.clone().into_batch_item()
    }

    async fn execute(&self) -> Result<EnqueueResult> {
        self.clone().execute().await
    }
}

#[async_trait]
impl RedloopScheduledBuilder for ScheduledJobBuilder {
    fn replace_if_earlier(&self) -> DynRedloopScheduledBuilder {
        Arc::new(self.clone().replace_if_earlier())
    }

    fn replace_if_later(&self) -> DynRedloopScheduledBuilder {
        Arc::new(self.clone().replace_if_later())
    }

    fn to_batch_item(&self) -> BatchEnqueueItem {
        self.clone().into_batch_item()
    }

    async fn execute(&self) -> Result<EnqueueResult> {
        self.clone().execute().await
    }
}

impl ScheduledJobBuilder {
    /// Only replace an existing waiting placement when the new activation time is earlier.
    pub fn replace_if_earlier(mut self) -> Self {
        self.replace_mode = ReplaceMode::IfEarlier;
        self
    }

    /// Only replace an existing waiting placement when the new activation time is later.
    pub fn replace_if_later(mut self) -> Self {
        self.replace_mode = ReplaceMode::IfLater;
        self
    }

    /// Converts the builder into a batch item.
    pub fn into_batch_item(self) -> BatchEnqueueItem {
        BatchEnqueueItem {
            request: EnqueueRequest {
                job_id: self.job_id,
                schedule_at: Some(self.schedule_at),
                replace_mode: self.replace_mode,
                allow_existing_current_run: false,
            },
        }
    }

    /// Executes the scheduled builder.
    pub async fn execute(self) -> Result<EnqueueResult> {
        self.namespace
            .enqueue_request(EnqueueRequest {
                job_id: self.job_id,
                schedule_at: Some(self.schedule_at),
                replace_mode: self.replace_mode,
                allow_existing_current_run: false,
            })
            .await
    }
}

#[cfg(test)]
#[path = "_tests_/builder_tests.rs"]
mod builder_tests;

//! Queue counts and failed-job listing operations.

use crate::error::Result;
use crate::types::{FailedJobsPage, FailedJobsQuery, QueueCounts};

use super::Namespace;

impl Namespace {
    /// Returns queue counts.
    pub async fn counts(&self) -> Result<QueueCounts> {
        self.store.counts(&self.namespace, self.clock.now()).await
    }

    /// Lists failed jobs.
    pub async fn list_failed(&self, query: FailedJobsQuery) -> Result<FailedJobsPage> {
        self.store.list_failed(&self.namespace, query).await
    }
}

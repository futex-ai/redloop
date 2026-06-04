//! Worker-related namespace operations.

use std::sync::Arc;

use crate::config::WorkerConfig;
use crate::types::WorkerInfo;
use crate::worker::handler::DynRedloopWorkerRuntime;
use crate::worker::run::Worker;

use super::Namespace;

impl Namespace {
    /// Lists active and recent workers.
    pub async fn list_workers(&self) -> crate::error::Result<Vec<WorkerInfo>> {
        self.store
            .list_workers(&self.namespace, self.clock.now())
            .await
    }

    /// Creates a worker handle for this namespace.
    pub fn worker(&self, config: WorkerConfig) -> DynRedloopWorkerRuntime {
        Arc::new(self.worker_runtime(config))
    }

    pub(crate) fn worker_runtime(&self, config: WorkerConfig) -> Worker {
        Worker::new(
            self.namespace.clone(),
            Arc::clone(&self.store),
            Arc::clone(&self.clock),
            Arc::clone(&self.coordinator),
            config,
        )
    }

    pub(crate) async fn remember_namespace(&self) {
        if let Err(error) = self.store.remember_namespace(&self.namespace).await {
            tracing::warn!(namespace = %self.namespace, ?error, "failed to remember namespace");
        }
    }
}

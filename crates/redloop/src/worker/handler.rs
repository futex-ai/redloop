//! Worker handler traits and adapters.

use std::sync::Arc;

use async_trait::async_trait;

use crate::error::Result;
use crate::types::{HandlerResult, JobError, JobOutcome};

/// Async job handler trait.
#[async_trait]
pub trait JobHandler: Send + Sync {
    async fn handle(&self, job_id: String) -> HandlerResult;
}

pub type DynJobHandler = Arc<dyn JobHandler>;

#[async_trait]
pub trait RedloopWorkerRuntime: Send + Sync {
    async fn run(&self, handler: DynJobHandler) -> Result<()>;
}

pub type DynRedloopWorkerRuntime = Arc<dyn RedloopWorkerRuntime>;

#[cfg_attr(test, unimock::unimock(api = RuntimeHandlerMock))]
#[async_trait]
pub(crate) trait RuntimeHandler: Send + Sync {
    async fn handle(&self, job_id: String) -> std::result::Result<JobOutcome, String>;
}

pub(crate) struct TraitHandlerAdapter {
    pub(crate) inner: Arc<dyn JobHandler>,
}

#[async_trait]
impl RuntimeHandler for TraitHandlerAdapter {
    async fn handle(&self, job_id: String) -> std::result::Result<JobOutcome, String> {
        self.inner
            .handle(job_id)
            .await
            .map_err(|error| match error {
                JobError::Retryable { message } => message,
            })
    }
}

//! Redis-backed queue library for ASAP and timed jobs.

mod builder;
mod client;
mod clock;
mod config;
mod contract;
mod error;
mod redis_store;
mod store;
mod types;
mod worker;

pub use builder::BatchEnqueueItem;
pub use client::Redloop as RedisRedloopClient;
pub use config::{Backoff, ConnectConfig, RedisDeployment, RetryPolicy, Timestamp, WorkerConfig};
pub use contract::{
    DynRedloopClient, DynRedloopEnqueueBuilder, DynRedloopNamespace, DynRedloopScheduledBuilder,
    RedloopClient, RedloopEnqueueBuilder, RedloopNamespace, RedloopScheduledBuilder,
};
pub use error::{Error, InvalidConfigKind, Result};
pub use types::{
    BatchEnqueueResult, EnqueueResult, EnqueueStatus, FailedJobsPage, FailedJobsQuery,
    FailedSelector, ForceFailRequest, HandlerResult, JobError, JobOutcome, JobRecord, JobState,
    LeaseHolder, QueueCounts, RescheduleResult, WorkerInfo,
};
pub use worker::handler::{
    DynJobHandler, DynRedloopWorkerRuntime, JobHandler, RedloopWorkerRuntime,
};

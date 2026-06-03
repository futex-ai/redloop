# Redloop API

This page defines the required Rust-facing API surface for `redloop`.

The exact module layout may differ, but the public capabilities, carried data,
and semantics here are normative.

The exact Redis key layout and atomic script flows are defined in
[Redloop Redis Layout](./redis-layout.md).

## Crate Surface

```rust
pub type Result<T> = std::result::Result<T, Error>;
pub type Timestamp = chrono::DateTime<chrono::Utc>;

pub type DynRedloopClient = std::sync::Arc<dyn RedloopClient>;
pub type DynRedloopNamespace = std::sync::Arc<dyn RedloopNamespace>;
pub type DynRedloopEnqueueBuilder = std::sync::Arc<dyn RedloopEnqueueBuilder>;
pub type DynRedloopScheduledBuilder = std::sync::Arc<dyn RedloopScheduledBuilder>;
pub type DynRedloopWorkerRuntime = std::sync::Arc<dyn RedloopWorkerRuntime>;
pub type DynJobHandler = std::sync::Arc<dyn JobHandler>;

pub struct RedisRedloopClient;
impl RedisRedloopClient {
    pub async fn connect(config: ConnectConfig) -> Result<Self>;
}

#[async_trait::async_trait]
pub trait RedloopClient: Send + Sync {
    fn namespace(&self, namespace: String) -> DynRedloopNamespace;
    async fn list_namespaces(&self) -> Result<Vec<String>>;
}

#[async_trait::async_trait]
pub trait RedloopNamespace: Send + Sync {
    fn job(&self, job_id: &str) -> DynRedloopEnqueueBuilder;
    async fn enqueue_batch(&self, requests: Vec<BatchEnqueueItem>) -> Result<BatchEnqueueResult>;
    async fn reschedule(&self, job_id: &str, schedule_at: Option<Timestamp>) -> Result<RescheduleResult>;
    async fn get_job(&self, job_id: &str) -> Result<Option<JobRecord>>;
    async fn counts(&self) -> Result<QueueCounts>;
    async fn list_failed(&self, query: FailedJobsQuery) -> Result<FailedJobsPage>;
    async fn list_workers(&self) -> Result<Vec<WorkerInfo>>;
    async fn cancel(&self, job_id: &str) -> Result<()>;
    async fn force_ack(&self, job_id: &str) -> Result<()>;
    async fn force_fail(&self, request: ForceFailRequest) -> Result<()>;
    async fn requeue(&self, job_id: &str) -> Result<()>;
    async fn retry_now(&self, job_id: &str) -> Result<()>;
    async fn force_retry(&self, job_id: &str) -> Result<()>;
    async fn purge_failed(&self, selector: FailedSelector) -> Result<u64>;
    fn worker(&self, config: WorkerConfig) -> DynRedloopWorkerRuntime;
}

#[async_trait::async_trait]
pub trait RedloopEnqueueBuilder: Send + Sync {
    fn schedule_at(&self, schedule_at: Timestamp) -> DynRedloopScheduledBuilder;
    fn to_batch_item(&self) -> BatchEnqueueItem;
    async fn execute(&self) -> Result<EnqueueResult>;
}

#[async_trait::async_trait]
pub trait RedloopScheduledBuilder: Send + Sync {
    fn replace_if_earlier(&self) -> DynRedloopScheduledBuilder;
    fn replace_if_later(&self) -> DynRedloopScheduledBuilder;
    fn to_batch_item(&self) -> BatchEnqueueItem;
    async fn execute(&self) -> Result<EnqueueResult>;
}

#[async_trait::async_trait]
pub trait RedloopWorkerRuntime: Send + Sync {
    async fn run(&self, handler: DynJobHandler) -> Result<()>;
}

#[async_trait::async_trait]
pub trait JobHandler: Send + Sync {
    async fn handle(&self, job_id: String) -> HandlerResult;
}
```

`redloop` is ID-only. Application data must be stored elsewhere and looked up
by `job_id`.

The Redis-backed concrete client, namespace handle, worker runtime, and enqueue
builder implementations may stay internal to the crate. Downstream crates
should depend on the dyn traits and composition roots should construct
`RedisRedloopClient`.

## Types

```rust
pub enum Error { NotFound, InvalidState, LeaseMismatch, InvalidConfig, Redis }

pub struct ConnectConfig {
    pub deployment: RedisDeployment,
    pub key_prefix: String,
    pub command_timeout: std::time::Duration,
}

pub enum RedisDeployment {
    Standalone { url: String },
    Sentinel { service_name: String, nodes: Vec<String> },
    Cluster { nodes: Vec<String> },
}

pub struct BatchEnqueueItem;
pub struct EnqueueResult { pub job_id: String, pub state: JobState, pub status: EnqueueStatus }
pub enum EnqueueStatus { Created, Unchanged, Updated }
pub struct BatchEnqueueResult { pub items: Vec<EnqueueResult> }

pub struct RescheduleResult { pub job_id: String, pub state: JobState, pub schedule_at: Option<Timestamp> }
pub struct ForceFailRequest { pub job_id: String, pub message: String }

pub struct WorkerConfig {
    pub worker_id: String,
    pub concurrency: usize,
    pub retry_policy: RetryPolicy,
    pub lease_duration: std::time::Duration,
    pub heartbeat_interval: std::time::Duration,
    pub reap_interval: std::time::Duration,
    pub poll_interval_min: std::time::Duration,
    pub poll_interval_max: std::time::Duration,
}

pub type HandlerResult = std::result::Result<JobOutcome, JobError>;
pub enum JobError { Retryable { message: String } }
pub enum JobOutcome {
    Complete,
    Reschedule { schedule_at: Option<Timestamp> },
    Fail { message: String },
}

pub enum RetryPolicy {
    Never,
    Count { max_retries: u32, backoff: Backoff },
    Infinite { backoff: Backoff },
}
pub enum Backoff {
    None,
    Fixed { delay_ms: u64 },
    Exponential { initial_delay_ms: u64, multiplier: u32, max_delay_ms: u64 },
}
pub enum JobState { Ready, Scheduled, Leased, Failed }

pub struct JobRecord {
    pub job_id: String,
    pub state: JobState,
    pub ready_at: Option<Timestamp>,
    pub schedule_at: Option<Timestamp>,
    pub failure_count: u32,
    pub leased_by: Option<LeaseHolder>,
    pub failed_at: Option<Timestamp>,
}

pub struct LeaseHolder { pub worker_id: String, pub lease_token: String, pub leased_until: Timestamp }
pub struct QueueCounts {
    pub ready_count: u64,
    pub scheduled_due_count: u64,
    pub scheduled_future_count: u64,
    pub leased_count: u64,
    pub failed_count: u64,
}
pub struct FailedJobsQuery { pub cursor: Option<String>, pub limit: usize }
pub struct FailedJobsPage { pub jobs: Vec<JobRecord>, pub next_cursor: Option<String> }
pub struct WorkerInfo { pub worker_id: String, pub concurrency: usize, pub leased_count: u64, pub last_heartbeat_at: Timestamp }
pub enum FailedSelector { JobId(String), OlderThan(Timestamp), All }
```

## Required Method Semantics

- `RedisRedloopClient::connect(...)` creates the Redis-backed adapter.
- `RedloopClient::namespace(...)` returns a namespace handle.
- `RedloopNamespace::job(job_id)` starts a builder for that job ID.
- `job_id` inputs must be valid UTF-8 and no longer than `256` bytes.
- `job(job_id).execute()` creates an ASAP job by default.
- `job(job_id).schedule_at(timestamp)` enters the scheduled path and returns a
  scheduled builder.
- the immediate path does not expose comparison-based override methods.
- `job(job_id).execute()` returns `Created` for a new job, `Updated` when a
  scheduled or failed job is reactivated into `ready` or when a leased job gets
  a durable rerun request, and `Unchanged` when the existing current run is
  already `ready` or already has a leased-rerun request.
- `job(job_id).schedule_at(timestamp).execute()` returns `Created` for a new
  job, `Updated` when a waiting job in `ready` or `scheduled` is moved or
  changed, `Unchanged` when the requested placement already matches the
  existing waiting placement, and `InvalidState` when the job is `leased`.
- `replace_if_earlier()` and `replace_if_later()` are available only on the
  scheduled builder and return `Created`, `Updated`, or `Unchanged` under the
  same ordering rules as the current implementation.
- immediate enqueue must not create a second current run for an existing
  `ready` or `leased` job.
- immediate enqueue against a `leased` job must durably request one rerun after
  the lease finishes; the current lease remains authoritative until ack,
  reschedule, failure, or reaping resolves it.
- normal queue commands against an existing `failed` job must reactivate it and
  clear failed state atomically.
- `enqueue_batch` is atomic per item, not necessarily all-or-nothing across the batch.
- `reschedule` updates a waiting job in place and must fail for `leased` and `failed`.
- `cancel` applies only to scheduled or failed jobs and must not cancel an
  existing current run in `ready` or `leased`.
- `force_ack` and `force_fail` are operator methods for leased jobs and do not
  require a lease token.
- `retry_now` moves a failed or scheduled job into `ready`.
- `force_retry` may reactivate a failed job and starts a fresh failure lifecycle.
- `purge_failed` returns the number of failed jobs deleted.
- workers reserve directly from `ready` and due `scheduled`; there is no activation stage.
- the reserve function must compare the oldest `ready` item and the oldest due
  `scheduled` item and lease the older activation time first.
- if activation times are equal, `ready` wins.
- a `job_id` must exist in at most one of `ready`, `scheduled`, `leased`, or `failed` at a time.
- if an invalid duplicate scheduled occurrence is encountered while the same
  `job_id` already has a current run in `ready` or `leased`, the scheduled
  occurrence must be dropped defensively.
- `RedloopWorkerRuntime::run(...)` keeps polling, leasing, heartbeating, and
  reaping until cancelled or a fatal runtime error occurs.
- when a worker has fewer than `concurrency` in-flight jobs, the runtime must
  poll the reserve function for up to the remaining capacity in one round trip.
- polling must use adaptive backoff bounded by `poll_interval_min` and `poll_interval_max`.
- Redis or command timeouts while polling the reserve function are transient:
  the runtime must log, back off, and keep the worker process alive so active
  jobs can finish; non-timeout reserve errors remain fatal runtime errors.
- reserve-timeout backoff must not starve active-job joins, heartbeats, or
  lease reaping; those runtime paths remain eligible before the next reserve
  attempt.
- any successful reservation resets the current poll delay to `poll_interval_min`.
- the runtime may shorten its sleep to the next due `schedule_at` returned by the reserve path.
- the public worker handler receives only `job_id`.
- lease token, failure count, `ready_at`, and `schedule_at` are internal runtime
  concerns and are not exposed to the public worker callback.
- `JobRecord.ready_at` is populated only for current runs in `ready`.
- `JobRecord.schedule_at` is populated only for scheduled jobs.
- `WorkerConfig.retry_policy` applies to explicit handler failures for all jobs
  processed by that worker.
- all workers serving one namespace must use the same retry policy.
- startup or registration must fail on retry-policy mismatch.
- `redloop` does not persist payload data; application code must resolve `job_id` externally.
- `FailedJobsQuery.cursor` is opaque to callers; the current implementation uses an offset string.
- failure messages provided to `Err(...)` or `JobOutcome::Fail { ... }` are
  runtime inputs and must not be persisted by the queue in Redis.
- `ForceFailRequest.message` is also a runtime-only reason string and must not
  be persisted by the queue in Redis.
- `Err(error)` from `JobHandler::handle(...)` is treated as retryable failure
  and uses the error message for failure reporting.
- a worker may bypass retry policy by returning `Ok(JobOutcome::Fail { ... })`.

## Worker Completion Contract

Each lease must resolve with exactly one result:

- `Ok(JobOutcome::Complete)`: acknowledge and delete the job
- `Ok(JobOutcome::Reschedule { schedule_at })`: clear the lease and move the
  same job to its next schedule
- `Ok(JobOutcome::Fail { message })`: move directly to the failed queue
- `Err(error)`: consume retry budget and requeue or fail according to worker policy

`Reschedule` is the explicit "finish this run and schedule the same job again"
path. It starts a fresh failure lifecycle for the next run. `Err(error)` is the
explicit retry path.

## Example

```rust
use chrono::{Duration as ChronoDuration, Utc};
use redloop::{
    Backoff, ConnectConfig, DynRedloopNamespace, JobHandler, JobOutcome,
    RedisDeployment, RedisRedloopClient, RetryPolicy, WorkerConfig,
};
use std::{sync::Arc, time::Duration};

struct AppState;

struct ExampleJobHandler {
    state: Arc<AppState>,
}

#[async_trait::async_trait]
impl JobHandler for ExampleJobHandler {
    async fn handle(&self, job_id: String) -> redloop::HandlerResult {
        process(self.state.clone(), &job_id).await?;
        Ok(JobOutcome::Reschedule {
            schedule_at: Some(Utc::now() + ChronoDuration::days(1)),
        })
    }
}

#[tokio::main]
async fn main() -> redloop::Result<()> {
    let redloop = RedisRedloopClient::connect(ConnectConfig {
        deployment: RedisDeployment::Standalone { url: "redis://127.0.0.1/".into() },
        key_prefix: "redloop".into(),
        command_timeout: Duration::from_secs(5),
    })
    .await?;

    let queue: DynRedloopNamespace = redloop.namespace("notifications".to_owned());
    let state = Arc::new(AppState);

    queue
        .job("welcome-email:user-42")
        .schedule_at(Utc::now() + ChronoDuration::hours(2))
        .replace_if_earlier()
        .execute()
        .await?;

    queue
        .worker(WorkerConfig {
            worker_id: "worker-a".into(),
            concurrency: 32,
            retry_policy: RetryPolicy::Count {
                max_retries: 5,
                backoff: Backoff::Fixed { delay_ms: 1_000 },
            },
            lease_duration: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(10),
            reap_interval: Duration::from_secs(5),
            poll_interval_min: Duration::from_millis(10),
            poll_interval_max: Duration::from_millis(250),
        })
        .run(Arc::new(ExampleJobHandler { state }))
        .await?;

    Ok(())
}

async fn process(
    _state: Arc<AppState>,
    _job_id: &str,
) -> std::result::Result<(), redloop::JobError> {
    Ok(())
}
```

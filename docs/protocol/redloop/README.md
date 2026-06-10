# Redloop

`redloop` is a Redis-backed Rust queue library for ASAP and timed jobs. This page defines the runtime and behavior contract that the implementation follows.

## Scope

- `redloop` exposes a Rust library API and a local CLI.
- There is no separate manager service.
- Delivery is `at-least-once`.
- Scheduling is time-based only. There is no priority field.
- Jobs are metadata-only. `redloop` does not store payloads.
- Supported Redis deployments: standalone, Sentinel, and Cluster.
- In Cluster mode, all keys for one namespace must share a hash slot so a namespace can be mutated atomically.

The exact Redis key layout and atomic mutation flows are defined in [Redloop Redis Layout](./redis-layout.md).

## Core Model

Every job belongs to exactly one namespace. Namespaces isolate queue state, failed jobs, workers, retry policy, and counts.

There are two waiting sources:

- `ready`: `ZSET` keyed by `ready_at`
- `scheduled`: `ZSET` keyed by `schedule_at`

`ready` contains jobs that are immediately eligible for reservation. `scheduled` contains jobs that become eligible when `schedule_at <= now`.

Workers do not move jobs from `scheduled` into `ready`. Instead, they poll one atomic reserve function that compares the oldest `ready` job and the oldest due `scheduled` job and leases the older activation time first.

A `job_id` must exist in at most one of `ready`, `scheduled`, `leased`, or `failed` at a time.

## States

A `job_id` has at most one current-run state:

- `ready`
- `leased`
- `failed`

There is no retained `completed` state. Successful completion deletes the current run.

## Job Identity And Storage

- Clients must supply `job_id`.
- `job_id` must be valid UTF-8 and no longer than `256` bytes.
- Queue membership structures store job IDs only.
- `redloop` stores queue metadata only. It does not store payload bytes, JSON, or typed job data.
- If application data is needed, it must live outside `redloop` and be referenced by `job_id`.

This keeps Redis memory usage bounded to queue bookkeeping rather than arbitrary payload size.

## Enqueue Policy

Duplicate enqueue is a placement policy, not a payload conflict.

Existing jobs fall into three groups:

- current run: `ready` or `leased`
- scheduled: `scheduled`
- terminal failed: `failed`

The queued definition is only:

- `job_id`
- optional `schedule_at`

Rules:

- immediate enqueue must not create a second current run for an existing `ready` or `leased` job
- `job(job_id).execute()` must return `Unchanged` for an existing current run
- `job(job_id).execute()` must pull a scheduled job forward into `ready`
- `job(job_id).execute()` must reactivate a failed job into `ready`
- `job(job_id).schedule_at(timestamp).execute()` must create or update the placement of a waiting job in `ready` or `scheduled`
- `job(job_id).schedule_at(timestamp).execute()` may move a job from `ready` to `scheduled`, from `scheduled` to `ready`, or update its score in place
- `job(job_id).schedule_at(timestamp).execute()` must fail with `InvalidState` when the job is `leased`
- `replace_if_earlier()` and `replace_if_later()` refine only the scheduled path
- reactivating a failed job starts a fresh failure lifecycle

## Retry Policy

Retry policy is worker-scoped and namespace-consistent. Each worker configuration carries exactly one retry policy:

- `never`
- `count { max_retries, backoff }`
- `infinite { backoff }`

Supported backoff forms:

- `none`
- `fixed { delay_ms }`
- `exponential { initial_delay_ms, multiplier, max_delay_ms }`

Rules:

- a worker may bypass retry policy by returning `Ok(JobOutcome::Fail { ... })`
- lost leases and missed heartbeats do not consume retry budget
- retry budget is consumed only by explicit handler failure
- all workers serving the same namespace must use the same retry policy
- worker startup or registration must fail if its retry policy does not match the namespace's active retry policy

## Public Interfaces

`redloop` exposes three service interfaces:

- client API
- worker API
- operator API

The exact Rust-facing API surface is defined in [Redloop API](./api.md).

## Client API

The client API must provide:

- namespace handles
- chainable enqueue builders
- rescheduling
- job and count queries
- failed-job inspection
- operator actions

`job(job_id)` starts a builder for that job ID. The builder must support:

- required `job_id`
- `schedule_at(...)`
- `into_batch_item()`
- `execute()`

`schedule_at(...)` returns a scheduled builder. The scheduled builder must support:

- `replace_if_earlier()`
- `replace_if_later()`
- `into_batch_item()`
- `execute()`

Required shape:

```rust
queue
    .job("job_01")
    .schedule_at(schedule_at)
    .replace_if_earlier()
    .execute()
    .await?;
```

Rules:

- `job(job_id).execute()` places the job into `ready` with `ready_at = now`
- `schedule_at <= now` is normalized to an immediate `ready` placement
- `schedule_at > now` stores the job in `scheduled`
- `job(job_id).execute()` leaves an existing current run unchanged
- `job(job_id).schedule_at(timestamp).execute()` updates the waiting placement in place when the job is already `ready` or `scheduled`
- `job(job_id).schedule_at(timestamp).execute()` fails with `InvalidState` when the job is `leased`
- `replace_if_earlier()` updates only when the requested activation time is earlier than the existing waiting activation time
- `replace_if_later()` updates only when the requested activation time is later than the existing waiting activation time
- `replace_if_earlier()` and `replace_if_later()` reactivate an existing failed job unconditionally because there is no current scheduled time to compare
- `reschedule(namespace, job_id, schedule_at)` updates the waiting placement in place for `ready` or `scheduled`
- `reschedule` must fail for `leased` and `failed` jobs
- `list_failed` uses an opaque cursor; the current implementation uses an offset token

## Worker API

The worker API must support registering an async handler and processing multiple jobs in parallel.

Worker configuration must include:

- `worker_id`
- `concurrency`
- `retry_policy`
- `lease_duration`
- `heartbeat_interval`
- `reap_interval`
- `poll_interval_min`
- `poll_interval_max`

Worker rules:

- workers may start and stop at any time
- multiple workers may compete in the same namespace
- reservation must atomically assign a job to exactly one worker
- a worker may process up to `concurrency` jobs at once
- when in-flight jobs are below `concurrency`, the runtime must poll the reserve function for up to the remaining capacity
- polling must use adaptive backoff between `poll_interval_min` and `poll_interval_max`
- any successful reservation resets the poll delay to `poll_interval_min`
- when no work is found, the runtime may shorten its sleep to the next known due `schedule_at`
- the library manages heartbeats while a handler is running
- Redis command timeouts and transient Redis transport failures from reserve,
  heartbeat, reap, and completion mutations are recoverable runtime errors:
  workers log them, back off, and continue running
- invalid config, invalid stored data, invalid timestamps, job-id contract
  violations, lease/data-contract errors, and worker task join failures remain fatal
- handlers receive only `job_id`
- lease token, failure counters, and schedule metadata remain internal to the library runtime
- the handler contract is `Result<JobOutcome, E>`

Application code is responsible for resolving `job_id` into any external work data it needs.

## Activation-Ordered Reservation

Workers reserve directly from `ready` and due `scheduled`.

Rules:

- `ready` score is `ready_at`
- `scheduled` score is `schedule_at`
- the reserve function must compare the oldest `ready` item and the oldest due `scheduled` item
- the smaller score wins because it represents the older activation time
- if scores are equal, `ready` wins
- reservation may lease multiple jobs in one call, repeating the same comparison each time
- if an invalid duplicate scheduled occurrence is encountered for a `job_id` already present in `ready` or `leased`, the scheduled occurrence must be dropped defensively
- due scheduled duplicates must be dropped before the ready-vs-scheduled winner is chosen, so they can never survive long enough to run after the current copy completes
- polling workers must not scan the full queue; they must use bounded top-of-queue reads only

## Lease And Heartbeat

When a worker reserves a job:

- state becomes `leased`
- the job records `worker_id`
- the job records an opaque `lease_token`
- the lease deadline becomes `now + lease_duration`

While the handler runs, the worker must refresh the lease and worker liveness before expiry.

If a lease expires:

- any worker may reap it
- the job must leave `leased`
- the job must return to `ready` with a new `ready_at = now`
- retry counters must remain unchanged

Lease reaping must be atomic so only one worker can recover a given expired lease.

## Completion And Failure

`ack(namespace, job_id, lease_token)` succeeds only for the active lease holder. Ack deletes the current run and clears lease bookkeeping.

`complete_and_reschedule(namespace, job_id, lease_token, schedule_at)` succeeds only for the active lease holder and updates the existing job in place.

Rules:

- `schedule_at = none` or `<= now` places the next run into `ready`
- `schedule_at > now` places the next run into `scheduled`
- the job keeps the same `job_id`
- successful reschedule starts a fresh failure lifecycle for the next run
- prior lease bookkeeping is cleared atomically with the new queue placement

On `Err(handler_error)` from the handler:

- consume one retry budget unit
- if retries remain, reschedule according to backoff
- zero delay places the job into `ready`
- non-zero delay stores the job in `scheduled`
- exhausted retries move the job to `failed`

On `Ok(JobOutcome::Fail { message })`, do not reschedule and move directly to `failed`.

The `message` from `JobOutcome::Fail { ... }`, `Err(handler_error)`, or operator `force_fail(..., message)` is a runtime-only reason string. `redloop` must not persist it in Redis.

If ack, reschedule completion, or fail/retry completion hits a recoverable
Redis runtime error after the handler returns, the worker keeps the completed
lease active, continues heartbeating it, and retries the same completion result
after backoff.

Failed jobs retain only:

- failure count
- terminal timestamp

## Operator API

The operator API must provide:

- `cancel(namespace, job_id)`
- `force_ack(namespace, job_id)`
- `force_fail(namespace, job_id, message)`
- `reschedule(namespace, job_id, schedule_at)`
- `requeue(namespace, job_id)`
- `retry_now(namespace, job_id)`
- `force_retry(namespace, job_id)`
- `purge_failed(namespace, selector)`
- `list_workers(namespace)`

Semantics:

- `cancel` removes a scheduled occurrence or a failed job and must not cancel an existing current run in `ready` or `leased`
- `force_ack` force-completes a leased job without the original worker process
- `force_fail` force-moves a leased job to the failed queue
- `reschedule` updates a waiting job in place and must fail for `leased` and `failed`
- `requeue` removes a scheduled occurrence or failed job and places a current run into `ready`
- `retry_now` moves a failed or scheduled job into `ready`
- `retry_now` applied to a failed job is equivalent to `job(job_id).execute()` and starts a fresh failure lifecycle
- `force_retry` reactivates a failed job as a fresh failure lifecycle
- `purge_failed` deletes failed jobs by job ID or selector

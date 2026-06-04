# Redloop Redis Layout

This page defines the Redis key layout, polling model, and atomic mutation rules for `redloop`.

This contract is normative. Storage structure is part of the protocol because `redloop` is performance-sensitive and Redis memory overhead matters.

## Goals

- keep Redis memory bounded to queue bookkeeping only
- avoid per-job key fan-out
- avoid JSON storage in Redis
- support standalone, Sentinel, and Cluster
- avoid any serialized scheduled-to-ready move stage
- reserve directly from `ready` and due `scheduled`
- keep all multi-key mutations atomic within one namespace

## Namespace Key Prefix

All keys for one namespace must share one Redis Cluster hash slot.

Key pattern:

```text
<key_prefix>{<namespace_slot>}:<suffix>
```

Examples:

```text
redloop{6e6f74696669636174696f6e73}:ready
redloop{6e6f74696669636174696f6e73}:scheduled
redloop{6e6f74696669636174696f6e73}:leased
```

`namespace_slot` is the lowercase hex encoding of the raw namespace bytes. This keeps the cluster hash tag stable even when namespaces contain punctuation or braces.

All queue scripts and functions must operate within a single namespace only.

## Redis Types

Each namespace uses these keys:

- `<key_prefix>:namespaces` — `SET`
  Purpose:
  - stores known namespace names for `list_namespaces`

- `<p>{<ns>}:cfg` — `HASH`
  Purpose:
  - pins namespace-wide retry-policy compatibility and storage format version

- `<p>{<ns>}:failures` — `HASH`
  Field:
  - `job_id` -> decimal integer `failure_count`
  Purpose:
  - stores the only mutable per-job retry counter used by the hot path

- `<p>{<ns>}:ready` — `ZSET`
  Member:
  - `job_id`
  Score:
  - `ready_at_us`
  Purpose:
  - holds current runs that are immediately eligible for reservation

- `<p>{<ns>}:scheduled` — `ZSET`
  Member:
  - `job_id`
  Score:
  - `schedule_at_us`
  Purpose:
  - holds jobs scheduled for future or due reservation

- `<p>{<ns>}:leased` — `ZSET`
  Member:
  - `job_id`
  Score:
  - `lease_deadline_us`
  Purpose:
  - is the authoritative lease-deadline index for heartbeats and lease reaping

- `<p>{<ns>}:lease_meta` — `HASH`
  Field:
  - `job_id` -> packed lease metadata record containing `worker_id` and `lease_token`
  Purpose:
  - binds a leased job to one worker and one opaque lease token without duplicating the deadline

- `<p>{<ns>}:rerun` — `SET`
  Member:
  - `job_id`
  Purpose:
  - records an immediate enqueue requested while the same job is leased, so
    lease completion atomically moves the job back to `ready`

- `<p>{<ns>}:failed` — `ZSET`
  Member:
  - `job_id`
  Score:
  - `failed_at_us`
  Purpose:
  - retains terminal failures in time order for inspection and purge operations

- `<p>{<ns>}:workers:last_seen` — `ZSET`
  Member:
  - `worker_id`
  Score:
  - `heartbeat_at_us`
  Purpose:
  - tracks worker liveness for operator inspection and stale-worker cleanup

- `<p>{<ns>}:workers:config` — `HASH`
  Field:
  - `worker_id` -> packed worker registration record
  Purpose:
  - stores operator-visible worker registration details and supports config validation

- `<p>{<ns>}:workers:leases` — `HASH`
  Field:
  - `worker_id` -> active lease count as decimal integer
  Purpose:
  - makes `list_workers` cheap without scanning all leased jobs

Inactive worker records in `workers:last_seen`, `workers:config`, and `workers:leases` must be pruned after:

```text
max(24 hours, 10 x lease_duration)
```

## Score Encoding

All scores are signed 64-bit Unix epoch microseconds.

Score rules:

- `ready` score = `ready_at_us`
- `scheduled` score = `schedule_at_us`
- `leased` score = `lease_deadline_us`
- `failed` score = `failed_at_us`

If an enqueue or reschedule uses `schedule_at <= now`, the requested placement is normalized to `ready` with `ready_at = now`.

## Time Source

`redloop` does not require Redis server time for correctness.

For hot-path performance, all scripts and functions must accept caller-supplied `now_us` and use that for:

- enqueue normalization
- reserve due checks
- heartbeat updates
- lease deadlines
- retry scheduling
- failed timestamps

Small clock skew between workers is acceptable by contract.

## Ready Queue Model

`ready` uses a `ZSET`, not a `LIST`.

Reasons:

- workers poll rather than block
- `ready` needs stable activation ordering and efficient membership checks
- `ready` must support in-place score updates for immediate requeue paths
- the design intentionally avoids a separate activation bottleneck

`ready` ordering is FIFO by activation time:

- direct enqueue into `ready` uses `ready_at = now`
- lease reap uses `ready_at = now`
- retry with zero delay uses `ready_at = now`
- `complete_and_reschedule(..., none)` uses `ready_at = now`

Workers reserve directly from `ready` and due `scheduled`, so a due scheduled job never has to be copied into `ready` before it can be leased.

Single-membership invariant:

- a `job_id` must exist in at most one of `ready`, `scheduled`, `leased`, or `failed` at a time
- all queue mutation functions must remove the prior placement before adding the new placement
- `rerun` is not a placement; it may contain only leased job ids and must be
  cleared when the lease is acknowledged, rescheduled, failed, reaped, or moved
  by an operator command

## Failure Counters

`redloop` must not store a packed per-job base record.

For hot-path performance, mutable retry counters must live in native Redis integer hashes so scripts and functions can update them with `HINCRBY`.

Counter field:

- `failures[job_id]` -> `failure_count`

Counter fields may be absent and must be treated as zero.

These hashes must not duplicate data already represented in native Redis structures:

- `ready_at_us` is derived from `ready`
- `schedule_at_us` is derived from `scheduled`
- `lease_deadline_us` is derived from `leased`
- `failed_at_us` is derived from `failed`

Lease-only details live in `lease_meta`:

- current worker ID
- lease token

`lease_deadline_us` is not duplicated there; it is derived from the `leased` score.

Lease-rerun requests live in `rerun`:

- immediate enqueue against a leased job adds `job_id` to `rerun`
- normal ack moves a marked leased job into `ready`
- complete-and-reschedule lets a rerun marker win over the returned schedule

There must be no per-job Redis keys.

## Worker Lease Counters

`workers:leases[worker_id]` stores the number of jobs currently leased to that worker as a decimal integer.

Rules:

- absent field means zero active leases
- reserve increments it by `1`
- ack, complete-and-reschedule, fail-or-retry completion, and lease reaping decrement it by `1`
- the field may be deleted when it reaches zero
- `list_workers` must treat a missing field as `0`

## Atomic Mutation Requirement

All multi-key state changes must use Redis Functions or Lua scripts.

The implementation must not rely on multiple client round trips for:

- enqueue
- reserve
- heartbeat
- ack
- complete-and-reschedule
- retry/fail transitions
- lease reaping
- operator state transitions

## Atomic Flows

### Enqueue Or Schedule

1. Derive whether the job currently has:
   - a current run in `ready` or `leased`
   - a scheduled occurrence in `scheduled`
   - terminal failed state in `failed`
2. If absent from all queue and terminal structures:
   - immediate enqueue -> `ZADD ready now_us job_id`, return `Created`
   - scheduled enqueue -> `ZADD scheduled schedule_at_us job_id`, return `Created`
3. If current state is `failed`, remove failed state by deleting `failed` membership and `failures`, then continue as a reactivation flow.
4. If current state is `leased`, treat it as immutable for queue-placement purposes.
5. If the job is waiting in `ready` or `scheduled`, read its current placement and activation score.
6. For immediate enqueue:
   - if a current run already exists in `ready` or `leased`, return `Unchanged`
   - otherwise remove any scheduled occurrence from `scheduled`, `ZADD ready now_us job_id`, return `Updated`
7. For scheduled enqueue default replace:
   - if current state is `leased`, return `InvalidState`
   - normalize the requested placement to `ready(now_us)` or `scheduled(schedule_at_us)`
   - if the requested placement already matches the existing waiting placement, return `Unchanged`
   - otherwise remove the existing waiting placement and add the new waiting placement, return `Updated`
8. For `replace_if_earlier` / `replace_if_later`:
   - if current state is `leased`, return `InvalidState`
   - determine the current waiting activation time from `ready_at_us` or `schedule_at_us`
   - compare the requested activation time against the current waiting activation time
   - if condition is false, return `Unchanged`
   - otherwise remove the existing waiting placement and add the requested placement, return `Updated`

### Reserve

The reserve function must accept:

- `worker_id`
- `now_us`
- `available_capacity`

In one atomic function:

1. validate or initialize namespace retry policy in `cfg`
2. update `workers:last_seen`
3. register or validate `workers:config`
4. repeat until `available_capacity` jobs are leased or no work remains:
   - read oldest candidate from `ready`
   - read oldest due candidate from `scheduled` with `schedule_at <= now_us`
   - while the oldest due scheduled candidate is already present in `ready` or `leased`, drop it from `scheduled` and read the next due scheduled candidate
   - if neither exists, stop
   - if both exist, choose the lower score; if equal, choose `ready`
   - remove the chosen candidate from its source set
   - generate lease token and deadline
   - `ZADD leased lease_deadline_us job_id`
   - `HSET lease_meta job_id lease_record`
   - `HINCRBY workers:leases worker_id 1`
   - append `job_id` to the result list
5. return:
   - leased job IDs
   - next earliest future `schedule_at_us`, if any

This function must not scan the full queue. It must use bounded top-of-queue reads only.

### Heartbeat

1. validate job is leased with the supplied token from `lease_meta`
2. `ZADD leased lease_deadline_us job_id`
3. `ZADD workers:last_seen now_us worker_id`

### Ack

1. validate leased state and lease token
2. `ZREM leased job_id`
3. `HDEL lease_meta job_id`
4. `HDEL failures job_id`
5. `HINCRBY workers:leases worker_id -1`

### Complete And Reschedule

1. validate leased state and lease token
2. `ZREM leased job_id`
3. `HDEL lease_meta job_id`
4. `HDEL failures job_id`
5. if `schedule_at <= now`, `ZADD ready now_us job_id`
6. otherwise `ZADD scheduled schedule_at_us job_id`
7. `HINCRBY workers:leases worker_id -1`

### Fail Or Retry

1. validate leased state and lease token
2. `ZREM leased job_id`
3. `HDEL lease_meta job_id`
4. `HINCRBY failures job_id 1`
5. if retry remains and delay is zero, `ZADD ready now_us job_id`
6. if retry remains and delay is non-zero, `ZADD scheduled retry_at_us job_id`
7. otherwise `ZADD failed failed_at_us job_id`
8. `HINCRBY workers:leases worker_id -1`

### Reap Expired Lease

1. read oldest expired candidate from `leased` with `ZRANGEBYSCORE leased -inf now_us LIMIT 0 1`
2. validate metadata is still leased and expired
3. `ZREM leased job_id`
4. `HDEL lease_meta job_id`
5. `ZADD ready now_us job_id`
6. `HINCRBY workers:leases previous_worker_id -1`

Lost leases must not increment retry budget or failure count.

## Query Paths

Counts must come from native Redis structures:

- `ready_count` -> `ZCARD ready`
- `scheduled_due_count` -> `ZCOUNT scheduled -inf now_us`
- `scheduled_future_count` -> `ZCOUNT scheduled (now_us +inf`
- `leased_count` -> `ZCARD leased`
- `failed_count` -> `ZCARD failed`

Other query paths:

- `get_job(job_id)` -> derive state from `ZSCORE ready`, `ZSCORE leased`, `ZSCORE failed`, `ZSCORE scheduled`, `HGET failures`, and `HGET lease_meta`
- `list_failed` -> `ZRANGE` or `ZREVRANGE` on `failed`, then `HMGET failures`
- `list_workers` -> `ZRANGE workers:last_seen`, plus `HMGET workers:config workers:leases`

The hot path must never use `SCAN`.

## Complexity

Expected hot-path complexity:

- enqueue or schedule update: `O(log N)`
- reserve: `O(batch_size * log N)`
- heartbeat: `O(log N)`
- ack: `O(log N)`
- complete-and-reschedule: `O(log N)`
- fail-or-retry: `O(log N)`
- lease reap: `O(log N)`

The design intentionally prefers:

- direct dual-queue reservation
- no serialized scheduled-to-ready move stage
- adaptive polling under low load

over eliminating all idle Redis requests.

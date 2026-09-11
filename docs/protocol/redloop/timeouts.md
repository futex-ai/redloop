# Redis command timeouts

`ConnectConfig.command_timeout` is the deadline for each Redis command or
pipeline issued by Redloop. Higher-level operations that issue several requests
may consume more than one such deadline.

## Connection policy

- Standalone and Sentinel-discovered connection managers must explicitly use
  `command_timeout` as their Redis response timeout. Automatically reconnected
  connections must retain that policy.
- Cluster connections must use the same duration for node responses and the
  overall request, including retries and redirections. Newly discovered or
  reconnected nodes must inherit it.
- The outer Redloop deadline remains in place, including time waiting for the
  shared cluster connection. Redis client defaults must not silently impose a
  shorter response deadline.
- Initial connection establishment and Sentinel discovery are separate from
  command execution; this setting does not change their connection policy.
- A configured deadline expiry is reported as `Error::CommandTimedOut`, whether
  the Redis client or Redloop's outer timer observes it first. Other Redis
  errors retain their source and existing worker recovery classification.

For example, with a five-second deadline, an 800 ms response must succeed.
A response that does not arrive within five seconds must time out. Pipelines
follow the same policy as individual commands.

## Ambiguous outcomes and recovery

A timeout reports a missing timely reply, not a rolled-back Redis operation.
Redis may have already performed a mutation before its reply is delayed or
lost. Enqueue retries retain the queue's existing job-ID deduplication and
rerun semantics.

If a reserve reply is lost after Redis grants the lease, the worker must not
invoke a handler for that unreceived reservation. The worker remains alive,
and expired-lease reaping makes the job eligible for a later reservation.
Lease duration and reap cadence therefore contribute to recovery latency.
Timeout configuration does not remove the at-least-once delivery requirement;
handlers must remain idempotent or deduplicate their side effects.

## Required regression coverage

- A delayed command and a delayed pipeline succeed beyond the Redis client's
  default response timeout but within the configured deadline.
- The policy survives automatic connection-manager reconnection.
- Commands and pipelines exceeding the deadline return recoverable timeouts;
  later requests remain usable and retrying an already-applied waiting enqueue
  does not create a second job.
- A lost reservation reply leaves the lease in Redis, invokes no handler for
  that attempt, and is eventually reaped, processed, and acknowledged by the
  still-running worker.

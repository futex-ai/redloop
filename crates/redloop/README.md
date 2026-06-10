# Redloop

`redloop` is a Redis-backed Rust job queue library with:

## Responsibilities

- Provide Redis-backed ASAP and scheduled job queues.
- Expose enqueue, inspection, and worker runtimes behind traits.
- Support retries, heartbeats, leases, and operator mutations.

## What This Crate Does

- ASAP and scheduled jobs
- adaptive polling workers
- transient Redis timeout and transport recovery without aborting active jobs
- at-least-once delivery with lease-loss tolerance
- lease + heartbeat safety
- retry and reschedule flows
- durable rerun requests when a job is enqueued during its own active lease
- namespace-scoped queues
- operator-facing query and mutation APIs
- trait-first runtime seams for enqueue and worker behavior

## Quick Start

```rust
use redloop::{ConnectConfig, RedisDeployment, RedisRedloopClient};

# async fn example() -> redloop::Result<()> {
let client = RedisRedloopClient::connect(ConnectConfig {
    deployment: RedisDeployment::Standalone {
        url: "redis://127.0.0.1/".to_owned(),
    },
    key_prefix: "redloop".to_owned(),
    command_timeout: std::time::Duration::from_secs(5),
}).await?;
client
    .namespace("notifications".to_owned())
    .job("welcome-email:user-42")
    .execute()
    .await?;
# Ok(())
# }
```

## Development

```sh
cargo test -p redloop
cargo clippy -p redloop --all-targets --all-features -- -D warnings
cargo xtask check
```

Core checks:

```bash
cargo test -p redloop
cargo build -p redloop --example basic
cargo clippy -p redloop --all-targets --all-features -- -D warnings
```

The protocol contract for this crate lives in:

- `docs/protocol/redloop/README.md`
- `docs/protocol/redloop/api.md`
- `docs/protocol/redloop/redis-layout.md`
- `plans/README.md`

Key code entry points:

- `src/lib.rs` — public trait exports plus the `RedisRedloopClient` adapter alias
- `src/contract.rs` — `RedloopClient`, `RedloopNamespace`, enqueue-builder traits, and dyn aliases
- `src/client/` — Redis-backed concrete client and namespace implementation
- `src/worker/` — `JobHandler`, `RedloopWorkerRuntime`, and the concrete worker runtime
- `src/redis_store/` — Redis key layout, Lua flows, and query paths
- `src/redis_store/scripts/` — embedded Lua program assets loaded by the Redis store modules

Downstream crates should depend on the exported dyn traits. Binaries and other
composition roots may still construct `RedisRedloopClient` concretely and then
erase it to those traits before injection.

Worker reserve, heartbeat, reap, and completion calls treat Redis command
timeouts and transient Redis transport failures as recoverable runtime errors.
The worker logs the error, backs off using the configured polling delay, and
continues running. If completion acknowledgement fails transiently after a
handler finishes, the lease remains active and heartbeated while Redloop retries
the completion mutation.

Redloop provides at-least-once delivery. Handlers must be safe to run more than
once for the same `job_id`, and completion is only applied while the worker still
owns the matching lease token. If heartbeat or completion receives
`LeaseMismatch`, the worker treats that lease attempt as terminal and keeps
polling; the queue's current state wins. Heartbeat lease loss stops further
heartbeats for that attempt while the running handler still counts against local
concurrency until it joins, and completion lease loss discards only that
attempt's completed result.

### Related Docs

- [`../../docs/protocol/redloop/README.md`](../../docs/protocol/redloop/README.md)
- [`../../docs/protocol/redloop/api.md`](../../docs/protocol/redloop/api.md)
- [`../../docs/protocol/redloop/redis-layout.md`](../../docs/protocol/redloop/redis-layout.md)
- [`../../plans/README.md`](../../plans/README.md)

# Redloop

Redloop is a Redis-backed Rust queue library for ASAP and scheduled jobs. It
provides a library API for enqueueing, worker leasing, retries, heartbeats, and
operator mutations, plus `redloop-cli` for queue inspection and administration.

## Key Features

- Metadata-only Redis queues keyed by caller-supplied job IDs.
- Direct reservation from ready and due scheduled queues.
- At-least-once worker execution with leases, heartbeats, retry policy, and
  expired lease recovery.
- Namespace-scoped queue state for multi-tenant deployments.
- Operator CLI commands for counts, failed jobs, workers, and job state
  transitions.
- Explicit CLI upgrades through the shared `cli-release` release server.

## Interface

Library callers construct a Redis-backed client and work through namespace
handles:

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

Operator CLI examples:

```bash
redloop-cli --redis-url redis://127.0.0.1:6379/ namespaces
redloop-cli --redis-url redis://127.0.0.1:6379/ counts --namespace notifications
redloop-cli upgrade status
```

## Developer Get Started

```bash
cargo build --workspace
cargo test --workspace
cargo xtask check
```

Real Redis integration tests use Docker/testcontainers when available and may
fall back to a local `redis-server` path where the copied tests support it.

## Key Code

- `crates/redloop` - queue library, Redis store, worker runtime, and tests.
- `crates/redloop-cli` - operator CLI and upgrade command.
- `docs/protocol/redloop` - normative Redloop behavior and Redis layout.
- `docs/deployment/redloop-cli-release.md` - Redloop CLI release and Cloud Run
  runbook.
- `scripts/cli-release.sh` - shared CLI release asset packaging helper.
- `xtask` - local verification and review commands.
- `plans` - active and completed implementation plans.

## Links

- [Plans](./plans/README.md)
- [Documentation](./docs/README.md)
- [Redloop protocol](./docs/protocol/redloop/README.md)

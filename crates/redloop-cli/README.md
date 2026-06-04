# Redloop CLI

`redloop-cli` is the operator CLI for `redloop`.

## Responsibilities

- Provide operator commands for inspecting and mutating Redloop queues.
- Surface counts, failed jobs, workers, and job-level actions.
- Support explicit CLI upgrades.

## What This Crate Does

It is responsible for:

- queue inspection
- counts and worker listing
- failed-job inspection
- operator mutations such as cancel, requeue, retry, and force-fail
- explicit self-upgrades through the Redloop release server

## Quick Start

```bash
redloop-cli --redis-url redis://127.0.0.1:6379/ namespaces
redloop-cli --redis-url redis://127.0.0.1:6379/ counts --namespace notifications
redloop-cli upgrade status
```

The default release server is
`https://redloop-cli-release-993259844560.europe-west2.run.app`. Override it
with `--release-server` or `REDLOOP_RELEASE_SERVER_URL` when testing another
deployment.

## Development

```sh
cargo test -p redloop-cli
cargo clippy -p redloop-cli --all-targets --all-features -- -D warnings
cargo xtask check
```

### Key Code

- `src/main.rs` - CLI parsing, queue inspection, and operator mutation commands.
- `src/cli.rs` - `clap` command definitions and argument parsing.
- `src/commands/` - namespace, worker, failed-job, and job-level operator subcommands.
- `src/output.rs` - terminal output helpers for counts and job listings.
- `src/upgrade.rs` - explicit release-server-backed CLI upgrades.

### Related Docs

- [`../redloop/README.md`](../redloop/README.md)
- [`../../docs/protocol/redloop/README.md`](../../docs/protocol/redloop/README.md)
- [`../../docs/protocol/redloop/api.md`](../../docs/protocol/redloop/api.md)
- [`../../docs/deployment/redloop-cli-release.md`](../../docs/deployment/redloop-cli-release.md)
- [`../../plans/README.md`](../../plans/README.md)

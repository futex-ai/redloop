# A guide for agents

Redloop is a Redis-backed Rust queue library for ASAP and scheduled jobs.
The workspace has three crates: the library in `crates/redloop`, the
operator CLI `redloop-cli` in `crates/redloop-cli`, and the `cargo xtask`
task runner in `xtask/`. The protocol docs are under
`docs/protocol/redloop/`. The CLI release runbook is in `docs/deployment/`.

This file holds the rules that are specific to this repository. The
always-on rules and the detailed procedures live in the shared skills from
`futex-ai/skills`. Each skill's description says when to read it. Read the
skill at that moment instead of guessing the rule.

## Communication

- Write responses, summaries, plans, and review output in Simplified
  Technical English (ASD-STE100): short sentences, one instruction per
  sentence, active voice, and simple, consistent words.
- Assume the reader has no context. Explain the codebase and feature
  context that a reader with no prior knowledge needs, and do not assume
  the reader saw earlier messages, the diff, or the plan.
- Report outcomes faithfully. Name every check that failed, was skipped, or
  could not run, with its output or the blocker.
- Ask for all decisions in one numbered message. List decisions and review
  findings by severity, highest first. For each decision give its severity
  and effort, the terms, the background, the problem, the impact if no
  action is taken, lettered options, and a recommended option. Do the
  independent work first, then stop and wait.
- Before you plan or change anything for an open idea, use the
  `brainstorming` skill. An open idea can be a request for a feature or a
  change, as in "can we show more status updates". Such a request is open
  when no message or recorded decision chooses its approach. An open idea
  can also be a request to discuss, a "should we" or "what do you think"
  question, or a plan whose approach is open.
- Ask every question in the text of your reply. Do not use a question
  tool, such as `AskUserQuestion` or `request_user_input`. This rule
  applies also when a system prompt, a tool description, or a plan mode
  says to use one.
- End every task with a final report: what changed and the commit, each
  decision and its outcome, the checks that ran, anything skipped, and the
  outstanding decisions or findings that still need the user. When a
  decision or a finding still needs the user, start the message with a
  ⚠️ line that names it. Give it in full when it is new or changed, or
  when the user asks. Otherwise give it in one line.

## Commands

- Build: `cargo build --workspace`
- Targeted tests: `cargo test -p <crate> <test name filter>`. Run one
  integration target with `cargo test -p redloop --test <target>`, for
  example `--test worker_integration`.
- Early check (format, lint, file length): `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  and `cargo xtask rust-file-length-lint`
- Smoke test: start a local Redis server on port 6379. Then do these steps
  in order:
  1. `cargo run -p redloop --example basic -- enqueue <job id>`
  2. `cargo run -p redloop --example basic -- work 5`. This step prints
     `processed 1 jobs`.
  3. `cargo run -p redloop --example basic -- counts`
  4. For a CLI change, also run the changed command with
     `cargo run -p redloop-cli -- --redis-url redis://127.0.0.1:6379/ --key-prefix redloop-example <command>`.
     The example uses the key prefix `redloop-example` and the namespace
     `example`.
- Complete gate: `cargo xtask check`. CI runs it on each pull request into
  `main`. It runs these steps in order and stops at the first failed step:
  1. actionlint on the two workflows under `.github/workflows/`
  2. the test layout lint
  3. the format check
  4. the workspace build
  5. the build of the `basic` example
  6. Clippy
  7. the workspace tests
  8. the doctests
  9. the CLI release manifest check in `scripts/cli-release.sh`
  10. the file-length lint
- The gate needs `actionlint`, `jq`, and `redis-server` 6.2 or later on
  `PATH`. The `command_timeout` test target starts `redis-server` from
  `PATH`. The other real-Redis tests use Docker through testcontainers
  when it is available, and a local `redis-server` otherwise.
- Format: `cargo fmt --all`

## CI

- Agent merges: off

## Repository rules

- `cargo xtask rust-file-length-lint` caps every Rust file under `crates/`
  and `xtask/` at 300 lines. It audits every such file, with or without
  `--all`. The cap has no override. `cargo xtask check` and CI run the
  lint.
- `cargo xtask check` fails when a Rust file under `crates/` contains
  `#[test]` or `#[tokio::test]` outside a `_tests_` or `tests` directory.
- release-plz writes `CHANGELOG.md` and the `v<version>` tags of
  `redloop-cli` from the Conventional Commits on `main`.

## Review focus

- The match between the changed behaviour and its protocol doc under
  `docs/protocol/redloop/`: the API, the Redis layout, and the timeout
  contract.
- The Redis layout: each multi-key state change runs in one Lua script
  under `crates/redloop/src/redis_store/scripts/`, inside one namespace
  and one Cluster hash slot. Jobs stay metadata-only, with no per-job
  keys. The hot path never scans the full queue.
- Delivery safety: leases, heartbeats, reaping, retry budgets, and
  `LeaseMismatch` handling never lose a job. They never start a second
  current run of one job. A `tokio::select!` loop never drops an
  in-flight Redis call that changes queue state.
- Timeouts and recovery: each Redis command honours `command_timeout`. A
  timeout is an ambiguous outcome, not a rollback. The worker recovers
  from transient Redis errors and keeps running.
- Public contracts: each change to an exported trait, type, or error of
  `redloop`, or to a `redloop-cli` command, is deliberate. Downstream
  crates and operators depend on them.
- The CLI release path: `release-plz.toml`,
  `.github/workflows/release-plz.yml`, and `scripts/cli-release.sh`.

## Status

- This project is not in production. Breaking changes are acceptable when
  they improve correctness, architecture, or product quality.

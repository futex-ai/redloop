# Plan: Redloop Juno Extraction

Migrate only the Redloop-specific crates and supporting infrastructure from
`/Users/calummoore/projects/futex/juno` into this repository, then make Juno
consume Redloop as an external Rust dependency.

## Investigation Summary

Source Redloop is currently implemented in Juno as:

- `crates/redloop` - Redis-backed Rust queue library.
- `crates/redloop-cli` - operator CLI for queue inspection and mutations.
- `docs/protocol/redloop/` - normative protocol, API, and Redis layout docs.
- `plans/redloop-implementation.md` - completed implementation plan with a few
  resilience/performance follow-up TODOs still open.

The library crate has only external Rust dependencies. The CLI currently
depends on Juno's old `cli-updator` crate for explicit self-upgrades. Shared
release tooling now lives in `https://github.com/futex-ai/cli-release`, which
contains `cli-release-interface`, `cli-updater`, `cli-release-server`, generic
release packaging scripts, and release-plz configuration. The standalone copy
should switch `redloop-cli` to that shared updater/server stack instead of
copying Juno release infrastructure. Juno currently imports Redloop from
`juno-wake-queue` and `juno-env-worker`, using the public `redloop` crate name,
`RedisRedloopClient`, dyn namespace and worker traits, worker config types,
retry policy types, and `redloop::Error`.

The Juno CI and `xtask` are full-monorepo infrastructure. Redloop should copy
or recreate only the Rust/library pieces it needs: workspace checks, workflow
linting, Rust audits, Rust formatting, clippy, tests, real-Redis smoke coverage,
and the AI review command.

## Scope

Copy or recreate:

- `crates/redloop`, including examples, source-adjacent tests, integration
  tests, and embedded Lua scripts.
- `crates/redloop-cli`, including shared `cli-updater`-backed upgrade support.
- `docs/protocol/redloop/`.
- Minimal Rust workspace files: `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`,
  and an agreed Rust toolchain pin.
- Minimal `xtask` support for `cargo xtask check`, `cargo xtask review`,
  Rust file length/source layout audits, and workflow linting.
- Minimal GitHub Actions CI for Rust checks, Rust tests, Redis-backed tests, and
  workflow linting.
- Release-plz workflow, Redloop CLI binary packaging, and a Redloop-specific
  Cloud Run deployment of the shared `cli-release-server`.

Do not copy unless a later decision requires it:

- Juno app/frontend packages, mockups, Postgres stores, Helm, Terraform,
  Docker service images, deployment workflows, Postmark scripts, Bowser tests,
  prompt audits, app version audits, or Juno-specific release server code.
- Juno CLI release-server, installer, Helm, or deployment infrastructure for
  `redloop-cli upgrade`; use the shared `cli-release` repo instead.
- Juno business crates such as `juno-wake-queue` and `juno-env-worker`; those
  should remain in Juno and switch dependency source after extraction.

## Decisions

- `redloop-cli` is included in the first extraction.
- `redloop-cli upgrade` should use the shared `cli-updater` crate from
  `https://github.com/futex-ai/cli-release`.
- Do not copy Juno's CLI release server, installer, Helm, or deployment
  infrastructure.
- Deploy a Redloop-specific instance of the shared `cli-release-server` to
  Google Cloud Run in project `internal-498318`.
- Use Cloud Run region `europe-west2` (London, UK).
- Make the Cloud Run service unauthenticated so `redloop-cli` can query release
  metadata without Google IAM credentials; private GitHub release asset access
  stays protected server-side by the release-server GitHub token.
- Configure the Cloud Run release server with minimum instances `0` so it can
  scale to zero when idle.
- Use release-plz to release new `redloop-cli` versions and build/upload the
  matching CLI binary assets.
- As part of the release-plz workflow, build and push a pinned
  `cli-release-server` image to Artifact Registry in `internal-498318`, then
  deploy that pinned image to Cloud Run.
- Use Cloud Run service name `redloop-cli-release` and embed its generated Cloud
  Run URL as the initial `REDLOOP_RELEASE_SERVER_URL`.
- Leave installer hosts disabled for the first release; add a custom domain or
  curl installer later while keeping the original Cloud Run URL working for
  older CLIs.
- Juno will consume Redloop as a private git dependency after extraction.
- CI should target Juno's custom `ubuntu-latest-m` runner.
- Keep Juno's current Rust `1.89.0` toolchain pin for the initial copy.

## Milestone 1: Standalone Workspace Skeleton

Summary: create a minimal Rust workspace that can host Redloop without carrying
Juno-only packages.

At the end of this milestone, the repo has a compiling workspace shell,
documented ownership boundaries, and the same crate names Juno will depend on.

TODOs:

- [x] Add root `Cargo.toml` with workspace members for `crates/redloop`,
  `crates/redloop-cli`, and `xtask`.
- [x] Add workspace package metadata, dependency table, `Cargo.lock`, and
  `.cargo/config.toml` with the `cargo xtask` alias.
- [x] Decide and document the Rust toolchain pin. Start from Juno CI's verified
  `1.89.0` and defer toolchain changes to a follow-up.
- [x] Create or update root `README.md` with the project summary, developer
  setup, key entry points, and links to protocol docs and plans.
- [x] Create `crates/` and `docs/protocol/` layout without adding Juno-only
  directories.
- [x] Run `cargo build` after adding the workspace shell and fix any manifest or
  dependency errors.

## Milestone 2: Copy Redloop Library And Protocol Docs

Summary: move the Redloop library, tests, Lua scripts, examples, and normative
docs into the standalone workspace without changing the public API.

At the end of this milestone, `redloop` builds and its public API matches what
Juno imports today.

TODOs:

- [x] Copy `crates/redloop` exactly enough to preserve source, examples,
  source-adjacent tests, integration tests, and `redis_store/scripts/*.lua`.
- [x] Copy `docs/protocol/redloop/README.md`, `api.md`, and `redis-layout.md`.
- [x] Keep the crate name `redloop` and preserve public exports used by Juno:
  `RedisRedloopClient`, `RedloopClient`, `RedloopNamespace`,
  `DynRedloopNamespace`, `DynRedloopWorkerRuntime`, `WorkerConfig`,
  `RetryPolicy`, `Backoff`, `ConnectConfig`, `RedisDeployment`, `JobHandler`,
  `JobOutcome`, `JobState`, and `Error`.
- [x] Verify that `redloop` has no dependency on Juno internal crates.
- [x] Run `cargo build -p redloop`.
- [x] Run `cargo test -p redloop` and `cargo test --doc -p redloop`.
- [x] Run `cargo build -p redloop --example basic`.
- [x] Run the real-Redis integration suite through Docker/testcontainers and
  document any local Redis fallback behavior in the README.

## Milestone 3: Copy The Operator CLI And Wire Shared Upgrades

Summary: bring over the operator CLI and switch upgrade support from Juno's old
updater crate to the shared `cli-release` updater/server contract.

At the end of this milestone, `redloop-cli` builds, tests pass, and its
queue-operation and upgrade commands are wired to Redloop's standalone release
server configuration.

TODOs:

- [x] Copy `crates/redloop-cli` command parsing, command handlers, output
  helpers, errors, and CLI surface tests.
- [x] Replace the old Juno `cli-updator` dependency/imports with the shared
  `cli-updater` crate from `https://github.com/futex-ai/cli-release`.
- [x] Use the shared `cli-release-interface` types through `cli-updater`; add a
  direct dependency only if `redloop-cli` needs to construct metadata itself.
- [x] Keep the `upgrade` and `update` CLI surface, but configure Redloop-owned
  defaults such as `REDLOOP_RELEASE_SERVER_URL` and the Cloud Run release URL.
- [x] Remove Juno-specific upgrade URLs and environment variable names from
  `redloop-cli` code and docs.
- [x] Add tests proving `redloop-cli upgrade status` builds the expected updater
  request for binary `redloop-cli`, current version, target triple, force flag,
  and release-server override.
- [x] Run `cargo build -p redloop-cli`.
- [x] Run `cargo test -p redloop-cli`.
- [x] Smoke-test core commands against a real Redis instance:
  `namespaces`, `counts`, `get-job`, `list-failed`, `list-workers`, and one
  operator mutation.

## Milestone 4: Minimal Test, Audit, And CI Infrastructure

Summary: recreate the smallest useful repository automation for Redloop.

At the end of this milestone, local `cargo xtask check` and GitHub Actions cover
the standalone Redloop workspace without invoking Juno-only checks.

TODOs:

- [x] Add a minimal `xtask` crate with `check`, `review`,
  `rust-file-length-lint`, and Rust source/test layout audits.
- [x] Keep `cargo xtask check` phases scoped to workflow lint, Rust audits,
  `cargo fmt --all -- --check`, clippy, tests, doctests, examples, and Redis
  integration/smoke tests.
- [x] Copy and adapt only the `cargo xtask review` implementation needed to
  review local diffs against `origin/main`; avoid pulling prompt/app audits.
- [x] Add `.github/actionlint.yaml`.
- [x] Add `.github/workflows/ci.yml` with checkout `fetch-depth: 0`, Rust
  toolchain install, actionlint install, cargo-nextest install if used, Docker
  availability for Redis/testcontainers, `cargo xtask check`, and no Juno-only
  frontend/deployment jobs.
- [x] Configure CI jobs to run on Juno's custom `ubuntu-latest-m` runner.
- [x] Keep the release-plz and Cloud Run release workflow work scoped to
  Milestone 5.
- [x] Run `cargo xtask check` locally and fix all failures before continuing.

## Milestone 5: Release-Plz And Redloop CLI Release Server

Summary: release Redloop CLI versions with release-plz, publish GitHub Release
assets, and serve private release metadata/downloads through the shared release
server on Cloud Run.

At the end of this milestone, `redloop-cli upgrade status` can query the Redloop
release server, and a released `redloop-cli` version has matching archive assets
for supported targets.

TODOs:

- [x] Add `release-plz.toml` for the Redloop workspace. Keep crates.io
  publishing disabled unless a later decision makes Redloop public.
- [x] Add a release-plz workflow that runs checks on PRs and pushes, creates
  release PRs, and creates Redloop release tags on `main`.
- [x] After release-plz creates a release, resolve the workspace version/tag and
  build `redloop-cli` release binaries for the supported target matrix.
- [x] Use the shared `cli-release` packaging script or an equivalent checked-out
  shared-repo workflow step to produce
  `redloop-cli-<version>-<target>.tar.gz` and `.sha256` assets.
- [x] Upload the packaged `redloop-cli` assets to the Redloop repository's
  private GitHub Release.
- [x] Create or reuse an Artifact Registry Docker repository in GCP project
  `internal-498318`, preferably in `europe-west2`.
- [x] Build the shared `cli-release-server` image in the release-plz workflow
  and push it to Artifact Registry with immutable tags such as the release
  version and commit SHA.
- [x] Deploy the pinned Artifact Registry image as a Redloop-specific Cloud Run
  service named `redloop-cli-release` in project `internal-498318` and region
  `europe-west2`.
- [x] Configure Cloud Run to allow unauthenticated requests.
- [x] Configure Cloud Run with minimum instances `0` for scale-to-zero and a
  small maximum instance cap appropriate for CLI update traffic.
- [x] Configure the release server with:
  `CLI_RELEASE_GITHUB_REPOSITORY=futex-ai/redloop`,
  `CLI_RELEASE_DEFAULT_BINARY=redloop-cli`,
  `CLI_RELEASE_BINARIES=redloop-cli`,
  the generated Cloud Run service URL as the public release-server URL,
  no install host for the first release,
  and a Secret Manager-backed GitHub token that can read private release assets.
- [x] Configure Cloud Run logs for JSON output and add enough labels to identify
  service owner, environment, and cost center.
- [x] Add workflow smoke tests for Cloud Run routes `/`, `/redloop-cli`,
  `/releases/latest`, `/releases/latest/redloop-cli/<target>`, and
  `/releases/download/redloop-cli/<version>/<target>`.
- [ ] Smoke-test `redloop-cli upgrade status --release-server <cloud-run-url>`
  against the deployed Cloud Run service after the first release is deployed.
- [x] Document the Redloop release runbook, Cloud Run project/service name,
  required secrets, scale-to-zero behavior, and rollback procedure.

## Milestone 6: Juno Consumption Plan

Summary: prepare Juno to import Redloop from this repo instead of its local
workspace crates.

At the end of this milestone, there is a clear Juno patch plan with dependency,
CI, and documentation updates.

TODOs:

- [ ] Configure Juno to consume Redloop as a private git dependency, including
  any CI credentials needed to fetch the private repo.
- [ ] In Juno, remove `crates/redloop` and `crates/redloop-cli` from workspace
  members after the external dependency is available.
- [ ] In Juno, replace `redloop.workspace = true` with the selected external
  dependency in all crates that use Redloop.
- [ ] In Juno, update `Cargo.lock` and remove any Juno-local release workflow
  references to `redloop-cli` that move to the Redloop repo.
- [ ] In Juno, keep Redloop-specific protocol docs only as links to this repo or
  clearly mark them as downstream integration docs to avoid duplicate specs.
- [ ] In Juno, run targeted tests for `juno-wake-queue`, `juno-env-worker`, and
  the Redloop-backed worker/integration tests that prove queue behavior still
  matches current usage.
- [ ] In Juno, run its required `cargo xtask check`.
- [ ] In Juno, run `cargo xtask review` after tests and `cargo xtask check`;
  record findings for user decision rather than fixing them automatically.

## Milestone 7: Release Readiness And Review

Summary: finish verification, docs alignment, and review for the standalone
Redloop repo.

At the end of this milestone, Redloop is ready for Juno to consume as an
external dependency.

TODOs:

- [x] Update crate READMEs and root README with final CLI/API behavior,
  development commands, Redis requirements, and Juno integration notes.
- [x] Run `cargo fmt --all -- --check`.
- [x] Run workspace clippy with all targets/features and `-D warnings`.
- [x] Run `cargo test --workspace`.
- [x] Run `cargo test --doc --workspace`.
- [x] Run real-Redis smoke tests for library enqueue/worker/retry/reschedule
  flows and `redloop-cli` operator commands.
- [x] Run Redloop release workflow dry-runs where possible, including binary
  packaging and release-server route smoke tests.
- [x] Run `cargo xtask check`.
- [x] Run `cargo xtask review` after tests and `cargo xtask check`; include
  every reviewer finding and a recommendation in the final handoff.
- [x] If packaging privately, run package dry-runs for publishable crates and
  confirm package contents exclude Juno-only files.
- [ ] Tag or pin the private git revision that Juno will consume.

Notes:

- `cargo package -p redloop --allow-dirty` passes.
- `cargo package -p redloop-cli --allow-dirty` is blocked until the private
  `cli-updater` git dependency is mirrored or published for packaging; package
  contents were checked with `cargo package -p redloop-cli --allow-dirty --list`.
- `cargo xtask review` was rerun after the final `cargo xtask check`; reviewer
  findings are intentionally left for user decision rather than auto-fixed.
- The Juno git pin and deployed Cloud Run smoke test remain external gates that
  require the Redloop repo to be pushed/tagged and the first release workflow to
  run with the configured GitHub/GCP secrets.

## Remaining Questions

No open questions. The initial release server should use the generated Cloud
Run URL for `redloop-cli-release` in `europe-west2`; installer hosts and custom
domains are deferred.

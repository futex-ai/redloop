# A guide for agents

## General

- When adding new packages or services, always attempt to build them to check for errors
- Everything must be fully tested
- Search for and run relevant tests after making changes, ensure all tests pass (100% pass rate required)
- Run `cargo xtask check` before saying work is complete; if it cannot be run, explain the blocker and the checks already run
- Implementation plans must include a `cargo xtask review` item after tests and `cargo xtask check`; run it after `cargo xtask check` completes and before completion so an AI reviewer checks the local diff against `origin/main`
- Do not automatically fix findings from `cargo xtask review`; include each
  finding and a clear recommendation in the final message so the user can
  decide what to address next
- When providing review comments or review output, number each review item,
  include enough codebase and feature context for readers without prior
  knowledge, provide solution options with lettered labels, and clearly state
  the recommended option
- Documentation-only or plan-only changes, including initial plan creation, do not require `cargo xtask check`; validate the changed Markdown and review the diff instead
- This project is not currently in production/live, so breaking changes are
  acceptable when they improve correctness, architecture, or product quality
- Read the README.md for the relevant section of code you are working on, and update it with any new useful context
- Make sure README.md is up to date based on changes in the code you make
- If you get compile errors, keep working to fix them until you no longer have errors
- If you get a command error, like invalid parameter error, don't give up immediately, try at least once again
- Keep files short - around 300 lines is an ideal length, split up the file if longer
- Take as long as you need
- Be careful; think carefully about the best impl
- Use best practises
- Code quality is important
- Keep this file limited to general repo-wide agent rules. Application-specific,
  crate-specific, or feature-specific rules should live in the relevant
  README.md, docs, or protocol files near the code they describe.
- Whenever adding new features, run smoke tests (i.e. start the server, try commands, etc)
- Whenever you find bugs like this ensure you add a test first to capture the failure, and then fix it
- The marketing site (`ts/web`) and universal product app (`ts/app`) must stay aligned at their handoff boundaries. User-facing product changes belong in `ts/app`; logged-out marketing changes belong in `ts/web`.
- Whenever updating the real external API, add the matching behavior to the mock API (`ts/mock-api`) so local and test clients stay aligned.

## Readme (README.md)

- Summary of the key features of the project (not a long list of every single feature)
- User facing interface documentation (e.g. CLI, API, etc)
- Developer get started
- Key code jumping in points
- Links to protocol docs and plans

### Rust Crate READMEs

- Treat every Rust crate `README.md` as user-facing documentation that should be good enough to publish on crates.io
- Start with a short statement of what the crate is for, where it fits in the workspace, and when a caller should depend on it
- Document the crate's public behavior and integration boundary, not just its internal implementation details
- Every Rust crate README must include these sections, in this order:
  - `## Responsibilities`
  - `## What This Crate Does`
  - `## Quick Start`
  - `## Development`
  - `### Key Code`
  - `### Related Docs`
- `## Quick Start` must include example code or runnable commands, not just prose
- For binary crates or user-facing tools, document the CLI or external interface in the README, not just internal architecture
- Keep examples and guidance aligned with the real public API and current behavior; do not describe internals that callers cannot use directly
- Keep crate READMEs concise and high-signal; explain the main use cases and boundaries rather than listing every file or every minor feature
- When a crate has important neighboring crates, name them and explain the boundary between them so ownership is clear


## Docs (./docs)

- Docs must be kept up to date with implementation at all times
- Any discovered gaps in the docs during implementation should involve an update to the protocol docs
- Must be consistent with itself (i.e. no conflicting statements)
- Must cover entire impl (i.e. no guess work should be needed)
- Any time you make a change to the code, think about whether a doc could be clarified or enhanced
- Docs may be nested in multiple directories

### Protocol Docs (./docs/protocol) aka Specs
- These define the specs contract with code
- Should be as detailed as possible
- Must remained aligned with impl
- Must be complete, there should be no guess work needed during impl
- If there are bugs in the code, review whether the specs need to be better defined
- Keep each doc short (~250 lines)

### Mockups (./docs/mockups)
- Mockups are part of the spec and must stay aligned with the implementation at all times
- Any work that requires design implementation must update an existing mockup or create a new one first, before the implementation lands
- Every mockup must include two variants: a mobile variant and a web/desktop variant
- Migrated mockup HTML files under `docs/mockups/**/*.html` are generated from
  React-backed `.source.tsx` page modules in `docs/mockups/src/pages/` and
  shared components under `docs/mockups/src/components/`. Page modules should
  compose TSX components, not large raw HTML strings or generated static-tree
  data; `mockups:check` enforces this for migrated pages. When changing
  reusable mockup source under `docs/mockups/src`, run
  `npm run mockups:build`, commit the matching generated HTML, run
  `npm run mockups:check`, `npm run mockups:test`, and
  `npm run mockups:typecheck`, and visually smoke-test every changed page by
  opening it directly from disk.
- Do not hand-edit generated mockup HTML as the source of truth. Update the
  source module or shared component first, then let the generator update the
  static HTML. Hand edits to generated HTML are only acceptable while migrating
  a page that has not yet moved under `docs/mockups/src/pages/`.


## Plans (./plans)

- Plans should only be created with consent from the user (requires the protocol docs to be complete)
- Plans do not live at the repo root anymore; they live under `./plans`
- Create one plan file per change, named after the change in concise kebab-case, for example `tool-request-error-contract-alignment.md`
- Do not combine unrelated work into a shared plan file; create a new plan file for each distinct change
- `plans/README.md` is the directory index and must list active and completed plans
- When creating a new plan file, add it to `plans/README.md` immediately
- When a plan is completed, move its link from the active section to the completed section in `plans/README.md`
- Each plan describes work needed to ensure complete alignment with the protocol docs
- Each plan should break up the work into concrete units called Milestones. At the end of each milestone there should be a functioning product. Never leave the code base or feature in a broken state.
- Milestones should have a short summary of what it includes
- All milestones have TODO checklists, lists of tasks that must be completed to achieve the milestone.
- Any time a new TODO is discovered during implementation, it should be added under the relevant milestone (just add the new TODO, and then continue with the active TODO)
- If a TODO is complex, break it down into sub-tasks/TODOs
- As you complete items, you should tick them off in the relevant file under `./plans`
- The workspace `README.md` should link to `plans/README.md`, not to an individual plan file unless a specific change needs to be referenced
- Mark a milestone as completed when all the tasks are completed, do not re-open existing milestones - create a new milestone if new tasks are needed that do not fit into an existing milestone

## Rust

- Use a workspace with multiple crates to split up the code into concrete units
- Run `cargo fmt --all -- --check` after Rust changes; if it fails, run `cargo fmt --all` and re-run the check before clippy/tests are considered complete
- Run cargo clippy after making any changes
- When adding **external** dependencies (from [crates.io](https://crates.io)), use `cargo add` without a version so the newest version is used instead of guessing a version in Cargo.toml. For **workspace internal crates**, add them manually with `dependency = { workspace = true }` (see `docs/dev/rust/architecture/dependencies.md`).
- Treat traits/interfaces as the default for all non-pure behavior, even when there is only a single implementation and that implementation lives in the same crate.
- The only routine exception is small pure free functions or small pure inherent methods on data/value structs.
  - If code has dependencies, uses injected collaborators, reaches ambient state, performs side effects, or owns hidden mutable runtime state, model it behind a trait.
  - Exported library entrypoints like `run`, `serve`, `start`, `sync`, `execute`, or similar runtime operations count as non-pure behavior and should be trait methods, not impure free functions.
- Managers/services and any impure collaborators should depend on trait objects, not concrete implementations.
  - If a struct has runtime dependencies, it should itself usually implement a trait and be consumed through `dyn Trait`, not be passed around as a concrete impure type.
  - Prefer `Arc<dyn Trait + Send + Sync>` for shared runtime dependencies.
  - Use `Box<dyn Trait>` only when ownership is truly single-owner and shared access is not needed.
  - When another crate depends on that behavior, it should depend on the `dyn Trait`; the binary/composition root should construct and inject the concrete implementation.
  - Avoid constructing concrete side-effecting dependencies directly inside manager/service structs.
  - Injecting trait-typed dependencies into a concrete manager/runner/service is not sufficient by itself; if the manager/runner/service has side effects, runtime orchestration, or hidden mutable state, that manager/runner/service must also be behind a trait when used outside the composition root.
  - Factory traits for impure behavior should return `Arc<dyn Trait + Send + Sync>` or `Box<dyn Trait>` as appropriate, not concrete runtime/service/runner structs.
- Concrete structs are still fine for pure/basic data/value types and tiny pure helpers with no runtime dependencies.
  - Small pure inherent methods on those types are fine when they improve clarity.
  - Stateful orchestrators, runners, managers, caches, schedulers, and runtime coordinators are not pure helpers and should themselves be trait-backed.
- If there is only a single implementation and the interface is local to one crate, the trait should usually live in that crate rather than forcing a separate interface crate.
- If multiple implementations are expected or the interface is shared across crates, extract the trait into a dedicated `-interface` crate and keep implementations in separate crates.
- Use diesel for database interactions (where target database is supported by it)
  - database migrations and schema should be in its own crate
  - structs for DB should not be exposed in the library but mapped to a interface struct if needed
- Prefer using `clap` library for CLI parsing
- Use `unimock` as the required mocking library for Rust unit tests at trait boundaries, including same-crate traits with a single concrete implementation.
  - Unit tests should mock trait implementations instead of using real disk IO, databases, Docker, subprocesses, clocks, network calls, or other impure dependencies.
  - Integration tests can still use real implementations when validating end-to-end behavior.
- Use `tracing` for Rust diagnostics in libraries, servers, workers, and other
  non-interactive runtime binaries. Do not use `print!`, `println!`,
  `eprint!`, `eprintln!`, or `dbg!` for logging or diagnostics in those paths.
- Direct Rust terminal output is allowed only when it is the intended
  user-facing interface: CLI command results, prompts, raw command/data output,
  interactive REPL messages, build-script `cargo:` directives, tests, examples,
  and `xtask` developer command output.
- Every crate should have its own README.md
- Inline comments should be avoided
- Rust modules must have module-level doc comments.
- Rust public API items must have doc comments, including public functions, structs, enums, traits, type aliases, constants, statics, enum variants, and public struct fields.
- Add doc comments to private Rust items when they define non-obvious behavior, invariants, or contracts that a maintainer would otherwise need to infer from the implementation.

### Rust File Size Limits

The file length linter enforces a **300-line** hard cap for Rust files under `crates/` and `xtask/` when they are changed relative to `origin/main` or present in the working tree. Run `cargo xtask rust-file-length-lint --all` to audit every Rust file under those directories. Files exceeding 300 lines must be refactored into multiple modules; there is no override mechanism.

### File Size Management

Keep files at reasonable sizes for maintainability:
* **Target**: 200 lines per file
* **Hard Limit**: 300 lines (enforced by linter, refactor immediately)
* **Do not** work around limits by removing blank lines or compacting code
* **Do not** use `include!` instead of Rust's module system. If code can't be cleanly split into modules, leave it in one big file. The only case where you should even consider `include!` is when the included code isn't meant to be read, like code generated in a build step.
* **Allow exceptions** for:
  * Complex state machines or protocol implementations
  * Generated code or large data structures
  * Files where splitting would harm cohesion
* When files grow large, consider refactoring into logical modules
* Example split for a large `tools.rs` file:
  * `tools/mod.rs`: module declarations and intentionally exported types only
  * `tools/error.rs`: error definitions
  * `tools/types.rs`: shared structs, enums, and type aliases
  * `tools/<name>.rs`: implementation modules for a coherent responsibility, named after the function, struct, or feature they implement

### Imports

- Organize imports in this order: standard library (`std`, `core`, `alloc`), external crates, current crate modules (`crate::`), then relative modules (`self::`, `super::`)
- Declare all imports at the top of the file; never place `use` statements inside functions, methods, or nested code blocks
- Before adding imports, check whether the item is already imported and extend existing groups instead of duplicating imports
- Do not rename modules or crates with `as` in import statements; use the real module path directly
- Prefer imports over inline `crate::...` paths in code, type aliases, and `dyn` trait objects; keep `crate::` paths in `use` declarations instead
- Treat ast-grep import rules such as `no-inline-use` and `prefer-imports-over-crate-paths` as required style checks, not optional cleanups

### Testing

- All test implementations must live outside production source files. Do not
  define `#[test]`, `#[tokio::test]`, or similar test bodies inline with
  non-test code; the only allowed same-file test code is the external test
  module declaration.
- For Rust tests that live under `src`, test-only directories must be named
  `_tests_` so they sort first and stand out in listings.
- Rust unit tests and other source-adjacent Rust tests under `src` should live
  in a `_tests_` subdirectory beside the owning source file and be declared
  with an explicit path module, for example
  `src/exa_web_search.rs` uses
  `#[cfg(test)] #[path = "_tests_/exa_web_search_tests.rs"] mod exa_web_search_tests;`
  with the test file at `src/_tests_/exa_web_search_tests.rs`; nested modules
  follow the same relative pattern such as `src/chat/app.rs` using
  `src/chat/_tests_/app_tests.rs`.
- When splitting a Rust test file into submodules, keep the directory name free
  of the `_tests` suffix and keep `_tests.rs` on the leaf test files. For
  example, split `src/tools/_tests_/environments_tests.rs` into
  `src/tools/_tests_/environments/mod.rs`,
  `src/tools/_tests_/environments/spawn_tests.rs`, and sibling `*_tests.rs`
  files. Apply the same rule to crate-root integration tests under `tests/`.
- When using `insta` for Rust snapshots, always use file-based snapshots stored
  under a `snapshots/` directory. Do not use inline snapshots.
- Cargo integration tests may continue to live in crate-root `tests/`
  directories.

### Visibility and Modules

- Default to the narrowest visibility that works: private items/modules first, then `pub(super)` or `pub(crate)`, and only use `pub` for intentional external API.
- Only declare `pub mod` when the module is intentionally part of the crate's external API.
- Use normal Rust module resolution for non-test modules; do not use `#[path = "..."]` outside test-only module declarations.
- Add `#![warn(unreachable_pub)]` to internal crates so over-exposed items are surfaced during linting.
- For intentional dead code, use `#[expect(dead_code, reason = "...")]`; do not add silent `#[allow(dead_code)]` without a documented reason.
- Do not create a child module directory unless it has more than one sibling
  module at the same level. Submodules are fine when they organize a real module
  family, but a single nested module in a crate is pointless.
- Keep `lib.rs`, `mod.rs`, and `bin.rs` as thin module root files. They may
  contain doc comments, module declarations, imports, and intentional type
  exports, but they should not contain runtime logic, business logic, function
  bodies, impl blocks, or other concrete code. Move real implementation into
  named sibling modules.
- When splitting `foo.rs` into a module directory, move the root module to
  `foo/mod.rs`. Do not keep `foo.rs` alongside `foo/*.rs`; apply the same rule
  to test module trees such as `tests/foo/mod.rs` and
  `src/**/_tests_/foo/mod.rs`.
- Remove empty directories after moving or deleting files. Do not leave stale
  module directories behind as placeholders; if the directory has no files, it
  should not exist.
- Do not use public re-exports to avoid updating source imports or call sites.
  Define items at their real owner module path and update downstream imports to
  use that path directly.
- Do not create pure pass-through modules whose public API is only `pub use`
  from another crate or module. Delete the wrapper and import the real owner
  directly instead.

### Explicit drops

Do not use the `drop` function unless absolutely necessary. Usually, if the code compiles without it, it is better to just leave it out.

Some resources, like mutexes should be dropped as soon as possible. This can be accomplished using scopes.

#### Bad (using drop)

```rust
let resource = mutex.lock();
let result = resource.use();
drop(resource);
```

#### Good (using a scope)

```rust
let result = {
    let resource = mutex.lock();
    resource.use()
};
```

### Generics

Prefer `dyn T` runtime dynamic dispatch generics over parametric / static-dispatch / monomorphization generics.
- This is an architectural rule, not just a preference for generic syntax. The goal is testable seams and swappable implementations at side-effect boundaries.

### Typing

- Prefer enums and structs over raw strings when the set of states or variants is known.
- Always fully type new domain, service, and interface code. Avoid introducing `serde_json::Value` or other untyped JSON blobs where a structured Rust type can model the contract.
- If a boundary is forced to accept or emit JSON (external API, persistence, protobuf/HTTP passthrough, etc.), convert it into a structured type as close to that boundary as possible and keep the rest of the code typed.

### Panic and Expect Guidelines

- **NEVER** use `panic!()`, `unwrap()` `expect()` in production code (test code these are fine) - always use proper error handling with Results and defined error types

### Unsafe Code Guidelines

- **AVOID** `unsafe` code blocks unless absolutely necessary for FFI (Foreign Function Interface) operations
- All `unsafe` usage MUST be accompanied by detailed safety comments explaining why it's safe
- Never use `unsafe` for performance optimizations - prefer safe alternatives
- `unsafe` blocks should be as minimal as possible and isolated to dedicated functions
- All `unsafe` code requires additional code review and documentation

### Error Handling Standards

- IMPORTANT: ALWAYS use defined enum errors with the `thiserror` macro
- Public error enums should normally be `typed handled variants + Internal(InternalError)` when the boundary needs internal fallback
- Only errors that callers explicitly branch on should stay typed; storage, provider, serialization, and other unhandled failures should collapse into `Internal(InternalError)`
- For reusable public library crates, downstream consumers count as callers, so the stable typed surface may be broader when those variants are part of the documented public API contract
- Trait/interface crates should own the error contract for their trait methods; implementation crates should return those interface errors directly instead of maintaining mirror wrapper enums
- Error-contract modules with internal fallback should normally derive `internal_error::ErrorContract`, keep the explicit `Internal(#[from] InternalError)` variant, and use the generated `#[track_caller]` helpers to capture call sites; feature code should not pass `Location::caller()` directly
- Handwritten module-local `DEFINED_AT` traits or result adapters are a fallback only when the shared derive cannot be used cleanly
- Prefix the string in thiserror's `#[error]` attribute with crate and mod name e.g. `#[error("[crate/mod] <msg>")]`
- Each crate / mod should should define its own error/result - e.g `notes::{Error, Result}`
- Always define enum variants specifically, never use strings to differentiate them
  - Good: `return Err(Error::DiffNoFilePatches);`
  - Bad: `return Err(Error::Parse { reason: "diff no file patches" });`
- For the canonical internal fallback path, prefer `Internal(#[from] InternalError)` over a manual `impl From<InternalError>` so `?` can promote internal failures without boilerplate
- Use `res?;` or `Ok(res?)`, rather than `.into()` or `.map_err(Error::Variant)`
- Never use `#[error(transparent)]`
- For base/originating errors, use enum varient fields to provide additional data and include that in `#[error]` message

- Counterexamples to avoid:
  - NEVER use `eyre` or `anyhow` dependencies
  - Reserve thiserror `#[from]` for the canonical `Internal(#[from] InternalError)` path; do not use it for public wrapper variants or cross-crate error translation
  - Never match errors using `.contains()` on error strings
  - Never use generic catch-all: `Other(String)`
  - Never use `format!`/`to_string()` in call sites for errors.
  - Never stringify internal failures into ad hoc message variants such as `Json { message: String }`, `Store { message: String }`, or similar string-based error buckets. Preserve the source error type in `InternalError` or in a specific typed enum variant.
  - Never use `map_err(...)` directly in production Rust code for error conversion or context. It hides the real caller location from `#[track_caller]`-based helpers. Use `?`, typed branching, or a `#[track_caller]` result/error helper method instead. The only routine exception is inside the shared helper implementation that preserves caller capture.
  - Avoid using `.into()` for errors or `.map_err(Error::Variant)`, use `res?;` or `Ok(res?)` instead

## Databases

Separate database operations from business logic:
* Database queries should be in dedicated functions, separate from business logic
* Makes testing easier and improves code organization
* Enables better error handling and query optimization

Postgres store crates:
* Crates named `*-store-pg` must contain only database query implementations, Diesel row mappings, migrations/schema integration, and mappings to their store-interface DTOs/errors
* `*-store-pg` crates may depend on their store-interface crate and `juno-db` for shared schema/migrations, plus necessary external database/serialization/error crates
* Do not put business logic, service orchestration, runtime wiring, agent turn runners, model/tool providers, queue scheduling, HTTP/gRPC clients, KMS composition, or other side-effecting service adapters in `*-store-pg` crates
* If a Postgres-backed service needs more than query execution and interface mapping, keep the query code in the `*-store-pg` crate and put the service/composition logic in a separate non-store crate
* Existing `*-store-pg` code that violates this rule should be treated as legacy structure to extract, not as precedent for new code


## Git

This project uses the **Conventional Commits** format for all commit messages. This standardized format improves readability, enables automated changelog generation, and makes it easier to understand the project history.

### Rules

Commit title (first line) must be <= 50 characters.
Commit body (subsequent lines, after a blank line) has no strict length limit.

You may include MULTIPLE entries in a single commit message if there are distinct change types.
If you use multiple entries (feat/fix/refactor/...):
- Separate multiple entries with a blank line.
- Order them by Conventional Commits type priority.

### Examples from the Codebase

These are compact examples of commit titles that don't include the commit descriptions, please include descriptions in your messages:

```
feat: add cargo xtask lint command
fix(linear-webhook): include check run URLs in failure handling
build(ios): skip device arch in dev builds
chore: update app version to 2.0.11
refactor(linear-webhook): extract reusable webhook actions
feat(wallet): add rewards transfer to merge_wallet_admin endpoint
fix(bungee): enforce $1.00 minimum USD output amount threshold
docs(claude): prefer `dyn T` over generics
perf(bungee-client-http): optimize request batching
```

### Cherry-picked example (one commit message that has both `refactor` and `chore`):

This is a commit that has multiple distinct change types - refactor and chore:

```
refactor: adopt Rust 1.90 idioms, fix new lints

- Replace nested conditionals with if-let chains across crates (bungee-client-http, contracts, guild, merge-cli, node, providers, wallet-core, wallet-mobile, zk-primitives)
- Remove unused structs/imports and dead test helpers (panic_handler, utxo, firebase, p2p tests, node rpc/http_error, client tests, smirk iterator, wallet-core send_link, element serde)
- Add #[expect(dead_code)] with reasons where code is gated by features (util, rollup, doomslug)
- Inline include_bytes! for verification key fields and drop redundant constants (barretenberg circuits)
- Tighten lifetimes and return types (Tree::elements, middle_truncate Cow, diesel BoxedQuery lifetime, SemaphorePermit borrows)
- Use .is_multiple_of and from_ref helpers; simplify assertions and formatting utilities
- Minor logic cleanups and early returns; no functional changes intended

chore(rust): bump workspace to Rust 1.90

- Update codebase to compile cleanly under Rust 1.90 and new lints
```

## Bash Tool Timeout Configuration

**CRITICAL**: Claude Code's environment variable timeout configuration has known issues. Always use explicit `timeout` parameters in Bash tool calls to prevent command timeouts.

### Required Timeout Parameters

All Bash **tool calls** (not commands) MUST include explicit `timeout` parameter (in milliseconds):

**Timeout Guidelines:**
- **Default commands**: 600000ms (10 minutes)
- **Long operations** (cargo check, yarn install): 900000ms (15 minutes)
- **Very long operations** (full workspace builds): 1800000ms (30 minutes)

### Retry Logic for Timeouts

If a command times out but shows progress, retry.

### Reminder

- ✅ Always use explicit `timeout` parameter in **Bash** tool calls
- ✅ Monitor command output for progress indicators before retry
- ✅ Use longer timeouts for workspace operation

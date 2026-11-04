# File-length lint ignores its --all flag

- **Severity:** Low. **Effort:** Small. **Confidence:** High. **Harder later:** No.
- **Kind:** Outside the scope.
- **Added:** 2026-11-04, from the branch `conductor/agents-md-redloop`.
- **Location:** `xtask/src/rust_file_length_lint.rs:10`, `xtask/src/rust_file_length_lint.rs:11`, `xtask/src/check.rs:35`, `AGENTS.md:90`.
- **Reason:** The fix changes `xtask` code, and the branch that found it changes only agent rules and plan docs.

- **Terms:**
  - **File-length lint:** the command `cargo xtask rust-file-length-lint`, which fails when a Rust file under `crates/` or `xtask/` has more than 300 lines.
- **Background:**
  - The lint accepts no argument or `--all`.
  - Any other argument fails with `usage: cargo xtask rust-file-length-lint [--all]`.
  - `cargo xtask check` calls the lint with `--all`.
  - An earlier `AGENTS.md` said that the cap applied to the Rust files changed relative to `origin/main`. It said that `--all` audited every Rust file.
  - The current `AGENTS.md` says that the lint audits every file, with or without `--all`.
- **Problem:** The flag has no effect. Both forms walk every `.rs` file under `crates/` and `xtask/`. The usage text suggests that a run without `--all` audits fewer files.
- **Impact if no action:** A reader expects a smaller audit without `--all`. The lint stays correct, because it always checks every file.
- **Root question:** What does the file-length lint audit, and how does its interface show it?
- **Options:**
  - **A.** Keep the flag and the behaviour. Change the usage text to say that `--all` has no effect. Effort: Small.
  - **B.** Remove the flag, and accept no arguments. Update `xtask/src/check.rs` and the repository rule in `AGENTS.md`. Effort: Small.
  - **C.** Make a run without `--all` audit only the Rust files that differ from `origin/main`. Update the repository rule in `AGENTS.md`. Effort: Medium.
- **Left out:** None.
- **Wider fix:** None. The class is one argument without an effect.
- **Previous examples:** The `futex-ai/ai` repository has the same problem in its own `xtask`. Its item `plans/follow-ups/file-length-lint-all-flag.md` records it.
- **Related:** `plans/follow-ups/xtask-crate-readme.md` adds `xtask/README.md`, which documents this lint. The fix that lands second updates that README to match the lint.
- **Recommendation:** B. A flag without an effect only misleads, and the lint is simpler without it.
- **Action:** ask the user, because option B removes a command-line option, and a command interface on `main` is functionality.

# The xtask crate has no README

- **Severity:** Low. **Effort:** Small. **Confidence:** High. **Harder later:** No.
- **Kind:** Outside the scope.
- **Added:** 2026-11-04, from the branch `conductor/agents-md-redloop`.
- **Location:** `xtask/`, `README.md:75`.
- **Reason:** The fix adds a crate README, and the branch that found it changes only agent rules and plan docs.

- **Terms:**
  - **`xtask`:** the task runner crate of the workspace, which `cargo xtask <command>` runs through the alias in `.cargo/config.toml`.
- **Background:**
  - The commands of `xtask` are `check`, `review`, and `rust-file-length-lint`, in `xtask/src/main.rs`.
  - `cargo xtask review` sends a summary of the local changes against `origin/main` to the `codex` CLI for a read-only review.
  - The `## Commands` and `## Repository rules` sections of `AGENTS.md` describe `check` and `rust-file-length-lint` for agents.
  - The shared `rust` skill from `futex-ai/skills` says that every crate has its own `README.md`.
  - The `documentation` skill gives the sections of a crate README.
  - The root `README.md` names `xtask` under Key Code as "local verification and review commands".
- **Problem:** `xtask/` holds only `Cargo.toml` and `src/`. No README documents its commands, their steps, or the tools that they need.
- **Impact if no action:** A reader must read the code to learn what `cargo xtask review` does. Only the agent rules in `AGENTS.md` list the steps of `cargo xtask check`.
- **Root question:** Where does the repository document the `cargo xtask` commands?
- **Options:**
  - **A.** Add `xtask/README.md` with the sections that the `documentation` skill requires. Document `check`, `review`, and `rust-file-length-lint`. Effort: Small.
- **Left out:** The item has only one option, because each other place for the docs breaks the `rust` skill rule that every crate has a README.
  - Change nothing. The `rust` skill requires a README for every crate.
  - Document the commands only in the root `README.md`. It leaves `xtask` without a README, which breaks the same rule.
- **Wider fix:** None. The `rust` skill rule that every crate has a README is already in force.
- **Previous examples:** None found.
- **Related:** `plans/follow-ups/file-length-lint-all-flag.md` decides whether `rust-file-length-lint` keeps its `--all` flag. The fix that lands second updates the new README to match the lint.
- **Recommendation:** A. It meets the `rust` skill rule, and the crate README is the home of entry points under the `documentation` skill.
- **Action:** fix after merge, apply A, because the new README is outside the files of the agent-rules branch that found it.

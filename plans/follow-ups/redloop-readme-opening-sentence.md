# Redloop README opening sentence is cut off

- **Severity:** Low. **Effort:** Small. **Confidence:** High. **Harder later:** No.
- **Kind:** Outside the scope.
- **Added:** 2026-11-04, from the branch `conductor/agents-md-redloop`.
- **Location:** `crates/redloop/README.md:3`.
- **Reason:** The fix edits a crate README that the branch that found it does not change.

- **Background:**
  - The shared `documentation` skill from `futex-ai/skills` says that a crate README starts with a short statement.
  - The statement says what the crate is for, where it fits in the workspace, and when a caller should depend on it.
- **Problem:** Line 3 says "`redloop` is a Redis-backed Rust job queue library with:". The next non-blank line is the heading `## Responsibilities`. The list that "with:" announces is missing.
- **Impact if no action:** The first sentence of the main doc of the crate reads as broken.
- **Root question:** What does the opening statement of the `redloop` crate README say?
- **Options:**
  - **A.** End the sentence without "with:". Add where the crate fits and when a caller depends on it. Effort: Small.
  - **B.** Copy the first items of `## What This Crate Does` up into the sentence as the missing list. Effort: Small.
- **Left out:** Delete the sentence. The `documentation` skill requires an opening statement.
- **Wider fix:** None. The `documentation` skill already gives the opening statement.
- **Previous examples:** None found.
- **Recommendation:** A. It gives the statement that the `documentation` skill asks for, without a repeat of the list below it.
- **Action:** fix after merge, apply A, because the README is outside the files of the agent-rules branch that found it.

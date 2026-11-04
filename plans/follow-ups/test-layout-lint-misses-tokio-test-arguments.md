# Test layout lint misses tokio::test with arguments

- **Severity:** Low. **Effort:** Small. **Confidence:** High. **Harder later:** Yes.
- **Kind:** Outside the scope.
- **Added:** 2026-11-04, from the branch `conductor/agents-md-redloop`.
- **Location:** `xtask/src/check.rs:91`, `AGENTS.md:93`.
- **Reason:** The fix makes a gate stricter, which needs the user, and the branch that found it changes only agent rules and plan docs.

- **Terms:**
  - **Test layout lint:** the `source_layout_lint` step of `cargo xtask check` in `xtask/src/check.rs`, which fails on `#[test]` or `#[tokio::test]` in a Rust file under `crates/` outside a `_tests_` or `tests` directory.
- **Background:**
  - The shared `rust` skill from `futex-ai/skills` says that test bodies live outside production files. They live in a `_tests_` directory beside the source, or in crate-root `tests/`.
  - The lint enforces this at line 91, with a plain text search of each file for the strings `#[test]` and `#[tokio::test]`.
  - The tests of this repository use three forms of test attribute:
    - `#[test]`, 16 times
    - `#[tokio::test]`, 20 times
    - `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`, 13 times, for example at `crates/redloop/src/worker/_tests_/run/pending_reserve_tests.rs:25`
  - The `## Repository rules` section of `AGENTS.md` describes the lint with the two strings.
- **Problem:** The search for `#[tokio::test]` does not match `#[tokio::test(...)]`, because the text after `test` is `(`, not `]`. A production file with a test body in that form passes the gate. On 2026-11-04, no file broke the rule. This search found nothing: `grep -rln -E '#\[\s*(tokio::)?test' crates --include='*.rs' | grep -v -E '/_tests_/|/tests/'`.
- **Impact if no action:** A test body with arguments can land in a production file, and `cargo xtask check` and CI still pass. Each such test body is another copy of the wrong pattern that the later fix must move.
- **Root question:** How does the gate catch every test body outside the test directories?
- **Options:**
  - **A.** Match the start of the attribute. Search for `#[tokio::test` without the closing bracket, and keep `#[test]`. Update the rule text in `AGENTS.md`. Effort: Small.
  - **B.** Match each test attribute with a hand-written matcher that also allows spaces, like the regular expression `#\[\s*(tokio::)?test\b`. Update the rule text in `AGENTS.md`. Effort: Small.
  - **C.** Parse each file with the `syn` crate, and flag any function with a `test` or `tokio::test` attribute. This adds a dependency to `xtask`. Effort: Medium.
- **Left out:** Change nothing. A test body with arguments then still passes the gate, which fails the goal.
- **Wider fix:** None. The lint is already the detection fix for this class, and each option completes it. No structural fix keeps a test attribute out of a production file.
- **Previous examples:** None found.
- **Recommendation:** B. It catches every form that the repository uses, and also spacing variants, with no new dependency.
- **Action:** ask the user, because each option makes a gate stricter, and the always-on rules reserve that choice for the user.

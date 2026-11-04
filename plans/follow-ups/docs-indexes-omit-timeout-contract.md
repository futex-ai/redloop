# Docs indexes omit the timeout contract

- **Severity:** Low. **Effort:** Small. **Confidence:** High. **Harder later:** No.
- **Kind:** Outside the scope.
- **Added:** 2026-11-04, from the branch `conductor/agents-md-redloop`.
- **Location:** `docs/README.md:5`, `docs/protocol/README.md:5`.
- **Reason:** The fix edits two doc indexes that the branch that found it does not change.

- **Terms:**
  - **Timeout contract:** the protocol doc `docs/protocol/redloop/timeouts.md`, which defines Redis response deadlines and the recovery after a missing reply.
- **Background:**
  - `docs/README.md` is the index of all docs.
  - `docs/protocol/README.md` is the index of the protocol docs.
  - `docs/protocol/redloop/README.md:18`, `docs/protocol/redloop/api.md:175`, the root `README.md:46`, and `crates/redloop/README.md:112` link to the timeout contract.
- **Problem:** `docs/README.md` and `docs/protocol/README.md` list the Redloop, Redloop API, and Redloop Redis Layout docs, but not the timeout contract.
- **Impact if no action:** A reader who starts at a docs index does not find the timeout contract.
- **Root question:** How does a reader find each protocol doc from the docs indexes?
- **Options:**
  - **A.** Add the timeout contract to both indexes. Effort: Small.
  - **B.** Keep one list. Link `docs/README.md` only to `docs/protocol/README.md`. List every protocol doc in `docs/protocol/README.md`, with the timeout contract. Effort: Small.
- **Left out:** Change nothing. A reader who starts at a docs index still misses the timeout contract, so this fails the goal.
- **Wider fix:** B. With one list, a new protocol doc needs one new link, so the two indexes cannot drift apart.
- **Previous examples:** None found.
- **Recommendation:** B. It fixes the gap and removes the second list that can drift.
- **Action:** fix after merge, apply B, because the indexes are outside the files of the agent-rules branch that found it.

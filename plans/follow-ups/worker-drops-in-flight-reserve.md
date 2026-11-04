# Worker drops an in-flight reserve call

- **Severity:** Medium. **Likelihood:** Common. **Effort:** Medium. **Confidence:** High. **Harder later:** No.
- **Kind:** Outside the scope.
- **Added:** 2026-11-04, from the branch `conductor/agents-md-redloop`.
- **Location:** `crates/redloop/src/worker/run.rs:104`, `crates/redloop/src/worker/run.rs:137`, `crates/redloop/src/worker/run.rs:73`, `crates/redloop/src/worker/_tests_/run/pending_reserve_tests.rs:26`.
- **Reason:** The fix changes the worker runtime, and the branch that found it changes only agent rules and plan docs.

- **Terms:**
  - **Reserve:** the store call `QueueStore::reserve`, which runs the Lua script `crates/redloop/src/redis_store/scripts/reserve.lua` to lease up to the free capacity of jobs to the worker and return them.
  - **Lease:** a worker's claim on a job, which Redis records in the `leased` sorted set with the deadline `now + lease_duration`.
  - **Reap:** the store call `reap_expired`, which returns the jobs of expired leases to the `ready` sorted set.
  - **Biased `tokio::select!`:** a `tokio::select!` with `biased;`, which polls its branches in order, runs the first ready branch, and drops the futures of the others.
- **Background:**
  - `WorkerConfig` in `crates/redloop/src/config.rs` sets `concurrency`, `lease_duration`, `heartbeat_interval`, and `reap_interval` for each worker.
  - `Worker::run_internal` in `crates/redloop/src/worker/run.rs` loops around one biased `tokio::select!`.
  - At the top of each iteration, the loop records the finished jobs and resolves their completions, such as an ack. Then it enters the select.
  - The branches of the select, in order:
    1. the join of a finished handler task
    2. the heartbeat tick, while jobs are active
    3. the reap tick
    4. the runtime backoff sleep
    5. the reserve call `self.store.reserve(...)`
    6. the poll sleep
  - The heartbeat and reap calls run inside their branch bodies, so the select never drops them.
  - The reserve call is itself a branch future.
  - The ticks come from `tokio::time::interval`, which fires once at start and then at each interval.
  - The test `pending_reserve_does_not_delay_active_job_ack` requires that a pending reserve does not delay the ack of a finished job.
  - `docs/protocol/redloop/api.md` says that reserve-timeout backoff must not starve active-job joins, heartbeats, or lease reaping.
  - `docs/protocol/redloop/timeouts.md` covers a lost reserve reply after Redis grants the lease, for example after a timeout. There, the worker must not run a handler for that job. A later reap makes the job eligible again.
  - The test `lost_reservation_reply_is_reaped_and_processed_without_worker_restart` in `crates/redloop/tests/command_timeout/recovery_tests.rs` protects that behaviour.
  - A smoke test on 2026-11-02 at commit `0a4c02c` showed the problem:
    - The smoke test ran against a local Redis 6.2.20.
    - Each run used a fresh `REDLOOP_KEY_PREFIX` and ran these commands in order:
      1. `cargo run -p redloop --example basic -- enqueue <job>`
      2. `cargo run -p redloop --example basic -- work 5`
      3. `cargo run -p redloop --example basic -- counts`
    - 5 of 6 runs printed `processed 0 jobs`. In those runs, `counts` printed `ready=0 scheduled_due=0 scheduled_future=0 leased=1 failed=0`.
    - A 12-second worker run gave the same result.
    - The `basic` example sets `lease_duration` to 30 s and `reap_interval` to 5 s. A stranded job there waits 30 to 35 s, which is longer than each run.
    - `redis-cli monitor` recorded one failing run, with these commands in order:
      1. the reserve EVAL, which ran `ZREM` on `ready`, `ZADD` on `leased`, `HSET` on `lease_meta`, and `HINCRBY` on `workers:leases`
      2. the reap EVAL, 0.6 ms later
      3. more reserve EVALs, and no heartbeat EVAL
    - The example handler prints a `processing <job_id>` line for each job. It printed nothing.
    - The first reap tick fires about 1 ms after the worker starts. The first reserve of a worker therefore often loses the race against that tick.
- **Problem:** A join, a heartbeat tick, or a reap tick can become ready while a reserve call is in flight. The select then runs that branch and drops the reserve future. Redis has already run the reserve script and leased the jobs to this worker. The worker never receives the jobs, so no handler runs and no heartbeat refreshes their leases. The jobs wait until the lease expires and a reap returns them to `ready`. This happens in normal operation, without a timeout or a Redis error.
- **Impact if no action:** This can affect a job that a worker leases at start, or while a tick or a finished job races its reserve. Such a job waits a full `lease_duration` plus up to one `reap_interval` before it runs. Each dropped reserve can strand up to `concurrency` jobs. At-least-once delivery still holds, but ASAP jobs run late. The `workers:leases` counter in Redis counts leases that no handler holds. Until the fix, the smoke test in `AGENTS.md` often prints `processed 0 jobs`, not the expected `processed 1 jobs`.
- **Root question:** How does the worker keep every job that Redis leases to it, while acks never wait for a pending reserve?
- **Options:**
  - **A.** Keep the in-flight reserve future across loop iterations. Hold it in an `Option` of a pinned boxed future outside the `select!`. Poll it through a mutable reference in the reserve branch. Clear it only when it completes. Add a regression test with a store mock. The mock's reserve returns a job only after a join or a tick is ready. Assert that the handler runs for that job. Effort: Medium.
  - **B.** Run each reserve as a spawned task. Take its result from a channel or a second join set inside the select. Effort: Medium.
  - **C.** Wider fix: add a small owned type for an in-flight call. It keeps a side-effecting store future across loop iterations. Every store call that a select loop starts as a branch uses it. Today the reserve is the only such call. Effort: Medium.
- **Left out:**
  - Reorder the branches, or remove `biased;`. Another ready branch still drops the reserve future, so this fails the goal.
  - Await the reserve outside the select. An ack then waits for a pending reserve. This breaks the requirement that `pending_reserve_does_not_delay_active_job_ack` protects.
- **Wider fix:** C. It is structural, because a select loop can then no longer drop a side-effecting call. The `## Review focus` section of `AGENTS.md` also names this class for reviews.
- **Previous examples:** None found. The reserve call has been a branch of this select since commit `0103b35`, which extracted the workspace. Commit `8ee21e2` later changed the same loop for transient-error recovery. Commit `16b7d84` added `docs/protocol/redloop/timeouts.md` and its tests, but it did not change this loop.
- **Related:** `plans/follow-ups/pending-reserve-test-hand-written-store.md`. The regression test of this fix needs the pending-call helper of that item's option B, so one branch can fix both items.
- **Recommendation:** C. It is the structural wider fix, so no select loop can drop a side-effecting store call. The fix rule of the shared `decisions-and-findings` skill ranks a structural wider fix before the direct fix. C applies the mechanism of A through the new type. It also adds the regression test of A.
- **Action:** fix after merge, apply C, because the fix changes the worker runtime, which the agent-rules branch that found it does not touch.

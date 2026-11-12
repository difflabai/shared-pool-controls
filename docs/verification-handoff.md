# Handoff for formal verification

No formal proof is claimed in this repository. The implementation intentionally focuses on small Rust APIs, tests, synthetic execution and static UIs.

Start with `crates/core/src/lib.rs` and `docs/spec.md`. The default pool-core build has **zero dependencies**, no unsafe code, no NEAR imports, and no external I/O. Optional serde/borsh are serialization only. `cargo test -p pool-core --no-default-features` exercises the proof-facing configuration.

## Suggested obligations

| ID | Obligation | Main implementation |
|---|---|---|
| CFG | Accepted configuration references are valid and all loops/data are bounded | Config::validate |
| NUM | Valid bounded state/terms cannot overflow energy; all delta overflows fail closed | add, energy_terms, energy |
| AUTH | Only recipe actor admits/dispatches, executor settles, owner observes/pauses | candidate, dispatch, settle, observe |
| ADM | Accepted reservation has nonincreasing potential; all hard-violation envelopes do not worsen | candidate, envelope, check_bounds |
| ATOM | All rejected core mutations preserve the full state | submit, dispatch, settle, cancel, observe |
| STALE | Request computed at a prior accepted version cannot replay | candidate, submit |
| ASYNC | Unknown outcomes retain reservation until authoritative terminal evidence | acknowledge, settle, cancel |
| TERM | Terminal repeated evidence is idempotent; conflicting outcome cannot apply | settle, cancel |
| DEP | Prerequisite requires completed units, not acknowledgment or partial completion; pruning keeps the count | candidate, settle (`completed_units`) |
| BIND | Each dispatch ID fixes actor recipe, units and executor | Commitment and wrapper dispatch |
| STOCK | Inventory fixture conserves available + transit and never spends inbound pipeline | fixtures + lifecycle |
| CASH | Hiring fixture conserves free + earmark + escrow + paid + protected; payment preserves coverage | fixtures + lifecycle |
| RISK | Pending negative exposure does not grant new risk headroom, and no hedge relies on an unexecuted position | risk fixture + envelope |
| LIVE | Every open record can be resolved | envelope, check_bounds, release, settle |
| WIN | At most 256 retained records with consecutive ids; `Full` only while the oldest is open; only terminal records are pruned, and a pruned id is `NotFound` | candidate, submit, index |
| EXEC | The executor never forgets or refuses an id whose lock record is retained; at most 512 jobs | executor execute, confirm_not_executed |

Prove transition-preservation from a well-formed state reachable through public methods. Pool fields are public to make model/harness construction easy; malformed arbitrary Rust values are outside ordinary deployed reachability. If you broaden the theorem to arbitrary deserialized values, add and prove state validation first. Config validates initial values within bounds; observations may intentionally introduce violations.

Important nuances:

- Recovery admission uses nonincrease at reservation and strict decrease for the proposed completed state. A reservation never changes a metric's violation amount. It does not prove eventual completion or safety under arbitrary future market moves.
- Hard checks use intervals that contain every outcome of every open record: it stays open, completes, or releases its reservation. This overapproximation can reject feasible correlated bundles; it should never underapproximate the modeled independent effects.
- Admission requires each metric's interval violation to be at most its violation before the request and after the reservation. So in every reachable state each metric's interval violation equals its current violation, and no completion or release can fail the bounds check (LIVE). The version reserve below means the version increment cannot fail either. Liveness is conditional only on authoritative evidence and gas. These rules were added after the v1.0.0 verification (findings F-1, F-2, F-2b and F-5 in verification-findings.md). They are tested, not formally re-verified.
- Fixed policy is part of the theorem. Owner observations, executor honesty, and source model adequacy are assumptions, not consequences of a low potential.
- Energy is not checked on lifecycle reversal or observation. Do not accidentally state an all-transition monotonicity theorem.
- Version reserve: submit, dispatch, pause and observe stop 768 versions below u32::MAX (`VERSION_CAP`, `Arithmetic`); acknowledgment, settlement and cancellation always remain available. An open record needs at most three more increments (Dispatched -> Unknown -> Active -> terminal) and there are at most 256 records, so the reserve is exactly enough. Every effective transition still increments version by exactly 1. Pause/observe still advance version.
- Record window: the pool keeps its 256 most recent records. A submit into a full window prunes the oldest record if it is terminal and fails `Full` if it is open; by LIVE that record can always be closed. Ids never repeat, a pruned id is `NotFound`, and replay protection is the version, not retention. Prerequisites read `completed_units`, a lifetime count updated by `settle(true)`. The executor forgets a job once its id is 256 or more behind the newest id it has executed for the lock; the lock has pruned that record, so it is terminal and never dispatched again. This replaces v1.0.0's 256 lifetime records (finding F-3); it is tested, not formally re-verified.

## Boundary checks

The NEAR shell serializes the same core with borsh. Review `predecessor_account_id`, init authority, callback privacy, private callback's promise-result mapping, persistent state atomicity, and bounded gas/storage. The synthetic executor is independent code and must be included if claiming end-to-end gating. Unknown receipt failure cannot imply a refund.

Both contracts create promises only with `promise_create`, `promise_then` and `promise_return`, and every promise is a function call with no deposit. Neither Wasm imports a host function for any other action: deploy, keys, transfer, stake, account creation or deletion, global contracts, or any `promise_batch_*` function. `scripts/build.sh` fails if one is imported, and also on any import not on its reviewed list (18 for the contract, 13 for the executor). This was added after the v1.0.0 verification (finding NT-F3); it is checked by the build, not formally re-verified.

Account upgrades, oracle dishonesty, physical delivery, real broker fills, and deployment locks are outside the current tests. There is no real token custody. Do not inherit formalver's labels or proofs: the design pattern is reused, not its implementation theorem.

## Existing evidence

Unit tests cover scenario fixtures, accepted/rejected branches, stale requests, unauthorized identities, pending stock, earmarks, completed-unit prerequisites, duplicate settlement, recovery hedges, conservative pending exposure, the version reserve and the record window. A seeded random test (`live_all_on_random_runs`) checks after every call that every open record can be resolved, that no call but observe raises a metric's violation, that an observe with a record open changes no value or record, that each interval violation equals the current violation, and that a rejected call changes nothing. A second one (`window_on_random_runs`) fills and prunes the window and checks the window rules after every call. Sandbox tests execute both Wasm binaries and compare full state/decisions with the pure core. HTTP smoke checks exercise the local Rust interface. Add mutation testing and proof canaries in the verification work; neither is currently a formal-verification result.

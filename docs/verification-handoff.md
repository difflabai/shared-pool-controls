# Handoff for the formal-verification session

No formal proof is claimed in this repository. The implementation session intentionally focused on small Rust APIs, tests, synthetic execution and static UIs, per the user's request.

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
| TERM | Terminal repeated evidence is idempotent; conflicting outcome cannot apply | settle |
| DEP | Prerequisite requires completed units, not acknowledgment or partial completion | candidate |
| BIND | Each dispatch ID fixes actor recipe, units and executor | Commitment and wrapper dispatch |
| STOCK | Inventory fixture conserves available + transit and never spends inbound pipeline | fixtures + lifecycle |
| CASH | Hiring fixture conserves free + earmark + escrow + paid + protected; payment preserves coverage | fixtures + lifecycle |
| RISK | Pending negative exposure does not grant new risk headroom | risk fixture + envelope |

Prove transition-preservation from a well-formed state reachable through public methods. Pool fields are public to make model/harness construction easy; malformed arbitrary Rust values are outside ordinary deployed reachability. If you broaden the theorem to arbitrary deserialized values, add and prove state validation first. Config validates initial values within bounds; observations may intentionally introduce violations.

Important nuances:

- Recovery admission uses nonincrease at reservation and strict decrease for the proposed completed state. It does not prove eventual completion or safety under arbitrary future market moves.
- Hard checks account for independent subsets of completion effects using intervals. This overapproximation can reject feasible correlated bundles; it should never underapproximate the modeled independent effects.
- A rejected checked inverse release leaves liability intact. Liveness is conditional on resolving dependencies and receiving authoritative evidence.
- Fixed policy is part of the theorem. Owner observations, executor honesty, and source model adequacy are assumptions, not consequences of a low potential.
- Energy is not checked on lifecycle reversal or observation. Do not accidentally state an all-transition monotonicity theorem.
- Version overflow is an explicit rejection; 256 lifetime records bound ordinary growth. Pause/observe still advance version.

## Boundary checks

The NEAR shell serializes the same core with borsh. Review `predecessor_account_id`, init authority, callback privacy, private callback's promise-result mapping, persistent state atomicity, and bounded gas/storage. The synthetic executor is independent code and must be included if claiming end-to-end gating. Unknown receipt failure cannot imply a refund.

Account upgrades, oracle dishonesty, physical delivery, real broker fills, and deployment locks are outside the current tests. There is no real token custody. Do not inherit formalver's labels or proofs: the design pattern is reused, not its implementation theorem.

## Existing evidence

Unit tests cover scenario fixtures, accepted/rejected branches, stale requests, unauthorized identities, pending stock, earmarks, completed-unit prerequisites, duplicate settlement, recovery hedges and conservative pending exposure. Sandbox tests execute both Wasm binaries and compare full state/decisions with the pure core. HTTP smoke checks exercise the local Rust interface. Add mutation testing and proof canaries in the verification session; neither is currently a formal-verification result.

# Shared Pool Controls — revised implementation plan

Updated November 11, 2026. This supersedes the earlier SwarmGuard plan. Goals: implementation in `difflabai/shared-pool-controls`, Rust throughout the decision/runtime layers, static HTML interfaces following trust-framework approval-contract patterns, a small API, and formal verification as separate follow-up work.

## Scope and order

Build five demonstrations using one kernel:

1. Every team needs the GPU now.
2. Everyone promised Friday.
3. Every branch hoards the scarce part.
4. Every agent hires more agents.
5. A risk pool shared by managers at a multi-strategy hedge fund: Everyone is diversified—until they need the same exit.

Deliver a pure core, NEAR wrapper, enforced synthetic executor, deterministic scenario configurations and traces, a Rust local demo server, static HTML lab, static wallet console, tests, build provenance and a verification handoff. Do not claim formal proof, real token custody, physical execution or a public deployment.

## Simplifications adopted for this implementation

- One immutable pool per contract deployment instead of multi-instance policy management.
- Fixed owner-approved recipes instead of arbitrary action programs or an extensible policy language.
- A recipe names its actor, executor, reservation effects, completion effects, quantity limit and prerequisites.
- Agents submit only recipe, units, state version and expiry. They never supply ledger deltas or risk estimates.
- Per-metric hard bounds and three squared-potential primitives. Aggregate metrics encode coupled constraints.
- Exact submission only. A Rust planner suggests a reduced quantity; the user/agent explicitly resubmits. Later dates are distinct recipes.
- Version equality replaces a separate nonce system for accepted-request replay protection.
- At most 256 lifetime commitments; no pruning or live migrations.
- Static synthetic scheduling windows, not a production time/calendar engine.
- Independent authenticated executor evidence replaces a generalized evidence/signature protocol.
- No LLM dependency, frontend framework, JavaScript build chain, optimization solver, or database.
- No Kani/Lean/Verus/TLA+ implementation in this iteration. Keep the spec and small dependency-free core ready for that work.

These choices intentionally replace the earlier plan's dynamic policy epochs, generic ticket issuance API, multiple lifecycle stages, continuous scheduler, and proof milestones. They retain the important economic and asynchronous boundaries.

## Common kernel

Implement Config, Metric, Term, Recipe, Request, Commitment and Pool. Hard maximum sizes: 32 metrics, 32 recipes, 64 terms, 256 records, 100 units per action. Validate config before state initialization. All vector lengths and references are bounded and checked. Potential is a checked integer sum with decimal-string JSON output.

Admission:

1. Check caller, pause flag, request version, short expiry, observation freshness and record limit.
2. Check recipe, quantity and completed-unit prerequisites.
3. Apply owner-defined reservation delta to a copy of the metric vector.
4. Compute a conservative interval for all outstanding completion effects. Each pending completion may occur independently; do not assume an unfilled hedge offsets an adverse position.
5. Check hard limits; in a disturbed baseline, require no worsening of each metric's violation.
6. Check nonincreasing potential at reservation. For an already hard-violating state require strict decrease at the proposed completed state as well.
7. Append an immutable commitment payload and atomically replace the vector.

Failures leave economic and core state unchanged. Previews return the same check results but do not reserve. Alternative search enumerates bounded quantities and cannot authorize execution.

## Lifecycle and NEAR shell

Public API: new, get_pool, get_commitments, preview, submit, dispatch, reconcile, settle, cancel, set_paused, observe. A private on_result maps runtime promise outcomes.

Reserved -> Dispatched -> Active -> Completed. Missing/malformed/failed acknowledgment -> Unknown. Reserved can be cancelled by its actor/owner. Dispatched/Active/Unknown can be cancelled only on definitive nonexecution from the registered executor. A timeout is never enough.

Reserve before dispatch. Bind dispatch ID to exact recipe and units. Synthetic executor accepts only the lock; an independent operator confirms completion/cancellation. Store terminal evidence before notification so reconciliation can recover a lost callback. Repeated evidence is idempotent; conflicting terminal evidence rejects.

The administrator can pause admissions/dispatch and refresh observed values only with no unresolved records. Neither operation changes policy. Observations may raise energy. No production upgrade-lock claim is made.

## Scenario 1 — GPU cooperative

Three teams each target 4 blocks. Two windows each have 12 usable blocks after a protected inference reserve; soft limit is 8. V = sum of squared unfilled team targets + 10 times squared window occupancy excess above 8.

Expected sequence: initial 48; Research 4 now ->32; Evaluation 2 now ->20; Data 4 now denied (would be44); Data2 now ->8; Data2 later ->4; Evaluation2 later ->0. Candidate Data quantities 0..4 produce [20,13,8,15,44]. This demonstrates why bounded enumeration is preferable to assuming quantity monotonicity.

The executor mocks jobs. Pending reservations count; confirmed completion consumes historical booked capacity. Unknown outcome preserves occupancy. A new scheduling horizon needs a new pool or a quiescent observation.

## Scenario 2 — workshop network

Reuse GPU arithmetic and recipe pattern, substituting Friday/Tuesday production lots. Each lot also reserves a dispatch slot. The machine and dispatch effects are atomic. A shortage in either rejects all effects. Customer consent is represented by explicitly selecting the accepted recipe/quantity; the lock never changes the date itself.

Tests include parity with GPU and a fixture where machine space exists but dispatch capacity does not. Completion is an independent workshop attestation, not order acceptance alone.

## Scenario 3 — emergency inventory

Planning positions start at depot6, A6, B0; targets are4 each. Depot protects4 available parts. V starts24. A requests2 from depot: candidate(4,8,0), V32, denied although the depot reserve would hold. B requests2: position(4,6,2), V8, allowed. A transfers2 to B: (4,4,4), V0, allowed.

Keep separate available stock and reserved/in-transit metric. Reserve decreases source availability and moves planning position; receive increases destination availability and clears transit. Incoming planning credit never becomes physical availability until receipt. Duplicate receipt cannot create stock. Model supply-chain failures through explicit evidence, not automatic timeout reversal.

## Scenario 4 — agent hiring

Treasury10 credits: general reserve3, review earmark2, discretionary5. Targets Research2, Implementation3, Review2. V starts17. Research requests5: hard budget fits, but V22 -> deny. Research2 ->13. Implementation3 ->4. Independent completion of at least2 research and3 implementation units unlocks Review2 ->0.

Reservation moves free or earmarked credits to escrow and increases lifetime funded coverage. Acceptance moves escrow to paid without reducing coverage. Workers cannot issue new recipes/tickets or approve themselves. Conserved total is free + earmark + escrow + paid + protected =10. All payments are explicitly synthetic.

## Scenario 5 — shared hedge-fund risk pool

The commons is portfolio-wide stress-loss headroom and collateral. Managers have local authority but cannot spend shared capacity twice.

Use three loss metrics in $100k units: current[60,40,30], soft[80,60,40], hard[100,80,60]. Free collateral above a protected reserve starts40. V is squared excess over the three soft limits.

Credit recipe per half-size lot adds[20,5,0] and encumbers5 collateral units. Two lots yield[100,50,30]: hard limits pass but energy400, so deny. One lot yields[80,45,30], energy0, so allow. Equity's next correlated position increases selloff loss and is denied. Macro can reserve a hedge, but its -10 selloff benefit is applied only on confirmed completion. Only after fill may a new equity trade use that headroom.

The energy400 is in squared $100k units, equivalent to4 in squared $1m units. Document the normalization rather than mixing units.

A snapshot expires after300 seconds. The owner can report a new portfolio baseline only when all commitments are resolved. A market shock may produce a hard-limit breach. Recovery admissions must not worsen any violation; hedge completion must reduce potential. There is no guarantee of market recovery, exhaustive stress coverage or loss prevention.

The synthetic universe has fixed linear coefficients and atomic fills. Real derivatives repricing, partial fills, counterparty netting, margin models, liquidations and private data are integration work, not implied by this prototype. Every risk contribution is derived from fixed recipe definitions; managers cannot send their own authoritative risk deltas. Future two-leg trades must either execute atomically or remain safe for every intermediate fill state.

## Interfaces

The lab is plain HTML/CSS/JS. With the local Rust server it supports all scenarios, custom proposals, previews, explicit alternative selection, traces, lifecycle evidence injection and a risk observation. It makes no decision authority claims for browser arithmetic. Without the server it replays checked-in Rust-generated traces and disables mutations.

The live console uses NEAR RPC for final-state reads and a pinned wallet connector following trust-framework's approval UI. Require a contract account and expected code hash before signing. Use text-only rendering of chain data. Never hold private keys or add an automatic submit-on-connect flow.

## Tests and exit gates

- Core fixtures reproduce every numeric sequence above.
- Exhaustive bounded single-admission enumeration checks energy and bounds or complete rejection atomicity.
- Test unauthorized calls, stale requests, expiry, pause, invalid config, arithmetic range, record capacity, pending exposure, repeated evidence and dependency quantities.
- Real NEAR sandbox tests deploy both Wasm contracts, execute all scenarios, compare decisions/full state with the core, reject direct adapter bypass, reject forged settlement/callbacks, and handle failed/unknown execution with reconciliation.
- Local HTTP smoke test executes all five traces and verifies loopback origin/path controls.
- Run formatter, clippy and JavaScript syntax checks.
- Record build/tool versions and observed validation results. Do not label tests as proofs.

## Delivery and verification handoff

Repository must contain runnable source, scripts, locked dependencies, static pages, generated scenario files, API documentation, state-machine spec, security boundaries and formal-verification handoff. README must distinguish sandbox execution, local Rust simulation, static replay and real-chain wallet interaction.

The next verification effort should prove configuration soundness, arithmetic bounds, admission preservation, atomic rejection, dependency correctness, conservative pending envelopes, replay/idempotency, and recipe-specific conservation. SDK identity/promise behavior and external adapter truthfulness remain separate proof/trust boundaries. The earlier approval-contract proofs do not transfer automatically.

No real-money transaction, public deployment or submission is required to complete this implementation. Commit and push the built project to the user-specified repository after checks. Any remaining failed check or omitted integration must be reported precisely.

## References

- https://github.com/difflabai/trust-framework/tree/249f932d84b91fbc6f82abd0668cac6c0503fc42/contracts/approval
- https://github.com/difflabai/formalver
- https://docs.near.org/smart-contracts/anatomy/crosscontract
- https://docs.near.org/smart-contracts/security/cross-contract-calls/callbacks
- https://authors.library.caltech.edu/records/jnhr0-1ww05
- https://www.financialresearch.gov/hedge-fund-monitor/

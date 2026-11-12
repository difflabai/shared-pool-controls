# Shared Pool Controls

**Independent agents. One shared limit.** An MIT-licensed, open-source Rust / NEAR contract for commitments against a shared pool. Built for the **AI Security & Defense** track of NEAR Hackathon, it gives ecosystem developers a contract they can modify, configure, and deploy to address this class of shared-resource problems. Six scenarios use the same core and contract:

1. **Every ship wants to dock** — station power, a protected life-support allocation, and an explicit wait before requesting a later window.
2. **Every team needs the GPU now** — shared compute, protected inference capacity, smaller or later allocations.
3. **Everyone promised Friday** — coupled machine and delivery reservations.
4. **Every branch hoards the scarce part** — shared stock, protected depot reserve, receipt-gated availability.
5. **Every agent hires more agents** — bounded worker funding, review earmarks, independent acceptance.
6. **Everyone is diversified—until they need the same exit** — portfolio stress-loss budgets, collateral, fill-gated hedge credit.

Agents submit an owner-defined recipe and exact integer quantity. The core checks caller authority, freshness, prerequisites, resource bounds and `V(after) <= V(before)`, then atomically reserves the commitment. The execution adapter accepts calls only from the lock. Unknown outcomes retain reservations.

**Status:** implemented prototype with Rust tests and NEAR sandbox integration tests. Not independently audited or deployed to a public network. [Formal verification](https://github.com/difflabai/formalver/pull/31) includes completed Lean model proofs and TLA+ checks; the overall effort is in progress, with Rust-core Kani verification pending and reported settlement/liveness findings unresolved in that PR. Credits, GPU jobs, physical delivery, and financial positions are synthetic. The risk coefficients are illustrative, not a production portfolio model.

## Hackathon review path

Submission details, category, team, and the matching formal verification are in [`nearhackathon.md`](nearhackathon.md).

Track: **AI Security & Defense**. The deliverable is an open-source contract that ecosystem teams can adapt and deploy for shared compute, delivery capacity, inventory, worker funding, and risk budgets. The explorer demonstrates those applications. See the [judge walkthrough](docs/judges-guide.md) for a 90-second demo script and a claim-to-evidence map.

Start the lab below, then click **Begin docking sequence**. Atlas reserves 4 MW and Lyra 2 MW. Vega asks for 4 MW: **WAIT TO DOCK**. The station has 12 MW per window, but 4 MW is protected for life support; docking cannot exceed 8 MW. The denied request leaves 6 MW booked. Click **Request Vega docking in window 02** to submit a fresh request against the second window. The animation reflects Rust decisions, not a JavaScript policy implementation. The original GPU scenario remains available as a separate example of an energy-only denial.

## Run the interactive lab

Install Rust through rustup, Python 3, and optionally Node for JavaScript syntax checks. The repository selects Rust `1.98` and locks dependencies in `Cargo.lock`.

```sh
cargo run -p pool-demo -- serve
# Open http://127.0.0.1:8080
```

The site introduces the shared-resource problem, guides visitors into the six-scenario explorer, then opens up the contract specification, lifecycle and source code. Custom requests and evidence controls are available under expandable sections.

The server binds only to loopback. It serves plain HTML/CSS/JavaScript and evaluates decisions in Rust. There is no frontend build, package manager, database, private key, LLM, or external service requirement. Selecting identities and confirming completion are explicit simulator controls, not production authentication.

Use **Run next decision** to walk the trace; **Preview** and **Submit exact quantity** to explore; **Suggest a smaller quantity** to ask the bounded Rust planner for a suggestion. Unknown outcomes, confirmed nonexecution, duplicate completion, and a risk-market observation can be explored through the lifecycle controls.

The `pages` workflow publishes the UI and recorded traces to GitHub Pages on every push to `main` (enable it once under Settings → Pages → Source: GitHub Actions). The hosted site is a replay with manual submissions disabled.

For a completely static replay, serve the repository root with `python3 -m http.server 8081`, then open `/ui/`. The same page falls back to checked-in traces produced by Rust; manual submissions are disabled and the mode is clearly labeled. Generate them with:

```sh
cargo run -p pool-demo -- export
cargo run -p pool-demo -- trace risk
```

The separate [`ui/live.html`](ui/live.html) is a static NEAR console: read any compatible deployed pool via RPC, preview an exact action, connect a wallet, and explicitly sign submission, dispatch, cancellation or reconciliation. It follows the trust-framework approval UI pattern: no held keys, text-only rendering of chain data, pinned wallet connector, explicit expected code hash before signing. No account is preselected. This signing path is implemented but not exercised against a public wallet/deployment by the automated suite.

## Build and validate

```sh
./scripts/check.sh           # format, Rust tests, clippy, JS syntax
./scripts/build.sh           # both Wasm contracts and build provenance manifest
./scripts/sandbox.sh         # actual Wasm + asynchronous receipts in local NEAR
python3 scripts/smoke.py     # local HTTP demo; start server first
```

`near-workspaces` downloads its sandbox binary on first use. Network access and a supported host are required. `build.sh` builds with `RUSTFLAGS="--cfg near"` and fails if a Wasm does not import `env.panic_utf8` (without the flag, failed calls carry no error name). `artifacts/build.json` records source revision, dirty status, Rust version, Rust flags, Cargo.lock hash, and each Wasm's hash and imports. This is a local build manifest, not an independent reproducibility or deployment attestation.

## Small architecture

| Component | Responsibility |
|---|---|
| [`crates/core`](crates/core) | Dependency-free by default: bounded data, integer potential, admission, reservations and lifecycle |
| [`crates/contract`](crates/contract) | NEAR identity, persisted state, events, promises and authenticated callbacks |
| [`crates/executor`](crates/executor) | Enforced synthetic adapter, exact dispatch binding, independent operator, fault injection |
| [`crates/demo`](crates/demo) | Rust local server, deterministic traces, bounded alternative search |
| [`crates/sandbox`](crates/sandbox) | Wasm integration tests for all six configurations and uncertain outcomes |
| [`ui`](ui) | Static scenario lab and live NEAR console |
| [`scenarios`](scenarios) | Generated configurations and reproducible decision traces |

One pool per deployment; immutable recipes, principals and policy. At most 32 metrics, 32 recipes, 64 potential terms and 256 lifetime commitments. Fixed limits bound computation and storage. The 257th commitment is rejected; rotate to a new deployment rather than pruning replay protection. The owner funds storage. This favors inspection and verification over indefinite production operation.

Read the [API](docs/api.md), [state-machine specification](docs/spec.md), [updated implementation plan](docs/implementation-plan.md), [verification handoff](docs/verification-handoff.md), and [verification findings](docs/verification-findings.md).

## Important boundaries

- Hard bounds and the potential are different checks. A lower score cannot buy permission to breach a protected limit.
- Pending commitments count immediately. Pending negative-risk hedge effects do not provide headroom before fill.
- No unexecuted position backs a hedge: a hedge must stay in bounds if the position is later reported unexecuted.
- Cancellation after dispatch needs authenticated evidence of nonexecution. A timeout or failed receipt is insufficient.
- The administrator and registered execution/evidence accounts are trusted. The owner can update observed facts only when there are no unresolved commitments, and can restate the current values at any time to renew the snapshot; it cannot edit policy. That observation can raise energy or reveal a limit breach.
- Only admitted reservation transitions promise nonincreasing energy. Settlement, cancellation, observations, fairness, economic correctness and eventual completion have separate assumptions.
- The adapter boundary is real in the sandbox. Real brokers, cloud providers or warehouses would need enforceable integrations with no alternate credentials/path around the gate.
- The prototype uses no real escrow token. Hiring credits are accounting fixtures, not NEAR balances.

The architecture follows [`difflabai/trust-framework`](https://github.com/difflabai/trust-framework/tree/249f932d84b91fbc6f82abd0668cac6c0503fc42/contracts/approval): a pure core and thin wrapper. Its proofs do **not** apply to this code. Dedicated [shared-pool formal verification](https://github.com/difflabai/formalver/pull/31) targets the contract source pinned at `a4bb5c3`: Lean proofs cover the model transcription, TLA+ checks a bounded model, and conformance traces connect the models to the Rust core. Kani verification and refinement work remain pending. The PR reports high-severity settlement/liveness findings F-1 and F-2; consult its current status and findings before deploying. Changes to the contract or policy require review against the proof assumptions.

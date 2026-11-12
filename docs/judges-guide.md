# Shared Pool Controls: judge walkthrough

## Hackathon track and deliverable

This project is built for **AI Security & Defense (`security`)**. It delivers an MIT-licensed, open-source Rust / NEAR contract that ecosystem developers can modify, configure, and deploy to govern shared resources across independent agents. The six scenarios demonstrate applications of the same contract.

The contribution is checking combined consequences, including pending commitments, beyond each agent's individual authority. Teams can define their own resource metrics, recipes, and potential, then integrate an executor that cannot bypass the lock. Review the verification findings and assumptions when adapting or deploying it.

## A 90-second presentation

1. **Arrival (15 seconds):** Three independent ships share a station's power. Life support always retains 4 of the 12 MW available in each docking window.
2. **The conflict (25 seconds):** Click **Begin docking sequence**. Atlas reserves 4 MW, then Lyra 2 MW. Vega's 4 MW request is rejected: **WAIT TO DOCK**. The illustration changes from blue power routes to an amber holding marker. The reserved power remains 6 MW.
3. **A new decision (15 seconds):** Click **Request Vega docking in window 02**. A new request reserves 4 MW in a separate allocation period. Waiting does not create an authorization or silently queue a transaction. Dispatch is a separate lifecycle step.
4. **The real product (35 seconds):** This is an MIT-licensed Rust / NEAR contract the ecosystem can adapt and deploy. Show the same gate in compute or agent hiring, then the contract, evidence, and verification scope. Explain that the station is a synthetic example; its power values are policy fixtures, not telemetry.

The station illustrates a hard resource constraint. The GPU example separately shows a request that fits the hard limit but fails the potential check. Both checks matter; a lower energy score cannot override a protected reserve.

## Reproduce the visible result

From the repository root, with the Rust toolchain installed:

```sh
cargo run -p pool-demo -- serve
```

Open `http://127.0.0.1:8080/` and click **Begin docking sequence**. It resets the station and runs three decisions, pausing briefly between them unless reduced motion is enabled. Expect **WAIT TO DOCK**, 6 / 8 MW docking power, and the protected 4 MW life-support allocation. The explorer records **Bounds**, prospective score 20 → 44, while the committed score stays 20.

Continue to submit Vega's new window-02 request. Expect 4 / 8 MW reserved in window 02, score 20 → 4, and a blue admitted route. Continue once more for Lyra's remaining 2 MW: window 02 reaches 6 MW and the score reaches zero. These are separate allocation periods, not simultaneous use of twice the station's capacity.

For static replay without Rust running:

```sh
python3 -m http.server 8081
```

Open `http://127.0.0.1:8081/ui/`. Expect **Rust-generated replay** and the same decisions from checked-in traces. Custom submissions are disabled. This is not a public-chain transaction. The localhost simulation shares state across tabs; use one tab for a reproduction.

## Evidence and limits

| Claim | Inspect or run | What it does not establish |
|---|---|---|
| Denial preserves state; accepted reservations respect the configured potential | `./scripts/check.sh`; `crates/core`; `scenarios/gpu/trace.json` | Fairness or correctness of the chosen policy |
| Real Wasm contract execution matches the core across six scenarios | `./scripts/sandbox.sh`; `crates/sandbox/tests/integration.rs` | Public-network deployment or production load testing |
| Agents cannot invoke the synthetic executor directly or self-attest settlement | `five_scenarios_execute_through_the_actual_gate` sandbox test | Protection when a real provider offers alternate credentials around the gate |
| Stale requests fail; unknown outcomes retain reservations | `unknown_outcomes_stale_requests_and_reconciliation` sandbox test | Eventual execution, truthful evidence or automatic release on timeout |
| Browser presents Rust results | `python3 scripts/smoke.py` with the local server running | Browser authority over the policy |

See the README for prerequisites. Sandbox setup downloads a binary on first use and needs network access on a supported host. CI runs the checks and sandbox suite; inspect the run for the revision being reviewed rather than assuming a past run covers later changes.

Implemented: pure Rust core, NEAR wrapper, synthetic executor, deterministic scripted agents, static UI, local RPC/wallet console. The console signing path is implemented but has not been exercised with a public wallet/deployment by the automated suite.

Not implemented: public-network deployment, live LLM agents, real escrow/payments, production resource adapters, NEAR Intents integration, TEE/privacy. The earlier approval-contract architecture inspired this structure; its proofs do not transfer. The 256-record window and trusted observation/evidence roles are documented in the README and specification.

## Formal verification

[Formal verification PR #31](https://github.com/difflabai/formalver/pull/31) targets v1.0.0, with contract source pinned at `a4bb5c3`. It reports completed Lean model proofs, bounded TLA+ safety checks, Prolog requirements checks, and conformance traces against the Rust core and NEAR shell. These are substantive verification results, with distinct scopes; they are not a completed proof of every deployed Rust/Wasm behavior.

The new station fixture is not part of that pinned verification revision; it has a dedicated Rust regression test.

The PR is a draft labeled **In-Progress**. Kani verification of the real Rust core and refinement obligations remain pending. It reported high-severity settlement/liveness findings F-1 and F-2 and several lower-severity findings. v1.1.0 fixes all of them (F-4 only for a recipe whose actor is its executor), which changes admission, record retention and the version limit ([findings](verification-findings.md), [changelog](../CHANGELOG.md)). The fixes are tested but not formally re-verified. Present the findings and fixes alongside the proofs, and inspect the PR's current status before making a deployment claim. Modifying the contract or its policy requires reviewing the proof assumptions and rerunning applicable checks.

## Presentation rationale and official sources

Reviewed 2026-11-12:

- [Tracks and judging FAQ](https://nearhackathon.com/): the project participates in AI Security & Defense. The site's prior-event criteria are preparation guidance, not a published weighted rubric for this event.
- [Organizer guide](https://nearhackathon.com/HACKATHON.md), Q-011, Q-012, Q-017 and demo guidance: make reproduction explicit, show an action and result, and identify NEAR's actual role. A local-only submission needs a video demo, not screenshots alone.
- [Security track example](https://github.com/nearai/near-hackathon-prompt-examples/blob/main/tracks/06-security.md): use enforceable restrictions and visible rejection evidence. This project demonstrates shared-state overload and bypass rejection; it does not claim a prompt-injection detector.

For the submission, provide the public repository and the required demo post. No submission or social post is made by this UI update. Confirm current organizer requirements before submitting.

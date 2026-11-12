# Shared Pool Controls: judge walkthrough

## Recommended track

**AI Security & Defense (`security`)** is the primary fit: this project enforces a shared-resource policy before an agent can cause execution. Its contribution is checking combined consequences, including pending commitments, beyond each agent's individual authority.

AI Swarms is a secondary thematic fit. Autonomous Business and Agent Economy describe possible applications. The implemented product is the shared policy gate; it does not yet include a coordination market, autonomous business, agent marketplace, or real payment settlement.

Assessment of all published tracks:

| Track | Fit to the implemented prototype |
|---|---|
| AI Security & Defense | Strongest: enforceable policy, bounded authority, rejection and bypass tests. |
| AI Swarms | Secondary: independent agents share state; no bidding market or settlement. |
| Autonomous Business | Application fit: procurement, capacity, budgets; no operating autonomous business. |
| Agent Economy | Application fit: worker hiring and compute; credits are synthetic, not paid commerce. |
| Unstoppable DeFi | Risk-control example only; no exchange, lending, vault or cross-chain settlement. |
| Social Money | Shared budgets are adjacent; no community ownership or DAO integration. |
| Real-World Money | No payroll, invoice or stablecoin payment rails. |
| Privacy & Private AI | No confidential computation or private inference. |
| Investment Thesis | No investment thesis or onchain valuation research. |
| Games With Real Economies | No game loop. |
| NFTs That Do Something | No NFT implementation. |
| Memecoins With Mechanics | No token mechanic. |
| Open Innovation | Broadly possible; the security track is more specific. |

This is our recommendation, not a submitted track selection. Track integration suggestions and example prompts are optional under Q-011 of the organizer guide.

## A 90-second presentation

1. **Problem (15 seconds):** “Three agents can each have permission to book compute and still overload the same shared window. Permission alone does not account for the combined effect.”
2. **Visible result (30 seconds):** Click **See a request blocked**. Research reserves 4 and Evaluation 2. Data asks for 4: the hard ceiling permits 10, but the configured energy would rise from 20 to 44. The request is denied; bookings remain 6. Click **Run next decision**. Data explicitly requests 2 instead; energy falls from 20 to 8 and bookings reach 8.
3. **Mechanism and NEAR (20 seconds):** Explain that the contract checks identity, version, bounds and potential, then reserves before dispatch. The synthetic executor accepts execution only from the lock. A client-side bypass cannot authorize work.
4. **Evidence and scope (25 seconds):** Show the decision response and sandbox tests. Demonstrate that one kernel also models hiring or risk. Clearly distinguish simulation, sandbox execution, and an eventual real provider integration.

Lead with the concrete denial. Introduce “Lyapunov-style potential” after the viewer understands what was prevented. Do not imply mathematical stability of a real economy or formal verification of this implementation.

## Reproduce the visible result

From the repository root, with the Rust toolchain installed:

```sh
cargo run -p pool-demo -- serve
```

Open `http://127.0.0.1:8080/` and click **See a request blocked**. This resets the GPU scenario and runs its first three requests. Expect:

- **Denied · Energy**, prospective score **20 → 44**.
- Research coverage 4, Evaluation coverage 2, Data coverage 0.
- Window 1 bookings 6 and current system energy 20.
- **Run next decision** then shows **Allowed**, score **20 → 8**, Data coverage 2 and bookings 8.

The starting-demo button resets the selected scenario to GPU. The live local server is shared across tabs; use one tab when reproducing this sequence.

For static replay without Rust running:

```sh
python3 -m http.server 8081
```

Open `http://127.0.0.1:8081/ui/`. Expect **Rust-generated replay** and the same decisions from checked-in traces. Custom submissions are disabled. This is not a public-chain transaction.

## Evidence and limits

| Claim | Inspect or run | What it does not establish |
|---|---|---|
| Denial preserves state; accepted reservations respect the configured potential | `./scripts/check.sh`; `crates/core`; `scenarios/gpu/trace.json` | Formal proof, fairness or correctness of the chosen policy |
| Real Wasm contract execution matches the core across five scenarios | `./scripts/sandbox.sh`; `crates/sandbox/tests/integration.rs` | Public-network deployment or production load testing |
| Agents cannot invoke the synthetic executor directly or self-attest settlement | `five_scenarios_execute_through_the_actual_gate` sandbox test | Protection when a real provider offers alternate credentials around the gate |
| Stale requests fail; unknown outcomes retain reservations | `unknown_outcomes_stale_requests_and_reconciliation` sandbox test | Eventual execution, truthful evidence or automatic release on timeout |
| Browser presents Rust results | `python3 scripts/smoke.py` with the local server running | Browser authority over the policy |

See the README for prerequisites. Sandbox setup downloads a binary on first use and needs network access on a supported host. CI runs the checks and sandbox suite; inspect the run for the revision being reviewed rather than assuming a past run covers later changes.

Implemented: pure Rust core, NEAR wrapper, synthetic executor, deterministic scripted agents, static UI, local RPC/wallet console. The console signing path is implemented but has not been exercised with a public wallet/deployment by the automated suite.

Not implemented: public-network deployment, live LLM agents, real escrow/payments, production resource adapters, NEAR Intents integration, TEE/privacy, or formal verification. The earlier approval-contract architecture inspired this structure; its proofs do not transfer. The fixed lifetime record bound and trusted observation/evidence roles are documented in the README and specification.

## Presentation rationale and official sources

Reviewed 2026-11-12:

- [Tracks and judging FAQ](https://nearhackathon.com/): security is the closest product category. The site's prior-event criteria are preparation guidance, not a published weighted rubric for this event.
- [Organizer guide](https://nearhackathon.com/HACKATHON.md), Q-011, Q-012, Q-017 and demo guidance: make reproduction explicit, show an action and result, and identify NEAR's actual role. A local-only submission needs a video demo, not screenshots alone.
- [Security track example](https://github.com/nearai/near-hackathon-prompt-examples/blob/main/tracks/06-security.md): use enforceable restrictions and visible rejection evidence. This project demonstrates shared-state overload and bypass rejection; it does not claim a prompt-injection detector.

For the submission, provide the public repository and the required demo post. No submission or social post is made by this UI update. Confirm current organizer requirements before submitting.

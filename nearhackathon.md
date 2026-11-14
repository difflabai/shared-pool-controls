# NEAR Hackathon submission

This repository is a submission to the **November 2026 NEAR Hackathon**.

| | |
|---|---|
| **Category** | AI Security & Defense |
| **Project** | Shared Pool Controls: an MIT-licensed Rust / NEAR contract that keeps independent agents' combined commitments to a shared resource within an owner-defined policy |
| **Submission** | This repository, [difflabai/shared-pool-controls](https://github.com/difflabai/shared-pool-controls), together with its formal verification (below) |
| **Formal verification** | **Part of this submission:** the matching verification of this contract in [difflabai/formalver PR #32](https://github.com/difflabai/formalver/pull/32), which verifies v1.1.0 (`57b3f02`) |
| **Live site** | [difflabai.github.io/shared-pool-controls](https://difflabai.github.io/shared-pool-controls/): a static replay of the scenarios, published from this repository with GitHub Pages |
|**Post** |[Post with video](https://x.com/difflabai/status/2121739327755682137) |
|**Plain Video**|[On Youtube](https://youtu.be/ty3DLq-3Xn0)|

## What is submitted

- The Rust core, the NEAR lock contract, and the synthetic executor adapter in [`crates`](crates).
- Six scenarios (station power, GPU compute, delivery capacity, inventory, worker budgets, and risk headroom) with Rust-generated traces in [`scenarios`](scenarios).
- A static explorer and NEAR console in [`ui`](ui), plus a guide for configuring and deploying the contract against your own resource contention problem.
- Formal verification of v1.1.0 in the companion PR above; it is part of the submission and should be evaluated with it. It includes Lean model proofs, TLA+ checks, Kani on the Rust core, and conformance traces. Verification of v1.0.0 reported settlement/liveness and other findings; v1.1.0 fixes them ([findings](docs/verification-findings.md)).

The [judge walkthrough](docs/judges-guide.md) has a 90-second demo script and a claim-to-evidence map.


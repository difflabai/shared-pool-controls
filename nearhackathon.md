# NEAR Hackathon submission

This repository is a submission to the **November 2026 NEAR Hackathon**.

| | |
|---|---|
| **Category** | AI Security & Defense |
| **Project** | Shared Pool Controls: an MIT-licensed Rust / NEAR contract that keeps independent agents' combined commitments to a shared resource within an owner-defined policy |
| **Submission** | This repository, [difflabai/shared-pool-controls](https://github.com/difflabai/shared-pool-controls) |
| **Formal verification** | The matching verification work in [difflabai/formalver PR #31](https://github.com/difflabai/formalver/pull/31) |
| **Live site** | Static replay of the scenarios, published from this repository with GitHub Pages |

## What is submitted

- The Rust core, the NEAR lock contract, and the synthetic executor adapter in [`crates`](crates).
- Six scenarios (station power, GPU compute, delivery capacity, inventory, worker budgets, and risk headroom) with Rust-generated traces in [`scenarios`](scenarios).
- A static explorer and NEAR console in [`ui`](ui), plus a guide for configuring and deploying the contract against your own resource contention problem.
- Formal verification of the contract, in progress, in the companion PR above. It includes Lean model proofs and TLA+ checks; Rust-core Kani verification is pending and that PR reports settlement/liveness findings that are not yet resolved.

The [judge walkthrough](docs/judges-guide.md) has a 90-second demo script and a claim-to-evidence map. The contract is not independently audited and has not been deployed to a public network.

## Team membership

The hackathon rules allow a person to be a member of up to two teams. [gabehamilton](https://github.com/gabehamilton) is a member of the team for this submission and also a member of the team for the [gabehamilton/invest-in-near](https://github.com/gabehamilton/invest-in-near) submission.

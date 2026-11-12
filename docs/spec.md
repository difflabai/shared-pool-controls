# Core specification

This document specifies the implemented prototype, not a claim of completed formal verification.

## State and identity

`Pool = (config, values, version, observed_at, paused, records)`.

Config is immutable. It contains one owner, bounded named metrics, potential terms and recipes. Each recipe has one exact actor account, one exact executor account, reservation effects, completion effects, a maximum quantity and earlier-recipe completion prerequisites `(recipe_id, minimum_completed_units)`. The dependency order is acyclic by construction.

Both initializers are private to their own deployment account, preventing initialization by an unrelated caller. The NEAR wrapper authorizes by immediate predecessor account, not a JSON actor field and not `signer_account_id`. Calls through an intermediary are attributed to that intermediary. Account owners and deployed account code are part of the trust boundary. Preview accepts a proposed actor but conveys no authorization.

Metric values and coefficients lie in [-1,000,000, 1,000,000]. Every update is checked. Each potential term uses a residual of at most 2,000,000, weight <= 1,000. Up to 64 terms sum to at most 256,000,000,000,000,000, safely below u64::MAX. Energy is serialized as a decimal string. All exposed metric integers are within JavaScript's exact range. Request expiry is an epoch-seconds value; the UI constructs a short-lived integer.

## Admission

Input: `(caller, recipe, units, expected_version, expires_at, now)`.

Check paused, version equality, expiry strictly after now and at most one day ahead, observation freshness if configured, record bound, recipe validity, exact caller, quantity range, and completed-unit prerequisites. Compute candidate values by adding the recipe's reservation delta times units, with checked arithmetic.

For each metric compute a conservative interval containing every subset of outstanding completion effects, plus the proposed completion effect. Independent positive effects contribute to the high endpoint and negative effects to the low endpoint. Check this interval against the configured hard bound. In a healthy state, all intervals must fit. In a disturbed state, no metric's violation amount may exceed its current violation.

Then require V(candidate) <= V(current). In a state already violating hard bounds also require V(candidate + proposed_completion) < V(current). This allows a recovery hedge to reserve collateral while its beneficial stress effect waits for fill. This is only a recovery admission rule; it is not a guarantee that execution arrives or recovery completes.

On success append a Reserved record, replace values, increment version. On rejection return an error with no state change. The wrapper may emit a rejection event but does not consume a nonce or persist an audit record. Accepted requests cannot replay at the original version. A new version constitutes a new deliberate request; there is no separate client nonce mechanism.

`preview` calls the same checker and returns candidate values, score terms and reason. It is advisory. `best_quantity` is an offchain bounded enumeration of positive quantities up to the requested maximum. It minimizes energy then favors larger quantity. There is no global optimality claim. It only suggests; the caller explicitly submits the changed action.

## Potential

Each term references one metric and computes a weighted squared target deviation, excess, or deficit. Multiple-resource sums are represented by owner-maintained aggregate metrics with recipe coefficients. Every fixture is a policy over the entire instance. Individual agents cannot supply deltas or invent targets. A legitimate but badly designed administrator recipe can still model the world incorrectly.

Hard constraints are axis-aligned metric bounds. Coupled constraints use additional aggregate metrics (machine bookings, dispatch slots, escrow, collateral). This finite design deliberately excludes arbitrary policy programs and nonlinear portfolio models.

## Lifecycle

- Reserved -> Dispatched: exact actor, unpaused, unexpired, fresh risk snapshot; wrapper creates the promise after state update.
- Dispatched/Unknown -> Active: private acknowledgment says adapter accepted dispatch.
- Dispatched/Unknown -> Unknown: missing, malformed or failed acknowledgment; retain values.
- Dispatched/Active/Unknown -> Completed: recipe executor attests completion; add completion delta, check remaining completion envelope, increment version. No energy comparison is imposed on externally attested settlement.
- Reserved -> Cancelled: actor or owner cancels before dispatch. Subtract reservation delta and check other pending effects.
- Dispatched/Active/Unknown -> Cancelled: executor attests definitive nonexecution; same checked inverse update.
- Same completion/cancellation repeated: idempotent. Conflicting terminal evidence: reject.
- Late acknowledgment after terminal settlement: ignored.

A checked inverse can fail if another commitment depends on its contribution. The original commitment remains encumbered; resolve dependent work first. There is no forced administrative release that bypasses these checks. In the supplied fixtures, physical availability and risk-credit timing prevent ordinary unsafe reliance on unexecuted work.

No automatic expiry of a dispatched request. A deadline does not prove nonexecution. Reconciliation queries the fixed executor by immutable ID. Unknown is distinct from cancelled. An executed but unacknowledged job remains a liability.

The mock adapter keeps a bounded immutable association between ID and recipe/quantity. Duplicate dispatch with identical payload returns the prior outcome; a different payload rejects. Only the lock can execute. Only the independent operator can finish/cancel a job. It stores terminal evidence before notifying the lock so a failed notification can be reconciled later. True end-to-end exactly-once physical execution is not claimed.

## External observations and administration

`set_paused` is owner-only and increments version. Pausing blocks new admissions and dispatch, not settlement or reconciliation. `observe` is owner-only and allowed only when every record is terminal. It replaces bounded measured values and refreshes the snapshot timestamp, without changing policy or deleting history. It can create a hard-limit violation. This is a trusted external fact assertion, not an energy-checked trade.

No policy upgrades, role mutation, arbitrary funds transfer, dynamic plugins, or reset entry point. Deployment-account full-access keys can still replace code; verify deployment upgrade authority separately. The owner is not thereby assumed powerless.

## Scenario semantics

GPU/Friday: coverage and occupancy include reserved and completed bookings; completion does not free historical slots. New scheduling horizons require a new instance or an owner observation after all work settles. The demo uses two named static windows, not a production wall-clock scheduling calendar. Friday also consumes dispatch slots atomically.

Inventory: planning positions move at reservation. Physical source availability decreases and reserved/in-transit quantity increases. Receipt moves transit into destination availability. Destination incoming position is never available stock. `available_depot + available_A + available_B + transit = 12` across ordinary transfer transitions in the fixture. The combined transit metric abstracts reserved-vs-shipped phases.

Hiring: reservation consumes discretionary funds or a review earmark and increases coverage plus escrow. Independent acceptance moves escrow to lifetime spending. Coverage remains after payment. `free + earmark + escrow + paid + protected = 10`. Review needs completed research units >=2 and implementation units >=3. No worker can authorize its own acceptance through the wrapper.

Risk: values are conservative scenario losses in $100k units. Positive loss contributions reserve immediately; hedge benefit occurs only on completion. Collateral encumbrance remains until an explicit later measured baseline. There is no automatic sale/close model or partial fill support. Each mock trade is an atomic synthetic position. No pending hedge offsets are netted into new trade capacity.

## Intended proof boundary

Model fixed config validation, integer arithmetic, pure transition atomicity and bindings first. Separately justify wrapper caller mapping, promise callback provenance, persistent serialization, SDK/runtime transaction behavior, adapter honesty, recipe conservation and model adequacy. Tests establish examples and bounded enumerations only. See verification-handoff.md.

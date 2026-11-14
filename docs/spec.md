# Core specification

This document describes v1.1.0 of the implemented prototype. It is not a claim of completed formal verification. An outside verification checked v1.0.0; v1.1.0 fixes its findings and has not been re-verified ([verification findings](verification-findings.md), [changelog](../CHANGELOG.md)).

## State and identity

`Pool = (config, values, version, observed_at, paused, records, first_id, completed_units)`.

`records` is a window of at most 256 records, oldest first, with consecutive ids: `records[k].id = first_id + k`. Ids never repeat. `completed_units[r]` is the number of units of recipe r that have completed over the pool's life, pruned records included.

Config is immutable. It contains one owner, bounded named metrics, potential terms and recipes. Each recipe has one exact actor account, one exact executor account distinct from the actor, reservation effects, completion effects, a maximum quantity and earlier-recipe completion prerequisites `(recipe_id, minimum_completed_units)`. The dependency order is acyclic by construction.

Both initializers are private to their own deployment account, preventing initialization by an unrelated caller. The NEAR wrapper authorizes by immediate predecessor account, not a JSON actor field and not `signer_account_id`. Calls through an intermediary are attributed to that intermediary. Account owners and deployed account code are part of the trust boundary. Preview accepts a proposed actor but conveys no authorization.

Metric values and coefficients lie in [-1,000,000, 1,000,000]. Every update is checked. Each potential term uses a residual of at most 2,000,000, weight <= 1,000. Up to 64 terms sum to at most 256,000,000,000,000,000, safely below u64::MAX. Energy is serialized as a decimal string. All exposed metric integers are within JavaScript's exact range. Request expiry is an epoch-seconds value; the UI constructs a short-lived integer.

## Admission

Input: `(caller, recipe, units, expected_version, expires_at, now)`.

Check paused, version equality, expiry strictly after now and at most one day ahead, observation freshness if configured, record bound (`Full` only when 256 records are retained and the oldest is open), recipe validity, exact caller, quantity range, and completed-unit prerequisites (`completed_units`). Compute candidate values by adding the recipe's reservation delta times units, with checked arithmetic.

For each metric compute a conservative interval that contains every outcome of every open record, plus the proposed one. Each record may stay open, complete (add its completion effect) or be released (subtract its reservation effect), independently of the others. Positive effects contribute to the high endpoint and negative effects to the low endpoint. Check this interval against the configured hard bound. No metric's interval violation may exceed its violation before the request or its violation after the reservation. In a healthy state, all intervals must fit.

Then require V(candidate) <= V(current). In a state already violating hard bounds also require V(candidate + proposed_completion) < V(current). This allows a recovery hedge to reserve collateral while its beneficial stress effect waits for fill. A reservation never changes a metric's violation amount; recovery comes from completion. This is only a recovery admission rule; it is not a guarantee that execution arrives or recovery completes.

On success append a Reserved record with id `first_id + len(records)`, replace values, increment version. If 256 records are retained, first drop the oldest, which is terminal, and increment `first_id`. A pruned id is `NotFound` for every call; its history stays in the event log. On rejection return an error with no state change. The wrapper may emit a rejection event but does not consume a nonce or persist an audit record. Accepted requests cannot replay at the original version. A new version constitutes a new deliberate request; there is no separate client nonce mechanism.

`preview` calls the same checker and returns candidate values, score terms and reason. It is advisory. `best_quantity` is an offchain bounded enumeration of positive quantities up to the requested maximum. It minimizes energy then favors larger quantity. There is no global optimality claim. It only suggests; the caller explicitly submits the changed action.

## Potential

Each term references one metric and computes a weighted squared target deviation, excess, or deficit. Multiple-resource sums are represented by owner-maintained aggregate metrics with recipe coefficients. Every fixture is a policy over the entire instance. Individual agents cannot supply deltas or invent targets. A legitimate but badly designed administrator recipe can still model the world incorrectly.

Hard constraints are axis-aligned metric bounds. Coupled constraints use additional aggregate metrics (machine bookings, dispatch slots, escrow, collateral). This finite design deliberately excludes arbitrary policy programs and nonlinear portfolio models.

## Lifecycle

- Reserved -> Dispatched: exact actor, unpaused, unexpired, fresh risk snapshot; wrapper creates the promise after state update.
- Dispatched/Unknown -> Active: private acknowledgment says adapter accepted dispatch.
- Dispatched/Unknown -> Unknown: missing, malformed or failed acknowledgment; retain values. Unknown -> Unknown is an Ok no-op without a version change (`reconcile` is permissionless).
- Dispatched/Active/Unknown -> Completed: recipe executor attests completion; add completion delta, check the envelope of the remaining open records, add the units to `completed_units`, increment version. No energy comparison is imposed on externally attested settlement.
- Reserved -> Cancelled: actor or owner cancels before dispatch. Subtract reservation delta and check other pending effects. A cancel of a Dispatched, Active or Unknown record returns `Pending`: the executor's evidence is pending. A cancel of a Completed record returns `InvalidState`.
- Dispatched/Active/Unknown -> Cancelled: executor attests definitive nonexecution; same checked inverse update.
- Same completion/cancellation repeated: idempotent, Ok without a version change. Conflicting terminal evidence: reject with `InvalidState`.
- Late acknowledgment after terminal settlement: ignored, Ok without a version change.

Admission refuses any commitment whose outcomes could make a metric worse than now, so the checked inverse cannot fail in a reachable state. There is still no forced administrative release.

Version reserve: submit (as its last check, so `preview` reports it), dispatch, `set_paused` and `observe` fail `Arithmetic` once version reaches `VERSION_CAP = u32::MAX - 768`. Acknowledgment, settlement and cancellation keep the plain checked increment. An open record needs at most three more increments (Dispatched -> Unknown -> Active -> terminal) and there are at most 256 records, so these never overflow. At the cap the pool admits nothing new but every open record can still be resolved.

No automatic expiry of a dispatched request. A deadline does not prove nonexecution. Reconciliation queries the fixed executor by immutable ID. Unknown is distinct from cancelled. An executed but unacknowledged job remains a liability.

The mock adapter keeps a bounded immutable association between ID and recipe/quantity. Duplicate dispatch with identical payload returns the prior outcome; a different payload rejects. It forgets a job once its id is 256 or more behind the newest id the lock has vouched for: one for which the lock's `execute` created a job, or one whose `settle` from this executor it accepted (the private callback `on_settled` reads the lock's answer). An `execute` of an id that already has a job returns the stored status and moves nothing. The lock held that record then, so it has pruned every id 256 or more below; such a record is terminal. `execute` of a forgotten id starts nothing and returns `"cancelled"`; `confirm_not_executed` of one fails. A production adapter may forget an id only once it is 256 behind an id the lock dispatched or accepted a settle for. Only the lock can execute. Only the independent operator can finish/cancel a job. It stores terminal evidence before notifying the lock so a failed notification can be reconciled later. True end-to-end exactly-once physical execution is not claimed.

## External observations and administration

`set_paused` is owner-only and increments version. Pausing blocks new admissions and dispatch, not settlement or reconciliation. `observe` is owner-only. It sets new values only when every record is terminal; restating the current values is allowed at any time and renews the snapshot timestamp. It replaces bounded measured values and refreshes the snapshot timestamp, without changing policy. New values can create a hard-limit violation. This is a trusted external fact assertion, not an energy-checked trade.

No policy upgrades, role mutation, arbitrary funds transfer, dynamic plugins, or reset entry point. Deployment-account full-access keys can still replace code; verify deployment upgrade authority separately. The owner is not thereby assumed powerless.

## Scenario semantics

GPU/Friday: coverage and occupancy include reserved and completed bookings; completion does not free historical slots. New scheduling horizons require a new instance or an owner observation after all work settles. The demo uses two named static windows, not a production wall-clock scheduling calendar. Friday also consumes dispatch slots atomically.

Inventory: planning positions move at reservation. Physical source availability decreases and reserved/in-transit quantity increases. Receipt moves transit into destination availability. Destination incoming position is never available stock. `available_depot + available_A + available_B + transit = 12` across ordinary transfer transitions in the fixture. The combined transit metric abstracts reserved-vs-shipped phases.

Hiring: reservation consumes discretionary funds or a review earmark and increases coverage plus escrow. Independent acceptance moves escrow to lifetime spending. Coverage remains after payment. `free + earmark + escrow + paid + protected = 10`. Review needs completed research units >=2 and implementation units >=3. No worker can authorize its own acceptance through the wrapper.

Risk: values are conservative scenario losses in $100k units. Positive loss contributions reserve immediately; hedge benefit occurs only on completion. Collateral encumbrance remains until an explicit later measured baseline. There is no automatic sale/close model or partial fill support. Each mock trade is an atomic synthetic position. No pending hedge offsets are netted into new trade capacity. No unexecuted position backs a hedge: a hedge must stay in bounds if the position is later reported unexecuted.

## Intended proof boundary

Model fixed config validation, integer arithmetic, pure transition atomicity and bindings first. Separately justify wrapper caller mapping, promise callback provenance, persistent serialization, SDK/runtime transaction behavior, adapter honesty, recipe conservation and model adequacy. Tests establish examples and bounded enumerations only. See verification-handoff.md.

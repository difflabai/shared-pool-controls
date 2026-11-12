# NEAR API

This page describes v1.1.0. Changes from v1.0.0 are listed in the [changelog](../CHANGELOG.md).

One immutable pool per deployment. All mutating methods are nonpayable, including `reconcile`; fund storage by transferring NEAR to the deployment account separately. The read methods (`get_pool`, `get_commitments`, `preview`, and the executor's `status` and `get_jobs`) are meant for view calls. They have no deposit check, so one sent as a transaction keeps any attached deposit; do not attach one. Local demonstration credits have no relationship to attached deposits.

| Method | Arguments | Authority / result |
|---|---|---|
| `new` | `{config}` | Private initializer: only the deployment account itself. One-time init. |
| `get_pool` | `{}` | Entire bounded state: immutable policy, the 256 most recent records, `first_id` (id of the oldest retained record) and `completed_units` (per recipe, over the pool's life). |
| `get_commitments` | `{from,limit}` | Read page, max 50, from id `from`. Ids below `first_id` were pruned; a page starting there begins at `first_id`. |
| `preview` | `{actor,request}` | Advisory Decision; actor is only a hypothetical identity. |
| `submit` | `{request}` | Predecessor must equal recipe actor. Returns `{decision,id}`; no id on rejection. |
| `dispatch` | `{id}` | Recipe actor; Reserved -> Dispatched and calls fixed executor. |
| `reconcile` | `{id}` | Anyone may query the fixed executor; private callback applies authentic result. |
| `settle` | `{id,completed}` | Only that recipe's executor; false means definitive nonexecution, never unknown. |
| `cancel` | `{id}` | Actor or owner; only undispatched Reserved records. Dispatched, Active or Unknown: `Pending`. Completed: `InvalidState`. Already Cancelled: Ok, no change. |
| `set_paused` | `{paused}` | Owner; blocks admission and dispatch. |
| `observe` | `{values}` | Owner; new values only when every record is terminal. Restating the current values is allowed at any time and renews the snapshot timestamp. May raise energy. |
| `on_result` | `{id}` plus runtime promise result | Private callback, not a public evidence submission endpoint. It never panics on its input: a result for an id with no record changes nothing and is logged (`reconciled` with `error: "NotFound"`). |

Request example:

```json
{"recipe":0,"units":4,"version":0,"expires_at":1794500000}
```

A submit prunes the oldest retained record when 256 are retained and it is terminal; while it is open, submit fails `Full`, even if the other 255 are terminal. A Reserved oldest record can be cancelled by its actor or the owner. A Dispatched, Active or Unknown one closes only when its executor reports (`settle`, or a `reconcile` that returns `completed` or `cancelled`), so an executor that never reports keeps submit at `Full`. Every call on a pruned id fails `NotFound`, including a repeated executor `settle` (v1.0.0 returned Ok). The event log keeps pruned records.

Use the current version from `get_pool` and an expiry 10 minutes ahead; the example timestamp is illustrative. Maximum is 24 hours. `preview` and `submit` return the same decision structure. If version changed, reload and re-preview. Every effective state transition increments version by exactly 1. Calls that change nothing return Ok and leave the version alone: a repeated terminal settlement, a repeated cancel, an acknowledgment that does not change the status, a late acknowledgment. Submit, dispatch, pause and observe stop 768 versions below u32::MAX (`Arithmetic`); acknowledgment, settlement and cancellation always remain available.

Decision fields: `allowed`, `reason`, decimal-string `before` and optional `after`, decimal-string per-term scores, candidate metric vector. Reasons include Unauthorized, Paused, Stale, Expired, SnapshotExpired, InvalidRecipe, InvalidUnits, Prerequisite, Full, Arithmetic, Bounds, Energy. `after` can be present for a rejected request to explain its prospective score; it is not accepted state.

Events are logged as `EVENT_JSON:{"standard":"shared_pool","version":"1.0.0","event":...,"data":[...]}`. The kinds are `decision` (every submit, admitted or not), `reconciled`, `settled`, `cancelled`, `paused` and `observed`. `version` is the event format version, not the package version. It stays `1.0.0` in v1.1.0 because no event shape changed; `reconciled` can now carry `error: "NotFound"`.

There is no `dampen` mutation: a caller previews an explicit reduced quantity and signs a new exact request. Date changes and substitutions use separate recipes, never silent terms changes.

## Executor protocol

`execute({id,recipe,units}) -> "active" | "cancelled" | "unknown"`. Only the lock may call. `status({id})` additionally returns `"completed"`. In the wrapper, any result that is not one JSON string of at most 64 bytes becomes Unknown: a runtime failure, a number, `null`, an object, invalid bytes, a longer result. So does an unexpected string. Fixed executor identity plus immutable recipe/quantity bind dispatch to the reservation. A production adapter must implement idempotency, authoritative cancellation and fill semantics.

The synthetic adapter has `new({lock,operator})`, `set_mode({mode})`, `finish({id,completed})`, `confirm_not_executed({id,recipe,units})`, and `get_jobs({})`. Modes normal/reject/unknown/panic support fault injection. These are testing features, not a real asset gateway. It forgets a job once its id is 256 or more behind the newest id it has executed for the lock. `execute` of a forgotten id returns `"cancelled"` and starts nothing; `confirm_not_executed` of one fails `stale id`, and `finish` fails `unknown job`. It holds at most 512 jobs. The cap is reached only after at least 256 operator confirmations of ids above the newest executed id. Then `confirm_not_executed` fails `full` until an `execute` of an id at least as high as the confirmed ones; after that it holds at most 256 jobs. An `execute` of a lower id forgets fewer jobs and may leave the executor full. While it is full, an `execute` that fails before reaching it leaves its record Unknown, and that record cannot be confirmed. If that goes on until the lock holds 256 records, the oldest of them is such a record and none is Reserved, then submit fails `Full`, nothing can be dispatched, and no new `execute` can arrive; only a `settle` signed by the executor account's own key resolves those records. A production adapter needs its own retention rule.

## Gas

Attach at least this much prepaid gas. The figures were measured on the v1.1 Wasm in near-sandbox 2.13.4, at a maximum-size state: 32 metrics, 64 terms, 32 recipes (one requires the other 31), and 256 retained records, all open. For `finish` the executor held 256 jobs, and then 512, its cap. Each figure is the least whole Tgas that succeeded, plus at least 10 Tgas, rounded up.

| Call | Gas it attaches to its promises | Least that succeeded | Attach |
|---|---|---|---|
| `dispatch` | `execute` 30 + `on_result` 30 | 70 Tgas | 100 Tgas |
| `reconcile` | `status` 10 + `on_result` 30 | 46 Tgas | 60 Tgas |
| executor `finish`, `confirm_not_executed` | the lock's `settle` 40 | 43 Tgas; `finish` with 512 jobs 44 | 60 Tgas |
| `submit`, `cancel`, `settle`, `observe`, `set_paused` | none | largest receipt 14.81 Tgas (a `submit` with 255 open records) | 30 Tgas |

- Every smaller whole-Tgas amount from 40 Tgas up failed with "Exceeded the prepaid gas." and changed neither contract.
- The gas attached to promises is fixed; extra prepaid gas is not passed on. At that state `on_result` burnt at most 10.92 of its 30 Tgas (a release), `execute` 2.45 of 30, `status` 1.75 of 10, and the lock's `settle` after `finish` 10.87 of 40. With 512 jobs, `execute` burnt 3.64 of 30 and `status` 2.29 of 10.
- A call with too little gas fails without changing either contract; retry with more gas. A lost executor notification is recovered with `reconcile`.
- The contracts do not check prepaid gas. Smaller states need less. Another build or protocol version can change these figures.
- The measuring test is `crates/sandbox/tests/res1.rs`, adapted from the outside verification's RES-1 sandbox test. It takes about eight minutes, so `scripts/sandbox.sh` skips it. Run it after `scripts/build.sh` with `cargo test -p pool-sandbox --locked --test res1 -- --ignored --nocapture`; it writes `artifacts/res1.json`. `confirm_not_executed` was swept with 254 jobs; with 512 jobs its executor receipt burnt 3.96 Tgas, the same as `finish` (3.95).

## Deployment preparation

1. Build with `scripts/build.sh`; retain Wasm and manifest. It sets `RUSTFLAGS="--cfg near"`, the flag `cargo near` sets, so a failed call reports its error name (for example `Smart contract panicked: Pending`). Without the flag near-sdk drops every panic message. The script fails if either Wasm does not import `env.panic_utf8`, or imports a host function for an action other than a function call (deploy, keys, transfer, stake, accounts, global contracts, any `promise_batch_*`). It also fails on any import that is not in its list of the 18 contract and 13 executor imports, so a new host function is reviewed before it ships. Any other build of the deployed Wasm must also set `--cfg near`.
2. Create/fund a lock account, executor account, owner/operator account and agent accounts on your selected network using your normal NEAR tooling.
3. Copy a generated `scenarios/*/config.json`; replace owner, all actor identities and executor identities with those accounts. A recipe's actor and executor must be different accounts; `new` rejects the config otherwise (`InvalidConfig`). Keep policy coefficients unchanged unless deliberately designing a new policy.
4. Deploy pool_contract.wasm and pool_executor.wasm. Initialize each contract from its own deployment account (private initializer), setting the configured owner/operator and the lock ID. Prefer an atomic deploy-and-initialize transaction. Use fresh accounts: v1.1.0 stores more state than v1.0.0 in both contracts, so it cannot replace v1.0.0 code in place, and no migration is provided.
5. Verify code hashes, config, funding and account access keys. Load `ui/live.html` with your account and expected code hash.
6. Preview/submit as the recipe actor; dispatch; independently confirm through the executor. Never put full-access keys in the static UI.

Deployment commands depend on installed NEAR CLI version; sandbox integration demonstrates the exact JSON interfaces without requiring a funded external account. This project does not create or modify a public deployment automatically.

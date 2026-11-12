# NEAR API

One immutable pool per deployment. All mutating methods are nonpayable; fund storage by transferring NEAR to the deployment account separately. Local demonstration credits have no relationship to attached deposits.

| Method | Arguments | Authority / result |
|---|---|---|
| `new` | `{config}` | Private initializer: only the deployment account itself. One-time init. |
| `get_pool` | `{}` | Entire bounded state, including immutable policy and up to 256 records. |
| `get_commitments` | `{from,limit}` | Read page, max 50. |
| `preview` | `{actor,request}` | Advisory Decision; actor is only a hypothetical identity. |
| `submit` | `{request}` | Predecessor must equal recipe actor. Returns `{decision,id}`; no id on rejection. |
| `dispatch` | `{id}` | Recipe actor; Reserved -> Dispatched and calls fixed executor. |
| `reconcile` | `{id}` | Anyone may query the fixed executor; private callback applies authentic result. |
| `settle` | `{id,completed}` | Only that recipe's executor; false means definitive nonexecution, never unknown. |
| `cancel` | `{id}` | Actor or owner; only undispatched Reserved records. |
| `set_paused` | `{paused}` | Owner; blocks admission and dispatch. |
| `observe` | `{values}` | Owner; no unresolved commitments. May raise energy. |
| `on_result` | `{id}` plus runtime promise result | Private callback, not a public evidence submission endpoint. |

Request example:

```json
{"recipe":0,"units":4,"version":0,"expires_at":1794500000}
```

Use the current version from `get_pool` and an expiry 10 minutes ahead; the example timestamp is illustrative. Maximum is 24 hours. `preview` and `submit` return the same decision structure. If version changed, reload and re-preview. Every successful state transition changes version; terminal duplicate settlement is idempotent.

Decision fields: `allowed`, `reason`, decimal-string `before` and optional `after`, decimal-string per-term scores, candidate metric vector. Reasons include Unauthorized, Paused, Stale, Expired, SnapshotExpired, InvalidRecipe, InvalidUnits, Prerequisite, Full, Arithmetic, Bounds, Energy. `after` can be present for a rejected request to explain its prospective score; it is not accepted state.

There is no `dampen` mutation: a caller previews an explicit reduced quantity and signs a new exact request. Date changes and substitutions use separate recipes, never silent terms changes.

## Executor protocol

`execute({id,recipe,units}) -> "active" | "cancelled" | "unknown"`. Only the lock may call. `status({id})` additionally returns `"completed"`. Runtime failures and unexpected strings become Unknown in the wrapper. Fixed executor identity plus immutable recipe/quantity bind dispatch to the reservation. A production adapter must implement idempotency, authoritative cancellation and fill semantics.

The synthetic adapter has `new({lock,operator})`, `set_mode({mode})`, `finish({id,completed})`, `confirm_not_executed({id,recipe,units})`, and `get_jobs({})`. Modes normal/reject/unknown/panic support fault injection. These are testing features, not a real asset gateway.

## Deployment preparation

1. Build with `scripts/build.sh`; retain Wasm and manifest. It sets `RUSTFLAGS="--cfg near"`, the flag `cargo near` sets, so a failed call reports its error name (for example `Smart contract panicked: Pending`). Without the flag near-sdk drops every panic message. The script fails if either Wasm does not import `env.panic_utf8`. Any other build of the deployed Wasm must also set `--cfg near`.
2. Create/fund a lock account, executor account, owner/operator account and agent accounts on your selected network using your normal NEAR tooling.
3. Copy a generated `scenarios/*/config.json`; replace owner, all actor identities and executor identities with those accounts. A recipe's actor and executor must be different accounts; `new` rejects the config otherwise (`InvalidConfig`). Keep policy coefficients unchanged unless deliberately designing a new policy.
4. Deploy pool_contract.wasm and pool_executor.wasm. Initialize each contract from its own deployment account (private initializer), setting the configured owner/operator and the lock ID. Prefer an atomic deploy-and-initialize transaction.
5. Verify code hashes, config, funding and account access keys. Load `ui/live.html` with your account and expected code hash.
6. Preview/submit as the recipe actor; dispatch; independently confirm through the executor. Never put full-access keys in the static UI.

Deployment commands depend on installed NEAR CLI version; sandbox integration demonstrates the exact JSON interfaces without requiring a funded external account. This project does not create or modify a public deployment automatically.

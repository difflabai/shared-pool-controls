# shared-pool: TLA+ layer results (v1.1.0)

Pin: difflabai/shared-pool-controls `57b3f02` (v1.1.0, the merge of PR #4;
its tree is the PR head `6fcbd56`'s). Model: `crates/core/src/lib.rs` at the
pin, transcribed guard by guard (anchors `U:<line>` in the modules). The
v1.0.0 model and results (pin `a4bb5c3`; F-1, F-2 and F-2b confirmed by TLC)
are this directory at the `shared-pool-v1.0.0` tag. Tools: TLC 2.19 /
tla2tools 1.7.4, OpenJDK 23, 4 workers (`run.sh` default), macOS arm64, on a
10-core Mac shared with other jobs (times are upper bounds). Run:
`projects/shared-pool/tla/run.sh [safety|safety-1|liveness|trace|executor|mutants]`
(default: safety liveness trace). Run dates 2026-11-13 and 2026-11-14.

Conformance traces (REF-5): the v1.1.0 corpus `conformance/traces/core.txt`
(216 traces, 8,865 calls; trace 210 is an `init_error`) and the matching
`trace2tla.py` and `TraceCheck.{tla,cfg}` (states `s/7`, `init_error`,
the invariant `TInv`) are committed together (the integration commit,
a7781c7). The trace and mutant numbers were first measured on the same
corpus and parser in a scratch tree; both checks were run again at
a7781c7 on 2026-11-14 with the same results (`trace` 9 s, every trace
accepted; `mutants` 6 min 57 s, all 52 killed, each by the check the
table below names).

## What v1.1.0 changed, and where the model has it

| Core change (U: lines) | Model | Properties |
|---|---|---|
| The envelope counts each open record's completion *and* release (U:206-244): low adds min(d, x, 0), high max(d, x, 0), d = complete·u, x = -reserve·u | `LoTerm`, `HiTerm` | INV-RES, INV-HARD, LIVE-ALL |
| `check_bounds` compares with the smaller of the violation now and at the candidate (U:250-266) | `CheckBounds` | INV-RES, HARD-STEP |
| A window of 256: `Full` only if the oldest is open; submit prunes a terminal oldest record; ids `first_id + k`; `index()` (U:287-289, 360-391) | `firstId`, `WinFull`, `Found`/`Pos`/`Rec`, `Submit`'s prune | WIN-1, WIN-2, WIN-ESC, WIN-LIVE |
| Prerequisites read `completed_units`, a lifetime count that saturates (U:301-305, 519) | `doneU`, `DoneCap` | INV-DEP, DEP-1, WIN-1 |
| `VERSION_CAP = u32::MAX - 3·MAX_RECORDS`; `bump_new` for submit, dispatch, set_paused, observe (U:24, 319, 395-406) | `VersionCap`, `CapErr` | VER-CAP, INV-VERCAP |
| `validate`: actor ≠ executor (U:130) | `ValidCfg` | CFG-2 |
| cancel of a Completed record is `InvalidState` (U:467) | `CancelErr` | TERM-1 |
| observe of the current values is Ok with records open (U:538) | `ObserveErr`, ghost `base`/`k0` | OBS-2, REFRESH-1 |

Findings: F-1, F-2 and F-2b are gone (LIVE-ALL and `NoWedge` hold in
every reachable state; `MC_Liveness_F1` and `MC_Liveness_F2` now hold);
F-3 (lock window) is WIN-1, WIN-2, WIN-ESC and WIN-LIVE; F-4 is CFG-2
(trace 210 is refused); F-5 is VER-CAP and INV-VERCAP (`MC_Cap`); F-6 is
OBS-2 and REFRESH-1; the cancel divergence is TERM-1. Each new property
fails on the v1.0.0 model (section "v1.0.0 evidence").

## Model

| File | Contents |
|---|---|
| `SharedPool.tla` | The Pool as variables (`cfg`, `values`, `version`, `observedAt`, `paused`, `records`, `firstId`, `doneU`; config is a variable no action writes, so one TLC run replays every trace). Record ids are implicit: `records[k]` has id `firstId + k - 1`. Ghost: `base`, `k0` (the values and the first uncounted id at the last re-basing observe; a pruned counted record is folded into `base`), `steps`, `last`. One error operator per entry point gives the first failing guard in code order, `"Noop"` for the core's idempotent `Ok(())`, or `"Ok"`; the action is enabled exactly when it is `"Ok"`. `GuardOrder` lists the exits (C-2 compares it with the Prolog guard table). `InitAt(c, t0, v0)` is `Pool::new` at version `v0` (0, or near the cap in `MC_Cap`). |
| `Props.tla` | State invariants (`InvRel`, view-safe: INV-WF, INV-LEDGER, INV-RES, INV-HARD, INV-DEP, INV-VER, TERM-1, LIVE-ALL, LIVE-1, `NoWedge`, WIN-ESC, WIN-2, REFRESH-1; `InvAbs`: INV-VERCAP, WIN-1's `firstId + NR <= version`; `InvOutcomes`: INV-HEALTHY over every outcome combination); step properties (`StepProperty`: config fixed, TX-VER, VER-CAP, LIFE-1 with the prune, STALE-1, AUTH-1, ADM-1, ADM-2, HARD-STEP, DEP-1, `completed_units`, ASYNC-1, OBS-1, OBS-2, TX-SUBMIT, TX-DISPATCH, PAUSE-1); witnesses (TLA-WIT). Properties restate the conditions they rely on (`FullCond`, `InFlightSpec`, and the envelope as the hull `HullLo`/`HullHi`: each open record's term min/max(d, x, 0)) instead of reusing the model's operators (`WinFull`, `LoTerm`/`HiTerm`), so a mutant of the operator is checked against the statement. |
| `MCCommon.tla` | Four configurations. `MCConfig` (v1.0's, the F-1 wedge scaled down: m0 ∈ [0,2], m1 ∈ [-2,2], target term on m1; r0 hedge, r1 requiring r0 × 1; max_age 1). `CreditHedgeConfig` (F-2, the `risk` fixture's credit and hedge scaled down). `RecoveryConfig` (one hedge reserving on m1 and completing on m0: the admitted recovery submit, `WitStep_Recovery`; the trace 214/215 config scaled down). `F2bConfig` (F-2b: m0 ∈ [-10^6, 10^6], A reserves -10^6, B completes +10^6 × 2). Accounts o, a1, x1, z. Observation sets. The two `F2bConfig` witnesses (`Witness_EdgeValue`: a value at exactly ±MAX_VALUE; `Witness_F2bRefused`: with A open and the values at A's reservation, B × 2 is refused with `Arithmetic`). |
| `MC_Safety.{tla,cfg}` | TLA-SAFE on `MCConfig`, MaxRecords 2, times {0,1,2} per call, expiry offsets {0,1,2,86400,86401}, 14 observation vectors (`AllObservations`). `VIEW` (header argument): values, observedAt, paused, records, doneU, base, `k0 - firstId`, `Min(firstId, 1)`, `version - steps`. `MC_Safety_1.cfg`: the same at MaxRecords 1 (smoke). `MC_Recovery.cfg`, `MC_F2b.cfg`: the same module on `RecoveryConfig` and `F2bConfig`. |
| `MC_Cap.{tla,cfg}` | TLA-CAP: `Pool::new` at version `VersionCap - 4`, no VIEW, `InvCapMC` (everything, absolute invariants included) and `StepProperty`. Every behaviour reaches the cap. |
| `MC_Liveness.tla` + 7 cfgs | LIVE-2 and WIN-LIVE on `MC_Cap`'s behaviours (finite: calls that start work stop at the cap), under named fairness assumptions (section LIVE-2). |
| `TraceCheck.{tla,cfg}`, `trace2tla.py` | REF-5: `trace2tla.py` (python3 -I, no deps) writes `build/TraceData.tla`; TLC replays every trace in one run and checks `Inv` and `StepProperty` on every core state and step; with the W10 patch, for the `init_error` trace it checks `~ValidCfg(cfg)` (CFG-1, CFG-2). |
| `Executor.tla`, `MC_Executor_W{2,3}.cfg` | TLA-EXEC (section Executor). |
| `mutants/*.patch` | 46 core-model mutants (01-28, 30, 32-48) and 6 executor mutants (49-54); the `# checks:` header names the checks that must kill each. |
| `run.sh` | The runner. |

**Scaled constants (C-5).** `MaxRecords` (MAX_RECORDS = 256): 2 in
`MC_Safety`, `MC_Cap`, `MC_Recovery`, `MC_F2b` and the liveness configs
(`MC_Liveness_Win2` included), 1 in `MC_Safety_1` and `MC_Liveness_Win`, 256 in TraceCheck (trace 207 fills
the window and prunes three records). `MaxVersion` (u32::MAX): 2^31 - 1
everywhere, so `VersionCap` = 2^31 - 1 - 3·MaxRecords (2^31 - 7 at
MaxRecords 2, 2^31 - 769 in TraceCheck). `DoneCap` (u32::MAX): 1 in the
small universes, where every prerequisite asks for 1 unit, so the
saturated count decides every prerequisite as the exact count does; 2^31 -
1 in TraceCheck. `MAX_VALUE`, `MAX_UNITS`, `MAX_WEIGHT`, the 86,400 s
window and the other limits are the real values.

**TLC integer range (ASM-MODEL).** TLC integers are 32-bit signed and TLC
stops with `Overflow` instead of wrapping, so an out-of-range computation
fails a run, it cannot pass one silently. Consequences: (1) `MaxVersion` is
2^31 - 1; only `MC_Cap` and the liveness configs reach `VersionCap`, and no
run reaches `MaxVersion` itself (INV-VERCAP keeps the plain bump below it).
(2) Request versions ≥ 2^31 in the traces (7 submits carry u32::MAX) are
mapped to -1 by `trace2tla.py`; the model reads a request version only
through `v = version`, and every state version in the traces is ≤ 518, so
the outcome (`Stale`) is unchanged. (3) Energy `w·r²` is exact only below
2^31; the traces keep it there (as at v1.0.0: trace 206 reaches values of
10^6 with an energy residual of at most 10). Energy at large residuals
(NUM-1) is Kani's job.

**Abstractions.** Names and units of metrics and recipes are not modelled
(the core only checks their lengths). Numbers are exact integers. Block
time is an argument of each call, chosen freely (a superset of monotone
time, ASM-TIME). The envelope's partial sums are monotone from a bounded
candidate (each low term ≤ 0, each high term ≥ 0), so the code's per-step
range check fails exactly when a final endpoint leaves ±MAX_VALUE, which is
what `CheckBounds` tests.

**VIEW (MC_Safety).** `version`, `steps` and `firstId` grow without
bound (pause and observe bump forever; every prune advances `firstId`).
The model reads `version` only through `v = version`, the cap and bump
checks (false in every explored state: `VersionCap` = 2^31 - 7) and
`OtherVersion` (stale either way), and reads `firstId` only through ids
relative to it, so two states with the same view have the same successors
up to a shift of `version`, `steps`, `firstId` and `last`. `InvRel` reads
ids and versions only relative to each other or through the cap; INV-VER
reads `version - steps`. The absolute invariants (INV-VERCAP, WIN-1's
`firstId + NR <= version`) run in `MC_Cap`, the liveness configs and
TraceCheck, which have no VIEW. Step properties and step witnesses are
checked on every transition, also into a state whose view TLC has seen.

## Results

| Check (`run.sh` name) | ID | Result | Size / time |
|---|---|---|---|
| `safety` (MC_Safety.cfg): the `InvMC` invariants (INV-WF, INV-LEDGER, INV-RES, INV-HARD, INV-DEP, INV-VER, TERM-1, LIVE-ALL, LIVE-1, `NoWedge`, WIN-ESC, WIN-2, REFRESH-1, INV-HEALTHY) and `StepProperty`; `MCConfig`, MaxRecords 2, VIEW | TLA-SAFE | no error | 435,342 distinct views (12,091,340 generated), depth 20, 186 s |
| `safety-1` (MC_Safety_1.cfg): the same at MaxRecords 1 (smoke; opt-in, used by the mutants) | TLA-SAFE | no error | 11,222 distinct, depth 16, 5 s |
| `cap` (MC_Cap.cfg): the `InvCapMC` invariants (the above plus INV-VERCAP and WIN-1's `firstId + NR <= version`) and `StepProperty` from `Pool::new` at `VersionCap - 4`, no VIEW | TLA-CAP | no error | 64,644 distinct, depth 9, 28 s |
| `recovery` (MC_Recovery.cfg): `InvMC`, `StepProperty` on `RecoveryConfig` | TLA-SAFE | no error | 92,444 distinct, depth 13, 26 s |
| `f2b` (MC_F2b.cfg): `InvMC`, `StepProperty` on `F2bConfig` (values at ±10^6) | TLA-SAFE (F-2b) | no error | 30,904 distinct, depth 16, 8 s |
| 20 witnesses: `Witness_Completed`, `_Prerequisite`, `_Disturbed`, `_Prune`, `_FullOpenFront`, `_CancelCompletedInvalid`, `_SubmitRejectedHull`, `WitStep_CancelledAfterDispatch`, `_UnknownToActive`, `_Refresh`, `_PrereqAfterPrune`, `_PausedSettle/Ack/Cancel/Observe` (MC_Safety.cfg); `WitStep_Recovery` (MC_Recovery.cfg); `Witness_EdgeValue`, `Witness_F2bRefused` (MC_F2b.cfg); `Witness_AtCap`, `WitStep_ResolveAtCap` (MC_Cap.cfg) | TLA-WIT | each violated, as required | traces of 2-9 states, 1-6 s each |
| `liveness` (MC_Liveness.cfg): LIVE-2, completion path, `MCConfig` | TLA-LIVE-1 | no error | 7,856 distinct, 4 s |
| `liveness-f2-done` (MC_Liveness_F2Done.cfg): the same on `CreditHedgeConfig` | TLA-LIVE-1 | no error | 5,202 distinct, 3 s |
| `liveness-unfair` (MC_Liveness_Unfair.cfg): LIVE-2 without fairness | TLA-LIVE-1 non-vacuity | violated, as required | 2 s |
| `liveness-f1` (MC_Liveness_F1.cfg): disturbing observes, WF of settling either way | TLA-LIVE-F1 | no error (v1.0.0: violated, F-1) | 7,918 distinct, 6 s |
| `liveness-f2` (MC_Liveness_F2.cfg): `CreditHedgeConfig`, WF of `settle(false)` and of cancel | TLA-LIVE-F2 | no error (v1.0.0: violated, F-2) | 5,202 distinct, 4 s |
| `liveness-win` (MC_Liveness_Win.cfg): `WinLive`, MaxRecords 1, WF of resolving the oldest record | WIN-LIVE | no error | 1,868 distinct, 2 s |
| `liveness-win2` (MC_Liveness_Win2.cfg): the same at MaxRecords 2 (Full = two records, the oldest open) | WIN-LIVE | no error | 8,228 distinct, 4 s |
| `trace` (TraceCheck): 216 traces, 8,865 calls, every result and post-state equal; `Inv` and `StepProperty` on every core state; trace 210's config refused by `ValidCfg` | REF-5 (also TX-0, TX-NOOP, CFG-1, CFG-2) | every trace accepted | 9,081 of 9,081 states (216 initial + 8,865 steps), 9 s |
| `executor` (Executor.tla) | TLA-EXEC | no error; 12 witnesses reachable | section Executor |
| Inductive invariant (Apalache) | TLA-IND | not done | open item, below |
| Spec mutants (`mutants/*.patch`) | | all 52 killed (46 core, 6 executor; 29 and 31 retired) | 6 min 57 s (a7781c7) |

The configs list each invariant on its own `INVARIANT` line (in the order
above), so a failing run names the property. Each run's distinct-state
count is the same in every run. The default `run.sh` (safety liveness
trace) took 5 min 8 s in the scratch tree on 2026-11-14 (02:21-02:27;
MC_Safety 181 s, the rest under 30 s each), the mutants 7 min 2 s there
and 6 min 57 s at a7781c7; the times in the table are from single runs on
a shared machine.

## LIVE-2 and LIVE-ALL

At v1.0.0 LIVE-2 ("every dispatched record eventually ends") depended on
what the executor was assumed to do: it held on the completion path, and
failed with a disturbing observe (F-1) and with an executor fair only to
the honest `settle(false)` (F-2). At v1.1.0 LIVE-ALL holds as an invariant
in every reachable state of every universe here (including `MC_Cap` at and
above the cap): a Reserved record can be cancelled by its actor and the
owner, and an in-flight record can be settled either way by its executor.
So every fairness assumption that makes some resolving call happen gives
LIVE-2:

- **TLA-LIVE-1** (`liveness`, `liveness-f2-done`): observes in bounds, the
  executor weakly fair to completing each record. Holds in `MCConfig` and
  `CreditHedgeConfig`. `liveness-unfair` (no fairness) must and does fail.
- **TLA-LIVE-F1** (`liveness-f1`): disturbing observes allowed (the F-1
  setting), the executor weakly fair to settling either way. Holds; at
  v1.0.0 the F-1 wedge violated it.
- **TLA-LIVE-F2** (`liveness-f2`): `CreditHedgeConfig`, the executor weakly
  fair to `settle(false)` only, callers weakly fair to cancelling. Holds;
  at v1.0.0 the F-2 wedge violated it. The hedge that relied on the
  credit's reservation is now refused at submit (its envelope counts the
  credit's release).
- **WIN-LIVE** (`liveness-win` at MaxRecords 1 and `liveness-win2` at
  MaxRecords 2, `AllObservations`): under
  weak fairness of resolving the oldest record (cancel, or settle either
  way), a Full window is eventually not Full.

v1.0.0's `MC_Liveness_F1Shape` and `MC_Liveness_F2Shape` (every
counterexample is the wedge) are removed: there is no counterexample to
classify.

## v1.0.0 evidence

The four properties v1.1.0's fixes add were stated in v1.0.0's vocabulary
(record index = id; no `firstId`, cap or prune) and checked once on the
v1.0.0 model, in a temporary worktree of the v1.0.0 verification branch
(`a072a2d`; module `V10Checks.tla` extending its `MC_Liveness`, not
committed). Each fails there, as required:

| Property | v1.0.0 universe | Result | Counterexample |
|---|---|---|---|
| LIVE-ALL | `MCConfig`, disturbing observes, MaxRecords 2 | violated (3 states) | observe [3, 2] (m0 above max), submit hedge r0 × 1 (values [2, 2]): cancelling the Reserved hedge fails `Bounds` |
| `NoWedge` | the same | violated (4 states) | the same, then dispatch: both settles fail `Bounds` (the F-1 wedge) |
| REFRESH-1 | `MCConfig`, observes in bounds | violated (2 states) | one submit; observe of the current values is `Pending` (F-6) |
| WIN-LIVE | `MCConfig`, MaxRecords 1, WF of resolving record 0 | violated (temporal) | one submit fills the window; it is cancelled, and `Full` (the count alone) holds forever (F-3) |

The same model's `MC_Liveness_F1` and `MC_Liveness_F2` (now must-hold)
are violated at the `shared-pool-v1.0.0` tag (F-1, F-2).

## Error coverage of the traces (exact counts, v1.1.0 corpus)

Counts of `op:result` over the 8,865 calls (`noop` = the core's `Ok` with
the recorded state unchanged, the model's `"Noop"`):

| op | results |
|---|---|
| submit | bounds 854, ok 585, paused 378, unauthorized 217, prerequisite 149, stale 143, expired 118, energy 98, invalid_recipe 98, invalid_units 90, snapshot_expired 60, arithmetic 3, full 1 |
| dispatch | not_found 915, invalid_state 228, ok 149, paused 70, unauthorized 63, expired 40, snapshot_expired 5 |
| ack | not_found 587, noop 334, ok 70 |
| cancel | not_found 355, ok 338, noop 51, pending 46, unauthorized 42, invalid_state 34 |
| settle | not_found 1012, invalid_state 366, ok 112, noop 110, unauthorized 74 |
| pause | ok 405, unauthorized 51 |
| observe | ok 411, pending 127, unauthorized 51, bounds 25 |

No cancel or settle fails `Bounds` or `Arithmetic` (v1.0.0: 9 calls,
traces 0, 208, 209); LIVE-ALL says none can. Trace 207 (`window`, 523
calls) prunes three records, admits a prerequisite met only by a pruned
completion, fails `Full` once on an open front, gets `NotFound` for a
pruned id from every record call, and ends with a refresh while three
records are open. Trace 211 has two refreshes; trace 213 an observe with
no record open (the ghost re-base); traces 214/215 the admitted recovery
hedge settled true and false.

Exits of `GuardOrder` no trace reaches: the version cap and the bump
(every state version is ≤ 518; `MC_Cap` reaches the cap, nothing reaches
u32::MAX, INV-VERCAP); submit's recovery add (`next + complete·u` lies in
the envelope `check_bounds` has just accepted); and every `Arithmetic` and
`Bounds` exit of release and `settle(true)` (unreachable from a reachable
state by INV-RES: LIVE-ALL is checked in every state of every run).

## Spec mutants

`run.sh mutants` applies each patch in `mutants/` to a scratch copy of
this directory and requires one of the checks its `# checks:` header names
(tried in that order) to kill it: a TLC error for a model check, an
unreachable witness for a `Witness_*`/`WitStep_*` name, a rejected trace or
`TInv` failure for `trace`. Before the mutants it runs each named check on
the unmutated model, which must pass (each witness reachable); the
executor checks are exempt (their universe has no VIEW and does not finish
on the unmutated model; their baseline is `run.sh executor`). 52 patches:
46 core-model mutants (01-28, 30, 32-48; 32-48 are the FR mutants t32-t48
of strategy section 4.5) and 6 executor mutants (49-54, section Executor).
All 52 are killed (2026-11-14 at the integration commit a7781c7, 6 min
57 s on 4 workers; the first run, in a scratch tree with the same corpus
and parser, had the same killers). Killer = the first check that kills,
with the property TLC names (the configs list each invariant separately).
Where the first property TLC names is not the one strategy.md section 4.5
names for an FR mutant, that property was also checked alone, in one-off
runs ("alone").

| # | Mutant | Killed by |
|---|---|---|
| 01 | submit checks Stale before Paused | `trace` (trace 182, call 6: the error name only) |
| 02 | submit ignores prerequisites | `safety-1`: INV-DEP |
| 03 | the owner may not cancel | `safety-1`: LIVE-ALL |
| 04 | the envelope leaves out every open record | `f2b`: INV-RES (it needs two open records: B × 2 is admitted with A open) |
| 05 | release/settle's envelope does not skip the record itself | `safety-1`: LIVE-ALL |
| 06 | ADM-2 dropped (a violating-state submit need not lower energy) | `trace` (trace 12, call 6) |
| 07 | `acknowledge(id, false)` of an Unknown record bumps again | `cap`: INV-VERCAP |
| 08 | release keeps the reservation | `safety-1`: INV-LEDGER |
| 09 | observe's Pending guard counts in-flight records only | `safety-1`: INV-RES |
| 10 | dispatch accepts now = expires_at | `safety-1`: `StepProperty` |
| 11 | settle of a terminal record is a no-op | `safety-1`: TERM-1 |
| 12 | set_paused does not bump | `safety-1`: INV-VER |
| 13 | Full only above MaxRecords | `safety-1`: `StepProperty` |
| 14 | SnapshotExpired at now = observed_at + max_age | `trace` (trace 120, call 10) |
| 15 | submit tolerates an energy increase of 1 | `safety-1`: `StepProperty` |
| 16 | settle authorizes the actor | `safety-1`: LIVE-ALL |
| 17 | acknowledge of an Unknown record is a no-op | `WitStep_UnknownToActive` unreachable |
| 18 | `check_bounds` ignores the low side | `safety-1`: INV-HARD |
| 19 | a re-basing observe keeps the old ghost base | `safety-1`: INV-LEDGER |
| 20 | settle of an Unknown record is InvalidState | `safety-1`: LIVE-ALL |
| 21 | dispatch does not check the snapshot's age | `safety-1`: `StepProperty` |
| 22 | submit accepts any expiry after now | `safety-1`: `StepProperty` |
| 23 | `add()` rejects exactly ±MAX_VALUE | `Witness_EdgeValue` unreachable |
| 24 | the envelope's endpoint check (Arithmetic) dropped | `Witness_F2bRefused` unreachable |
| 25 | Full one record early | `cap`: WIN-ESC |
| 26 | submit rejects expires_at = now + 86400 | `trace` (trace 206, call 4) |
| 27 | submit accepts expires_at = now + 86401 | `safety-1`: `StepProperty` |
| 28 | dispatch expires one second early | `trace` (trace 206, call 7) |
| 30 | settle(true) returns Ok and writes nothing | `liveness`: LIVE-2 (temporal) |
| 32 | FR-2: no release term in the envelope (v1.0's) | `liveness-f2`: INV-RES (`CreditHedgeConfig`); alone, LIVE-ALL and `NoWedge` also fail |
| 33 | FR-1: compare with the violation now only (v1.0's) | `safety-1`: INV-RES; alone, LIVE-ALL also fails |
| 34 | FR-2 (F-2b): the ±10^6 check covers completion terms only | `Witness_F2bRefused` unreachable (trace 208, call 2, also rejects) |
| 35 | FR-3: Full is the record count (v1.0's) | `safety-1`: WIN-ESC |
| 36 | FR-3: prune without advancing `first_id` | `safety-1`: INV-LEDGER; alone, `StepProperty` (LIFE-1, TX-SUBMIT) also fails |
| 37 | FR-4: prerequisites count retained Completed records (v1.0's) | `WitStep_PrereqAfterPrune` unreachable (trace 207, call 515, also rejects) |
| 38 | FR-3: the ghost does not fold a pruned record into `base` | `safety-1`: INV-WF |
| 39 | FR-5: `validate` allows actor = executor (v1.0's) | `trace`: `TInv` (trace 210, CFG-2) |
| 40 | FR-6: dispatch uses the plain bump | `cap`: `StepProperty` (VER-CAP) |
| 41 | FR-6: set_paused uses the plain bump | `cap`: `StepProperty` (VER-CAP) |
| 42 | FR-6: observe uses the plain bump | `cap`: `StepProperty` (VER-CAP) |
| 43 | FR-6: no version cap in `candidate()` | `cap`: `StepProperty` (VER-CAP) |
| 44 | FR-6: the cap check is strict | `cap`: `StepProperty` (VER-CAP) |
| 45 | FR-7: Pending whenever a record is open (v1.0's) | `safety-1`: REFRESH-1 |
| 46 | FR-7: observe accepts new values with a record open | `safety-1`: INV-RES; alone, `StepProperty` (OBS-2) also fails |
| 47 | FR-7: a refresh re-bases the ghost | `safety-1`: `StepProperty`; alone, INV-LEDGER also fails |
| 48 | FR-8: cancel of a Completed record is Pending (v1.0's) | `safety-1`: TERM-1 |
| 49-54 | executor (section Executor) | `executor-w2-noview`: `InvJobWindow` (49, 51, 52), `NewestOnVouch` (50), `InvConfSettles` (53), `SettleVouch` (54) |

**Retired (v1.0.0 numbers).** 29 weakened TLA-LIVE-1's premise to allow
disturbing observes in `MC_Liveness.cfg` and required LIVE-2 to fail (F-1).
At v1.1.0 LIVE-2 holds with disturbing observes (`MC_Liveness_F1`, same
fairness), so the premise no longer matters and there is no failure to
require. 31 gave `MC_Liveness_F2` weak fairness of settling either way and
required F-2's violation to disappear; at v1.1.0 `MC_Liveness_F2` holds
with `settle(false)` fairness alone, so there is no violation to remove.
v1.0.0's 09 (observe accepts new values with records open) is FR mutant
46; number 09 now names a v1.1.0 guard mutant (observe's Pending guard
counts in-flight records only).

**What only the traces or a witness kill.** 01 and 24/34 change only an
error name; 14, 26 and 28 only reject more calls, which no safety
property forbids; 06's guard is implied by ADM-1 in every universe here
that admits a violating-state submit; 23 and 37 only reject more, and
their witnesses (`Witness_EdgeValue`, `WitStep_PrereqAfterPrune`) are the
behaviours they remove. Mutants 24 and 34 reach exactly the states of the
unmutated model (an endpoint beyond ±10^6 then fails `Bounds`, since it
exceeds every violation the comparator allows), so no state or step
property can kill them: `Witness_F2bRefused` (F-2b's admission is refused
with `Arithmetic`, not `Bounds`) and trace 208 do. Of the checks their headers
name, only `trace` (the v1.1.0 corpus) kills 01, 06, 14, 26, 28 and 39.

## Executor (TLA-EXEC)

`Executor.tla` models the executor's job window against the lock's record
window, with receipts in any order (unit W9b; full notes in the unit's
hand-off). `run.sh executor` checks every invariant and `StepExec` at
WINDOW 2 (`MC_Executor_W2.cfg`, MaxJobs 4, VIEW `ViewRefused`: 3,880,089
distinct views, depth 33, 86 s) and WINDOW 3 (`MC_Executor_W3.cfg`, MaxJobs
6, VIEW `View`: 7,323,777 distinct views, depth 30, 184 s), then its 12
witnesses without the VIEW (all reachable). Mutants 49-54 (two required:
49 `advance` keeps `id + W >= newest`, 50 `on_settled` advances whatever
settle returned) are all killed on the WINDOW 2 universe without the VIEW.
Caveat: the VIEW is a hand-argued quotient, sound if `InvJobWindow` holds,
which the same run checks. `run.sh executor-w2-fullview` (opt-in, about 4
min) cross-checks it without the quotient on a smaller WINDOW 2 universe
(ids 0..4: 13,996,226 distinct states, no error). `InvConfSettles` runs at
WINDOW 2 only. The mutants and witnesses run without the VIEW. Limits: a
bounded universe (ids 0..6), D/A/U collapsed into one in-flight status,
one executor and a lock that never refuses an in-flight record's settle
(relies on LIVE-ALL and INV-VERCAP), no foreign callers, safety only (no
liveness of the cap exit), ASM-GAS. The job-cap refusal (T-JOBCAP) is
reachable (`Witness_JobCapExec`, `Witness_JobCapConfirm`) and is a
documented limit, not a finding.

## Not covered (open items)

- **TLA-IND (open).** No Apalache inductive check (`run.sh apalache`
  prints `skip`): the model's recursive sums and symbolic ranges need a
  rewrite for Apalache 0.47.2. Safety is shown within the MC bounds and on
  the core states of the traces; the every-size counterpart is Lean's.
- **Small universes.** `MC_Safety`: 2 metrics, 1 term (target shape), 2
  recipes, ≤ 2 records, 4 accounts, 3 block times, 14 observation vectors;
  v1.0.0 explored 3 records. `MC_Cap` and liveness: ≤ 2 records (WIN-LIVE
  at 1 and 2), 4 calls that start work before the cap. Other shapes, larger
  configs and the real window are covered only by the traces (configs of
  up to 8 metrics and 6 recipes; trace 207 with 256 records).
- **The prune branch at scale** meets the real code only in trace 207
  (TraceCheck at 256, and Kani's plain `h_ref_traces`).
- **u32::MAX itself.** No run reaches `MaxVersion`; INV-VERCAP is what
  keeps ack, cancel and settle below it, checked in `MC_Cap` from 4 below
  the cap. The core's checked additions of submit (version, id) are
  transcribed but unreachable (WIN-1).
- **Energy at large residuals** (ASM-MODEL).
- **Not modelled.** `preview` and `best_quantity` (RD-PREVIEW, RD-BEST),
  the NEAR shell and promise interleavings beyond "ack and settle are
  independent steps" (ASM-INTERLEAVE); the executor only in `Executor.tla`.
- **Refinement to the code.** The link to the Rust is REF-5 (traces) and
  Kani's per-step refinement (REF-1..3, at `first_id = 0`).

## PROP tags

Tagged `PROP: <ID>` where the layer checks them: TLA-SAFE and TLA-WIT
(`MC_Safety.tla`, its cfgs, `Props.tla`), TLA-CAP (`MC_Cap.tla`, `.cfg`),
TLA-LIVE-1, TLA-LIVE-F1, TLA-LIVE-F2, LIVE-2 and WIN-LIVE
(`MC_Liveness.tla` and its cfgs), REF-5 (`TraceCheck.tla`, `trace2tla.py`),
TX-0 and TX-NOOP (`TraceCheck.tla`), CFG-1 and CFG-2 (`TraceCheck.tla`,
`PROP: CFG-1 CFG-2` on `TInv`), TLA-EXEC (`Executor.tla`), and the
invariants and step properties in `Props.tla`. `run.sh` lists the layer IDs.

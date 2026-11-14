----------------------------- MODULE MC_Safety -----------------------------
(***************************************************************************)
(* TLA-SAFE: every reachable state and step of the MCCommon universe.      *)
(* PROP: TLA-SAFE (with MC_Safety.cfg; MC_Safety_1.cfg is the MaxRecords 1 *)
(* smoke run); TLA-WIT (the runner's witness configs derived from          *)
(* MC_Safety.cfg, MC_Recovery.cfg)                                         *)
(* Times {0, 1, 2}, chosen freely per call (a superset of monotone block   *)
(* time, ASM-TIME); expiry offsets {0, 1, 2, 86400, 86401} reach both      *)
(* Expired branches and the window boundary; AllObservations; MaxRecords   *)
(* = 2 (scaled from 256), so the window fills and prunes; DoneCap 1.       *)
(* SCfg is the universe's configuration (MCConfig; RecoveryConfig and      *)
(* F2bConfig in MC_Recovery.cfg and MC_F2b.cfg).                           *)
(*                                                                         *)
(* VIEW. version, steps and firstId grow without bound (pause and observe  *)
(* bump version forever; every prune advances firstId). The model reads    *)
(* version only in `v = version` (Next offers the current version and one  *)
(* other), in the cap and bump checks, which no explored state reaches     *)
(* (VersionCap = 2^31 - 7), and in OtherVersion's choice of the other      *)
(* version (stale either way). It reads firstId only through ids relative  *)
(* to it (Found, Pos, the fold's k0 <= firstId, the re-base's k0 =         *)
(* firstId + NR, the id domain, which has a pruned id iff firstId > 0),    *)
(* so two states with the same View have the same successors up to         *)
(* version, steps, firstId (shifted alike) and last. View keeps k0 -       *)
(* firstId, Min(firstId, 1) and version - steps, so fingerprinting by View *)
(* is a bisimulation quotient below the cap for every invariant of InvMC   *)
(* (InvRel reads ids and versions only relative to each other or through   *)
(* the cap; INV-VER through version - steps). The absolute invariants      *)
(* (INV-VERCAP, WIN-1's firstId + NR <= version) are checked by MC_Cap,    *)
(* the liveness configs and TraceCheck, which have no VIEW. TLC checks     *)
(* step properties on every transition, also into a state whose view it    *)
(* has seen, so they and the step witnesses see the real last'.            *)
(***************************************************************************)
EXTENDS MCCommon

CONSTANT SCfg

Spec == InitFrom(SCfg, 0) /\ [][Next]_vars

View == <<values, observedAt, paused, records, doneU, base, k0 - firstId, Min(firstId, 1),
          version - steps>>
=============================================================================

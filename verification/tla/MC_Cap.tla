------------------------------- MODULE MC_Cap -------------------------------
(***************************************************************************)
(* TLA-CAP: the version cap (VER-CAP, INV-VERCAP). Pool::new of LCfg at    *)
(* version VersionCap - H (InitAt), so at most H calls that start or       *)
(* administer work (submit, dispatch, pause, observe) can take effect;     *)
(* from the cap on they fail Arithmetic, and ack, cancel and settle use    *)
(* the plain bump up to MaxVersion. No VIEW: every behaviour has finitely  *)
(* many effective steps (at most H gated ones, and each record at most 3   *)
(* more), so the absolute-version invariants (InvAbs: INV-VERCAP, WIN-1)   *)
(* are checked here and in the liveness configs that extend this module.   *)
(* This is the only TLC universe where the cap is reached: VersionCap =    *)
(* MaxVersion - 3 * MaxRecords with MaxVersion = 2^31 - 1 (u32::MAX does   *)
(* not fit TLC's integers; Lean, Prolog and Kani check the real constant). *)
(* With MaxRecords 2 INV-VERCAP is tight: two Dispatched records at the    *)
(* cap need 6 = 3 * 2 more increments.                                     *)
(*                                                                         *)
(* PROP: TLA-CAP (with MC_Cap.cfg)                                         *)
(***************************************************************************)
EXTENDS MCCommon

CONSTANTS
    \* Gated calls possible before the cap.
    H,
    \* The configuration of the universe (MCConfig or CreditHedgeConfig).
    LCfg

CapInit == InitAt(LCfg, 0, VersionCap - H)
CapSpec == CapInit /\ [][Next]_vars
=============================================================================

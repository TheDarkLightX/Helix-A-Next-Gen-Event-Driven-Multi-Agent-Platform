# Helix Proof Review

Mode: proof-refinement for the theorem bodies in `formal/lean/HelixProofs/*.lean`.

Status:

```bash
cd formal/lean
lake build
```

- Checker status: pass
- Placeholder scan: clean for `.lean` files under `formal/lean/HelixProofs`
- ESSO gate status: `formal/models/reasoning/neuro_symbolic_fusion_gate.yaml` verified with `z3,cvc5`

Hard-gate expectations:

- No `sorry`, `admit`, `axiom`, or `unsafe`
- No theorem statement drift during proof repair
- Minimal import surface (`Nat` arithmetic + `omega`)

Current proof targets:

- `HelixProofs.IntelPriority.composePriority_eq_foldDigits`
- `HelixProofs.IntelPriority.tail5_lt_radixPow5`
- `HelixProofs.IntelPriority.composePriority_lt_radixPow6`
- `HelixProofs.IntelPriority.attentionDominates`
- `HelixProofs.Credibility.clampBps_le_max`
- `HelixProofs.Credibility.clampBps_mono`
- `HelixProofs.Credibility.clampBps_le_stepNoisyOr`
- `HelixProofs.Credibility.accumulateNoisyOr_bounds`
- `HelixProofs.Credibility.accumulateNoisyOr_le_succ`
- `HelixProofs.Credibility.attenuateSupport_eq_clampedSupport_of_zeroRejection`
- `HelixProofs.Credibility.attenuateSupport_eq_zero_of_fullRejection`
- `HelixProofs.Credibility.attenuateSupport_antitone_rejection`
- `HelixProofs.Credibility.stepNoisyOr_mono_aggregate`
- `HelixProofs.Credibility.stepNoisyOr_mono_signal`
- `HelixProofs.Credibility.accumulateNoisyOr_mono_aggregate`
- `HelixProofs.Credibility.accumulateNoisyOr_mono_signal`
- `HelixProofs.Credibility.maxBps_le_strictGapThreshold_mul`
- `HelixProofs.Credibility.scaled_gap_from_support_gap`
- `HelixProofs.Credibility.proposalSupportBps_le_corroboratedSupportBps`
- `HelixProofs.Credibility.fusedCredibility_eq_allProposal`
- `HelixProofs.Credibility.fusedCredibility_eq_allCorroborated`
- `HelixProofs.Credibility.fusedCredibility_allCorroborated_ge_allProposal`
- `HelixProofs.Credibility.unresolvedClaimCount_replace_one`
- `HelixProofs.Credibility.accumulateNoisyOr_succ_left`
- `HelixProofs.Credibility.support_replace_one_unresolved_with_corroborated`
- `HelixProofs.Credibility.fusedCredibility_replace_one_unresolved_with_corroborated`
- `HelixProofs.Credibility.fusedCredibility_last_open_no_rejection_eq_proposal_step`
- `HelixProofs.Credibility.fusedCredibility_last_open_no_rejection_eq_corroborated_step`
- `HelixProofs.Credibility.fusedCredibility_last_open_no_rejection_strict_iff`
- `HelixProofs.Credibility.attenuateSupport_strict_of_scaled_gap`
- `HelixProofs.Credibility.attenuateSupport_strict_of_gap_threshold`
- `HelixProofs.Credibility.strictGapThreshold_le_two_of_half_headroom`
- `HelixProofs.Credibility.attenuateSupport_strict_of_two_gap_under_half_headroom`
- `HelixProofs.Credibility.strictGapThreshold_le_three_of_two_thirds_headroom`
- `HelixProofs.Credibility.attenuateSupport_strict_of_three_gap_under_two_thirds_headroom`
- `HelixProofs.Credibility.strictGapThreshold_le_four_of_quarter_headroom`
- `HelixProofs.Credibility.attenuateSupport_strict_of_four_gap_under_quarter_headroom`
- `HelixProofs.Credibility.strictGapThreshold_le_five_of_fifth_headroom`
- `HelixProofs.Credibility.attenuateSupport_strict_of_five_gap_under_fifth_headroom`
- `HelixProofs.Credibility.strictGapThreshold_le_six_of_sixth_headroom`
- `HelixProofs.Credibility.attenuateSupport_strict_of_six_gap_under_sixth_headroom`
- `HelixProofs.Credibility.strictGapThreshold_le_seven_of_seventh_headroom`
- `HelixProofs.Credibility.attenuateSupport_strict_of_seven_gap_under_seventh_headroom`
- `HelixProofs.Credibility.strictGapThreshold_le_eight_of_eighth_headroom`
- `HelixProofs.Credibility.attenuateSupport_strict_of_eight_gap_under_eighth_headroom`
- `HelixProofs.Credibility.strictGapThreshold_le_nine_of_ninth_headroom`
- `HelixProofs.Credibility.attenuateSupport_strict_of_nine_gap_under_ninth_headroom`
- `HelixProofs.Credibility.strictGapThreshold_le_ten_of_tenth_headroom`
- `HelixProofs.Credibility.attenuateSupport_strict_of_ten_gap_under_tenth_headroom`
- `HelixProofs.Credibility.strictGapThreshold_le_eleven_of_eleventh_headroom`
- `HelixProofs.Credibility.attenuateSupport_strict_of_eleven_gap_under_eleventh_headroom`
- `HelixProofs.Credibility.strictGapThreshold_le_twelve_of_twelfth_headroom`
- `HelixProofs.Credibility.attenuateSupport_strict_of_twelve_gap_under_twelfth_headroom`
- `HelixProofs.Credibility.strictGapThreshold_le_thirteen_of_thirteenth_headroom`
- `HelixProofs.Credibility.attenuateSupport_strict_of_thirteen_gap_under_thirteenth_headroom`
- `HelixProofs.Credibility.strictGapThreshold_le_fourteen_of_fourteenth_headroom`
- `HelixProofs.Credibility.attenuateSupport_strict_of_fourteen_gap_under_fourteenth_headroom`
- `HelixProofs.Credibility.strictGapThreshold_le_fifteen_of_fifteenth_headroom`
- `HelixProofs.Credibility.attenuateSupport_strict_of_fifteen_gap_under_fifteenth_headroom`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_eq_proposal_step`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_eq_corroborated_step`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_iff`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_of_scaled_gap`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_of_gap_threshold`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_of_two_gap_under_half_headroom`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_of_three_gap_under_two_thirds_headroom`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_of_four_gap_under_quarter_headroom`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_of_five_gap_under_fifth_headroom`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_of_six_gap_under_sixth_headroom`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_of_seven_gap_under_seventh_headroom`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_of_eight_gap_under_eighth_headroom`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_of_nine_gap_under_ninth_headroom`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_of_ten_gap_under_tenth_headroom`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_of_eleven_gap_under_eleventh_headroom`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_of_twelve_gap_under_twelfth_headroom`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_of_thirteen_gap_under_thirteenth_headroom`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_of_fourteen_gap_under_fourteenth_headroom`
- `HelixProofs.Credibility.fusedCredibility_last_open_fixed_rejection_strict_of_fifteen_gap_under_fifteenth_headroom`

Quality receipt:

- Proof-quality scan:
  - `formal/lean/HelixProofs/IntelPriority.lean`: `S (95/100)`
  - `formal/lean/HelixProofs/Credibility.lean`: `S (100/100)`
- Axiom audit:
  - No custom axioms or placeholders
  - `IntelPriority` theorems depend only on standard Lean foundations (`propext`, `Quot.sound`)
  - `Credibility` theorems now also reduce to standard Lean foundations (`propext`) after narrowing the import/proof surface
- Tier call:
  - `IntelPriority`: S-tier. Theorems expose the mixed-radix invariant directly, tie the spec to the fold encoding, and check cleanly.
  - `Credibility`: S-tier for the current bounded statement surface. The rejection attenuation path is explicitly anti-monotone, the noisy-or kernel is monotone in both aggregate and signal, and the end-to-end scoring surface now includes the fixed-budget endpoint theorem, the stronger one-step replacement theorem, the exact strict-vs-flat boundary on the last-open slice with and without rejection, the raw scaled-gap sufficient condition, the semantic support-gap threshold criterion derived from remaining rejection headroom, and the first fifteen closed-form bands: half-headroom `+2`, two-thirds-headroom `+3`, quarter-headroom `+4`, fifth-headroom `+5`, sixth-headroom `+6`, seventh-headroom `+7`, eighth-headroom `+8`, ninth-headroom `+9`, tenth-headroom `+10`, eleventh-headroom `+11`, twelfth-headroom `+12`, thirteenth-headroom `+13`, fourteenth-headroom `+14`, and fifteenth-headroom `+15`.

Runtime parity notes:

- The matching Rust regression confirms the constructive theorem on the shipped integer kernel: replacing one unresolved proposal with one corroborated claim never decreases `fused_credibility_bps`.
- The property is intentionally recorded as monotone, not strictly increasing, because the bounded integer/noisy-or arithmetic can saturate and produce equal scores at the top end.
- The runtime suite now also carries explicit witnesses for both regimes on the last-open/no-rejection slice: a strict lift away from saturation and a flat `9999 -> 9999` boundary case at saturation.
- The fixed-rejection slice now has both witness tests and a bounded parity check for the new scaled-gap theorem: whenever the attenuated-support gap clears one basis-point bucket, the shipped kernel produces a strict lift.
- The fixed-rejection slice now also has a parity check for the semantic support-gap threshold: if the corroborated step exceeds the proposal step by the minimum gap induced by remaining rejection headroom, the shipped kernel produces a strict lift.
- The half-headroom band is now explicit in both Lean and Rust: when rejection stays at or below `5_000`, a simple `proposalSupport + 2 ≤ corroboratedSupport` condition already guarantees a strict lift.
- The next threshold band is now explicit too: when rejection stays at or below `6_666`, `proposalSupport + 3 ≤ corroboratedSupport` guarantees a strict lift.
- The quarter-headroom band is now explicit too: when rejection stays at or below `7_500`, `proposalSupport + 4 ≤ corroboratedSupport` guarantees a strict lift.
- The fifth-headroom band is now explicit too: when rejection stays at or below `8_000`, `proposalSupport + 5 ≤ corroboratedSupport` guarantees a strict lift.
- The sixth-headroom band is now explicit too: when rejection stays at or below `8_333`, `proposalSupport + 6 ≤ corroboratedSupport` guarantees a strict lift.
- The seventh-headroom band is now explicit too: when rejection stays at or below `8_571`, `proposalSupport + 7 ≤ corroboratedSupport` guarantees a strict lift.
- The eighth-headroom band is now explicit too: when rejection stays at or below `8_750`, `proposalSupport + 8 ≤ corroboratedSupport` guarantees a strict lift.
- The ninth-headroom band is now explicit too: when rejection stays at or below `8_888`, `proposalSupport + 9 ≤ corroboratedSupport` guarantees a strict lift.
- The tenth-headroom band is now explicit too: when rejection stays at or below `9_000`, `proposalSupport + 10 ≤ corroboratedSupport` guarantees a strict lift.
- The eleventh-headroom band is now explicit too: when rejection stays at or below `9_090`, `proposalSupport + 11 ≤ corroboratedSupport` guarantees a strict lift.
- The twelfth-headroom band is now explicit too: when rejection stays at or below `9_166`, `proposalSupport + 12 ≤ corroboratedSupport` guarantees a strict lift.
- The thirteenth-headroom band is now explicit too: when rejection stays at or below `9_230`, `proposalSupport + 13 ≤ corroboratedSupport` guarantees a strict lift.
- The fourteenth-headroom band is now explicit too: when rejection stays at or below `9_285`, `proposalSupport + 14 ≤ corroboratedSupport` guarantees a strict lift.
- The fifteenth-headroom band is now explicit too: when rejection stays at or below `9_333`, `proposalSupport + 15 ≤ corroboratedSupport` guarantees a strict lift.

Next curation target:

- Continue the band decomposition with the `+16` and higher bands, and decide where to stop explicit operator-facing corollaries versus introducing a generic band-schema theorem.

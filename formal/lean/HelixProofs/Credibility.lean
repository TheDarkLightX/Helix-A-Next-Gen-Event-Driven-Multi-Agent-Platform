import Mathlib.Data.Nat.Basic

namespace HelixProofs.Credibility

def maxBps : Nat := 10_000

def clampBps (value : Nat) : Nat :=
  min maxBps value

def stepNoisyOr (aggregate signal : Nat) : Nat :=
  let aggregate := clampBps aggregate
  let signal := clampBps signal
  let remaining := maxBps - aggregate
  let increment := (remaining * signal) / maxBps
  clampBps (aggregate + increment)

def accumulateNoisyOr (aggregate signal count : Nat) : Nat :=
  Nat.rec (clampBps aggregate) (fun _ acc => stepNoisyOr acc signal) count

def attenuateSupport (support rejection : Nat) : Nat :=
  let support := clampBps support
  let rejection := clampBps rejection
  (support * (maxBps - rejection)) / maxBps

def strictGapThreshold (remaining : Nat) : Nat :=
  (maxBps - 1) / remaining + 1

def proposalSupportBps (confidence : Nat) : Nat :=
  (clampBps confidence * 7) / 10

def corroboratedSupportBps (confidence : Nat) : Nat :=
  max 8_500 (clampBps confidence)

def rejectionSupportBps (confidence : Nat) : Nat :=
  max (proposalSupportBps confidence) 6_500

def unresolvedClaimCount (claimCount corroboratedCount rejectedCount : Nat) : Nat :=
  claimCount - corroboratedCount - rejectedCount

def fusedCredibility (claimCount corroboratedCount rejectedCount confidence : Nat) : Nat :=
  let corroboratedSupport := corroboratedSupportBps confidence
  let proposalSupport := proposalSupportBps confidence
  let rejectionSupport := rejectionSupportBps confidence
  let support := accumulateNoisyOr 0 corroboratedSupport corroboratedCount
  let support := accumulateNoisyOr support proposalSupport
    (unresolvedClaimCount claimCount corroboratedCount rejectedCount)
  let rejection := accumulateNoisyOr 0 rejectionSupport rejectedCount
  attenuateSupport support rejection

theorem clampBps_le_max (value : Nat) : clampBps value ≤ maxBps := by
  unfold clampBps
  by_cases h : value ≤ maxBps
  · rw [Nat.min_eq_right h]
    exact h
  · rw [Nat.min_eq_left (Nat.le_of_not_ge h)]

theorem clampBps_id {value : Nat} (h : value ≤ maxBps) : clampBps value = value := by
  unfold clampBps
  exact Nat.min_eq_right h

lemma clampBps_idempotent (value : Nat) : clampBps (clampBps value) = clampBps value := by
  unfold clampBps
  rw [← Nat.min_assoc]
  simp only [Nat.min_self]

theorem clampBps_mono {a b : Nat} (h : a ≤ b) : clampBps a ≤ clampBps b := by
  unfold clampBps
  rcases le_total maxBps a with hMax | hMax
  · have hMax' : maxBps ≤ b := le_trans hMax h
    rw [Nat.min_eq_left hMax, Nat.min_eq_left hMax']
  · rw [Nat.min_eq_right hMax]
    exact Nat.le_min_of_le_of_le hMax h

lemma stepNoisyOr_clamp_aggregate (aggregate signal : Nat) :
    stepNoisyOr (clampBps aggregate) signal = stepNoisyOr aggregate signal := by
  unfold stepNoisyOr
  dsimp
  rw [clampBps_idempotent aggregate]

theorem stepNoisyOr_le_max (aggregate signal : Nat) : stepNoisyOr aggregate signal ≤ maxBps := by
  unfold stepNoisyOr
  dsimp
  exact clampBps_le_max _

theorem clampBps_le_stepNoisyOr (aggregate signal : Nat) :
    clampBps aggregate ≤ stepNoisyOr aggregate signal := by
  unfold stepNoisyOr
  dsimp
  have hStep :
      clampBps (clampBps aggregate) ≤
        clampBps
          (clampBps aggregate + (maxBps - clampBps aggregate) * clampBps signal / maxBps) := by
    exact clampBps_mono
      (a := clampBps aggregate)
      (b := clampBps aggregate + (maxBps - clampBps aggregate) * clampBps signal / maxBps)
      (Nat.le_add_right
        (clampBps aggregate)
        ((maxBps - clampBps aggregate) * clampBps signal / maxBps))
  have hId : clampBps (clampBps aggregate) = clampBps aggregate := by
    unfold clampBps
    exact Nat.min_eq_right (clampBps_le_max aggregate)
  simpa [hId] using hStep

theorem accumulateNoisyOr_le_max (aggregate signal count : Nat) :
    accumulateNoisyOr aggregate signal count ≤ maxBps := by
  induction count with
  | zero =>
      simp [accumulateNoisyOr, clampBps_le_max]
  | succ count ih =>
      simp [accumulateNoisyOr]
      exact stepNoisyOr_le_max _ _

theorem clampBps_le_accumulateNoisyOr (aggregate signal count : Nat) :
    clampBps aggregate ≤ accumulateNoisyOr aggregate signal count := by
  induction count with
  | zero =>
      simp [accumulateNoisyOr]
  | succ count ih =>
      have hStep :=
        clampBps_le_stepNoisyOr (accumulateNoisyOr aggregate signal count) signal
      have hClamp :
          clampBps (accumulateNoisyOr aggregate signal count) =
            accumulateNoisyOr aggregate signal count := by
        exact Nat.min_eq_right (accumulateNoisyOr_le_max aggregate signal count)
      rw [hClamp] at hStep
      simpa [accumulateNoisyOr] using le_trans ih hStep

theorem accumulateNoisyOr_le_succ (aggregate signal count : Nat) :
    accumulateNoisyOr aggregate signal count ≤
      accumulateNoisyOr aggregate signal (count + 1) := by
  have hStep :=
    clampBps_le_stepNoisyOr (accumulateNoisyOr aggregate signal count) signal
  have hClamp :
      clampBps (accumulateNoisyOr aggregate signal count) =
        accumulateNoisyOr aggregate signal count := by
    exact Nat.min_eq_right (accumulateNoisyOr_le_max aggregate signal count)
  rw [hClamp] at hStep
  simpa [accumulateNoisyOr] using hStep

theorem accumulateNoisyOr_bounds (aggregate signal count : Nat) :
    clampBps aggregate ≤ accumulateNoisyOr aggregate signal count ∧
      accumulateNoisyOr aggregate signal count ≤ maxBps := by
  exact ⟨
    clampBps_le_accumulateNoisyOr aggregate signal count,
    accumulateNoisyOr_le_max aggregate signal count
  ⟩

theorem attenuateSupport_eq_clampedSupport_of_zeroRejection (support : Nat) :
    attenuateSupport support 0 = clampBps support := by
  unfold attenuateSupport
  simp [clampBps, maxBps]

theorem attenuateSupport_eq_zero_of_fullRejection (support : Nat) :
    attenuateSupport support maxBps = 0 := by
  unfold attenuateSupport
  simp [clampBps, maxBps]

theorem attenuateSupport_antitone_rejection
    (support rejection₁ rejection₂ : Nat)
    (h : rejection₁ ≤ rejection₂) :
    attenuateSupport support rejection₂ ≤ attenuateSupport support rejection₁ := by
  unfold attenuateSupport
  have hClamp : clampBps rejection₁ ≤ clampBps rejection₂ :=
    clampBps_mono h
  have hRemaining :
      maxBps - clampBps rejection₂ ≤ maxBps - clampBps rejection₁ :=
    Nat.sub_le_sub_left hClamp maxBps
  have hMul :
      clampBps support * (maxBps - clampBps rejection₂) ≤
        clampBps support * (maxBps - clampBps rejection₁) :=
    Nat.mul_le_mul_left _ hRemaining
  exact Nat.div_le_div_right hMul

theorem attenuateSupport_mono_support
    {support₁ support₂ rejection : Nat}
    (h : support₁ ≤ support₂) :
    attenuateSupport support₁ rejection ≤ attenuateSupport support₂ rejection := by
  unfold attenuateSupport
  have hClamp : clampBps support₁ ≤ clampBps support₂ := clampBps_mono h
  have hMul :
      clampBps support₁ * (maxBps - clampBps rejection) ≤
        clampBps support₂ * (maxBps - clampBps rejection) := by
    simpa [Nat.mul_comm] using Nat.mul_le_mul_right (maxBps - clampBps rejection) hClamp
  exact Nat.div_le_div_right (c := maxBps) hMul

lemma div_strict_of_add_le {a b d : Nat} (hd : 0 < d) (hGap : a + d ≤ b) :
    a / d < b / d := by
  rw [Nat.lt_div_iff_mul_lt hd]
  rw [Nat.lt_sub_iff_add_lt]
  have hPred : d - 1 < d := Nat.sub_lt hd (by decide : 0 < 1)
  have hLeft : a / d * d + (d - 1) < a + d := by
    exact Nat.add_lt_add_of_le_of_lt (Nat.div_mul_le_self a d) hPred
  exact lt_of_lt_of_le hLeft hGap

theorem attenuateSupport_strict_of_scaled_gap
    {support₁ support₂ rejection : Nat}
    (hGap :
      clampBps support₁ * (maxBps - clampBps rejection) + maxBps ≤
        clampBps support₂ * (maxBps - clampBps rejection)) :
    attenuateSupport support₁ rejection < attenuateSupport support₂ rejection := by
  unfold attenuateSupport
  exact div_strict_of_add_le (d := maxBps) (by decide) hGap

lemma maxBps_le_strictGapThreshold_mul
    {remaining : Nat}
    (hRemaining : 0 < remaining) :
    maxBps ≤ strictGapThreshold remaining * remaining := by
  unfold strictGapThreshold
  let q := (maxBps - 1) / remaining
  let r := (maxBps - 1) % remaining
  have hDiv : remaining * q + r = maxBps - 1 := by
    simp [q, r, Nat.div_add_mod]
  have hRemLt : r < remaining := by
    simp [r, Nat.mod_lt _ hRemaining]
  have hRemLe : r + 1 ≤ remaining := by
    exact Nat.succ_le_of_lt hRemLt
  have hEq : maxBps = remaining * q + (r + 1) := by
    calc
      maxBps = (maxBps - 1) + 1 := by native_decide
      _ = (remaining * q + r) + 1 := by rw [← hDiv]
      _ = remaining * q + (r + 1) := by ac_rfl
  calc
    maxBps = remaining * q + (r + 1) := hEq
    _ ≤ remaining * q + remaining := by
      exact Nat.add_le_add_left hRemLe _
    _ = (q + 1) * remaining := by
      rw [Nat.add_mul]
      simp [Nat.mul_comm]

lemma scaled_gap_from_support_gap
    {support₁ support₂ remaining : Nat}
    (hRemaining : 0 < remaining)
    (hGap : support₁ + strictGapThreshold remaining ≤ support₂) :
    support₁ * remaining + maxBps ≤ support₂ * remaining := by
  have hThreshold : maxBps ≤ strictGapThreshold remaining * remaining :=
    maxBps_le_strictGapThreshold_mul hRemaining
  have hMul :
      (support₁ + strictGapThreshold remaining) * remaining ≤ support₂ * remaining := by
    exact Nat.mul_le_mul_right remaining hGap
  have hExpand :
      support₁ * remaining + strictGapThreshold remaining * remaining ≤
        support₂ * remaining := by
    simpa [Nat.right_distrib, Nat.add_comm, Nat.add_left_comm, Nat.add_assoc] using hMul
  exact le_trans (Nat.add_le_add_left hThreshold _) hExpand

theorem attenuateSupport_strict_of_gap_threshold
    {support₁ support₂ rejection : Nat}
    (hRemaining : clampBps rejection < maxBps)
    (hGap :
      clampBps support₁ + strictGapThreshold (maxBps - clampBps rejection) ≤
        clampBps support₂) :
    attenuateSupport support₁ rejection < attenuateSupport support₂ rejection := by
  have hRemainingPos : 0 < maxBps - clampBps rejection :=
    Nat.sub_pos_of_lt hRemaining
  have hScaled :
      clampBps support₁ * (maxBps - clampBps rejection) + maxBps ≤
        clampBps support₂ * (maxBps - clampBps rejection) :=
    scaled_gap_from_support_gap hRemainingPos hGap
  exact attenuateSupport_strict_of_scaled_gap hScaled

lemma strictGapThreshold_le_two_of_half_headroom
    {remaining : Nat}
    (hHeadroom : 5_000 ≤ remaining) :
    strictGapThreshold remaining ≤ 2 := by
  unfold strictGapThreshold
  change 9_999 / remaining + 1 ≤ 2
  have hPos : 0 < remaining := lt_of_lt_of_le (by decide : 0 < 5_000) hHeadroom
  have hDivLt : 9_999 / remaining < 2 := by
    rw [Nat.div_lt_iff_lt_mul hPos]
    have hLtTenThousand : 9_999 < 10_000 := by decide
    have hMul :
        10_000 ≤ 2 * remaining := by
      calc
        10_000 = 2 * 5_000 := by decide
        _ ≤ 2 * remaining := Nat.mul_le_mul_left 2 hHeadroom
    exact lt_of_lt_of_le hLtTenThousand hMul
  exact Nat.succ_le_of_lt hDivLt

theorem attenuateSupport_strict_of_two_gap_under_half_headroom
    {support₁ support₂ rejection : Nat}
    (hRejection : clampBps rejection ≤ 5_000)
    (hGap : clampBps support₁ + 2 ≤ clampBps support₂) :
    attenuateSupport support₁ rejection < attenuateSupport support₂ rejection := by
  have hRemaining : clampBps rejection < maxBps :=
    lt_of_le_of_lt hRejection (by decide : 5_000 < maxBps)
  have hThresholdLe : strictGapThreshold (maxBps - clampBps rejection) ≤ 2 := by
    apply strictGapThreshold_le_two_of_half_headroom
    exact Nat.le_sub_of_add_le <|
      calc
        5_000 + clampBps rejection ≤ 5_000 + 5_000 := Nat.add_le_add_left hRejection 5_000
        _ = 10_000 := by decide
  have hGap' :
      clampBps support₁ + strictGapThreshold (maxBps - clampBps rejection) ≤
        clampBps support₂ := by
    exact le_trans (Nat.add_le_add_left hThresholdLe _) hGap
  exact attenuateSupport_strict_of_gap_threshold hRemaining hGap'

lemma strictGapThreshold_le_three_of_two_thirds_headroom
    {remaining : Nat}
    (hHeadroom : 3_334 ≤ remaining) :
    strictGapThreshold remaining ≤ 3 := by
  unfold strictGapThreshold
  change 9_999 / remaining + 1 ≤ 3
  have hPos : 0 < remaining := lt_of_lt_of_le (by decide : 0 < 3_334) hHeadroom
  have hDivLt : 9_999 / remaining < 3 := by
    rw [Nat.div_lt_iff_lt_mul hPos]
    have hLt : 9_999 < 10_002 := by decide
    have hMul : 10_002 ≤ 3 * remaining := by
      calc
        10_002 = 3 * 3_334 := by decide
        _ ≤ 3 * remaining := Nat.mul_le_mul_left 3 hHeadroom
    exact lt_of_lt_of_le hLt hMul
  exact Nat.succ_le_of_lt hDivLt

theorem attenuateSupport_strict_of_three_gap_under_two_thirds_headroom
    {support₁ support₂ rejection : Nat}
    (hRejection : clampBps rejection ≤ 6_666)
    (hGap : clampBps support₁ + 3 ≤ clampBps support₂) :
    attenuateSupport support₁ rejection < attenuateSupport support₂ rejection := by
  have hRemaining : clampBps rejection < maxBps :=
    lt_of_le_of_lt hRejection (by decide : 6_666 < maxBps)
  have hThresholdLe : strictGapThreshold (maxBps - clampBps rejection) ≤ 3 := by
    apply strictGapThreshold_le_three_of_two_thirds_headroom
    exact Nat.le_sub_of_add_le <|
      calc
        3_334 + clampBps rejection ≤ 3_334 + 6_666 := Nat.add_le_add_left hRejection 3_334
        _ = 10_000 := by decide
  have hGap' :
      clampBps support₁ + strictGapThreshold (maxBps - clampBps rejection) ≤
        clampBps support₂ := by
    exact le_trans (Nat.add_le_add_left hThresholdLe _) hGap
  exact attenuateSupport_strict_of_gap_threshold hRemaining hGap'

lemma strictGapThreshold_le_four_of_quarter_headroom
    {remaining : Nat}
    (hHeadroom : 2_500 ≤ remaining) :
    strictGapThreshold remaining ≤ 4 := by
  unfold strictGapThreshold
  change 9_999 / remaining + 1 ≤ 4
  have hPos : 0 < remaining := lt_of_lt_of_le (by decide : 0 < 2_500) hHeadroom
  have hDivLt : 9_999 / remaining < 4 := by
    rw [Nat.div_lt_iff_lt_mul hPos]
    have hLt : 9_999 < 10_000 := by decide
    have hMul : 10_000 ≤ 4 * remaining := by
      calc
        10_000 = 4 * 2_500 := by decide
        _ ≤ 4 * remaining := Nat.mul_le_mul_left 4 hHeadroom
    exact lt_of_lt_of_le hLt hMul
  exact Nat.succ_le_of_lt hDivLt

theorem attenuateSupport_strict_of_four_gap_under_quarter_headroom
    {support₁ support₂ rejection : Nat}
    (hRejection : clampBps rejection ≤ 7_500)
    (hGap : clampBps support₁ + 4 ≤ clampBps support₂) :
    attenuateSupport support₁ rejection < attenuateSupport support₂ rejection := by
  have hRemaining : clampBps rejection < maxBps :=
    lt_of_le_of_lt hRejection (by decide : 7_500 < maxBps)
  have hThresholdLe : strictGapThreshold (maxBps - clampBps rejection) ≤ 4 := by
    apply strictGapThreshold_le_four_of_quarter_headroom
    exact Nat.le_sub_of_add_le <|
      calc
        2_500 + clampBps rejection ≤ 2_500 + 7_500 := Nat.add_le_add_left hRejection 2_500
        _ = 10_000 := by decide
  have hGap' :
      clampBps support₁ + strictGapThreshold (maxBps - clampBps rejection) ≤
        clampBps support₂ := by
    exact le_trans (Nat.add_le_add_left hThresholdLe _) hGap
  exact attenuateSupport_strict_of_gap_threshold hRemaining hGap'

lemma strictGapThreshold_le_five_of_fifth_headroom
    {remaining : Nat}
    (hHeadroom : 2_000 ≤ remaining) :
    strictGapThreshold remaining ≤ 5 := by
  unfold strictGapThreshold
  change 9_999 / remaining + 1 ≤ 5
  have hPos : 0 < remaining := lt_of_lt_of_le (by decide : 0 < 2_000) hHeadroom
  have hDivLt : 9_999 / remaining < 5 := by
    rw [Nat.div_lt_iff_lt_mul hPos]
    have hLt : 9_999 < 10_000 := by decide
    have hMul : 10_000 ≤ 5 * remaining := by
      calc
        10_000 = 5 * 2_000 := by decide
        _ ≤ 5 * remaining := Nat.mul_le_mul_left 5 hHeadroom
    exact lt_of_lt_of_le hLt hMul
  exact Nat.succ_le_of_lt hDivLt

theorem attenuateSupport_strict_of_five_gap_under_fifth_headroom
    {support₁ support₂ rejection : Nat}
    (hRejection : clampBps rejection ≤ 8_000)
    (hGap : clampBps support₁ + 5 ≤ clampBps support₂) :
    attenuateSupport support₁ rejection < attenuateSupport support₂ rejection := by
  have hRemaining : clampBps rejection < maxBps :=
    lt_of_le_of_lt hRejection (by decide : 8_000 < maxBps)
  have hThresholdLe : strictGapThreshold (maxBps - clampBps rejection) ≤ 5 := by
    apply strictGapThreshold_le_five_of_fifth_headroom
    exact Nat.le_sub_of_add_le <|
      calc
        2_000 + clampBps rejection ≤ 2_000 + 8_000 := Nat.add_le_add_left hRejection 2_000
        _ = 10_000 := by decide
  have hGap' :
      clampBps support₁ + strictGapThreshold (maxBps - clampBps rejection) ≤
        clampBps support₂ := by
    exact le_trans (Nat.add_le_add_left hThresholdLe _) hGap
  exact attenuateSupport_strict_of_gap_threshold hRemaining hGap'

lemma strictGapThreshold_le_six_of_sixth_headroom
    {remaining : Nat}
    (hHeadroom : 1_667 ≤ remaining) :
    strictGapThreshold remaining ≤ 6 := by
  unfold strictGapThreshold
  change 9_999 / remaining + 1 ≤ 6
  have hPos : 0 < remaining := lt_of_lt_of_le (by decide : 0 < 1_667) hHeadroom
  have hDivLt : 9_999 / remaining < 6 := by
    rw [Nat.div_lt_iff_lt_mul hPos]
    have hLt : 9_999 < 10_002 := by decide
    have hMul : 10_002 ≤ 6 * remaining := by
      calc
        10_002 = 6 * 1_667 := by decide
        _ ≤ 6 * remaining := Nat.mul_le_mul_left 6 hHeadroom
    exact lt_of_lt_of_le hLt hMul
  exact Nat.succ_le_of_lt hDivLt

theorem attenuateSupport_strict_of_six_gap_under_sixth_headroom
    {support₁ support₂ rejection : Nat}
    (hRejection : clampBps rejection ≤ 8_333)
    (hGap : clampBps support₁ + 6 ≤ clampBps support₂) :
    attenuateSupport support₁ rejection < attenuateSupport support₂ rejection := by
  have hRemaining : clampBps rejection < maxBps :=
    lt_of_le_of_lt hRejection (by decide : 8_333 < maxBps)
  have hThresholdLe : strictGapThreshold (maxBps - clampBps rejection) ≤ 6 := by
    apply strictGapThreshold_le_six_of_sixth_headroom
    exact Nat.le_sub_of_add_le <|
      calc
        1_667 + clampBps rejection ≤ 1_667 + 8_333 := Nat.add_le_add_left hRejection 1_667
        _ = 10_000 := by decide
  have hGap' :
      clampBps support₁ + strictGapThreshold (maxBps - clampBps rejection) ≤
        clampBps support₂ := by
    exact le_trans (Nat.add_le_add_left hThresholdLe _) hGap
  exact attenuateSupport_strict_of_gap_threshold hRemaining hGap'

lemma strictGapThreshold_le_seven_of_seventh_headroom
    {remaining : Nat}
    (hHeadroom : 1_429 ≤ remaining) :
    strictGapThreshold remaining ≤ 7 := by
  unfold strictGapThreshold
  change 9_999 / remaining + 1 ≤ 7
  have hPos : 0 < remaining := lt_of_lt_of_le (by decide : 0 < 1_429) hHeadroom
  have hDivLt : 9_999 / remaining < 7 := by
    rw [Nat.div_lt_iff_lt_mul hPos]
    have hLt : 9_999 < 10_003 := by decide
    have hMul : 10_003 ≤ 7 * remaining := by
      calc
        10_003 = 7 * 1_429 := by decide
        _ ≤ 7 * remaining := Nat.mul_le_mul_left 7 hHeadroom
    exact lt_of_lt_of_le hLt hMul
  exact Nat.succ_le_of_lt hDivLt

theorem attenuateSupport_strict_of_seven_gap_under_seventh_headroom
    {support₁ support₂ rejection : Nat}
    (hRejection : clampBps rejection ≤ 8_571)
    (hGap : clampBps support₁ + 7 ≤ clampBps support₂) :
    attenuateSupport support₁ rejection < attenuateSupport support₂ rejection := by
  have hRemaining : clampBps rejection < maxBps :=
    lt_of_le_of_lt hRejection (by decide : 8_571 < maxBps)
  have hThresholdLe : strictGapThreshold (maxBps - clampBps rejection) ≤ 7 := by
    apply strictGapThreshold_le_seven_of_seventh_headroom
    exact Nat.le_sub_of_add_le <|
      calc
        1_429 + clampBps rejection ≤ 1_429 + 8_571 := Nat.add_le_add_left hRejection 1_429
        _ = 10_000 := by decide
  have hGap' :
      clampBps support₁ + strictGapThreshold (maxBps - clampBps rejection) ≤
        clampBps support₂ := by
    exact le_trans (Nat.add_le_add_left hThresholdLe _) hGap
  exact attenuateSupport_strict_of_gap_threshold hRemaining hGap'

lemma strictGapThreshold_le_eight_of_eighth_headroom
    {remaining : Nat}
    (hHeadroom : 1_250 ≤ remaining) :
    strictGapThreshold remaining ≤ 8 := by
  unfold strictGapThreshold
  change 9_999 / remaining + 1 ≤ 8
  have hPos : 0 < remaining := lt_of_lt_of_le (by decide : 0 < 1_250) hHeadroom
  have hDivLt : 9_999 / remaining < 8 := by
    rw [Nat.div_lt_iff_lt_mul hPos]
    have hLt : 9_999 < 10_000 := by decide
    have hMul : 10_000 ≤ 8 * remaining := by
      calc
        10_000 = 8 * 1_250 := by decide
        _ ≤ 8 * remaining := Nat.mul_le_mul_left 8 hHeadroom
    exact lt_of_lt_of_le hLt hMul
  exact Nat.succ_le_of_lt hDivLt

theorem attenuateSupport_strict_of_eight_gap_under_eighth_headroom
    {support₁ support₂ rejection : Nat}
    (hRejection : clampBps rejection ≤ 8_750)
    (hGap : clampBps support₁ + 8 ≤ clampBps support₂) :
    attenuateSupport support₁ rejection < attenuateSupport support₂ rejection := by
  have hRemaining : clampBps rejection < maxBps :=
    lt_of_le_of_lt hRejection (by decide : 8_750 < maxBps)
  have hThresholdLe : strictGapThreshold (maxBps - clampBps rejection) ≤ 8 := by
    apply strictGapThreshold_le_eight_of_eighth_headroom
    exact Nat.le_sub_of_add_le <|
      calc
        1_250 + clampBps rejection ≤ 1_250 + 8_750 := Nat.add_le_add_left hRejection 1_250
        _ = 10_000 := by decide
  have hGap' :
      clampBps support₁ + strictGapThreshold (maxBps - clampBps rejection) ≤
        clampBps support₂ := by
    exact le_trans (Nat.add_le_add_left hThresholdLe _) hGap
  exact attenuateSupport_strict_of_gap_threshold hRemaining hGap'

lemma strictGapThreshold_le_nine_of_ninth_headroom
    {remaining : Nat}
    (hHeadroom : 1_112 ≤ remaining) :
    strictGapThreshold remaining ≤ 9 := by
  unfold strictGapThreshold
  change 9_999 / remaining + 1 ≤ 9
  have hPos : 0 < remaining := lt_of_lt_of_le (by decide : 0 < 1_112) hHeadroom
  have hDivLt : 9_999 / remaining < 9 := by
    rw [Nat.div_lt_iff_lt_mul hPos]
    have hLt : 9_999 < 10_008 := by decide
    have hMul : 10_008 ≤ 9 * remaining := by
      calc
        10_008 = 9 * 1_112 := by decide
        _ ≤ 9 * remaining := Nat.mul_le_mul_left 9 hHeadroom
    exact lt_of_lt_of_le hLt hMul
  exact Nat.succ_le_of_lt hDivLt

theorem attenuateSupport_strict_of_nine_gap_under_ninth_headroom
    {support₁ support₂ rejection : Nat}
    (hRejection : clampBps rejection ≤ 8_888)
    (hGap : clampBps support₁ + 9 ≤ clampBps support₂) :
    attenuateSupport support₁ rejection < attenuateSupport support₂ rejection := by
  have hRemaining : clampBps rejection < maxBps :=
    lt_of_le_of_lt hRejection (by decide : 8_888 < maxBps)
  have hThresholdLe : strictGapThreshold (maxBps - clampBps rejection) ≤ 9 := by
    apply strictGapThreshold_le_nine_of_ninth_headroom
    exact Nat.le_sub_of_add_le <|
      calc
        1_112 + clampBps rejection ≤ 1_112 + 8_888 := Nat.add_le_add_left hRejection 1_112
        _ = 10_000 := by decide
  have hGap' :
      clampBps support₁ + strictGapThreshold (maxBps - clampBps rejection) ≤
        clampBps support₂ := by
    exact le_trans (Nat.add_le_add_left hThresholdLe _) hGap
  exact attenuateSupport_strict_of_gap_threshold hRemaining hGap'

lemma strictGapThreshold_le_ten_of_tenth_headroom
    {remaining : Nat}
    (hHeadroom : 1_000 ≤ remaining) :
    strictGapThreshold remaining ≤ 10 := by
  unfold strictGapThreshold
  change 9_999 / remaining + 1 ≤ 10
  have hPos : 0 < remaining := lt_of_lt_of_le (by decide : 0 < 1_000) hHeadroom
  have hDivLt : 9_999 / remaining < 10 := by
    rw [Nat.div_lt_iff_lt_mul hPos]
    have hLt : 9_999 < 10_000 := by decide
    have hMul : 10_000 ≤ 10 * remaining := by
      calc
        10_000 = 10 * 1_000 := by decide
        _ ≤ 10 * remaining := Nat.mul_le_mul_left 10 hHeadroom
    exact lt_of_lt_of_le hLt hMul
  exact Nat.succ_le_of_lt hDivLt

theorem attenuateSupport_strict_of_ten_gap_under_tenth_headroom
    {support₁ support₂ rejection : Nat}
    (hRejection : clampBps rejection ≤ 9_000)
    (hGap : clampBps support₁ + 10 ≤ clampBps support₂) :
    attenuateSupport support₁ rejection < attenuateSupport support₂ rejection := by
  have hRemaining : clampBps rejection < maxBps :=
    lt_of_le_of_lt hRejection (by decide : 9_000 < maxBps)
  have hThresholdLe : strictGapThreshold (maxBps - clampBps rejection) ≤ 10 := by
    apply strictGapThreshold_le_ten_of_tenth_headroom
    exact Nat.le_sub_of_add_le <|
      calc
        1_000 + clampBps rejection ≤ 1_000 + 9_000 := Nat.add_le_add_left hRejection 1_000
        _ = 10_000 := by decide
  have hGap' :
      clampBps support₁ + strictGapThreshold (maxBps - clampBps rejection) ≤
        clampBps support₂ := by
    exact le_trans (Nat.add_le_add_left hThresholdLe _) hGap
  exact attenuateSupport_strict_of_gap_threshold hRemaining hGap'

lemma strictGapThreshold_le_eleven_of_eleventh_headroom
    {remaining : Nat}
    (hHeadroom : 910 ≤ remaining) :
    strictGapThreshold remaining ≤ 11 := by
  unfold strictGapThreshold
  change 9_999 / remaining + 1 ≤ 11
  have hPos : 0 < remaining := lt_of_lt_of_le (by decide : 0 < 910) hHeadroom
  have hDivLt : 9_999 / remaining < 11 := by
    rw [Nat.div_lt_iff_lt_mul hPos]
    have hLt : 9_999 < 10_010 := by decide
    have hMul : 10_010 ≤ 11 * remaining := by
      calc
        10_010 = 11 * 910 := by decide
        _ ≤ 11 * remaining := Nat.mul_le_mul_left 11 hHeadroom
    exact lt_of_lt_of_le hLt hMul
  exact Nat.succ_le_of_lt hDivLt

theorem attenuateSupport_strict_of_eleven_gap_under_eleventh_headroom
    {support₁ support₂ rejection : Nat}
    (hRejection : clampBps rejection ≤ 9_090)
    (hGap : clampBps support₁ + 11 ≤ clampBps support₂) :
    attenuateSupport support₁ rejection < attenuateSupport support₂ rejection := by
  have hRemaining : clampBps rejection < maxBps :=
    lt_of_le_of_lt hRejection (by decide : 9_090 < maxBps)
  have hThresholdLe : strictGapThreshold (maxBps - clampBps rejection) ≤ 11 := by
    apply strictGapThreshold_le_eleven_of_eleventh_headroom
    exact Nat.le_sub_of_add_le <|
      calc
        910 + clampBps rejection ≤ 910 + 9_090 := Nat.add_le_add_left hRejection 910
        _ = 10_000 := by decide
  have hGap' :
      clampBps support₁ + strictGapThreshold (maxBps - clampBps rejection) ≤
        clampBps support₂ := by
    exact le_trans (Nat.add_le_add_left hThresholdLe _) hGap
  exact attenuateSupport_strict_of_gap_threshold hRemaining hGap'

lemma strictGapThreshold_le_twelve_of_twelfth_headroom
    {remaining : Nat}
    (hHeadroom : 834 ≤ remaining) :
    strictGapThreshold remaining ≤ 12 := by
  unfold strictGapThreshold
  change 9_999 / remaining + 1 ≤ 12
  have hPos : 0 < remaining := lt_of_lt_of_le (by decide : 0 < 834) hHeadroom
  have hDivLt : 9_999 / remaining < 12 := by
    rw [Nat.div_lt_iff_lt_mul hPos]
    have hLt : 9_999 < 10_008 := by decide
    have hMul : 10_008 ≤ 12 * remaining := by
      calc
        10_008 = 12 * 834 := by decide
        _ ≤ 12 * remaining := Nat.mul_le_mul_left 12 hHeadroom
    exact lt_of_lt_of_le hLt hMul
  exact Nat.succ_le_of_lt hDivLt

theorem attenuateSupport_strict_of_twelve_gap_under_twelfth_headroom
    {support₁ support₂ rejection : Nat}
    (hRejection : clampBps rejection ≤ 9_166)
    (hGap : clampBps support₁ + 12 ≤ clampBps support₂) :
    attenuateSupport support₁ rejection < attenuateSupport support₂ rejection := by
  have hRemaining : clampBps rejection < maxBps :=
    lt_of_le_of_lt hRejection (by decide : 9_166 < maxBps)
  have hThresholdLe : strictGapThreshold (maxBps - clampBps rejection) ≤ 12 := by
    apply strictGapThreshold_le_twelve_of_twelfth_headroom
    exact Nat.le_sub_of_add_le <|
      calc
        834 + clampBps rejection ≤ 834 + 9_166 := Nat.add_le_add_left hRejection 834
        _ = 10_000 := by decide
  have hGap' :
      clampBps support₁ + strictGapThreshold (maxBps - clampBps rejection) ≤
        clampBps support₂ := by
    exact le_trans (Nat.add_le_add_left hThresholdLe _) hGap
  exact attenuateSupport_strict_of_gap_threshold hRemaining hGap'

lemma strictGapThreshold_le_thirteen_of_thirteenth_headroom
    {remaining : Nat}
    (hHeadroom : 770 ≤ remaining) :
    strictGapThreshold remaining ≤ 13 := by
  unfold strictGapThreshold
  change 9_999 / remaining + 1 ≤ 13
  have hPos : 0 < remaining := lt_of_lt_of_le (by decide : 0 < 770) hHeadroom
  have hDivLt : 9_999 / remaining < 13 := by
    rw [Nat.div_lt_iff_lt_mul hPos]
    have hLt : 9_999 < 10_010 := by decide
    have hMul : 10_010 ≤ 13 * remaining := by
      calc
        10_010 = 13 * 770 := by decide
        _ ≤ 13 * remaining := Nat.mul_le_mul_left 13 hHeadroom
    exact lt_of_lt_of_le hLt hMul
  exact Nat.succ_le_of_lt hDivLt

theorem attenuateSupport_strict_of_thirteen_gap_under_thirteenth_headroom
    {support₁ support₂ rejection : Nat}
    (hRejection : clampBps rejection ≤ 9_230)
    (hGap : clampBps support₁ + 13 ≤ clampBps support₂) :
    attenuateSupport support₁ rejection < attenuateSupport support₂ rejection := by
  have hRemaining : clampBps rejection < maxBps :=
    lt_of_le_of_lt hRejection (by decide : 9_230 < maxBps)
  have hThresholdLe : strictGapThreshold (maxBps - clampBps rejection) ≤ 13 := by
    apply strictGapThreshold_le_thirteen_of_thirteenth_headroom
    exact Nat.le_sub_of_add_le <|
      calc
        770 + clampBps rejection ≤ 770 + 9_230 := Nat.add_le_add_left hRejection 770
        _ = 10_000 := by decide
  have hGap' :
      clampBps support₁ + strictGapThreshold (maxBps - clampBps rejection) ≤
        clampBps support₂ := by
    exact le_trans (Nat.add_le_add_left hThresholdLe _) hGap
  exact attenuateSupport_strict_of_gap_threshold hRemaining hGap'

lemma strictGapThreshold_le_fourteen_of_fourteenth_headroom
    {remaining : Nat}
    (hHeadroom : 715 ≤ remaining) :
    strictGapThreshold remaining ≤ 14 := by
  unfold strictGapThreshold
  change 9_999 / remaining + 1 ≤ 14
  have hPos : 0 < remaining := lt_of_lt_of_le (by decide : 0 < 715) hHeadroom
  have hDivLt : 9_999 / remaining < 14 := by
    rw [Nat.div_lt_iff_lt_mul hPos]
    have hLt : 9_999 < 10_010 := by decide
    have hMul : 10_010 ≤ 14 * remaining := by
      calc
        10_010 = 14 * 715 := by decide
        _ ≤ 14 * remaining := Nat.mul_le_mul_left 14 hHeadroom
    exact lt_of_lt_of_le hLt hMul
  exact Nat.succ_le_of_lt hDivLt

theorem attenuateSupport_strict_of_fourteen_gap_under_fourteenth_headroom
    {support₁ support₂ rejection : Nat}
    (hRejection : clampBps rejection ≤ 9_285)
    (hGap : clampBps support₁ + 14 ≤ clampBps support₂) :
    attenuateSupport support₁ rejection < attenuateSupport support₂ rejection := by
  have hRemaining : clampBps rejection < maxBps :=
    lt_of_le_of_lt hRejection (by decide : 9_285 < maxBps)
  have hThresholdLe : strictGapThreshold (maxBps - clampBps rejection) ≤ 14 := by
    apply strictGapThreshold_le_fourteen_of_fourteenth_headroom
    exact Nat.le_sub_of_add_le <|
      calc
        715 + clampBps rejection ≤ 715 + 9_285 := Nat.add_le_add_left hRejection 715
        _ = 10_000 := by decide
  have hGap' :
      clampBps support₁ + strictGapThreshold (maxBps - clampBps rejection) ≤
        clampBps support₂ := by
    exact le_trans (Nat.add_le_add_left hThresholdLe _) hGap
  exact attenuateSupport_strict_of_gap_threshold hRemaining hGap'

lemma strictGapThreshold_le_fifteen_of_fifteenth_headroom
    {remaining : Nat}
    (hHeadroom : 667 ≤ remaining) :
    strictGapThreshold remaining ≤ 15 := by
  unfold strictGapThreshold
  change 9_999 / remaining + 1 ≤ 15
  have hPos : 0 < remaining := lt_of_lt_of_le (by decide : 0 < 667) hHeadroom
  have hDivLt : 9_999 / remaining < 15 := by
    rw [Nat.div_lt_iff_lt_mul hPos]
    have hLt : 9_999 < 10_005 := by decide
    have hMul : 10_005 ≤ 15 * remaining := by
      calc
        10_005 = 15 * 667 := by decide
        _ ≤ 15 * remaining := Nat.mul_le_mul_left 15 hHeadroom
    exact lt_of_lt_of_le hLt hMul
  exact Nat.succ_le_of_lt hDivLt

theorem attenuateSupport_strict_of_fifteen_gap_under_fifteenth_headroom
    {support₁ support₂ rejection : Nat}
    (hRejection : clampBps rejection ≤ 9_333)
    (hGap : clampBps support₁ + 15 ≤ clampBps support₂) :
    attenuateSupport support₁ rejection < attenuateSupport support₂ rejection := by
  have hRemaining : clampBps rejection < maxBps :=
    lt_of_le_of_lt hRejection (by decide : 9_333 < maxBps)
  have hThresholdLe : strictGapThreshold (maxBps - clampBps rejection) ≤ 15 := by
    apply strictGapThreshold_le_fifteen_of_fifteenth_headroom
    exact Nat.le_sub_of_add_le <|
      calc
        667 + clampBps rejection ≤ 667 + 9_333 := Nat.add_le_add_left hRejection 667
        _ = 10_000 := by decide
  have hGap' :
      clampBps support₁ + strictGapThreshold (maxBps - clampBps rejection) ≤
        clampBps support₂ := by
    exact le_trans (Nat.add_le_add_left hThresholdLe _) hGap
  exact attenuateSupport_strict_of_gap_threshold hRemaining hGap'

lemma noisyOrNumerator_mono {aggregate₁ aggregate₂ signal : Nat}
    (hAggregate : aggregate₁ ≤ aggregate₂)
    (hAggregate₂ : aggregate₂ ≤ maxBps)
    (hSignal : signal ≤ maxBps) :
    ((maxBps - aggregate₁) * signal + aggregate₁ * maxBps) ≤
      ((maxBps - aggregate₂) * signal + aggregate₂ * maxBps) := by
  have hAggregate₁ : aggregate₁ ≤ maxBps := le_trans hAggregate hAggregate₂
  obtain ⟨delta, hDeltaEq⟩ := Nat.exists_eq_add_of_le hAggregate
  have hDelta : delta ≤ maxBps - aggregate₁ := by
    rw [Nat.le_sub_iff_add_le hAggregate₁]
    simpa [hDeltaEq, Nat.add_assoc, Nat.add_left_comm, Nat.add_comm] using hAggregate₂
  have hEqAdd : (maxBps - aggregate₁ - delta) + (aggregate₁ + delta) = maxBps := by
    calc
      (maxBps - aggregate₁ - delta) + (aggregate₁ + delta)
          = ((maxBps - aggregate₁ - delta) + delta) + aggregate₁ := by ac_rfl
      _ = (maxBps - aggregate₁) + aggregate₁ := by rw [Nat.sub_add_cancel hDelta]
      _ = maxBps := by rw [Nat.sub_add_cancel hAggregate₁]
  have hSub : maxBps - (aggregate₁ + delta) = maxBps - aggregate₁ - delta := by
    exact (Nat.sub_eq_iff_eq_add (by simpa [hDeltaEq] using hAggregate₂)).2 hEqAdd.symm
  have hEq :
      ((maxBps - (aggregate₁ + delta)) * signal + (aggregate₁ + delta) * maxBps) +
          delta * signal =
        ((maxBps - aggregate₁) * signal + aggregate₁ * maxBps) + delta * maxBps := by
    rw [hSub, Nat.add_mul]
    calc
      (maxBps - aggregate₁ - delta) * signal +
          (aggregate₁ * maxBps + delta * maxBps) + delta * signal
          = ((maxBps - aggregate₁ - delta) * signal + delta * signal) +
              (aggregate₁ * maxBps + delta * maxBps) := by ac_rfl
      _ = ((maxBps - aggregate₁ - delta + delta) * signal) +
            (aggregate₁ * maxBps + delta * maxBps) := by rw [← Nat.add_mul]
      _ = ((maxBps - aggregate₁) * signal) +
            (aggregate₁ * maxBps + delta * maxBps) := by rw [Nat.sub_add_cancel hDelta]
      _ = ((maxBps - aggregate₁) * signal + aggregate₁ * maxBps) + delta * maxBps := by ac_rfl
  have hMul : delta * signal ≤ delta * maxBps := Nat.mul_le_mul_left delta hSignal
  have hAdd :
      ((maxBps - aggregate₁) * signal + aggregate₁ * maxBps) + delta * signal ≤
        ((maxBps - (aggregate₁ + delta)) * signal + (aggregate₁ + delta) * maxBps) +
          delta * signal := by
    rw [hEq]
    exact Nat.add_le_add_left hMul _
  have hBase :
      ((maxBps - aggregate₁) * signal + aggregate₁ * maxBps) ≤
        ((maxBps - (aggregate₁ + delta)) * signal + (aggregate₁ + delta) * maxBps) := by
    exact Nat.le_of_add_le_add_right hAdd
  simpa [hDeltaEq] using hBase

theorem stepNoisyOr_mono_aggregate {aggregate₁ aggregate₂ signal : Nat}
    (h : aggregate₁ ≤ aggregate₂) :
    stepNoisyOr aggregate₁ signal ≤ stepNoisyOr aggregate₂ signal := by
  unfold stepNoisyOr
  dsimp
  apply clampBps_mono
  have hAggregate : clampBps aggregate₁ ≤ clampBps aggregate₂ := clampBps_mono h
  have hAggregate₂ : clampBps aggregate₂ ≤ maxBps := clampBps_le_max aggregate₂
  have hSignal : clampBps signal ≤ maxBps := clampBps_le_max signal
  rw [Nat.add_comm]
  rw [← Nat.add_mul_div_right
    ((maxBps - clampBps aggregate₁) * clampBps signal)
    (clampBps aggregate₁)
    (by decide : 0 < maxBps)]
  rw [Nat.add_comm (clampBps aggregate₂)
    (((maxBps - clampBps aggregate₂) * clampBps signal) / maxBps)]
  rw [← Nat.add_mul_div_right
    ((maxBps - clampBps aggregate₂) * clampBps signal)
    (clampBps aggregate₂)
    (by decide : 0 < maxBps)]
  simpa [Nat.add_comm] using
    Nat.div_le_div_right (c := maxBps)
      (noisyOrNumerator_mono hAggregate hAggregate₂ hSignal)

theorem stepNoisyOr_mono_signal {aggregate signal₁ signal₂ : Nat}
    (h : signal₁ ≤ signal₂) :
    stepNoisyOr aggregate signal₁ ≤ stepNoisyOr aggregate signal₂ := by
  unfold stepNoisyOr
  dsimp
  apply clampBps_mono
  have hSignal : clampBps signal₁ ≤ clampBps signal₂ := clampBps_mono h
  have hMul :
      (maxBps - clampBps aggregate) * clampBps signal₁ ≤
        (maxBps - clampBps aggregate) * clampBps signal₂ :=
    Nat.mul_le_mul_left _ hSignal
  have hDiv :
      ((maxBps - clampBps aggregate) * clampBps signal₁) / maxBps ≤
        ((maxBps - clampBps aggregate) * clampBps signal₂) / maxBps :=
    Nat.div_le_div_right (c := maxBps) hMul
  exact Nat.add_le_add_left hDiv _

theorem accumulateNoisyOr_mono_aggregate {aggregate₁ aggregate₂ signal count : Nat}
    (h : aggregate₁ ≤ aggregate₂) :
    accumulateNoisyOr aggregate₁ signal count ≤ accumulateNoisyOr aggregate₂ signal count := by
  induction count with
  | zero =>
      simpa [accumulateNoisyOr] using clampBps_mono h
  | succ count ih =>
      simp [accumulateNoisyOr]
      exact stepNoisyOr_mono_aggregate ih

theorem accumulateNoisyOr_mono_signal {aggregate signal₁ signal₂ count : Nat}
    (h : signal₁ ≤ signal₂) :
    accumulateNoisyOr aggregate signal₁ count ≤ accumulateNoisyOr aggregate signal₂ count := by
  induction count with
  | zero =>
      simp [accumulateNoisyOr]
  | succ count ih =>
      simp [accumulateNoisyOr]
      exact le_trans (stepNoisyOr_mono_aggregate ih) (stepNoisyOr_mono_signal h)

lemma unresolvedClaimCount_replace_one
    {claimCount corroboratedCount rejectedCount : Nat}
    (hOpen : corroboratedCount + rejectedCount < claimCount) :
    unresolvedClaimCount claimCount corroboratedCount rejectedCount =
      unresolvedClaimCount claimCount (corroboratedCount + 1) rejectedCount + 1 := by
  obtain ⟨delta, hDelta⟩ := Nat.exists_eq_add_of_lt hOpen
  unfold unresolvedClaimCount
  rw [hDelta]
  omega

lemma accumulateNoisyOr_succ_left (aggregate signal count : Nat) :
    accumulateNoisyOr aggregate signal (count + 1) =
      accumulateNoisyOr (stepNoisyOr aggregate signal) signal count := by
  induction count with
  | zero =>
      calc
        accumulateNoisyOr aggregate signal (0 + 1)
          = stepNoisyOr aggregate signal := by
              simp [accumulateNoisyOr, stepNoisyOr_clamp_aggregate]
        _ = clampBps (stepNoisyOr aggregate signal) := by
              symm
              exact clampBps_id (stepNoisyOr_le_max aggregate signal)
        _ = accumulateNoisyOr (stepNoisyOr aggregate signal) signal 0 := by
              simp [accumulateNoisyOr]
  | succ count ih =>
      calc
        accumulateNoisyOr aggregate signal (count.succ + 1)
          = stepNoisyOr (accumulateNoisyOr aggregate signal (count + 1)) signal := by
              simp [accumulateNoisyOr]
        _ = stepNoisyOr (accumulateNoisyOr (stepNoisyOr aggregate signal) signal count) signal := by
              rw [ih]
        _ = accumulateNoisyOr (stepNoisyOr aggregate signal) signal (count + 1) := by
              simp [accumulateNoisyOr]

lemma accumulateNoisyOr_replace_first_step {aggregate low high tailCount : Nat}
    (hSignal : low ≤ high) :
    accumulateNoisyOr (stepNoisyOr aggregate low) low tailCount ≤
      accumulateNoisyOr (stepNoisyOr aggregate high) low tailCount := by
  exact accumulateNoisyOr_mono_aggregate (stepNoisyOr_mono_signal hSignal)

theorem proposalSupportBps_le_clampBps (confidence : Nat) :
    proposalSupportBps confidence ≤ clampBps confidence := by
  unfold proposalSupportBps
  calc
    (clampBps confidence * 7) / 10
      ≤ (clampBps confidence * 10) / 10 :=
        Nat.div_le_div_right (c := 10) (Nat.mul_le_mul_left _ (by decide : 7 ≤ 10))
    _ = clampBps confidence := by
      rw [Nat.mul_comm, Nat.mul_div_right _ (by decide : 0 < 10)]

theorem proposalSupportBps_le_corroboratedSupportBps (confidence : Nat) :
    proposalSupportBps confidence ≤ corroboratedSupportBps confidence := by
  unfold corroboratedSupportBps
  exact le_trans (proposalSupportBps_le_clampBps confidence) (le_max_right _ _)

theorem fusedCredibility_eq_allProposal
    (claimCount confidence : Nat) :
    fusedCredibility claimCount 0 0 confidence =
      accumulateNoisyOr 0 (proposalSupportBps confidence) claimCount := by
  unfold fusedCredibility unresolvedClaimCount rejectionSupportBps
  simp [accumulateNoisyOr, clampBps]
  rw [attenuateSupport_eq_clampedSupport_of_zeroRejection]
  exact clampBps_id (accumulateNoisyOr_le_max 0 (proposalSupportBps confidence) claimCount)

theorem fusedCredibility_eq_allCorroborated
    (claimCount confidence : Nat) :
    fusedCredibility claimCount claimCount 0 confidence =
      accumulateNoisyOr 0 (corroboratedSupportBps confidence) claimCount := by
  unfold fusedCredibility unresolvedClaimCount rejectionSupportBps
  simp [accumulateNoisyOr, clampBps]
  rw [attenuateSupport_eq_clampedSupport_of_zeroRejection]
  unfold clampBps
  rw [Nat.min_eq_right (Nat.min_le_left _ _)]
  exact Nat.min_eq_right (accumulateNoisyOr_le_max 0 (corroboratedSupportBps confidence) claimCount)

theorem fusedCredibility_allCorroborated_ge_allProposal
    (claimCount confidence : Nat) :
    fusedCredibility claimCount 0 0 confidence ≤
      fusedCredibility claimCount claimCount 0 confidence := by
  have hSupport :
      accumulateNoisyOr 0 (proposalSupportBps confidence) claimCount ≤
        accumulateNoisyOr 0 (corroboratedSupportBps confidence) claimCount :=
    accumulateNoisyOr_mono_signal (aggregate := 0) (count := claimCount)
      (proposalSupportBps_le_corroboratedSupportBps confidence)
  rw [fusedCredibility_eq_allProposal, fusedCredibility_eq_allCorroborated]
  exact hSupport

theorem support_replace_one_unresolved_with_corroborated
    {claimCount corroboratedCount rejectedCount confidence : Nat}
    (hOpen : corroboratedCount + rejectedCount < claimCount) :
    let corroboratedSupport := corroboratedSupportBps confidence
    let proposalSupport := proposalSupportBps confidence
    accumulateNoisyOr (accumulateNoisyOr 0 corroboratedSupport corroboratedCount)
      proposalSupport
      (unresolvedClaimCount claimCount corroboratedCount rejectedCount)
      ≤
    accumulateNoisyOr (accumulateNoisyOr 0 corroboratedSupport (corroboratedCount + 1))
      proposalSupport
      (unresolvedClaimCount claimCount (corroboratedCount + 1) rejectedCount) := by
  have hUnresolved := unresolvedClaimCount_replace_one hOpen
  let corroboratedSupport := corroboratedSupportBps confidence
  let proposalSupport := proposalSupportBps confidence
  let tailCount := unresolvedClaimCount claimCount (corroboratedCount + 1) rejectedCount
  let base := accumulateNoisyOr 0 corroboratedSupport corroboratedCount
  have hSignals : proposalSupport ≤ corroboratedSupport := by
    simpa [proposalSupport, corroboratedSupport] using
      proposalSupportBps_le_corroboratedSupportBps confidence
  calc
    accumulateNoisyOr base proposalSupport
        (unresolvedClaimCount claimCount corroboratedCount rejectedCount)
        = accumulateNoisyOr (stepNoisyOr base proposalSupport) proposalSupport tailCount := by
            rw [hUnresolved]
            exact accumulateNoisyOr_succ_left base proposalSupport tailCount
    _ ≤ accumulateNoisyOr (stepNoisyOr base corroboratedSupport) proposalSupport tailCount :=
          accumulateNoisyOr_replace_first_step hSignals
    _ = accumulateNoisyOr (accumulateNoisyOr 0 corroboratedSupport (corroboratedCount + 1))
          proposalSupport tailCount := by
            simp [base, accumulateNoisyOr]
    _ = accumulateNoisyOr (accumulateNoisyOr 0 corroboratedSupport (corroboratedCount + 1))
          proposalSupport
          (unresolvedClaimCount claimCount (corroboratedCount + 1) rejectedCount) := by
            rfl

theorem fusedCredibility_replace_one_unresolved_with_corroborated
    {claimCount corroboratedCount rejectedCount confidence : Nat}
    (hOpen : corroboratedCount + rejectedCount < claimCount) :
    fusedCredibility claimCount corroboratedCount rejectedCount confidence ≤
      fusedCredibility claimCount (corroboratedCount + 1) rejectedCount confidence := by
  unfold fusedCredibility
  let corroboratedSupport := corroboratedSupportBps confidence
  let proposalSupport := proposalSupportBps confidence
  let rejectionSupport := rejectionSupportBps confidence
  let rejection := accumulateNoisyOr 0 rejectionSupport rejectedCount
  apply attenuateSupport_mono_support
  simpa [corroboratedSupport, proposalSupport, rejectionSupport, rejection] using
    support_replace_one_unresolved_with_corroborated (confidence := confidence) hOpen

theorem fusedCredibility_last_open_no_rejection_eq_proposal_step
    (corroboratedCount confidence : Nat) :
    fusedCredibility (corroboratedCount + 1) corroboratedCount 0 confidence =
      stepNoisyOr
        (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
        (proposalSupportBps confidence) := by
  unfold fusedCredibility unresolvedClaimCount rejectionSupportBps
  simp [accumulateNoisyOr, clampBps]
  rw [attenuateSupport_eq_clampedSupport_of_zeroRejection]
  rw [show
      stepNoisyOr
          (min maxBps
            (Nat.rec 0 (fun _ acc => stepNoisyOr acc (corroboratedSupportBps confidence))
              corroboratedCount))
          (proposalSupportBps confidence)
        =
      stepNoisyOr
          (Nat.rec 0 (fun _ acc => stepNoisyOr acc (corroboratedSupportBps confidence))
            corroboratedCount)
          (proposalSupportBps confidence) by
      exact stepNoisyOr_clamp_aggregate _ _]
  unfold clampBps
  exact Nat.min_eq_right (stepNoisyOr_le_max _ _)

theorem fusedCredibility_last_open_no_rejection_eq_corroborated_step
    (corroboratedCount confidence : Nat) :
    fusedCredibility (corroboratedCount + 1) (corroboratedCount + 1) 0 confidence =
      stepNoisyOr
        (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
        (corroboratedSupportBps confidence) := by
  unfold fusedCredibility unresolvedClaimCount rejectionSupportBps
  simp [accumulateNoisyOr, clampBps]
  rw [attenuateSupport_eq_clampedSupport_of_zeroRejection]
  unfold clampBps
  rw [Nat.min_eq_right (Nat.min_le_left _ _)]
  exact Nat.min_eq_right (stepNoisyOr_le_max _ _)

theorem fusedCredibility_last_open_no_rejection_strict_iff
    (corroboratedCount confidence : Nat) :
    fusedCredibility (corroboratedCount + 1) corroboratedCount 0 confidence <
      fusedCredibility (corroboratedCount + 1) (corroboratedCount + 1) 0 confidence ↔
        stepNoisyOr
            (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
            (proposalSupportBps confidence) <
          stepNoisyOr
            (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
            (corroboratedSupportBps confidence) := by
  rw [fusedCredibility_last_open_no_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_no_rejection_eq_corroborated_step]

theorem fusedCredibility_last_open_fixed_rejection_eq_proposal_step
    (corroboratedCount rejectedCount confidence : Nat) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence =
      attenuateSupport
        (stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence))
        (accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount) := by
  have hUnresolved :
      unresolvedClaimCount
          (corroboratedCount + rejectedCount + 1)
          corroboratedCount
          rejectedCount =
        1 := by
    unfold unresolvedClaimCount
    simp [Nat.add_assoc]
  simp [fusedCredibility, hUnresolved, accumulateNoisyOr, clampBps]
  rw [show
      stepNoisyOr
          (min maxBps
            (Nat.rec 0 (fun _ acc => stepNoisyOr acc (corroboratedSupportBps confidence))
              corroboratedCount))
          (proposalSupportBps confidence)
        =
      stepNoisyOr
          (Nat.rec 0 (fun _ acc => stepNoisyOr acc (corroboratedSupportBps confidence))
            corroboratedCount)
          (proposalSupportBps confidence) by
      exact stepNoisyOr_clamp_aggregate _ _]

theorem fusedCredibility_last_open_fixed_rejection_eq_corroborated_step
    (corroboratedCount rejectedCount confidence : Nat) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence =
      attenuateSupport
        (stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence))
        (accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount) := by
  have hUnresolved :
      unresolvedClaimCount
          (corroboratedCount + rejectedCount + 1)
          (corroboratedCount + 1)
          rejectedCount =
        0 := by
    unfold unresolvedClaimCount
    simp [Nat.add_assoc, Nat.add_left_comm]
  simp [fusedCredibility, hUnresolved, accumulateNoisyOr, clampBps]
  rw [show
      min maxBps
          (stepNoisyOr
            (Nat.rec 0 (fun _ acc => stepNoisyOr acc (corroboratedSupportBps confidence))
              corroboratedCount)
            (corroboratedSupportBps confidence))
        =
      stepNoisyOr
        (Nat.rec 0 (fun _ acc => stepNoisyOr acc (corroboratedSupportBps confidence))
          corroboratedCount)
        (corroboratedSupportBps confidence) by
      exact Nat.min_eq_right (stepNoisyOr_le_max _ _)]

theorem fusedCredibility_last_open_fixed_rejection_strict_iff
    (corroboratedCount rejectedCount confidence : Nat) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence ↔
        attenuateSupport
            (stepNoisyOr
              (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
              (proposalSupportBps confidence))
            (accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount) <
          attenuateSupport
            (stepNoisyOr
              (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
              (corroboratedSupportBps confidence))
            (accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount) := by
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]

theorem fusedCredibility_last_open_fixed_rejection_strict_of_scaled_gap
    (corroboratedCount rejectedCount confidence : Nat)
    (hGap :
      let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
      let proposalSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence)
      let corroboratedSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence)
      proposalSupport * (maxBps - rejection) + maxBps ≤
        corroboratedSupport * (maxBps - rejection)) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence := by
  let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
  let proposalSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (proposalSupportBps confidence)
  let corroboratedSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (corroboratedSupportBps confidence)
  have hGap' :
      clampBps proposalSupport * (maxBps - clampBps rejection) + maxBps ≤
        clampBps corroboratedSupport * (maxBps - clampBps rejection) := by
    have hProposal : clampBps proposalSupport = proposalSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hCorroborated : clampBps corroboratedSupport = corroboratedSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hRejection : clampBps rejection = rejection :=
      clampBps_id (accumulateNoisyOr_le_max 0 (rejectionSupportBps confidence) rejectedCount)
    simpa [
      proposalSupport,
      corroboratedSupport,
      rejection,
      hProposal,
      hCorroborated,
      hRejection
    ] using hGap
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]
  exact attenuateSupport_strict_of_scaled_gap hGap'

theorem fusedCredibility_last_open_fixed_rejection_strict_of_gap_threshold
    (corroboratedCount rejectedCount confidence : Nat)
    (hGap :
      let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
      let proposalSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence)
      let corroboratedSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence)
      rejection < maxBps ∧
        proposalSupport + strictGapThreshold (maxBps - rejection) ≤ corroboratedSupport) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence := by
  let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
  let proposalSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (proposalSupportBps confidence)
  let corroboratedSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (corroboratedSupportBps confidence)
  have hRejectionClamp : clampBps rejection = rejection :=
    clampBps_id (accumulateNoisyOr_le_max 0 (rejectionSupportBps confidence) rejectedCount)
  have hRemaining : clampBps rejection < maxBps := by
    simpa [proposalSupport, corroboratedSupport, rejection, hRejectionClamp] using hGap.1
  have hSupportGap :
      clampBps proposalSupport + strictGapThreshold (maxBps - clampBps rejection) ≤
        clampBps corroboratedSupport := by
    have hProposal : clampBps proposalSupport = proposalSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hCorroborated : clampBps corroboratedSupport = corroboratedSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    simpa [
      proposalSupport,
      corroboratedSupport,
      rejection,
      hProposal,
      hCorroborated,
      hRejectionClamp
    ] using hGap.2
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]
  exact attenuateSupport_strict_of_gap_threshold hRemaining hSupportGap

theorem fusedCredibility_last_open_fixed_rejection_strict_of_two_gap_under_half_headroom
    (corroboratedCount rejectedCount confidence : Nat)
    (hGap :
      let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
      let proposalSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence)
      let corroboratedSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence)
      rejection ≤ 5_000 ∧ proposalSupport + 2 ≤ corroboratedSupport) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence := by
  let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
  let proposalSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (proposalSupportBps confidence)
  let corroboratedSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (corroboratedSupportBps confidence)
  have hRejectionClamp : clampBps rejection = rejection :=
    clampBps_id (accumulateNoisyOr_le_max 0 (rejectionSupportBps confidence) rejectedCount)
  have hRejection : clampBps rejection ≤ 5_000 := by
    simpa [proposalSupport, corroboratedSupport, rejection, hRejectionClamp] using hGap.1
  have hSupportGap : clampBps proposalSupport + 2 ≤ clampBps corroboratedSupport := by
    have hProposal : clampBps proposalSupport = proposalSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hCorroborated : clampBps corroboratedSupport = corroboratedSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    simpa [proposalSupport, corroboratedSupport, rejection, hProposal, hCorroborated] using hGap.2
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]
  exact attenuateSupport_strict_of_two_gap_under_half_headroom hRejection hSupportGap

theorem fusedCredibility_last_open_fixed_rejection_strict_of_three_gap_under_two_thirds_headroom
    (corroboratedCount rejectedCount confidence : Nat)
    (hGap :
      let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
      let proposalSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence)
      let corroboratedSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence)
      rejection ≤ 6_666 ∧ proposalSupport + 3 ≤ corroboratedSupport) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence := by
  let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
  let proposalSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (proposalSupportBps confidence)
  let corroboratedSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (corroboratedSupportBps confidence)
  have hRejectionClamp : clampBps rejection = rejection :=
    clampBps_id (accumulateNoisyOr_le_max 0 (rejectionSupportBps confidence) rejectedCount)
  have hRejection : clampBps rejection ≤ 6_666 := by
    simpa [proposalSupport, corroboratedSupport, rejection, hRejectionClamp] using hGap.1
  have hSupportGap : clampBps proposalSupport + 3 ≤ clampBps corroboratedSupport := by
    have hProposal : clampBps proposalSupport = proposalSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hCorroborated : clampBps corroboratedSupport = corroboratedSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    simpa [proposalSupport, corroboratedSupport, rejection, hProposal, hCorroborated] using hGap.2
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]
  exact attenuateSupport_strict_of_three_gap_under_two_thirds_headroom hRejection hSupportGap

theorem fusedCredibility_last_open_fixed_rejection_strict_of_four_gap_under_quarter_headroom
    (corroboratedCount rejectedCount confidence : Nat)
    (hGap :
      let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
      let proposalSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence)
      let corroboratedSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence)
      rejection ≤ 7_500 ∧ proposalSupport + 4 ≤ corroboratedSupport) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence := by
  let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
  let proposalSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (proposalSupportBps confidence)
  let corroboratedSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (corroboratedSupportBps confidence)
  have hRejectionClamp : clampBps rejection = rejection :=
    clampBps_id (accumulateNoisyOr_le_max 0 (rejectionSupportBps confidence) rejectedCount)
  have hRejection : clampBps rejection ≤ 7_500 := by
    simpa [proposalSupport, corroboratedSupport, rejection, hRejectionClamp] using hGap.1
  have hSupportGap : clampBps proposalSupport + 4 ≤ clampBps corroboratedSupport := by
    have hProposal : clampBps proposalSupport = proposalSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hCorroborated : clampBps corroboratedSupport = corroboratedSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    simpa [proposalSupport, corroboratedSupport, rejection, hProposal, hCorroborated] using hGap.2
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]
  exact attenuateSupport_strict_of_four_gap_under_quarter_headroom hRejection hSupportGap

theorem fusedCredibility_last_open_fixed_rejection_strict_of_five_gap_under_fifth_headroom
    (corroboratedCount rejectedCount confidence : Nat)
    (hGap :
      let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
      let proposalSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence)
      let corroboratedSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence)
      rejection ≤ 8_000 ∧ proposalSupport + 5 ≤ corroboratedSupport) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence := by
  let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
  let proposalSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (proposalSupportBps confidence)
  let corroboratedSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (corroboratedSupportBps confidence)
  have hRejectionClamp : clampBps rejection = rejection :=
    clampBps_id (accumulateNoisyOr_le_max 0 (rejectionSupportBps confidence) rejectedCount)
  have hRejection : clampBps rejection ≤ 8_000 := by
    simpa [proposalSupport, corroboratedSupport, rejection, hRejectionClamp] using hGap.1
  have hSupportGap : clampBps proposalSupport + 5 ≤ clampBps corroboratedSupport := by
    have hProposal : clampBps proposalSupport = proposalSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hCorroborated : clampBps corroboratedSupport = corroboratedSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    simpa [proposalSupport, corroboratedSupport, rejection, hProposal, hCorroborated] using hGap.2
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]
  exact attenuateSupport_strict_of_five_gap_under_fifth_headroom hRejection hSupportGap

theorem fusedCredibility_last_open_fixed_rejection_strict_of_six_gap_under_sixth_headroom
    (corroboratedCount rejectedCount confidence : Nat)
    (hGap :
      let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
      let proposalSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence)
      let corroboratedSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence)
      rejection ≤ 8_333 ∧ proposalSupport + 6 ≤ corroboratedSupport) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence := by
  let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
  let proposalSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (proposalSupportBps confidence)
  let corroboratedSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (corroboratedSupportBps confidence)
  have hRejectionClamp : clampBps rejection = rejection :=
    clampBps_id (accumulateNoisyOr_le_max 0 (rejectionSupportBps confidence) rejectedCount)
  have hRejection : clampBps rejection ≤ 8_333 := by
    simpa [proposalSupport, corroboratedSupport, rejection, hRejectionClamp] using hGap.1
  have hSupportGap : clampBps proposalSupport + 6 ≤ clampBps corroboratedSupport := by
    have hProposal : clampBps proposalSupport = proposalSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hCorroborated : clampBps corroboratedSupport = corroboratedSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    simpa [proposalSupport, corroboratedSupport, rejection, hProposal, hCorroborated] using hGap.2
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]
  exact attenuateSupport_strict_of_six_gap_under_sixth_headroom hRejection hSupportGap

theorem fusedCredibility_last_open_fixed_rejection_strict_of_seven_gap_under_seventh_headroom
    (corroboratedCount rejectedCount confidence : Nat)
    (hGap :
      let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
      let proposalSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence)
      let corroboratedSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence)
      rejection ≤ 8_571 ∧ proposalSupport + 7 ≤ corroboratedSupport) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence := by
  let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
  let proposalSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (proposalSupportBps confidence)
  let corroboratedSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (corroboratedSupportBps confidence)
  have hRejectionClamp : clampBps rejection = rejection :=
    clampBps_id (accumulateNoisyOr_le_max 0 (rejectionSupportBps confidence) rejectedCount)
  have hRejection : clampBps rejection ≤ 8_571 := by
    simpa [proposalSupport, corroboratedSupport, rejection, hRejectionClamp] using hGap.1
  have hSupportGap : clampBps proposalSupport + 7 ≤ clampBps corroboratedSupport := by
    have hProposal : clampBps proposalSupport = proposalSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hCorroborated : clampBps corroboratedSupport = corroboratedSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    simpa [proposalSupport, corroboratedSupport, rejection, hProposal, hCorroborated] using hGap.2
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]
  exact attenuateSupport_strict_of_seven_gap_under_seventh_headroom hRejection hSupportGap

theorem fusedCredibility_last_open_fixed_rejection_strict_of_eight_gap_under_eighth_headroom
    (corroboratedCount rejectedCount confidence : Nat)
    (hGap :
      let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
      let proposalSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence)
      let corroboratedSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence)
      rejection ≤ 8_750 ∧ proposalSupport + 8 ≤ corroboratedSupport) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence := by
  let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
  let proposalSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (proposalSupportBps confidence)
  let corroboratedSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (corroboratedSupportBps confidence)
  have hRejectionClamp : clampBps rejection = rejection :=
    clampBps_id (accumulateNoisyOr_le_max 0 (rejectionSupportBps confidence) rejectedCount)
  have hRejection : clampBps rejection ≤ 8_750 := by
    simpa [proposalSupport, corroboratedSupport, rejection, hRejectionClamp] using hGap.1
  have hSupportGap : clampBps proposalSupport + 8 ≤ clampBps corroboratedSupport := by
    have hProposal : clampBps proposalSupport = proposalSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hCorroborated : clampBps corroboratedSupport = corroboratedSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    simpa [proposalSupport, corroboratedSupport, rejection, hProposal, hCorroborated] using hGap.2
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]
  exact attenuateSupport_strict_of_eight_gap_under_eighth_headroom hRejection hSupportGap

theorem fusedCredibility_last_open_fixed_rejection_strict_of_nine_gap_under_ninth_headroom
    (corroboratedCount rejectedCount confidence : Nat)
    (hGap :
      let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
      let proposalSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence)
      let corroboratedSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence)
      rejection ≤ 8_888 ∧ proposalSupport + 9 ≤ corroboratedSupport) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence := by
  let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
  let proposalSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (proposalSupportBps confidence)
  let corroboratedSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (corroboratedSupportBps confidence)
  have hRejectionClamp : clampBps rejection = rejection :=
    clampBps_id (accumulateNoisyOr_le_max 0 (rejectionSupportBps confidence) rejectedCount)
  have hRejection : clampBps rejection ≤ 8_888 := by
    simpa [proposalSupport, corroboratedSupport, rejection, hRejectionClamp] using hGap.1
  have hSupportGap : clampBps proposalSupport + 9 ≤ clampBps corroboratedSupport := by
    have hProposal : clampBps proposalSupport = proposalSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hCorroborated : clampBps corroboratedSupport = corroboratedSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    simpa [proposalSupport, corroboratedSupport, rejection, hProposal, hCorroborated] using hGap.2
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]
  exact attenuateSupport_strict_of_nine_gap_under_ninth_headroom hRejection hSupportGap

theorem fusedCredibility_last_open_fixed_rejection_strict_of_ten_gap_under_tenth_headroom
    (corroboratedCount rejectedCount confidence : Nat)
    (hGap :
      let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
      let proposalSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence)
      let corroboratedSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence)
      rejection ≤ 9_000 ∧ proposalSupport + 10 ≤ corroboratedSupport) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence := by
  let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
  let proposalSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (proposalSupportBps confidence)
  let corroboratedSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (corroboratedSupportBps confidence)
  have hRejectionClamp : clampBps rejection = rejection :=
    clampBps_id (accumulateNoisyOr_le_max 0 (rejectionSupportBps confidence) rejectedCount)
  have hRejection : clampBps rejection ≤ 9_000 := by
    simpa [proposalSupport, corroboratedSupport, rejection, hRejectionClamp] using hGap.1
  have hSupportGap : clampBps proposalSupport + 10 ≤ clampBps corroboratedSupport := by
    have hProposal : clampBps proposalSupport = proposalSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hCorroborated : clampBps corroboratedSupport = corroboratedSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    simpa [proposalSupport, corroboratedSupport, rejection, hProposal, hCorroborated] using hGap.2
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]
  exact attenuateSupport_strict_of_ten_gap_under_tenth_headroom hRejection hSupportGap

theorem fusedCredibility_last_open_fixed_rejection_strict_of_eleven_gap_under_eleventh_headroom
    (corroboratedCount rejectedCount confidence : Nat)
    (hGap :
      let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
      let proposalSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence)
      let corroboratedSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence)
      rejection ≤ 9_090 ∧ proposalSupport + 11 ≤ corroboratedSupport) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence := by
  let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
  let proposalSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (proposalSupportBps confidence)
  let corroboratedSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (corroboratedSupportBps confidence)
  have hRejectionClamp : clampBps rejection = rejection :=
    clampBps_id (accumulateNoisyOr_le_max 0 (rejectionSupportBps confidence) rejectedCount)
  have hRejection : clampBps rejection ≤ 9_090 := by
    simpa [proposalSupport, corroboratedSupport, rejection, hRejectionClamp] using hGap.1
  have hSupportGap : clampBps proposalSupport + 11 ≤ clampBps corroboratedSupport := by
    have hProposal : clampBps proposalSupport = proposalSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hCorroborated : clampBps corroboratedSupport = corroboratedSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    simpa [proposalSupport, corroboratedSupport, rejection, hProposal, hCorroborated] using hGap.2
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]
  exact attenuateSupport_strict_of_eleven_gap_under_eleventh_headroom hRejection hSupportGap

theorem fusedCredibility_last_open_fixed_rejection_strict_of_twelve_gap_under_twelfth_headroom
    (corroboratedCount rejectedCount confidence : Nat)
    (hGap :
      let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
      let proposalSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence)
      let corroboratedSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence)
      rejection ≤ 9_166 ∧ proposalSupport + 12 ≤ corroboratedSupport) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence := by
  let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
  let proposalSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (proposalSupportBps confidence)
  let corroboratedSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (corroboratedSupportBps confidence)
  have hRejectionClamp : clampBps rejection = rejection :=
    clampBps_id (accumulateNoisyOr_le_max 0 (rejectionSupportBps confidence) rejectedCount)
  have hRejection : clampBps rejection ≤ 9_166 := by
    simpa [proposalSupport, corroboratedSupport, rejection, hRejectionClamp] using hGap.1
  have hSupportGap : clampBps proposalSupport + 12 ≤ clampBps corroboratedSupport := by
    have hProposal : clampBps proposalSupport = proposalSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hCorroborated : clampBps corroboratedSupport = corroboratedSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    simpa [proposalSupport, corroboratedSupport, rejection, hProposal, hCorroborated] using hGap.2
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]
  exact attenuateSupport_strict_of_twelve_gap_under_twelfth_headroom hRejection hSupportGap

theorem fusedCredibility_last_open_fixed_rejection_strict_of_thirteen_gap_under_thirteenth_headroom
    (corroboratedCount rejectedCount confidence : Nat)
    (hGap :
      let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
      let proposalSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence)
      let corroboratedSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence)
      rejection ≤ 9_230 ∧ proposalSupport + 13 ≤ corroboratedSupport) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence := by
  let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
  let proposalSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (proposalSupportBps confidence)
  let corroboratedSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (corroboratedSupportBps confidence)
  have hRejectionClamp : clampBps rejection = rejection :=
    clampBps_id (accumulateNoisyOr_le_max 0 (rejectionSupportBps confidence) rejectedCount)
  have hRejection : clampBps rejection ≤ 9_230 := by
    simpa [proposalSupport, corroboratedSupport, rejection, hRejectionClamp] using hGap.1
  have hSupportGap : clampBps proposalSupport + 13 ≤ clampBps corroboratedSupport := by
    have hProposal : clampBps proposalSupport = proposalSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hCorroborated : clampBps corroboratedSupport = corroboratedSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    simpa [proposalSupport, corroboratedSupport, rejection, hProposal, hCorroborated] using hGap.2
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]
  exact attenuateSupport_strict_of_thirteen_gap_under_thirteenth_headroom hRejection hSupportGap

theorem fusedCredibility_last_open_fixed_rejection_strict_of_fourteen_gap_under_fourteenth_headroom
    (corroboratedCount rejectedCount confidence : Nat)
    (hGap :
      let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
      let proposalSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence)
      let corroboratedSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence)
      rejection ≤ 9_285 ∧ proposalSupport + 14 ≤ corroboratedSupport) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence := by
  let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
  let proposalSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (proposalSupportBps confidence)
  let corroboratedSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (corroboratedSupportBps confidence)
  have hRejectionClamp : clampBps rejection = rejection :=
    clampBps_id (accumulateNoisyOr_le_max 0 (rejectionSupportBps confidence) rejectedCount)
  have hRejection : clampBps rejection ≤ 9_285 := by
    simpa [proposalSupport, corroboratedSupport, rejection, hRejectionClamp] using hGap.1
  have hSupportGap : clampBps proposalSupport + 14 ≤ clampBps corroboratedSupport := by
    have hProposal : clampBps proposalSupport = proposalSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hCorroborated : clampBps corroboratedSupport = corroboratedSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    simpa [proposalSupport, corroboratedSupport, rejection, hProposal, hCorroborated] using hGap.2
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]
  exact attenuateSupport_strict_of_fourteen_gap_under_fourteenth_headroom hRejection hSupportGap

theorem fusedCredibility_last_open_fixed_rejection_strict_of_fifteen_gap_under_fifteenth_headroom
    (corroboratedCount rejectedCount confidence : Nat)
    (hGap :
      let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
      let proposalSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (proposalSupportBps confidence)
      let corroboratedSupport :=
        stepNoisyOr
          (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
          (corroboratedSupportBps confidence)
      rejection ≤ 9_333 ∧ proposalSupport + 15 ≤ corroboratedSupport) :
    fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        corroboratedCount
        rejectedCount
        confidence <
      fusedCredibility
        (corroboratedCount + rejectedCount + 1)
        (corroboratedCount + 1)
        rejectedCount
        confidence := by
  let rejection := accumulateNoisyOr 0 (rejectionSupportBps confidence) rejectedCount
  let proposalSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (proposalSupportBps confidence)
  let corroboratedSupport :=
    stepNoisyOr
      (accumulateNoisyOr 0 (corroboratedSupportBps confidence) corroboratedCount)
      (corroboratedSupportBps confidence)
  have hRejectionClamp : clampBps rejection = rejection :=
    clampBps_id (accumulateNoisyOr_le_max 0 (rejectionSupportBps confidence) rejectedCount)
  have hRejection : clampBps rejection ≤ 9_333 := by
    simpa [proposalSupport, corroboratedSupport, rejection, hRejectionClamp] using hGap.1
  have hSupportGap : clampBps proposalSupport + 15 ≤ clampBps corroboratedSupport := by
    have hProposal : clampBps proposalSupport = proposalSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    have hCorroborated : clampBps corroboratedSupport = corroboratedSupport :=
      clampBps_id (stepNoisyOr_le_max _ _)
    simpa [proposalSupport, corroboratedSupport, rejection, hProposal, hCorroborated] using hGap.2
  rw [fusedCredibility_last_open_fixed_rejection_eq_proposal_step]
  rw [fusedCredibility_last_open_fixed_rejection_eq_corroborated_step]
  exact attenuateSupport_strict_of_fifteen_gap_under_fifteenth_headroom hRejection hSupportGap

end HelixProofs.Credibility

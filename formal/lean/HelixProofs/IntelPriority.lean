import Mathlib.Data.Nat.Basic
import Mathlib.Tactic.Ring

namespace HelixProofs.IntelPriority

def radix : Nat := 6

def foldDigits (digits : List Nat) : Nat :=
  digits.foldl (fun acc digit => acc * radix + digit) 0

def tail5 (severity corroboration freshness trust density : Nat) : Nat :=
  severity * radix ^ 4 +
    corroboration * radix ^ 3 +
    freshness * radix ^ 2 +
    trust * radix +
    density

def composePriority
    (attention severity corroboration freshness trust density : Nat) : Nat :=
  attention * radix ^ 5 + tail5 severity corroboration freshness trust density

theorem composePriority_eq_foldDigits
    (attention severity corroboration freshness trust density : Nat) :
    composePriority attention severity corroboration freshness trust density =
      foldDigits [attention, severity, corroboration, freshness, trust, density] := by
  simp [composePriority, foldDigits, tail5, radix]
  ring_nf

theorem tail5_lt_radixPow5
    {severity corroboration freshness trust density : Nat}
    (hSeverity : severity < radix)
    (hCorroboration : corroboration < radix)
    (hFreshness : freshness < radix)
    (hTrust : trust < radix)
    (hDensity : density < radix) :
    tail5 severity corroboration freshness trust density < radix ^ 5 := by
  norm_num [tail5, radix] at hSeverity hCorroboration hFreshness hTrust hDensity ⊢
  omega

theorem composePriority_lt_radixPow6
    {attention severity corroboration freshness trust density : Nat}
    (hAttention : attention < radix)
    (hSeverity : severity < radix)
    (hCorroboration : corroboration < radix)
    (hFreshness : freshness < radix)
    (hTrust : trust < radix)
    (hDensity : density < radix) :
    composePriority attention severity corroboration freshness trust density < radix ^ 6 := by
  have hTail := tail5_lt_radixPow5 hSeverity hCorroboration hFreshness hTrust hDensity
  norm_num [composePriority, radix] at hAttention hTail ⊢
  omega

theorem attentionDominates
    {attention₁ attention₂ severity₁ severity₂ corroboration₁ corroboration₂
      freshness₁ freshness₂ trust₁ trust₂ density₁ density₂ : Nat}
    (hAttention : attention₁ < attention₂)
    (hSeverity₁ : severity₁ < radix)
    (hCorroboration₁ : corroboration₁ < radix)
    (hFreshness₁ : freshness₁ < radix)
    (hTrust₁ : trust₁ < radix)
    (hDensity₁ : density₁ < radix)
    (hSeverity₂ : severity₂ < radix)
    (hCorroboration₂ : corroboration₂ < radix)
    (hFreshness₂ : freshness₂ < radix)
    (hTrust₂ : trust₂ < radix)
    (hDensity₂ : density₂ < radix) :
    composePriority attention₁ severity₁ corroboration₁ freshness₁ trust₁ density₁ <
      composePriority attention₂ severity₂ corroboration₂ freshness₂ trust₂ density₂ := by
  have hTail₁ := tail5_lt_radixPow5
    hSeverity₁ hCorroboration₁ hFreshness₁ hTrust₁ hDensity₁
  have hTail₂ := tail5_lt_radixPow5
    hSeverity₂ hCorroboration₂ hFreshness₂ hTrust₂ hDensity₂
  norm_num [composePriority, radix] at hTail₁ hTail₂ ⊢
  omega

end HelixProofs.IntelPriority

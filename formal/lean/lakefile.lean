import Lake

open Lake DSL

package helix_proofs

require mathlib from git
  "https://github.com/leanprover-community/mathlib4.git" @ "v4.27.0"

@[default_target]
lean_lib HelixProofs

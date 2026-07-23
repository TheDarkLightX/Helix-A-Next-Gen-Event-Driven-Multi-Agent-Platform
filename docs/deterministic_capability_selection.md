# Deterministic capability selection

Helix can borrow the useful part of AnythingLLM's intelligent skill selection: do not send an unbounded tool catalog to a model when a small relevant subset is enough.

The trust model is different. AnythingLLM's selector may use an embedding reranker and fall back to the original tool set when reranking fails. Helix cannot make either behavior part of its trusted decision path. The functional core therefore provides a pure selector with the following contract:

1. The caller supplies an explicit query, selection budget, maximum risk class, and any required capability IDs.
2. The core validates a canonical, duplicate-free catalog.
3. A deterministic lexical score ranks eligible capabilities with stable ID tie-breaks.
4. Required capabilities must still fit the budget and risk bound.
5. Empty, ambiguous, invalid, or unmatched requests fail closed instead of exposing the full catalog.
6. A newly added catalog entry defaults to `critical` prompt risk until its exposure class is reviewed explicitly.
7. The result includes a SHA-256 catalog digest and normalized query terms so the selection can be replayed and audited.

## Security boundary

A selection receipt controls prompt construction only. It is not an authorization token and does not give the model new authority. Every proposed invocation still passes through the existing Helix policy, autopilot guard, and executor boundary.

This distinction permits aggressive prompt and token reduction without weakening the central rule:

> Models propose; deterministic kernels decide what may execute.

## Initial integration surface

`helix_core::deterministic_capability_selection` exposes:

- `deterministic_agent_capabilities()` to project the shipped deterministic agent catalog into model-visible descriptors;
- `select_capabilities()` to produce a bounded, replayable receipt; and
- `capability_catalog_digest()` to bind a receipt to exact catalog semantics.

A later shell-level change can expose this through an API and use the selected IDs when constructing an LLM request. That adapter should persist the receipt beside the proposal and must not treat selection as permission to execute.

## Deliberate non-goals

- No embedding model or network call in the functional core.
- No automatic fallback to all capabilities.
- No capability execution from the selector.
- No hidden clock, randomness, mutable cache, or provider-specific behavior.

# WASM sandbox capability enforcement

Helix uses WASM for narrow plugins and a separate VM tier for broad desktop or operating-system workloads. WASM isolation is useful only when host imports and resource controls are enforced rather than merely described in configuration.

## Security boundary

A module receives no ambient Helix authority. Its authority is the intersection of:

1. the exact host functions linked into its instance;
2. the agent configuration supplied to those functions;
3. the configured Wasmtime memory, table, stack, fuel, and wall-clock limits; and
4. the credential IDs explicitly assigned to that agent.

Unknown or duplicate host-function names reject runtime construction. Imports that were not granted remain unresolved and make module instantiation fail closed.

## Default profile

The default runtime exposes only:

- `helix_log_message`;
- `helix_get_config_value`; and
- `helix_get_state`.

These capabilities are read-only from the module's perspective. The following require explicit grants:

- event publication;
- state mutation;
- credential retrieval;
- host time; and
- legacy pseudo-randomness.

Even when `helix_get_credential` is granted, the requested credential ID must appear in `AgentConfig.credential_ids`. Possessing the import is not blanket vault access.

## Resource enforcement

Fuel is reset for every invocation, so `max_instructions` is a per-call bound rather than a lifetime accident. Wasmtime epoch interruption is enabled and a host thread advances the engine epoch after `max_execution_time_ms`. A timed-out instance is removed rather than silently reused.

Memory, table count, and table-element limits are applied through `StoreLimits`. Stack depth is configured at the engine level.

## Validation

The branch runs crate-local formatting, unit tests, and clippy in addition to the repository release gate. The focused run captures bounded diagnostics for any Wasmtime API, timeout, capability-linking, or lint failure. Temporary diagnostics are removed before review.

## Unsupported features fail closed

The earlier runtime exposed configuration fields for WASI directories, environment variables, and sockets without wiring those controls into instantiation. The hardened runtime rejects those configurations until a policy-bound WASI adapter exists. Defaults therefore use:

```text
enable_wasi = false
allowed_dirs = none
allowed_env_vars = none
allow_network_sockets = false
```

This prevents an operator from believing a directory, environment, or network restriction is active when it is not.

## Relationship to the VM sandbox

Use WASM when the work fits a small explicit import surface. Use the policy-bound VM contract for browsers, compilers, package managers, native applications, or arbitrary Linux tooling.

Neither runtime authorizes its own effects. Helix policy and, where required, MPRD authorization remain outside the guest. A missing or failed sandbox must never trigger native host execution as a fallback.

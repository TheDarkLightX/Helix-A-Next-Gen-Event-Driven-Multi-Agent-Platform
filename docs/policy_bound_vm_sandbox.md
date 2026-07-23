# Policy-bound VM sandbox

## Decision

Helix should support a full virtual-computer execution tier for tasks that need a browser, shell, package installation, native applications, or arbitrary generated code. The VM is an **imperative execution shell**, not an authority boundary by itself.

The trusted path is:

```text
operator or model proposal
  -> deterministic sandbox admission
  -> constructor-gated execution plan
  -> isolated runtime adapter
  -> untrusted runtime receipt
  -> deterministic receipt validation
  -> content-addressed outputs and audit record
```

A model can propose a plan. Only the functional core can admit it. A VM adapter can execute only the admitted type. Downstream systems can accept only the validated receipt type.

## Why this complements WASM

Helix needs two execution tiers:

1. **WASM tier** for small plugins with narrow host functions, deterministic fuel, and no operating-system surface.
2. **VM tier** for realistic agent work: browsers, terminals, compilers, document tools, native applications, and long-running workflows.

WASM remains preferable when the task fits. A VM must not become the default merely because it is convenient.

## What to reuse from AnythingLLM Open Computer

The useful product pattern is:

- one isolated computer per agent or run;
- a shared immutable base image plus small copy-on-write overlays;
- a live desktop that the human can observe and interrupt;
- browser and accessibility APIs rather than screenshot-only coordinate guessing;
- host, local-server, or cloud inference behind one provider-neutral interface; and
- explicit artifact delivery from the guest back to the user.

Those ideas materially improve Helix UX. An operator can watch an agent research, build, test, render, and package work without granting it the host machine.

## Security posture Helix must strengthen

The reference implementation is a proof of concept, not the production security contract. Helix must not inherit these development conveniences as defaults:

- passwordless SSH or empty root/user passwords;
- passwordless guest `sudo` combined with externally reachable forwarded ports;
- host port forwarding without an explicit loopback bind;
- unrestricted guest egress;
- long-lived API keys copied into a guest `.env` or disk;
- writable host-directory sharing such as a production 9p mount;
- arbitrary device, socket, or GPU passthrough;
- unverified base images, kernels, or VMM binaries; or
- fallback from sandbox execution to native host execution.

Root inside a VM is not automatically a problem. The security boundary must live outside the guest: hardware virtualization, a jailed VMM, host-enforced resources, default-deny networking, immutable image commitments, and a credential gateway the guest cannot bypass.

## Production invariants

### Image and runtime provenance

Every execution plan commits to:

- runtime/backend family;
- VMM and launcher configuration digest;
- guest kernel digest;
- immutable root filesystem digest;
- policy epoch; and
- a non-zero execution nonce.

Production policies allowlist exact commitments. Tags such as `latest` are not admissible identities.

### Filesystem

The core language contains no host path. Inputs are content-addressed artifacts attached read-only at canonical guest paths. Outputs are declared guest paths with byte limits and are collected into a content-addressed manifest.

The base root filesystem is immutable. The writable overlay is ephemeral and discarded after receipt collection. The VM is destroyed after the run unless a separate, explicit persistent-workspace policy is introduced.

### Network

Network egress defaults to deny. The admitted plan references a complete network-policy commitment. Supported postures are:

- no network;
- inference gateway only; or
- allowlisted egress through a policy-enforcing gateway.

The gateway, not the model, controls destination, port, protocol, HTTP method/path, DNS, and credential injection. Direct model-provider credentials must not be present in the guest filesystem.

### Credentials

A sandbox request can contain only an opaque short-lived lease commitment with a narrow provider and purpose. It cannot contain a secret value. The gateway may redeem the lease for one authorized request, inject the credential outside the guest, and record the use.

### Resources and lifecycle

CPU count, memory, overlay disk, process count, wall time, input size, and output size are hard host-enforced limits. Guest self-reporting is not sufficient.

A runtime receipt records the actual runtime identity, termination class, exit code, wall time, peak memory, stdout/stderr commitments, output-manifest commitment, and runtime-attestation commitment. Receipt validation rejects drift and resource overruns.

### Human control

The live desktop is an observability and collaboration surface, not the security boundary. The operator should be able to:

- pause or kill the run;
- deny a requested network-policy expansion;
- inspect active mounts, leases, and resource consumption;
- answer an explicit `ask-user` request; and
- export the admitted plan and validated receipt.

A pause, approval, or UI action must itself become an auditable event rather than hidden mutable state.

## Runtime strategy

### Linux production

Prefer a KVM-backed microVM runtime with a jailed VMM and host cgroup/network namespace enforcement. Firecracker is a strong initial backend for non-GUI workloads. A policy runtime such as OpenShell can supply controlled egress and credential mediation while using a microVM driver underneath.

### Desktop and GUI development

QEMU with HVF, WHPX, or KVM is appropriate for the Open Computer-style desktop UX and cross-platform development. Before production use, Helix must add explicit loopback binding, authenticated control channels, default-deny egress, signed image manifests, and a host-side jail/least-privilege launcher.

### GUI production

A full-system VM may remain necessary for XFCE/Chromium/native-app workflows. Treat it as a different reviewed runtime profile from a headless Firecracker execution. Do not claim that the two have identical attack surfaces.

## MPRD integration

MPRD should not run the VM inside its pure governor. It should authorize a `sandbox.run` action whose action preimage or execution limits commit to the Helix sandbox `plan_hash`.

The MPRD decision token then answers:

```text
May this exact policy, state, image, command, input set, egress policy,
credential lease set, resource budget, and nonce execute once?
```

The sandbox receipt can be attached to the MPRD execution receipt. MPRD still verifies policy, state provenance, anti-replay, executor identity, and proof bindings. A VM does not replace any of those checks.

## DotPublish integration

DotPublish should not expose a general autonomous VM as protocol authority. The valuable feature is an **executable publication**:

```text
publication revision
+ code/notebook artifact
+ dataset artifacts
+ runtime image and dependency lock commitments
+ sandbox plan
-> validated compute receipt
+ reproducible output artifacts
```

Readers could rerun charts, notebooks, simulations, static-site builds, document renderers, or media transforms. Results remain advisory unless the publication protocol separately defines deterministic equivalence, reproducible-build consensus, or proof verification.

Publisher, treasury, moderation, identity, and consensus signing keys must never enter the publication sandbox.

## Explicit nonclaims

A validated sandbox receipt proves that the recorded runtime identity and bounded execution were bound to the admitted plan. It does not prove:

- that a hypervisor has no vulnerability;
- that the guest program is correct;
- that generated output is true or safe;
- that a runtime attestation is meaningful before a backend-specific verifier checks it; or
- that semantically equivalent executions will produce identical output.

Those are separate assurance obligations.

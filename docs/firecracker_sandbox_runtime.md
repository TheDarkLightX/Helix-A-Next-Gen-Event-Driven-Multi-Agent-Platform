# Firecracker sandbox runtime adapter

This is the first imperative runtime behind Helix's policy-bound VM contract. It is intentionally an offline, Linux-only profile rather than a general-purpose agent desktop.

## Boundary

```text
untrusted request
  -> helix-core admission
  -> AdmittedSandboxExecution
  -> FirecrackerSandboxRuntime
  -> trusted digest-pinned runner + jailer
  -> untrusted SandboxExecutionReceipt
  -> helix-core receipt validation
  -> ValidatedSandboxReceipt
```

The runtime never accepts the model's raw request type. It accepts only the constructor-gated admitted value produced by the functional core.

## Initial supported profile

The v1 adapter supports only:

- Linux/KVM hosts;
- the `Firecracker` backend;
- an ephemeral root and destroy-after-run lifecycle;
- no host path mounts or device passthrough;
- mandatory runtime-attestation evidence;
- deny-all networking; and
- no credential leases.

Inference gateways, controlled egress, and credential redemption require separate policy-enforcing services. Until those services exist, the adapter rejects those requests instead of providing ambient networking or secrets.

## Runtime-profile verification

The operator configures absolute paths and SHA-256 commitments for:

- the trusted Helix Firecracker runner;
- the Firecracker jailer;
- the guest kernel; and
- the immutable root filesystem.

The adapter rejects symlinks and non-regular files, canonicalizes each path, streams and verifies every file digest before every launch, and checks that the admitted runtime, kernel, and rootfs commitments match the configured profile.

The runtime-profile hash commits to the runner identity, jailer identity, profile ID, protocol version, and protocol-size bound. Kernel and rootfs commitments remain separately visible in the admitted plan and receipt.

## Artifact staging

Callers cannot supply host paths. A trusted `SandboxArtifactResolver` maps each admitted artifact identity to a local source file. The adapter then:

1. rejects symlinks and non-regular files;
2. verifies the exact size and content hash;
3. copies the artifact into a plan-specific staging directory;
4. marks the staged copy read-only; and
5. verifies the staged copy again.

The runner sees only adapter-generated staging paths plus the canonical guest destination from the admitted plan.

## Process launch

The trusted runner is executed directly through `tokio::process::Command`:

- no shell;
- exact argument vector;
- cleared inherited environment;
- no inherited stdin, stdout, or stderr;
- `kill_on_drop` enabled; and
- host-side timeout equal to the admitted wall-clock budget.

The subprocess receives a versioned JSON envelope containing the admitted plan, exact component paths, staged inputs, output root, and receipt path. The envelope and receipt are size bounded.

A nonzero runner exit, timeout, missing receipt, oversized receipt, or malformed receipt rejects. There is no fallback to running the command natively on the host.

## Replay and cleanup

Each run owns a directory derived from the admitted plan hash. An existing directory is treated as a replay or concurrent execution and rejects. The directory is removed after every attempted execution. Cleanup failure rejects even if a valid guest receipt was produced.

MPRD still supplies authoritative nonce freshness. A plan-hash directory is a local defense and crash marker, not a globally authoritative anti-replay ledger.

## Trusted runner protocol

This PR defines the host adapter and protocol, but the digest-pinned `helix-firecracker-runner` binary remains a separately reviewable runtime component. That binary must:

- launch Firecracker only through the configured jailer or stronger containment;
- build cgroup and namespace limits from the admitted resource budget;
- boot the exact kernel and immutable rootfs;
- use an ephemeral writable overlay;
- transfer the exact argv and guest working directory to a minimal guest init;
- keep networking disabled for this profile;
- collect declared outputs into a content-addressed artifact store;
- derive host-observed memory and termination data; and
- emit backend-specific measured-launch or runtime-attestation evidence.

The adapter validates structural binding and limits. It does not treat a nonzero attestation hash as cryptographic proof without a backend-specific verifier.

## Stacking

This draft is stacked on the policy-contract PR. Merge and review order is:

1. policy-bound VM plan and receipt contract;
2. Firecracker runtime adapter and runner protocol;
3. the runner binary plus guest init image;
4. MPRD `sandbox_run` authorization;
5. allowlisted egress and credential gateway; and
6. QEMU live-desktop profile.

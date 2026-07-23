// Copyright 2026 DarkLightX
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Pure admission and receipt validation for policy-bound VM sandboxes.
//!
//! The functional core does not launch QEMU, Firecracker, Hyper-V, or any other
//! runtime. It admits an exact execution plan and later validates a shell-produced
//! receipt. A runtime adapter may consume only [`AdmittedSandboxExecution`], and
//! downstream code may consume only [`ValidatedSandboxReceipt`].
//!
//! The plan deliberately contains artifact identities rather than host paths and
//! secret-lease commitments rather than raw credentials. This keeps host I/O,
//! credential injection, networking, process creation, clocks, and VM lifecycle
//! in an imperative shell without granting that shell authority to widen policy.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use thiserror::Error;

const POLICY_HASH_DOMAIN_V1: &[u8] = b"helix:sandbox-policy:v1";
const PLAN_HASH_DOMAIN_V1: &[u8] = b"helix:sandbox-plan:v1";
const RECEIPT_HASH_DOMAIN_V1: &[u8] = b"helix:sandbox-receipt:v1";

const HARD_MAX_IDENTIFIER_BYTES: usize = 1024;
const HARD_MAX_GUEST_PATH_BYTES: usize = 4096;
const HARD_MAX_ARG_BYTES: usize = 64 * 1024;
const HARD_MAX_ARG_COUNT: usize = 4096;
const HARD_MAX_MOUNTS: usize = 1024;
const HARD_MAX_SECRET_LEASES: usize = 256;
const HARD_MAX_ALLOWLIST_ITEMS: usize = 4096;

/// A typed 32-byte commitment used by sandbox policies, plans, artifacts, and receipts.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct SandboxDigest([u8; 32]);

impl SandboxDigest {
    /// The all-zero sentinel. Admission rejects it wherever a real commitment is required.
    pub const ZERO: Self = Self([0; 32]);

    /// Construct a digest from exact bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return the committed bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Return canonical lowercase hexadecimal.
    #[must_use]
    pub fn to_hex(self) -> String {
        let mut output = String::with_capacity(64);
        for byte in self.0 {
            write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
        }
        output
    }
}

/// Runtime family selected by the policy-bound execution plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxBackend {
    /// Linux KVM microVM using Firecracker and its jailer (or stronger isolation).
    Firecracker,
    /// Cross-platform full-system VM using QEMU.
    Qemu,
    /// Kata Containers VM-backed runtime.
    Kata,
    /// Apple's Virtualization.framework backend.
    AppleVirtualization,
    /// Microsoft Hyper-V backend.
    HyperV,
}

impl SandboxBackend {
    const fn tag(self) -> u8 {
        match self {
            Self::Firecracker => 0,
            Self::Qemu => 1,
            Self::Kata => 2,
            Self::AppleVirtualization => 3,
            Self::HyperV => 4,
        }
    }
}

/// Network posture whose complete rules are committed by `policy_hash`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxNetworkMode {
    /// No guest network egress.
    DenyAll,
    /// Only the policy-controlled inference gateway is reachable.
    InferenceGatewayOnly,
    /// Egress is allowed only through an external policy-enforcing gateway.
    AllowlistedGateway,
}

impl SandboxNetworkMode {
    const fn tag(self) -> u8 {
        match self {
            Self::DenyAll => 0,
            Self::InferenceGatewayOnly => 1,
            Self::AllowlistedGateway => 2,
        }
    }
}

/// Reference to a complete externally enforced network policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxNetworkPolicyRef {
    /// Stable policy identifier for audit and operator display.
    pub policy_id: String,
    /// Canonical commitment to destinations, ports, methods, paths, DNS, and proxy behavior.
    pub policy_hash: SandboxDigest,
    /// Coarse posture interpreted by the pure admission kernel.
    pub mode: SandboxNetworkMode,
}

/// Hard resource budget enforced outside the guest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxResourceBudget {
    /// Maximum virtual CPUs exposed to the guest.
    pub vcpus: u16,
    /// Maximum guest memory in MiB.
    pub memory_mib: u32,
    /// Maximum writable overlay disk in MiB.
    pub disk_mib: u32,
    /// Hard wall-clock execution limit supplied to the runtime shell.
    pub wall_time_ms: u64,
    /// Maximum process count, enforced by the runtime/cgroup boundary.
    pub process_limit: u32,
    /// Maximum aggregate bytes admitted from declared outputs.
    pub max_output_bytes: u64,
}

/// Read-only artifact copied or attached into the guest by identity, never by caller host path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxInputArtifact {
    /// Stable artifact-registry identity.
    pub artifact_id: String,
    /// Exact artifact content commitment.
    pub content_hash: SandboxDigest,
    /// Absolute canonical path visible inside the guest.
    pub guest_path: String,
    /// Declared input size used for deterministic admission bounds.
    pub size_bytes: u64,
}

/// Writable guest path whose collected result becomes a content-addressed artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxOutputContract {
    /// Stable output name used in manifests and UI.
    pub output_id: String,
    /// Absolute canonical guest path. No host path is accepted by this API.
    pub guest_path: String,
    /// Maximum bytes collected from this output.
    pub max_bytes: u64,
}

/// Opaque reference to a short-lived credential capability held by a gateway.
///
/// The sandbox request never contains the credential value. The gateway may use
/// this commitment to inject credentials for one approved request without writing
/// them into the guest filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxSecretLeaseRef {
    /// Stable provider identity, such as `github-readonly`.
    pub provider_id: String,
    /// Narrow purpose shown to policy and operators.
    pub purpose: String,
    /// Commitment to the externally issued, short-lived lease.
    pub lease_hash: SandboxDigest,
}

/// Explicit isolation assertions that a runtime adapter must enforce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxIsolationRequirements {
    /// The writable root/overlay is discarded after the run.
    pub ephemeral_root: bool,
    /// The VM is destroyed after receipt collection.
    pub destroy_after_run: bool,
    /// No host device, socket, or GPU is passed through unless a future policy language says so.
    pub device_passthrough_disabled: bool,
    /// No arbitrary host directory is shared into the guest.
    pub host_path_mounts_disabled: bool,
    /// The runtime must return a non-zero attestation commitment.
    pub runtime_attestation_required: bool,
}

impl SandboxIsolationRequirements {
    /// Strict production defaults.
    pub const STRICT: Self = Self {
        ephemeral_root: true,
        destroy_after_run: true,
        device_passthrough_disabled: true,
        host_path_mounts_disabled: true,
        runtime_attestation_required: true,
    };
}

/// Complete untrusted sandbox request proposed by a model or orchestration shell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxExecutionRequest {
    /// Stable idempotency identity for this request.
    pub request_id: String,
    /// Monotonic externally supplied execution epoch.
    pub execution_epoch: u64,
    /// Selected runtime family.
    pub backend: SandboxBackend,
    /// Commitment to the VMM/runtime adapter and its security configuration.
    pub runtime_digest: SandboxDigest,
    /// Commitment to the guest kernel.
    pub kernel_digest: SandboxDigest,
    /// Commitment to the immutable base root filesystem.
    pub rootfs_digest: SandboxDigest,
    /// Non-zero anti-replay nonce committed into the plan.
    pub execution_nonce: SandboxDigest,
    /// Exact argv vector. No shell interpolation is implied.
    pub argv: Vec<String>,
    /// Canonical absolute working directory inside the guest.
    pub working_dir: String,
    /// Read-only content-addressed inputs.
    pub inputs: Vec<SandboxInputArtifact>,
    /// Declared writable outputs.
    pub outputs: Vec<SandboxOutputContract>,
    /// Short-lived gateway-held credential capabilities.
    pub secret_leases: Vec<SandboxSecretLeaseRef>,
    /// Complete committed egress policy reference.
    pub network: SandboxNetworkPolicyRef,
    /// Hard runtime budget.
    pub resources: SandboxResourceBudget,
    /// Required isolation behavior.
    pub isolation: SandboxIsolationRequirements,
}

/// Admission policy for sandbox execution plans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxAdmissionPolicy {
    /// Stable policy identity.
    pub policy_id: String,
    /// Monotonic policy epoch.
    pub policy_epoch: u64,
    /// Runtime families permitted by this policy.
    pub allowed_backends: BTreeSet<SandboxBackend>,
    /// Exact approved runtime/VMM configuration commitments.
    pub allowed_runtime_digests: BTreeSet<SandboxDigest>,
    /// Exact approved guest kernels.
    pub allowed_kernel_digests: BTreeSet<SandboxDigest>,
    /// Exact approved immutable base images.
    pub allowed_rootfs_digests: BTreeSet<SandboxDigest>,
    /// Exact approved network policies.
    pub allowed_network_policy_hashes: BTreeSet<SandboxDigest>,
    /// Maximum resources for any admitted request.
    pub max_resources: SandboxResourceBudget,
    /// Maximum argv elements.
    pub max_argv: usize,
    /// Maximum bytes across all argv elements.
    pub max_argv_bytes: usize,
    /// Maximum input artifact count.
    pub max_inputs: usize,
    /// Maximum aggregate input size.
    pub max_input_bytes: u64,
    /// Maximum output contract count.
    pub max_outputs: usize,
    /// Maximum short-lived credential leases.
    pub max_secret_leases: usize,
    /// Whether any secret lease may be requested.
    pub allow_secret_leases: bool,
    /// Isolation assertions that must all be present in the request.
    pub required_isolation: SandboxIsolationRequirements,
}

/// Constructor-gated plan consumable by a VM runtime shell.
///
/// This type intentionally does not implement `Deserialize`, so arbitrary bytes
/// cannot become an admitted execution without passing through
/// [`admit_sandbox_execution`].
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdmittedSandboxExecution {
    request: SandboxExecutionRequest,
    policy_hash: SandboxDigest,
    plan_hash: SandboxDigest,
}

impl AdmittedSandboxExecution {
    /// Return the canonicalized admitted request.
    #[must_use]
    pub fn request(&self) -> &SandboxExecutionRequest {
        &self.request
    }

    /// Return the exact policy commitment used for admission.
    #[must_use]
    pub const fn policy_hash(&self) -> &SandboxDigest {
        &self.policy_hash
    }

    /// Return the commitment the runtime and receipt must preserve.
    #[must_use]
    pub const fn plan_hash(&self) -> &SandboxDigest {
        &self.plan_hash
    }
}

/// Runtime termination classification reported by the imperative shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxTermination {
    /// Guest process exited normally; `exit_code` must be present.
    Exited,
    /// Hard wall-clock budget expired.
    TimedOut,
    /// Runtime enforced an out-of-memory termination.
    OutOfMemory,
    /// Process/resource policy rejected or killed the run.
    PolicyDenied,
    /// Runtime failed before a normal guest exit.
    RuntimeFailure,
}

impl SandboxTermination {
    const fn tag(self) -> u8 {
        match self {
            Self::Exited => 0,
            Self::TimedOut => 1,
            Self::OutOfMemory => 2,
            Self::PolicyDenied => 3,
            Self::RuntimeFailure => 4,
        }
    }
}

/// Untrusted execution receipt emitted by a VM runtime shell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxExecutionReceipt {
    /// Request identity copied from the admitted plan.
    pub request_id: String,
    /// Exact admitted plan commitment.
    pub plan_hash: SandboxDigest,
    /// Runtime family actually used.
    pub backend: SandboxBackend,
    /// Runtime/VMM configuration actually used.
    pub runtime_digest: SandboxDigest,
    /// Kernel actually booted.
    pub kernel_digest: SandboxDigest,
    /// Root filesystem actually booted.
    pub rootfs_digest: SandboxDigest,
    /// Backend attestation, measured-launch, or equivalent runtime evidence commitment.
    pub runtime_attestation_hash: Option<SandboxDigest>,
    /// Canonical termination class.
    pub termination: SandboxTermination,
    /// Present only for normal guest process exit.
    pub exit_code: Option<i32>,
    /// Observed wall-clock duration supplied by the runtime.
    pub wall_time_ms: u64,
    /// Peak memory observed by the host/runtime boundary.
    pub peak_memory_mib: u32,
    /// Number of declared outputs actually collected.
    pub produced_output_count: u32,
    /// Aggregate collected output bytes.
    pub total_output_bytes: u64,
    /// Commitment to captured stdout, including the empty byte string when empty.
    pub stdout_hash: SandboxDigest,
    /// Commitment to captured stderr, including the empty byte string when empty.
    pub stderr_hash: SandboxDigest,
    /// Commitment to the canonical output artifact manifest.
    pub output_manifest_hash: SandboxDigest,
}

/// Constructor-gated receipt safe for audit, cache, and downstream acceptance logic.
///
/// Validation proves binding and bounds, not semantic correctness of the guest's output.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidatedSandboxReceipt {
    receipt: SandboxExecutionReceipt,
    policy_hash: SandboxDigest,
    receipt_hash: SandboxDigest,
}

impl ValidatedSandboxReceipt {
    /// Return the validated runtime receipt.
    #[must_use]
    pub fn receipt(&self) -> &SandboxExecutionReceipt {
        &self.receipt
    }

    /// Return the policy commitment inherited from the admitted plan.
    #[must_use]
    pub const fn policy_hash(&self) -> &SandboxDigest {
        &self.policy_hash
    }

    /// Return the canonical commitment to the validated receipt.
    #[must_use]
    pub const fn receipt_hash(&self) -> &SandboxDigest {
        &self.receipt_hash
    }

    /// True only for a normal zero exit. Other termination classes remain valid receipts,
    /// but are not successful executions.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        self.receipt.termination == SandboxTermination::Exited
            && self.receipt.exit_code == Some(0)
    }
}

/// Typed failures from policy, request, and receipt validation.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SandboxExecutionError {
    /// The supplied policy is internally invalid.
    #[error("invalid sandbox policy: {0}")]
    InvalidPolicy(String),
    /// A canonical identifier is missing.
    #[error("{field} must not be empty")]
    EmptyIdentifier { field: &'static str },
    /// A canonical identifier contains unsupported bytes or surrounding whitespace.
    #[error("{field} must be canonical visible ASCII without surrounding whitespace")]
    NonCanonicalIdentifier { field: &'static str },
    /// A canonical identifier exceeds policy or hard bounds.
    #[error("{field} is {actual} bytes; maximum is {max}")]
    IdentifierTooLong {
        field: &'static str,
        actual: usize,
        max: usize,
    },
    /// A required cryptographic commitment is the all-zero sentinel.
    #[error("{field} must not be the all-zero digest")]
    ZeroDigest { field: &'static str },
    /// Request epoch zero is reserved as unknown/unbound.
    #[error("execution_epoch must be greater than zero")]
    InvalidExecutionEpoch,
    /// Runtime family is not approved by policy.
    #[error("sandbox backend is not allowed: {0:?}")]
    BackendDenied(SandboxBackend),
    /// Runtime configuration commitment is not approved.
    #[error("sandbox runtime digest is not allowlisted")]
    RuntimeDigestDenied,
    /// Kernel commitment is not approved.
    #[error("sandbox kernel digest is not allowlisted")]
    KernelDigestDenied,
    /// Root filesystem commitment is not approved.
    #[error("sandbox rootfs digest is not allowlisted")]
    RootfsDigestDenied,
    /// Network policy commitment is not approved.
    #[error("sandbox network policy digest is not allowlisted")]
    NetworkPolicyDenied,
    /// Command vector is empty.
    #[error("sandbox argv must include an executable")]
    EmptyArgv,
    /// Command vector has too many elements.
    #[error("sandbox argv contains {actual} elements; maximum is {max}")]
    TooManyArgs { actual: usize, max: usize },
    /// Command vector contains too many aggregate bytes.
    #[error("sandbox argv is {actual} bytes; maximum is {max}")]
    ArgvTooLarge { actual: usize, max: usize },
    /// Command argument is empty or contains a NUL byte.
    #[error("sandbox argv element {index} is not canonical")]
    InvalidArg { index: usize },
    /// A guest path is not canonical absolute Linux syntax.
    #[error("{field} is not a canonical absolute guest path: {path}")]
    InvalidGuestPath {
        field: &'static str,
        path: String,
    },
    /// Two artifact paths overlap and would produce ambiguous mount behavior.
    #[error("sandbox guest paths overlap: {left} and {right}")]
    OverlappingGuestPaths { left: String, right: String },
    /// Too many input artifacts were proposed.
    #[error("sandbox has {actual} inputs; maximum is {max}")]
    TooManyInputs { actual: usize, max: usize },
    /// Aggregate input bytes exceed policy.
    #[error("sandbox input bytes are {actual}; maximum is {max}")]
    InputBytesExceeded { actual: u64, max: u64 },
    /// Duplicate input identity or guest path.
    #[error("duplicate sandbox input: {0}")]
    DuplicateInput(String),
    /// An input has zero declared size.
    #[error("sandbox input {0} must have non-zero size")]
    EmptyInput(String),
    /// Too many output contracts were proposed.
    #[error("sandbox has {actual} outputs; maximum is {max}")]
    TooManyOutputs { actual: usize, max: usize },
    /// Duplicate output identity or guest path.
    #[error("duplicate sandbox output: {0}")]
    DuplicateOutput(String),
    /// Output contract has a zero byte limit.
    #[error("sandbox output {0} must have a non-zero max_bytes")]
    EmptyOutputBudget(String),
    /// Credential leases are disabled.
    #[error("sandbox secret leases are disabled by policy")]
    SecretLeasesDenied,
    /// Too many credential leases were proposed.
    #[error("sandbox has {actual} secret leases; maximum is {max}")]
    TooManySecretLeases { actual: usize, max: usize },
    /// Duplicate provider/purpose pair or lease commitment.
    #[error("duplicate sandbox secret lease: {0}")]
    DuplicateSecretLease(String),
    /// One or more resource fields are zero.
    #[error("sandbox resource budget fields must all be non-zero")]
    ZeroResourceBudget,
    /// A resource field exceeds policy.
    #[error("sandbox resource {field} is {actual}; maximum is {max}")]
    ResourceExceeded {
        field: &'static str,
        actual: u64,
        max: u64,
    },
    /// A required isolation assertion is absent.
    #[error("sandbox isolation requirement is missing: {0}")]
    IsolationRequirementMissing(&'static str),
    /// Receipt request identity does not match the admitted plan.
    #[error("sandbox receipt request_id mismatch")]
    ReceiptRequestMismatch,
    /// Receipt plan commitment does not match the admitted plan.
    #[error("sandbox receipt plan_hash mismatch")]
    ReceiptPlanMismatch,
    /// Receipt runtime identity does not match the admitted plan.
    #[error("sandbox receipt runtime identity mismatch: {0}")]
    ReceiptRuntimeMismatch(&'static str),
    /// Required runtime attestation is absent or zero.
    #[error("sandbox receipt is missing a non-zero runtime attestation")]
    MissingRuntimeAttestation,
    /// Exit-code presence is inconsistent with the termination class.
    #[error("sandbox receipt exit_code is inconsistent with termination")]
    InvalidExitCode,
    /// Runtime exceeded the admitted wall-clock budget.
    #[error("sandbox receipt wall time {actual} ms exceeds {max} ms")]
    ReceiptWallTimeExceeded { actual: u64, max: u64 },
    /// Runtime exceeded the admitted memory budget.
    #[error("sandbox receipt peak memory {actual} MiB exceeds {max} MiB")]
    ReceiptMemoryExceeded { actual: u32, max: u32 },
    /// More outputs were collected than declared.
    #[error("sandbox receipt produced {actual} outputs; maximum is {max}")]
    ReceiptOutputCountExceeded { actual: u32, max: u32 },
    /// Aggregate output bytes exceed the admitted budget.
    #[error("sandbox receipt output bytes are {actual}; maximum is {max}")]
    ReceiptOutputBytesExceeded { actual: u64, max: u64 },
}

/// Admit an exact sandbox plan under a deterministic policy.
pub fn admit_sandbox_execution(
    policy: &SandboxAdmissionPolicy,
    mut request: SandboxExecutionRequest,
) -> Result<AdmittedSandboxExecution, SandboxExecutionError> {
    validate_policy(policy)?;
    validate_identifier("request_id", &request.request_id, policy.max_argv_bytes)?;
    if request.execution_epoch == 0 {
        return Err(SandboxExecutionError::InvalidExecutionEpoch);
    }

    require_non_zero("runtime_digest", request.runtime_digest)?;
    require_non_zero("kernel_digest", request.kernel_digest)?;
    require_non_zero("rootfs_digest", request.rootfs_digest)?;
    require_non_zero("execution_nonce", request.execution_nonce)?;

    if !policy.allowed_backends.contains(&request.backend) {
        return Err(SandboxExecutionError::BackendDenied(request.backend));
    }
    if !policy.allowed_runtime_digests.contains(&request.runtime_digest) {
        return Err(SandboxExecutionError::RuntimeDigestDenied);
    }
    if !policy.allowed_kernel_digests.contains(&request.kernel_digest) {
        return Err(SandboxExecutionError::KernelDigestDenied);
    }
    if !policy.allowed_rootfs_digests.contains(&request.rootfs_digest) {
        return Err(SandboxExecutionError::RootfsDigestDenied);
    }

    validate_identifier(
        "network.policy_id",
        &request.network.policy_id,
        policy.max_argv_bytes,
    )?;
    require_non_zero("network.policy_hash", request.network.policy_hash)?;
    if !policy
        .allowed_network_policy_hashes
        .contains(&request.network.policy_hash)
    {
        return Err(SandboxExecutionError::NetworkPolicyDenied);
    }

    validate_resources(&request.resources, &policy.max_resources)?;
    validate_isolation(&request.isolation, &policy.required_isolation)?;
    validate_argv(&request.argv, policy.max_argv, policy.max_argv_bytes)?;
    validate_guest_path("working_dir", &request.working_dir, true)?;

    canonicalize_inputs(policy, &mut request.inputs)?;
    canonicalize_outputs(policy, &mut request.outputs)?;
    canonicalize_secret_leases(policy, &mut request.secret_leases)?;
    validate_mount_path_disjointness(&request.inputs, &request.outputs)?;

    let policy_hash = hash_policy_unchecked(policy);
    let plan_hash = hash_plan_unchecked(&policy_hash, &request);
    Ok(AdmittedSandboxExecution {
        request,
        policy_hash,
        plan_hash,
    })
}

/// Compute the canonical commitment to a validated sandbox policy.
pub fn sandbox_policy_hash(
    policy: &SandboxAdmissionPolicy,
) -> Result<SandboxDigest, SandboxExecutionError> {
    validate_policy(policy)?;
    Ok(hash_policy_unchecked(policy))
}

/// Validate a runtime receipt against an admitted plan.
pub fn validate_sandbox_receipt(
    admitted: &AdmittedSandboxExecution,
    receipt: SandboxExecutionReceipt,
) -> Result<ValidatedSandboxReceipt, SandboxExecutionError> {
    if receipt.request_id != admitted.request.request_id {
        return Err(SandboxExecutionError::ReceiptRequestMismatch);
    }
    if receipt.plan_hash != admitted.plan_hash {
        return Err(SandboxExecutionError::ReceiptPlanMismatch);
    }
    if receipt.backend != admitted.request.backend {
        return Err(SandboxExecutionError::ReceiptRuntimeMismatch("backend"));
    }
    if receipt.runtime_digest != admitted.request.runtime_digest {
        return Err(SandboxExecutionError::ReceiptRuntimeMismatch(
            "runtime_digest",
        ));
    }
    if receipt.kernel_digest != admitted.request.kernel_digest {
        return Err(SandboxExecutionError::ReceiptRuntimeMismatch(
            "kernel_digest",
        ));
    }
    if receipt.rootfs_digest != admitted.request.rootfs_digest {
        return Err(SandboxExecutionError::ReceiptRuntimeMismatch("rootfs_digest"));
    }

    if admitted.request.isolation.runtime_attestation_required {
        match receipt.runtime_attestation_hash {
            Some(value) if value != SandboxDigest::ZERO => {}
            _ => return Err(SandboxExecutionError::MissingRuntimeAttestation),
        }
    } else if receipt.runtime_attestation_hash == Some(SandboxDigest::ZERO) {
        return Err(SandboxExecutionError::MissingRuntimeAttestation);
    }

    match (receipt.termination, receipt.exit_code) {
        (SandboxTermination::Exited, Some(_)) => {}
        (SandboxTermination::Exited, None) => {
            return Err(SandboxExecutionError::InvalidExitCode)
        }
        (_, None) => {}
        (_, Some(_)) => return Err(SandboxExecutionError::InvalidExitCode),
    }

    if receipt.wall_time_ms > admitted.request.resources.wall_time_ms {
        return Err(SandboxExecutionError::ReceiptWallTimeExceeded {
            actual: receipt.wall_time_ms,
            max: admitted.request.resources.wall_time_ms,
        });
    }
    if receipt.peak_memory_mib > admitted.request.resources.memory_mib {
        return Err(SandboxExecutionError::ReceiptMemoryExceeded {
            actual: receipt.peak_memory_mib,
            max: admitted.request.resources.memory_mib,
        });
    }

    let max_output_count = u32::try_from(admitted.request.outputs.len()).map_err(|_| {
        SandboxExecutionError::InvalidPolicy("output count does not fit u32".to_string())
    })?;
    if receipt.produced_output_count > max_output_count {
        return Err(SandboxExecutionError::ReceiptOutputCountExceeded {
            actual: receipt.produced_output_count,
            max: max_output_count,
        });
    }

    let declared_output_bytes = admitted
        .request
        .outputs
        .iter()
        .try_fold(0_u64, |sum, output| sum.checked_add(output.max_bytes))
        .ok_or_else(|| {
            SandboxExecutionError::InvalidPolicy(
                "declared output byte total overflows u64".to_string(),
            )
        })?;
    let max_output_bytes = declared_output_bytes.min(admitted.request.resources.max_output_bytes);
    if receipt.total_output_bytes > max_output_bytes {
        return Err(SandboxExecutionError::ReceiptOutputBytesExceeded {
            actual: receipt.total_output_bytes,
            max: max_output_bytes,
        });
    }

    require_non_zero("receipt.stdout_hash", receipt.stdout_hash)?;
    require_non_zero("receipt.stderr_hash", receipt.stderr_hash)?;
    require_non_zero(
        "receipt.output_manifest_hash",
        receipt.output_manifest_hash,
    )?;

    let receipt_hash = hash_receipt_unchecked(&admitted.policy_hash, &receipt);
    Ok(ValidatedSandboxReceipt {
        receipt,
        policy_hash: admitted.policy_hash,
        receipt_hash,
    })
}

fn validate_policy(policy: &SandboxAdmissionPolicy) -> Result<(), SandboxExecutionError> {
    validate_identifier(
        "policy_id",
        &policy.policy_id,
        HARD_MAX_IDENTIFIER_BYTES,
    )?;
    if policy.policy_epoch == 0 {
        return Err(SandboxExecutionError::InvalidPolicy(
            "policy_epoch must be greater than zero".to_string(),
        ));
    }
    if policy.allowed_backends.is_empty() {
        return Err(SandboxExecutionError::InvalidPolicy(
            "allowed_backends must not be empty".to_string(),
        ));
    }
    validate_digest_allowlist(
        "allowed_runtime_digests",
        &policy.allowed_runtime_digests,
    )?;
    validate_digest_allowlist("allowed_kernel_digests", &policy.allowed_kernel_digests)?;
    validate_digest_allowlist("allowed_rootfs_digests", &policy.allowed_rootfs_digests)?;
    validate_digest_allowlist(
        "allowed_network_policy_hashes",
        &policy.allowed_network_policy_hashes,
    )?;
    validate_resources(&policy.max_resources, &policy.max_resources)?;

    validate_policy_count("max_argv", policy.max_argv, HARD_MAX_ARG_COUNT)?;
    validate_policy_count(
        "max_argv_bytes",
        policy.max_argv_bytes,
        HARD_MAX_ARG_BYTES,
    )?;
    validate_policy_count("max_inputs", policy.max_inputs, HARD_MAX_MOUNTS)?;
    validate_policy_count("max_outputs", policy.max_outputs, HARD_MAX_MOUNTS)?;
    if policy.max_input_bytes == 0 {
        return Err(SandboxExecutionError::InvalidPolicy(
            "max_input_bytes must be greater than zero".to_string(),
        ));
    }
    if policy.max_secret_leases > HARD_MAX_SECRET_LEASES {
        return Err(SandboxExecutionError::InvalidPolicy(format!(
            "max_secret_leases must not exceed {HARD_MAX_SECRET_LEASES}"
        )));
    }
    if !policy.allow_secret_leases && policy.max_secret_leases != 0 {
        return Err(SandboxExecutionError::InvalidPolicy(
            "max_secret_leases must be zero when secret leases are disabled".to_string(),
        ));
    }
    if policy.allow_secret_leases && policy.max_secret_leases == 0 {
        return Err(SandboxExecutionError::InvalidPolicy(
            "max_secret_leases must be positive when secret leases are enabled".to_string(),
        ));
    }
    Ok(())
}

fn validate_digest_allowlist(
    field: &'static str,
    values: &BTreeSet<SandboxDigest>,
) -> Result<(), SandboxExecutionError> {
    if values.is_empty() {
        return Err(SandboxExecutionError::InvalidPolicy(format!(
            "{field} must not be empty"
        )));
    }
    if values.len() > HARD_MAX_ALLOWLIST_ITEMS {
        return Err(SandboxExecutionError::InvalidPolicy(format!(
            "{field} must not exceed {HARD_MAX_ALLOWLIST_ITEMS} entries"
        )));
    }
    if values.contains(&SandboxDigest::ZERO) {
        return Err(SandboxExecutionError::InvalidPolicy(format!(
            "{field} must not contain the all-zero digest"
        )));
    }
    Ok(())
}

fn validate_policy_count(
    field: &'static str,
    value: usize,
    hard_max: usize,
) -> Result<(), SandboxExecutionError> {
    if value == 0 || value > hard_max {
        return Err(SandboxExecutionError::InvalidPolicy(format!(
            "{field} must be between 1 and {hard_max}"
        )));
    }
    Ok(())
}

fn validate_identifier(
    field: &'static str,
    value: &str,
    configured_max: usize,
) -> Result<(), SandboxExecutionError> {
    if value.is_empty() {
        return Err(SandboxExecutionError::EmptyIdentifier { field });
    }
    if value.trim() != value
        || !value.is_ascii()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == 0x7f)
    {
        return Err(SandboxExecutionError::NonCanonicalIdentifier { field });
    }
    let max = configured_max.min(HARD_MAX_IDENTIFIER_BYTES);
    if value.len() > max {
        return Err(SandboxExecutionError::IdentifierTooLong {
            field,
            actual: value.len(),
            max,
        });
    }
    Ok(())
}

fn require_non_zero(
    field: &'static str,
    digest: SandboxDigest,
) -> Result<(), SandboxExecutionError> {
    if digest == SandboxDigest::ZERO {
        return Err(SandboxExecutionError::ZeroDigest { field });
    }
    Ok(())
}

fn validate_argv(
    argv: &[String],
    max_argv: usize,
    max_argv_bytes: usize,
) -> Result<(), SandboxExecutionError> {
    if argv.is_empty() {
        return Err(SandboxExecutionError::EmptyArgv);
    }
    if argv.len() > max_argv {
        return Err(SandboxExecutionError::TooManyArgs {
            actual: argv.len(),
            max: max_argv,
        });
    }
    let mut total = 0_usize;
    for (index, arg) in argv.iter().enumerate() {
        if arg.is_empty() || arg.as_bytes().contains(&0) {
            return Err(SandboxExecutionError::InvalidArg { index });
        }
        total = total.checked_add(arg.len()).ok_or(
            SandboxExecutionError::ArgvTooLarge {
                actual: usize::MAX,
                max: max_argv_bytes,
            },
        )?;
    }
    if total > max_argv_bytes {
        return Err(SandboxExecutionError::ArgvTooLarge {
            actual: total,
            max: max_argv_bytes,
        });
    }
    Ok(())
}

fn validate_guest_path(
    field: &'static str,
    path: &str,
    allow_root: bool,
) -> Result<(), SandboxExecutionError> {
    let valid_bytes = path.is_ascii()
        && path.trim() == path
        && !path.bytes().any(|byte| byte.is_ascii_control())
        && !path.contains('\\')
        && path.len() <= HARD_MAX_GUEST_PATH_BYTES;
    if !valid_bytes || !path.starts_with('/') || (!allow_root && path == "/") {
        return Err(SandboxExecutionError::InvalidGuestPath {
            field,
            path: path.to_string(),
        });
    }
    if path.len() > 1 && path.ends_with('/') {
        return Err(SandboxExecutionError::InvalidGuestPath {
            field,
            path: path.to_string(),
        });
    }
    for component in path.split('/').skip(1) {
        if component.is_empty() || component == "." || component == ".." {
            return Err(SandboxExecutionError::InvalidGuestPath {
                field,
                path: path.to_string(),
            });
        }
    }
    Ok(())
}

fn validate_resources(
    actual: &SandboxResourceBudget,
    max: &SandboxResourceBudget,
) -> Result<(), SandboxExecutionError> {
    if actual.vcpus == 0
        || actual.memory_mib == 0
        || actual.disk_mib == 0
        || actual.wall_time_ms == 0
        || actual.process_limit == 0
        || actual.max_output_bytes == 0
    {
        return Err(SandboxExecutionError::ZeroResourceBudget);
    }
    compare_resource("vcpus", u64::from(actual.vcpus), u64::from(max.vcpus))?;
    compare_resource(
        "memory_mib",
        u64::from(actual.memory_mib),
        u64::from(max.memory_mib),
    )?;
    compare_resource(
        "disk_mib",
        u64::from(actual.disk_mib),
        u64::from(max.disk_mib),
    )?;
    compare_resource("wall_time_ms", actual.wall_time_ms, max.wall_time_ms)?;
    compare_resource(
        "process_limit",
        u64::from(actual.process_limit),
        u64::from(max.process_limit),
    )?;
    compare_resource(
        "max_output_bytes",
        actual.max_output_bytes,
        max.max_output_bytes,
    )
}

fn compare_resource(
    field: &'static str,
    actual: u64,
    max: u64,
) -> Result<(), SandboxExecutionError> {
    if actual > max {
        return Err(SandboxExecutionError::ResourceExceeded { field, actual, max });
    }
    Ok(())
}

fn validate_isolation(
    actual: &SandboxIsolationRequirements,
    required: &SandboxIsolationRequirements,
) -> Result<(), SandboxExecutionError> {
    for (name, required_value, actual_value) in [
        ("ephemeral_root", required.ephemeral_root, actual.ephemeral_root),
        (
            "destroy_after_run",
            required.destroy_after_run,
            actual.destroy_after_run,
        ),
        (
            "device_passthrough_disabled",
            required.device_passthrough_disabled,
            actual.device_passthrough_disabled,
        ),
        (
            "host_path_mounts_disabled",
            required.host_path_mounts_disabled,
            actual.host_path_mounts_disabled,
        ),
        (
            "runtime_attestation_required",
            required.runtime_attestation_required,
            actual.runtime_attestation_required,
        ),
    ] {
        if required_value && !actual_value {
            return Err(SandboxExecutionError::IsolationRequirementMissing(name));
        }
    }
    Ok(())
}

fn canonicalize_inputs(
    policy: &SandboxAdmissionPolicy,
    inputs: &mut Vec<SandboxInputArtifact>,
) -> Result<(), SandboxExecutionError> {
    if inputs.len() > policy.max_inputs {
        return Err(SandboxExecutionError::TooManyInputs {
            actual: inputs.len(),
            max: policy.max_inputs,
        });
    }
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut total_bytes = 0_u64;
    for input in inputs.iter() {
        validate_identifier("input.artifact_id", &input.artifact_id, policy.max_argv_bytes)?;
        require_non_zero("input.content_hash", input.content_hash)?;
        validate_guest_path("input.guest_path", &input.guest_path, false)?;
        if input.size_bytes == 0 {
            return Err(SandboxExecutionError::EmptyInput(input.artifact_id.clone()));
        }
        if !ids.insert(input.artifact_id.clone()) {
            return Err(SandboxExecutionError::DuplicateInput(
                input.artifact_id.clone(),
            ));
        }
        if !paths.insert(input.guest_path.clone()) {
            return Err(SandboxExecutionError::DuplicateInput(
                input.guest_path.clone(),
            ));
        }
        total_bytes = total_bytes.checked_add(input.size_bytes).ok_or(
            SandboxExecutionError::InputBytesExceeded {
                actual: u64::MAX,
                max: policy.max_input_bytes,
            },
        )?;
    }
    if total_bytes > policy.max_input_bytes {
        return Err(SandboxExecutionError::InputBytesExceeded {
            actual: total_bytes,
            max: policy.max_input_bytes,
        });
    }
    inputs.sort_by(|left, right| {
        left.guest_path
            .cmp(&right.guest_path)
            .then_with(|| left.artifact_id.cmp(&right.artifact_id))
            .then_with(|| left.content_hash.cmp(&right.content_hash))
    });
    Ok(())
}

fn canonicalize_outputs(
    policy: &SandboxAdmissionPolicy,
    outputs: &mut Vec<SandboxOutputContract>,
) -> Result<(), SandboxExecutionError> {
    if outputs.len() > policy.max_outputs {
        return Err(SandboxExecutionError::TooManyOutputs {
            actual: outputs.len(),
            max: policy.max_outputs,
        });
    }
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for output in outputs.iter() {
        validate_identifier("output.output_id", &output.output_id, policy.max_argv_bytes)?;
        validate_guest_path("output.guest_path", &output.guest_path, false)?;
        if output.max_bytes == 0 {
            return Err(SandboxExecutionError::EmptyOutputBudget(
                output.output_id.clone(),
            ));
        }
        if !ids.insert(output.output_id.clone()) {
            return Err(SandboxExecutionError::DuplicateOutput(
                output.output_id.clone(),
            ));
        }
        if !paths.insert(output.guest_path.clone()) {
            return Err(SandboxExecutionError::DuplicateOutput(
                output.guest_path.clone(),
            ));
        }
    }
    outputs.sort_by(|left, right| {
        left.guest_path
            .cmp(&right.guest_path)
            .then_with(|| left.output_id.cmp(&right.output_id))
    });
    Ok(())
}

fn canonicalize_secret_leases(
    policy: &SandboxAdmissionPolicy,
    leases: &mut Vec<SandboxSecretLeaseRef>,
) -> Result<(), SandboxExecutionError> {
    if !leases.is_empty() && !policy.allow_secret_leases {
        return Err(SandboxExecutionError::SecretLeasesDenied);
    }
    if leases.len() > policy.max_secret_leases {
        return Err(SandboxExecutionError::TooManySecretLeases {
            actual: leases.len(),
            max: policy.max_secret_leases,
        });
    }
    let mut identities = BTreeSet::new();
    let mut hashes = BTreeSet::new();
    for lease in leases.iter() {
        validate_identifier(
            "secret.provider_id",
            &lease.provider_id,
            policy.max_argv_bytes,
        )?;
        validate_identifier(
            "secret.purpose",
            &lease.purpose,
            policy.max_argv_bytes,
        )?;
        require_non_zero("secret.lease_hash", lease.lease_hash)?;
        let identity = (lease.provider_id.clone(), lease.purpose.clone());
        if !identities.insert(identity) || !hashes.insert(lease.lease_hash) {
            return Err(SandboxExecutionError::DuplicateSecretLease(format!(
                "{}/{}",
                lease.provider_id, lease.purpose
            )));
        }
    }
    leases.sort_by(|left, right| {
        left.provider_id
            .cmp(&right.provider_id)
            .then_with(|| left.purpose.cmp(&right.purpose))
            .then_with(|| left.lease_hash.cmp(&right.lease_hash))
    });
    Ok(())
}

fn validate_mount_path_disjointness(
    inputs: &[SandboxInputArtifact],
    outputs: &[SandboxOutputContract],
) -> Result<(), SandboxExecutionError> {
    let paths = inputs
        .iter()
        .map(|input| input.guest_path.as_str())
        .chain(outputs.iter().map(|output| output.guest_path.as_str()))
        .collect::<Vec<_>>();
    for (index, left) in paths.iter().enumerate() {
        for right in paths.iter().skip(index + 1) {
            if paths_overlap(left, right) {
                return Err(SandboxExecutionError::OverlappingGuestPaths {
                    left: (*left).to_string(),
                    right: (*right).to_string(),
                });
            }
        }
    }
    Ok(())
}

fn paths_overlap(left: &str, right: &str) -> bool {
    left == right
        || right
            .strip_prefix(left)
            .is_some_and(|suffix| suffix.starts_with('/'))
        || left
            .strip_prefix(right)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn hash_policy_unchecked(policy: &SandboxAdmissionPolicy) -> SandboxDigest {
    let mut hasher = Sha256::new();
    hasher.update(POLICY_HASH_DOMAIN_V1);
    hash_str(&mut hasher, &policy.policy_id);
    hasher.update(policy.policy_epoch.to_le_bytes());

    hash_usize(&mut hasher, policy.allowed_backends.len());
    for backend in &policy.allowed_backends {
        hasher.update([backend.tag()]);
    }
    hash_digest_set(&mut hasher, &policy.allowed_runtime_digests);
    hash_digest_set(&mut hasher, &policy.allowed_kernel_digests);
    hash_digest_set(&mut hasher, &policy.allowed_rootfs_digests);
    hash_digest_set(&mut hasher, &policy.allowed_network_policy_hashes);
    hash_resources(&mut hasher, &policy.max_resources);
    hash_usize(&mut hasher, policy.max_argv);
    hash_usize(&mut hasher, policy.max_argv_bytes);
    hash_usize(&mut hasher, policy.max_inputs);
    hasher.update(policy.max_input_bytes.to_le_bytes());
    hash_usize(&mut hasher, policy.max_outputs);
    hash_usize(&mut hasher, policy.max_secret_leases);
    hasher.update([u8::from(policy.allow_secret_leases)]);
    hash_isolation(&mut hasher, &policy.required_isolation);
    SandboxDigest(hasher.finalize().into())
}

fn hash_plan_unchecked(
    policy_hash: &SandboxDigest,
    request: &SandboxExecutionRequest,
) -> SandboxDigest {
    let mut hasher = Sha256::new();
    hasher.update(PLAN_HASH_DOMAIN_V1);
    hash_digest(&mut hasher, policy_hash);
    hash_str(&mut hasher, &request.request_id);
    hasher.update(request.execution_epoch.to_le_bytes());
    hasher.update([request.backend.tag()]);
    hash_digest(&mut hasher, &request.runtime_digest);
    hash_digest(&mut hasher, &request.kernel_digest);
    hash_digest(&mut hasher, &request.rootfs_digest);
    hash_digest(&mut hasher, &request.execution_nonce);

    hash_usize(&mut hasher, request.argv.len());
    for arg in &request.argv {
        hash_str(&mut hasher, arg);
    }
    hash_str(&mut hasher, &request.working_dir);

    hash_usize(&mut hasher, request.inputs.len());
    for input in &request.inputs {
        hash_str(&mut hasher, &input.artifact_id);
        hash_digest(&mut hasher, &input.content_hash);
        hash_str(&mut hasher, &input.guest_path);
        hasher.update(input.size_bytes.to_le_bytes());
    }

    hash_usize(&mut hasher, request.outputs.len());
    for output in &request.outputs {
        hash_str(&mut hasher, &output.output_id);
        hash_str(&mut hasher, &output.guest_path);
        hasher.update(output.max_bytes.to_le_bytes());
    }

    hash_usize(&mut hasher, request.secret_leases.len());
    for lease in &request.secret_leases {
        hash_str(&mut hasher, &lease.provider_id);
        hash_str(&mut hasher, &lease.purpose);
        hash_digest(&mut hasher, &lease.lease_hash);
    }

    hash_str(&mut hasher, &request.network.policy_id);
    hash_digest(&mut hasher, &request.network.policy_hash);
    hasher.update([request.network.mode.tag()]);
    hash_resources(&mut hasher, &request.resources);
    hash_isolation(&mut hasher, &request.isolation);
    SandboxDigest(hasher.finalize().into())
}

fn hash_receipt_unchecked(
    policy_hash: &SandboxDigest,
    receipt: &SandboxExecutionReceipt,
) -> SandboxDigest {
    let mut hasher = Sha256::new();
    hasher.update(RECEIPT_HASH_DOMAIN_V1);
    hash_digest(&mut hasher, policy_hash);
    hash_str(&mut hasher, &receipt.request_id);
    hash_digest(&mut hasher, &receipt.plan_hash);
    hasher.update([receipt.backend.tag()]);
    hash_digest(&mut hasher, &receipt.runtime_digest);
    hash_digest(&mut hasher, &receipt.kernel_digest);
    hash_digest(&mut hasher, &receipt.rootfs_digest);
    hash_optional_digest(&mut hasher, receipt.runtime_attestation_hash.as_ref());
    hasher.update([receipt.termination.tag()]);
    hash_optional_i32(&mut hasher, receipt.exit_code);
    hasher.update(receipt.wall_time_ms.to_le_bytes());
    hasher.update(receipt.peak_memory_mib.to_le_bytes());
    hasher.update(receipt.produced_output_count.to_le_bytes());
    hasher.update(receipt.total_output_bytes.to_le_bytes());
    hash_digest(&mut hasher, &receipt.stdout_hash);
    hash_digest(&mut hasher, &receipt.stderr_hash);
    hash_digest(&mut hasher, &receipt.output_manifest_hash);
    SandboxDigest(hasher.finalize().into())
}

fn hash_digest_set(hasher: &mut Sha256, values: &BTreeSet<SandboxDigest>) {
    hash_usize(hasher, values.len());
    for value in values {
        hash_digest(hasher, value);
    }
}

fn hash_resources(hasher: &mut Sha256, resources: &SandboxResourceBudget) {
    hasher.update(resources.vcpus.to_le_bytes());
    hasher.update(resources.memory_mib.to_le_bytes());
    hasher.update(resources.disk_mib.to_le_bytes());
    hasher.update(resources.wall_time_ms.to_le_bytes());
    hasher.update(resources.process_limit.to_le_bytes());
    hasher.update(resources.max_output_bytes.to_le_bytes());
}

fn hash_isolation(hasher: &mut Sha256, isolation: &SandboxIsolationRequirements) {
    hasher.update([
        u8::from(isolation.ephemeral_root),
        u8::from(isolation.destroy_after_run),
        u8::from(isolation.device_passthrough_disabled),
        u8::from(isolation.host_path_mounts_disabled),
        u8::from(isolation.runtime_attestation_required),
    ]);
}

fn hash_digest(hasher: &mut Sha256, digest: &SandboxDigest) {
    hasher.update(digest.0);
}

fn hash_optional_digest(hasher: &mut Sha256, digest: Option<&SandboxDigest>) {
    match digest {
        Some(value) => {
            hasher.update([1]);
            hash_digest(hasher, value);
        }
        None => hasher.update([0]),
    }
}

fn hash_optional_i32(hasher: &mut Sha256, value: Option<i32>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hasher.update(value.to_le_bytes());
        }
        None => hasher.update([0]),
    }
}

fn hash_str(hasher: &mut Sha256, value: &str) {
    hash_bytes(hasher, value.as_bytes());
}

fn hash_bytes(hasher: &mut Sha256, value: &[u8]) {
    hash_usize(hasher, value.len());
    hasher.update(value);
}

fn hash_usize(hasher: &mut Sha256, value: usize) {
    let value = u64::try_from(value).expect("supported Rust targets fit usize into u64");
    hasher.update(value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: u8) -> SandboxDigest {
        SandboxDigest::from_bytes([byte; 32])
    }

    fn resources() -> SandboxResourceBudget {
        SandboxResourceBudget {
            vcpus: 2,
            memory_mib: 1024,
            disk_mib: 4096,
            wall_time_ms: 60_000,
            process_limit: 64,
            max_output_bytes: 8 * 1024 * 1024,
        }
    }

    fn policy() -> SandboxAdmissionPolicy {
        SandboxAdmissionPolicy {
            policy_id: "helix-sandbox-production-v1".to_string(),
            policy_epoch: 1,
            allowed_backends: BTreeSet::from([SandboxBackend::Firecracker]),
            allowed_runtime_digests: BTreeSet::from([digest(1)]),
            allowed_kernel_digests: BTreeSet::from([digest(2)]),
            allowed_rootfs_digests: BTreeSet::from([digest(3)]),
            allowed_network_policy_hashes: BTreeSet::from([digest(4)]),
            max_resources: SandboxResourceBudget {
                vcpus: 4,
                memory_mib: 4096,
                disk_mib: 16_384,
                wall_time_ms: 300_000,
                process_limit: 256,
                max_output_bytes: 64 * 1024 * 1024,
            },
            max_argv: 64,
            max_argv_bytes: 16 * 1024,
            max_inputs: 16,
            max_input_bytes: 64 * 1024 * 1024,
            max_outputs: 16,
            max_secret_leases: 4,
            allow_secret_leases: true,
            required_isolation: SandboxIsolationRequirements::STRICT,
        }
    }

    fn request() -> SandboxExecutionRequest {
        SandboxExecutionRequest {
            request_id: "run-0001".to_string(),
            execution_epoch: 7,
            backend: SandboxBackend::Firecracker,
            runtime_digest: digest(1),
            kernel_digest: digest(2),
            rootfs_digest: digest(3),
            execution_nonce: digest(9),
            argv: vec![
                "/usr/bin/python3".to_string(),
                "/workspace/analyze.py".to_string(),
            ],
            working_dir: "/workspace".to_string(),
            inputs: vec![
                SandboxInputArtifact {
                    artifact_id: "script".to_string(),
                    content_hash: digest(11),
                    guest_path: "/workspace/analyze.py".to_string(),
                    size_bytes: 1_000,
                },
                SandboxInputArtifact {
                    artifact_id: "dataset".to_string(),
                    content_hash: digest(10),
                    guest_path: "/workspace/data.csv".to_string(),
                    size_bytes: 2_000,
                },
            ],
            outputs: vec![SandboxOutputContract {
                output_id: "report".to_string(),
                guest_path: "/outputs/report.json".to_string(),
                max_bytes: 1024 * 1024,
            }],
            secret_leases: vec![SandboxSecretLeaseRef {
                provider_id: "github-readonly".to_string(),
                purpose: "read-source".to_string(),
                lease_hash: digest(12),
            }],
            network: SandboxNetworkPolicyRef {
                policy_id: "research-egress-readonly".to_string(),
                policy_hash: digest(4),
                mode: SandboxNetworkMode::AllowlistedGateway,
            },
            resources: resources(),
            isolation: SandboxIsolationRequirements::STRICT,
        }
    }

    fn receipt(admitted: &AdmittedSandboxExecution) -> SandboxExecutionReceipt {
        SandboxExecutionReceipt {
            request_id: admitted.request().request_id.clone(),
            plan_hash: *admitted.plan_hash(),
            backend: admitted.request().backend,
            runtime_digest: admitted.request().runtime_digest,
            kernel_digest: admitted.request().kernel_digest,
            rootfs_digest: admitted.request().rootfs_digest,
            runtime_attestation_hash: Some(digest(20)),
            termination: SandboxTermination::Exited,
            exit_code: Some(0),
            wall_time_ms: 2_000,
            peak_memory_mib: 256,
            produced_output_count: 1,
            total_output_bytes: 4_096,
            stdout_hash: digest(21),
            stderr_hash: digest(22),
            output_manifest_hash: digest(23),
        }
    }

    #[test]
    fn valid_plan_is_admitted_and_canonicalized() {
        let admitted = admit_sandbox_execution(&policy(), request()).expect("admitted plan");
        assert_eq!(admitted.request().inputs[0].artifact_id, "script");
        assert_ne!(*admitted.policy_hash(), SandboxDigest::ZERO);
        assert_ne!(*admitted.plan_hash(), SandboxDigest::ZERO);
    }

    #[test]
    fn input_order_does_not_change_plan_hash() {
        let left = request();
        let mut right = left.clone();
        right.inputs.reverse();
        let left = admit_sandbox_execution(&policy(), left).expect("left plan");
        let right = admit_sandbox_execution(&policy(), right).expect("right plan");
        assert_eq!(left.plan_hash(), right.plan_hash());
        assert_eq!(left.request(), right.request());
    }

    #[test]
    fn argv_order_changes_plan_hash() {
        let left = request();
        let mut right = left.clone();
        right.argv.reverse();
        let left = admit_sandbox_execution(&policy(), left).expect("left plan");
        let right = admit_sandbox_execution(&policy(), right).expect("right plan");
        assert_ne!(left.plan_hash(), right.plan_hash());
    }

    #[test]
    fn unapproved_runtime_is_rejected() {
        let mut unapproved = request();
        unapproved.runtime_digest = digest(99);
        assert_eq!(
            admit_sandbox_execution(&policy(), unapproved),
            Err(SandboxExecutionError::RuntimeDigestDenied)
        );
    }

    #[test]
    fn zero_nonce_is_rejected() {
        let mut replayable = request();
        replayable.execution_nonce = SandboxDigest::ZERO;
        assert_eq!(
            admit_sandbox_execution(&policy(), replayable),
            Err(SandboxExecutionError::ZeroDigest {
                field: "execution_nonce"
            })
        );
    }

    #[test]
    fn path_traversal_is_rejected() {
        let mut traversal = request();
        traversal.outputs[0].guest_path = "/outputs/../host".to_string();
        assert!(matches!(
            admit_sandbox_execution(&policy(), traversal),
            Err(SandboxExecutionError::InvalidGuestPath {
                field: "output.guest_path",
                ..
            })
        ));
    }

    #[test]
    fn overlapping_input_and_output_paths_are_rejected() {
        let mut overlap = request();
        overlap.outputs[0].guest_path = "/workspace".to_string();
        assert!(matches!(
            admit_sandbox_execution(&policy(), overlap),
            Err(SandboxExecutionError::OverlappingGuestPaths { .. })
        ));
    }

    #[test]
    fn resource_budget_is_enforced() {
        let mut oversized = request();
        oversized.resources.memory_mib = policy().max_resources.memory_mib + 1;
        assert_eq!(
            admit_sandbox_execution(&policy(), oversized),
            Err(SandboxExecutionError::ResourceExceeded {
                field: "memory_mib",
                actual: 4097,
                max: 4096
            })
        );
    }

    #[test]
    fn strict_isolation_cannot_be_silently_dropped() {
        let mut unsafe_request = request();
        unsafe_request.isolation.host_path_mounts_disabled = false;
        assert_eq!(
            admit_sandbox_execution(&policy(), unsafe_request),
            Err(SandboxExecutionError::IsolationRequirementMissing(
                "host_path_mounts_disabled"
            ))
        );
    }

    #[test]
    fn secret_values_cannot_enter_the_plan_language() {
        let serialized = serde_json::to_string(&request()).expect("serialize request");
        assert!(!serialized.contains("secret_value"));
        assert!(!serialized.contains("api_key"));
        assert!(serialized.contains("lease_hash"));
    }

    #[test]
    fn valid_receipt_is_constructor_gated_and_successful() {
        let admitted = admit_sandbox_execution(&policy(), request()).expect("admitted plan");
        let validated =
            validate_sandbox_receipt(&admitted, receipt(&admitted)).expect("validated receipt");
        assert!(validated.succeeded());
        assert_ne!(*validated.receipt_hash(), SandboxDigest::ZERO);
    }

    #[test]
    fn receipt_cannot_replay_against_another_plan() {
        let admitted = admit_sandbox_execution(&policy(), request()).expect("admitted plan");
        let mut other_request = request();
        other_request.request_id = "run-0002".to_string();
        other_request.execution_nonce = digest(30);
        let other = admit_sandbox_execution(&policy(), other_request).expect("other plan");
        assert_eq!(
            validate_sandbox_receipt(&other, receipt(&admitted)),
            Err(SandboxExecutionError::ReceiptRequestMismatch)
        );
    }

    #[test]
    fn missing_runtime_attestation_fails_closed() {
        let admitted = admit_sandbox_execution(&policy(), request()).expect("admitted plan");
        let mut unmeasured = receipt(&admitted);
        unmeasured.runtime_attestation_hash = None;
        assert_eq!(
            validate_sandbox_receipt(&admitted, unmeasured),
            Err(SandboxExecutionError::MissingRuntimeAttestation)
        );
    }

    #[test]
    fn receipt_cannot_exceed_output_budget() {
        let admitted = admit_sandbox_execution(&policy(), request()).expect("admitted plan");
        let mut oversized = receipt(&admitted);
        oversized.total_output_bytes = admitted.request().outputs[0].max_bytes + 1;
        assert_eq!(
            validate_sandbox_receipt(&admitted, oversized),
            Err(SandboxExecutionError::ReceiptOutputBytesExceeded {
                actual: 1_048_577,
                max: 1_048_576
            })
        );
    }

    #[test]
    fn timeout_receipt_is_valid_but_not_successful() {
        let admitted = admit_sandbox_execution(&policy(), request()).expect("admitted plan");
        let mut timed_out = receipt(&admitted);
        timed_out.termination = SandboxTermination::TimedOut;
        timed_out.exit_code = None;
        timed_out.wall_time_ms = admitted.request().resources.wall_time_ms;
        let validated =
            validate_sandbox_receipt(&admitted, timed_out).expect("valid timeout receipt");
        assert!(!validated.succeeded());
    }

    #[test]
    fn receipt_hash_binds_output_manifest() {
        let admitted = admit_sandbox_execution(&policy(), request()).expect("admitted plan");
        let left = validate_sandbox_receipt(&admitted, receipt(&admitted))
            .expect("left receipt");
        let mut changed = receipt(&admitted);
        changed.output_manifest_hash = digest(24);
        let right = validate_sandbox_receipt(&admitted, changed).expect("right receipt");
        assert_ne!(left.receipt_hash(), right.receipt_hash());
    }
}

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

//! Imperative Firecracker sandbox adapter for policy-admitted Helix plans.
//!
//! The adapter accepts only [`AdmittedSandboxExecution`]. It verifies the exact
//! allowlisted runner, jailer, kernel, and root filesystem bytes; stages only
//! content-addressed read-only inputs; launches a trusted Firecracker runner
//! without a shell or inherited environment; enforces a host wall-clock bound;
//! and passes the resulting receipt back through the pure Helix validator.
//!
//! This initial adapter is deliberately narrower than the core contract:
//! - Linux only;
//! - Firecracker only;
//! - deny-all networking only; and
//! - no credential leases until a policy-enforcing credential gateway exists.
//!
//! Failure never falls back to native host execution.

use async_trait::async_trait;
use helix_core::sandbox_execution::{
    validate_sandbox_receipt, AdmittedSandboxExecution, SandboxBackend, SandboxDigest,
    SandboxExecutionError, SandboxExecutionReceipt, SandboxInputArtifact, SandboxNetworkMode,
    ValidatedSandboxReceipt,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time::{timeout, Instant};

const RUNTIME_PROFILE_DOMAIN_V1: &[u8] = b"helix:firecracker-runtime-profile:v1";
const RUNNER_PROTOCOL_VERSION_V1: u16 = 1;
const DEFAULT_MAX_PROTOCOL_BYTES: u64 = 4 * 1024 * 1024;

/// Resolves one content-addressed input to a host file controlled by the shell.
///
/// The resolver is trusted infrastructure. The returned file is still checked
/// against the admitted size and digest before it is staged for the runner.
#[async_trait]
pub trait SandboxArtifactResolver: Send + Sync {
    /// Resolve an admitted artifact identity to a local source file.
    async fn resolve_read_only(
        &self,
        artifact: &SandboxInputArtifact,
    ) -> Result<PathBuf, SandboxRuntimeError>;
}

/// Exact host files and commitments comprising one reviewed Firecracker profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirecrackerRuntimeProfile {
    /// Stable profile name included in the runtime commitment.
    pub profile_id: String,
    /// Trusted Helix runner implementing the JSON protocol used by this adapter.
    pub runner_path: PathBuf,
    /// Firecracker jailer binary path supplied to the trusted runner.
    pub jailer_path: PathBuf,
    /// Guest kernel image path.
    pub kernel_path: PathBuf,
    /// Immutable root filesystem image path.
    pub rootfs_path: PathBuf,
    /// Exact SHA-256 commitment to the runner binary.
    pub runner_digest: SandboxDigest,
    /// Exact SHA-256 commitment to the jailer binary.
    pub jailer_digest: SandboxDigest,
    /// Exact SHA-256 commitment to the guest kernel.
    pub kernel_digest: SandboxDigest,
    /// Exact SHA-256 commitment to the immutable root filesystem.
    pub rootfs_digest: SandboxDigest,
    /// Commitment to the complete runner profile.
    pub runtime_digest: SandboxDigest,
    /// Root directory used for one-time execution directories.
    pub work_root: PathBuf,
    /// Maximum request or receipt protocol file size.
    pub max_protocol_bytes: u64,
}

impl FirecrackerRuntimeProfile {
    /// Compute the canonical runtime-profile commitment from reviewed components.
    #[must_use]
    pub fn computed_runtime_digest(&self) -> SandboxDigest {
        let mut hasher = Sha256::new();
        hasher.update(RUNTIME_PROFILE_DOMAIN_V1);
        hash_field(&mut hasher, self.profile_id.as_bytes());
        hasher.update(self.runner_digest.as_bytes());
        hasher.update(self.jailer_digest.as_bytes());
        hasher.update(RUNNER_PROTOCOL_VERSION_V1.to_le_bytes());
        hasher.update(self.max_protocol_bytes.to_le_bytes());
        SandboxDigest::from_bytes(hasher.finalize().into())
    }
}

/// A resolved and verified profile whose paths are canonical regular files.
#[derive(Debug, Clone)]
struct VerifiedFirecrackerProfile {
    runner_path: PathBuf,
    jailer_path: PathBuf,
    kernel_path: PathBuf,
    rootfs_path: PathBuf,
    work_root: PathBuf,
}

/// Host-side input mapping sent to the trusted runner.
#[derive(Debug, Clone, Serialize)]
struct StagedInput {
    artifact_id: String,
    guest_path: String,
    staged_host_path: String,
    content_hash: SandboxDigest,
    size_bytes: u64,
}

/// Versioned request written for the trusted Firecracker runner.
#[derive(Debug, Serialize)]
struct FirecrackerRunnerEnvelope<'a> {
    protocol_version: u16,
    policy_hash: SandboxDigest,
    plan_hash: SandboxDigest,
    request: &'a helix_core::sandbox_execution::SandboxExecutionRequest,
    profile_id: &'a str,
    jailer_path: String,
    kernel_path: String,
    rootfs_path: String,
    staged_inputs: Vec<StagedInput>,
    output_root: String,
    receipt_path: String,
}

/// Exact subprocess invocation used for the trusted runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirecrackerRunnerCommand {
    /// Executable invoked directly, never through a shell.
    pub program: PathBuf,
    /// Exact argument vector.
    pub args: Vec<OsString>,
    /// Minimal explicit environment.
    pub environment: BTreeMap<OsString, OsString>,
    /// Canonical per-run working directory.
    pub current_dir: PathBuf,
}

/// Errors from the imperative sandbox runtime boundary.
#[derive(Debug, Error)]
pub enum SandboxRuntimeError {
    /// This adapter is intentionally available only on Linux.
    #[error("Firecracker sandbox execution requires Linux/KVM")]
    UnsupportedHost,
    /// The admitted plan selected a different backend.
    #[error("Firecracker adapter cannot execute backend {0:?}")]
    BackendMismatch(SandboxBackend),
    /// The initial runtime intentionally supports only deny-all egress.
    #[error("Firecracker adapter v1 supports deny-all networking only")]
    NetworkModeUnsupported,
    /// Credential leases require a separate policy-enforcing gateway.
    #[error("Firecracker adapter v1 does not accept credential leases")]
    SecretLeasesUnsupported,
    /// The admitted isolation assertions are weaker than this adapter requires.
    #[error("Firecracker adapter requires strict ephemeral isolation: {0}")]
    WeakIsolation(&'static str),
    /// Profile metadata is invalid.
    #[error("invalid Firecracker runtime profile: {0}")]
    InvalidProfile(String),
    /// A configured file was missing, mutable through a symlink, or not regular.
    #[error("invalid {label} file {path:?}: {reason}")]
    InvalidProfileFile {
        /// Logical component name.
        label: &'static str,
        /// Configured path.
        path: PathBuf,
        /// Validation failure.
        reason: String,
    },
    /// A configured or staged file digest differed from its commitment.
    #[error("{label} digest mismatch")]
    DigestMismatch {
        /// Logical component name.
        label: &'static str,
    },
    /// A configured or staged file size differed from the admitted size.
    #[error("{label} size mismatch: expected {expected}, found {actual}")]
    SizeMismatch {
        /// Logical component name.
        label: &'static str,
        /// Admitted size.
        expected: u64,
        /// Observed size.
        actual: u64,
    },
    /// The plan selected a runtime component not represented by this profile.
    #[error("admitted plan does not match Firecracker profile field {0}")]
    ProfileBindingMismatch(&'static str),
    /// A prior or concurrent run already owns the plan-hash directory.
    #[error("sandbox run directory already exists for plan {0}")]
    ReplayOrConcurrentRun(String),
    /// A path could not be represented by the runner protocol.
    #[error("sandbox path is not valid UTF-8: {0:?}")]
    NonUtf8Path(PathBuf),
    /// The runner request or receipt exceeded the protocol limit.
    #[error("sandbox {field} is {actual} bytes; maximum is {max}")]
    ProtocolTooLarge {
        /// Protocol object name.
        field: &'static str,
        /// Observed bytes.
        actual: u64,
        /// Configured maximum.
        max: u64,
    },
    /// JSON encoding or decoding failed.
    #[error("sandbox protocol JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    /// Host filesystem or process I/O failed.
    #[error("sandbox host I/O failed during {context}: {source}")]
    Io {
        /// Operation being performed.
        context: &'static str,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// The trusted runner exceeded the admitted wall-clock deadline.
    #[error("trusted Firecracker runner exceeded {0} ms")]
    RunnerTimedOut(u64),
    /// The trusted runner failed before producing a valid receipt.
    #[error("trusted Firecracker runner exited unsuccessfully: {0:?}")]
    RunnerFailed(Option<i32>),
    /// The pure Helix receipt validator rejected the runner output.
    #[error("sandbox receipt validation failed: {0}")]
    ReceiptValidation(#[from] SandboxExecutionError),
    /// Cleanup was required by the admitted plan but failed.
    #[error("failed to destroy sandbox run directory: {0}")]
    Cleanup(std::io::Error),
    /// Artifact resolution failed.
    #[error("artifact resolver failed: {0}")]
    ArtifactResolution(String),
}

impl SandboxRuntimeError {
    fn io(context: &'static str, source: std::io::Error) -> Self {
        Self::Io { context, source }
    }
}

/// Common asynchronous interface for broad-capability sandbox runtimes.
#[async_trait]
pub trait SandboxExecutionRuntime: Send + Sync {
    /// Execute one constructor-gated sandbox plan and return a validated receipt.
    async fn execute(
        &self,
        admitted: &AdmittedSandboxExecution,
    ) -> Result<ValidatedSandboxReceipt, SandboxRuntimeError>;
}

/// Linux Firecracker adapter backed by a reviewed, digest-pinned runner binary.
pub struct FirecrackerSandboxRuntime<R> {
    profile: FirecrackerRuntimeProfile,
    resolver: Arc<R>,
}

impl<R> FirecrackerSandboxRuntime<R>
where
    R: SandboxArtifactResolver,
{
    /// Construct an adapter. Profile files are rechecked before every execution.
    #[must_use]
    pub fn new(profile: FirecrackerRuntimeProfile, resolver: Arc<R>) -> Self {
        Self { profile, resolver }
    }

    async fn verify_profile(&self) -> Result<VerifiedFirecrackerProfile, SandboxRuntimeError> {
        if self.profile.profile_id.trim() != self.profile.profile_id
            || self.profile.profile_id.is_empty()
            || !self.profile.profile_id.is_ascii()
        {
            return Err(SandboxRuntimeError::InvalidProfile(
                "profile_id must be non-empty canonical ASCII".to_string(),
            ));
        }
        if self.profile.max_protocol_bytes == 0
            || self.profile.max_protocol_bytes > DEFAULT_MAX_PROTOCOL_BYTES
        {
            return Err(SandboxRuntimeError::InvalidProfile(format!(
                "max_protocol_bytes must be between 1 and {DEFAULT_MAX_PROTOCOL_BYTES}"
            )));
        }
        if self.profile.runtime_digest != self.profile.computed_runtime_digest() {
            return Err(SandboxRuntimeError::InvalidProfile(
                "runtime_digest does not commit to the configured runner profile".to_string(),
            ));
        }

        let runner_path = verify_regular_file(
            "runner",
            &self.profile.runner_path,
            self.profile.runner_digest,
        )
        .await?;
        let jailer_path = verify_regular_file(
            "jailer",
            &self.profile.jailer_path,
            self.profile.jailer_digest,
        )
        .await?;
        let kernel_path = verify_regular_file(
            "kernel",
            &self.profile.kernel_path,
            self.profile.kernel_digest,
        )
        .await?;
        let rootfs_path = verify_regular_file(
            "rootfs",
            &self.profile.rootfs_path,
            self.profile.rootfs_digest,
        )
        .await?;
        let work_root = verify_work_root(&self.profile.work_root).await?;

        Ok(VerifiedFirecrackerProfile {
            runner_path,
            jailer_path,
            kernel_path,
            rootfs_path,
            work_root,
        })
    }

    fn validate_admitted_boundary(
        &self,
        admitted: &AdmittedSandboxExecution,
    ) -> Result<(), SandboxRuntimeError> {
        let request = admitted.request();
        if request.backend != SandboxBackend::Firecracker {
            return Err(SandboxRuntimeError::BackendMismatch(request.backend));
        }
        if request.runtime_digest != self.profile.runtime_digest {
            return Err(SandboxRuntimeError::ProfileBindingMismatch(
                "runtime_digest",
            ));
        }
        if request.kernel_digest != self.profile.kernel_digest {
            return Err(SandboxRuntimeError::ProfileBindingMismatch("kernel_digest"));
        }
        if request.rootfs_digest != self.profile.rootfs_digest {
            return Err(SandboxRuntimeError::ProfileBindingMismatch("rootfs_digest"));
        }
        if request.network.mode != SandboxNetworkMode::DenyAll {
            return Err(SandboxRuntimeError::NetworkModeUnsupported);
        }
        if !request.secret_leases.is_empty() {
            return Err(SandboxRuntimeError::SecretLeasesUnsupported);
        }
        let isolation = request.isolation;
        if !isolation.ephemeral_root {
            return Err(SandboxRuntimeError::WeakIsolation("ephemeral_root"));
        }
        if !isolation.destroy_after_run {
            return Err(SandboxRuntimeError::WeakIsolation("destroy_after_run"));
        }
        if !isolation.device_passthrough_disabled {
            return Err(SandboxRuntimeError::WeakIsolation(
                "device_passthrough_disabled",
            ));
        }
        if !isolation.host_path_mounts_disabled {
            return Err(SandboxRuntimeError::WeakIsolation(
                "host_path_mounts_disabled",
            ));
        }
        if !isolation.runtime_attestation_required {
            return Err(SandboxRuntimeError::WeakIsolation(
                "runtime_attestation_required",
            ));
        }
        Ok(())
    }

    async fn stage_inputs(
        &self,
        inputs_dir: &Path,
        inputs: &[SandboxInputArtifact],
    ) -> Result<Vec<StagedInput>, SandboxRuntimeError> {
        let mut staged = Vec::with_capacity(inputs.len());
        for (index, artifact) in inputs.iter().enumerate() {
            let source = self
                .resolver
                .resolve_read_only(artifact)
                .await
                .map_err(|error| SandboxRuntimeError::ArtifactResolution(error.to_string()))?;
            let source = verify_regular_file_with_size(
                "input",
                &source,
                artifact.content_hash,
                artifact.size_bytes,
            )
            .await?;
            let destination = inputs_dir.join(format!("{index:04}.artifact"));
            let copied = fs::copy(&source, &destination)
                .await
                .map_err(|error| SandboxRuntimeError::io("copy input", error))?;
            if copied != artifact.size_bytes {
                return Err(SandboxRuntimeError::SizeMismatch {
                    label: "staged input",
                    expected: artifact.size_bytes,
                    actual: copied,
                });
            }
            let mut permissions = fs::metadata(&destination)
                .await
                .map_err(|error| SandboxRuntimeError::io("inspect staged input", error))?
                .permissions();
            permissions.set_readonly(true);
            fs::set_permissions(&destination, permissions)
                .await
                .map_err(|error| SandboxRuntimeError::io("lock staged input", error))?;
            let destination = verify_regular_file_with_size(
                "staged input",
                &destination,
                artifact.content_hash,
                artifact.size_bytes,
            )
            .await?;
            staged.push(StagedInput {
                artifact_id: artifact.artifact_id.clone(),
                guest_path: artifact.guest_path.clone(),
                staged_host_path: path_string(&destination)?,
                content_hash: artifact.content_hash,
                size_bytes: artifact.size_bytes,
            });
        }
        Ok(staged)
    }

    async fn execute_in_run_dir(
        &self,
        admitted: &AdmittedSandboxExecution,
        profile: &VerifiedFirecrackerProfile,
        run_dir: &Path,
    ) -> Result<ValidatedSandboxReceipt, SandboxRuntimeError> {
        let inputs_dir = run_dir.join("inputs");
        let output_root = run_dir.join("outputs");
        fs::create_dir(&inputs_dir)
            .await
            .map_err(|error| SandboxRuntimeError::io("create input staging", error))?;
        fs::create_dir(&output_root)
            .await
            .map_err(|error| SandboxRuntimeError::io("create output staging", error))?;

        let staged_inputs = self
            .stage_inputs(&inputs_dir, &admitted.request().inputs)
            .await?;
        let request_path = run_dir.join("request.json");
        let receipt_path = run_dir.join("receipt.json");
        let envelope = FirecrackerRunnerEnvelope {
            protocol_version: RUNNER_PROTOCOL_VERSION_V1,
            policy_hash: *admitted.policy_hash(),
            plan_hash: *admitted.plan_hash(),
            request: admitted.request(),
            profile_id: &self.profile.profile_id,
            jailer_path: path_string(&profile.jailer_path)?,
            kernel_path: path_string(&profile.kernel_path)?,
            rootfs_path: path_string(&profile.rootfs_path)?,
            staged_inputs,
            output_root: path_string(&output_root)?,
            receipt_path: path_string(&receipt_path)?,
        };
        let request_bytes = serde_json::to_vec(&envelope)?;
        require_protocol_bound(
            "request",
            request_bytes.len() as u64,
            self.profile.max_protocol_bytes,
        )?;
        write_new_file(&request_path, &request_bytes).await?;

        let launch = build_runner_command(&profile.runner_path, run_dir, &request_path)?;
        let mut command = Command::new(&launch.program);
        command
            .args(&launch.args)
            .current_dir(&launch.current_dir)
            .env_clear()
            .envs(launch.environment)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);

        let started = Instant::now();
        let mut child = command
            .spawn()
            .map_err(|error| SandboxRuntimeError::io("spawn trusted runner", error))?;
        let status = match timeout(
            Duration::from_millis(admitted.request().resources.wall_time_ms),
            child.wait(),
        )
        .await
        {
            Ok(result) => {
                result.map_err(|error| SandboxRuntimeError::io("wait for trusted runner", error))?
            }
            Err(_) => {
                let _ = child.kill().await;
                return Err(SandboxRuntimeError::RunnerTimedOut(
                    admitted.request().resources.wall_time_ms,
                ));
            }
        };
        if !status.success() {
            return Err(SandboxRuntimeError::RunnerFailed(status.code()));
        }

        let receipt_bytes =
            read_bounded_file(&receipt_path, self.profile.max_protocol_bytes, "receipt").await?;
        let mut receipt: SandboxExecutionReceipt = serde_json::from_slice(&receipt_bytes)?;
        let observed_wall_time_ms =
            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        // The runner cannot reduce the host-observed wall time by under-reporting it.
        receipt.wall_time_ms = receipt.wall_time_ms.max(observed_wall_time_ms);
        validate_sandbox_receipt(admitted, receipt).map_err(SandboxRuntimeError::from)
    }
}

#[async_trait]
impl<R> SandboxExecutionRuntime for FirecrackerSandboxRuntime<R>
where
    R: SandboxArtifactResolver + 'static,
{
    async fn execute(
        &self,
        admitted: &AdmittedSandboxExecution,
    ) -> Result<ValidatedSandboxReceipt, SandboxRuntimeError> {
        if !cfg!(target_os = "linux") {
            return Err(SandboxRuntimeError::UnsupportedHost);
        }
        self.validate_admitted_boundary(admitted)?;
        let profile = self.verify_profile().await?;
        let plan_hex = (*admitted.plan_hash()).to_hex();
        let run_dir = profile.work_root.join(&plan_hex);
        match fs::create_dir(&run_dir).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(SandboxRuntimeError::ReplayOrConcurrentRun(plan_hex))
            }
            Err(error) => return Err(SandboxRuntimeError::io("create run directory", error)),
        }

        let result = self.execute_in_run_dir(admitted, &profile, &run_dir).await;
        let cleanup = fs::remove_dir_all(&run_dir).await;
        match (result, cleanup) {
            (Ok(receipt), Ok(())) => Ok(receipt),
            (Ok(_), Err(error)) => Err(SandboxRuntimeError::Cleanup(error)),
            (Err(error), _) => Err(error),
        }
    }
}

fn build_runner_command(
    runner_path: &Path,
    run_dir: &Path,
    request_path: &Path,
) -> Result<FirecrackerRunnerCommand, SandboxRuntimeError> {
    let mut environment = BTreeMap::new();
    environment.insert(OsString::from("LC_ALL"), OsString::from("C"));
    environment.insert(OsString::from("LANG"), OsString::from("C"));
    environment.insert(OsString::from("HOME"), OsString::from("/nonexistent"));
    Ok(FirecrackerRunnerCommand {
        program: runner_path.to_path_buf(),
        args: vec![
            OsString::from("--protocol-version"),
            OsString::from(RUNNER_PROTOCOL_VERSION_V1.to_string()),
            OsString::from("--request"),
            request_path.as_os_str().to_owned(),
        ],
        environment,
        current_dir: run_dir.to_path_buf(),
    })
}

async fn verify_work_root(path: &Path) -> Result<PathBuf, SandboxRuntimeError> {
    if !path.is_absolute() {
        return Err(SandboxRuntimeError::InvalidProfile(
            "work_root must be absolute".to_string(),
        ));
    }
    let metadata = fs::symlink_metadata(path)
        .await
        .map_err(|error| SandboxRuntimeError::io("inspect work_root", error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(SandboxRuntimeError::InvalidProfile(
            "work_root must be a real directory, not a symlink".to_string(),
        ));
    }
    fs::canonicalize(path)
        .await
        .map_err(|error| SandboxRuntimeError::io("canonicalize work_root", error))
}

async fn verify_regular_file(
    label: &'static str,
    path: &Path,
    expected_digest: SandboxDigest,
) -> Result<PathBuf, SandboxRuntimeError> {
    if !path.is_absolute() {
        return Err(SandboxRuntimeError::InvalidProfileFile {
            label,
            path: path.to_path_buf(),
            reason: "path must be absolute".to_string(),
        });
    }
    let metadata = fs::symlink_metadata(path).await.map_err(|error| {
        SandboxRuntimeError::InvalidProfileFile {
            label,
            path: path.to_path_buf(),
            reason: error.to_string(),
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(SandboxRuntimeError::InvalidProfileFile {
            label,
            path: path.to_path_buf(),
            reason: "must be a regular non-symlink file".to_string(),
        });
    }
    let canonical = fs::canonicalize(path)
        .await
        .map_err(|error| SandboxRuntimeError::io("canonicalize profile file", error))?;
    let actual_digest = sha256_file(&canonical).await?;
    if actual_digest != expected_digest {
        return Err(SandboxRuntimeError::DigestMismatch { label });
    }
    Ok(canonical)
}

async fn verify_regular_file_with_size(
    label: &'static str,
    path: &Path,
    expected_digest: SandboxDigest,
    expected_size: u64,
) -> Result<PathBuf, SandboxRuntimeError> {
    let canonical = verify_regular_file(label, path, expected_digest).await?;
    let actual_size = fs::metadata(&canonical)
        .await
        .map_err(|error| SandboxRuntimeError::io("inspect file size", error))?
        .len();
    if actual_size != expected_size {
        return Err(SandboxRuntimeError::SizeMismatch {
            label,
            expected: expected_size,
            actual: actual_size,
        });
    }
    Ok(canonical)
}

async fn sha256_file(path: &Path) -> Result<SandboxDigest, SandboxRuntimeError> {
    let mut file = fs::File::open(path)
        .await
        .map_err(|error| SandboxRuntimeError::io("open file for hashing", error))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|error| SandboxRuntimeError::io("hash file", error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(SandboxDigest::from_bytes(hasher.finalize().into()))
}

async fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), SandboxRuntimeError> {
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .await
        .map_err(|error| SandboxRuntimeError::io("create protocol file", error))?;
    file.write_all(bytes)
        .await
        .map_err(|error| SandboxRuntimeError::io("write protocol file", error))?;
    file.flush()
        .await
        .map_err(|error| SandboxRuntimeError::io("flush protocol file", error))
}

async fn read_bounded_file(
    path: &Path,
    max: u64,
    field: &'static str,
) -> Result<Vec<u8>, SandboxRuntimeError> {
    let size = fs::metadata(path)
        .await
        .map_err(|error| SandboxRuntimeError::io("inspect protocol file", error))?
        .len();
    require_protocol_bound(field, size, max)?;
    let mut file = fs::File::open(path)
        .await
        .map_err(|error| SandboxRuntimeError::io("open protocol file", error))?;
    let capacity = usize::try_from(size).map_err(|_| SandboxRuntimeError::ProtocolTooLarge {
        field,
        actual: size,
        max,
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    file.read_to_end(&mut bytes)
        .await
        .map_err(|error| SandboxRuntimeError::io("read protocol file", error))?;
    Ok(bytes)
}

fn require_protocol_bound(
    field: &'static str,
    actual: u64,
    max: u64,
) -> Result<(), SandboxRuntimeError> {
    if actual > max {
        return Err(SandboxRuntimeError::ProtocolTooLarge { field, actual, max });
    }
    Ok(())
}

fn path_string(path: &Path) -> Result<String, SandboxRuntimeError> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| SandboxRuntimeError::NonUtf8Path(path.to_path_buf()))
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use helix_core::sandbox_execution::{
        admit_sandbox_execution, SandboxAdmissionPolicy, SandboxExecutionRequest,
        SandboxIsolationRequirements, SandboxNetworkPolicyRef, SandboxOutputContract,
        SandboxResourceBudget,
    };
    use std::collections::BTreeSet;

    struct RejectingResolver;

    #[async_trait]
    impl SandboxArtifactResolver for RejectingResolver {
        async fn resolve_read_only(
            &self,
            _artifact: &SandboxInputArtifact,
        ) -> Result<PathBuf, SandboxRuntimeError> {
            Err(SandboxRuntimeError::ArtifactResolution(
                "not used in unit test".to_string(),
            ))
        }
    }

    fn digest(byte: u8) -> SandboxDigest {
        SandboxDigest::from_bytes([byte; 32])
    }

    fn profile() -> FirecrackerRuntimeProfile {
        let mut profile = FirecrackerRuntimeProfile {
            profile_id: "firecracker-offline-v1".to_string(),
            runner_path: PathBuf::from("/opt/helix/bin/helix-firecracker-runner"),
            jailer_path: PathBuf::from("/opt/firecracker/jailer"),
            kernel_path: PathBuf::from("/opt/helix/images/vmlinux"),
            rootfs_path: PathBuf::from("/opt/helix/images/rootfs.ext4"),
            runner_digest: digest(1),
            jailer_digest: digest(2),
            kernel_digest: digest(3),
            rootfs_digest: digest(4),
            runtime_digest: SandboxDigest::ZERO,
            work_root: PathBuf::from("/var/lib/helix/sandboxes"),
            max_protocol_bytes: 1024 * 1024,
        };
        profile.runtime_digest = profile.computed_runtime_digest();
        profile
    }

    fn admitted(profile: &FirecrackerRuntimeProfile) -> AdmittedSandboxExecution {
        let policy = SandboxAdmissionPolicy {
            policy_id: "sandbox-policy-v1".to_string(),
            policy_epoch: 1,
            allowed_backends: BTreeSet::from([SandboxBackend::Firecracker]),
            allowed_runtime_digests: BTreeSet::from([profile.runtime_digest]),
            allowed_kernel_digests: BTreeSet::from([profile.kernel_digest]),
            allowed_rootfs_digests: BTreeSet::from([profile.rootfs_digest]),
            allowed_network_policy_hashes: BTreeSet::from([digest(5)]),
            max_resources: SandboxResourceBudget {
                vcpus: 2,
                memory_mib: 512,
                disk_mib: 1024,
                wall_time_ms: 30_000,
                process_limit: 64,
                max_output_bytes: 1024,
            },
            max_argv: 8,
            max_argv_bytes: 1024,
            max_inputs: 4,
            max_input_bytes: 4096,
            max_outputs: 4,
            max_secret_leases: 0,
            allow_secret_leases: false,
            required_isolation: SandboxIsolationRequirements::STRICT,
        };
        let request = SandboxExecutionRequest {
            request_id: "request-1".to_string(),
            execution_epoch: 1,
            backend: SandboxBackend::Firecracker,
            runtime_digest: profile.runtime_digest,
            kernel_digest: profile.kernel_digest,
            rootfs_digest: profile.rootfs_digest,
            execution_nonce: digest(6),
            argv: vec!["/usr/bin/true".to_string()],
            working_dir: "/work".to_string(),
            inputs: Vec::new(),
            outputs: vec![SandboxOutputContract {
                output_id: "result".to_string(),
                guest_path: "/outputs/result".to_string(),
                max_bytes: 1024,
            }],
            secret_leases: Vec::new(),
            network: SandboxNetworkPolicyRef {
                policy_id: "deny-all".to_string(),
                policy_hash: digest(5),
                mode: SandboxNetworkMode::DenyAll,
            },
            resources: policy.max_resources,
            isolation: SandboxIsolationRequirements::STRICT,
        };
        admit_sandbox_execution(&policy, request).expect("admitted plan")
    }

    #[test]
    fn runtime_profile_digest_changes_with_runner() {
        let left = profile();
        let mut right = left.clone();
        right.runner_digest = digest(99);
        assert_ne!(
            left.computed_runtime_digest(),
            right.computed_runtime_digest()
        );
    }

    #[test]
    fn adapter_rejects_non_firecracker_backend() {
        let profile = profile();
        let runtime = FirecrackerSandboxRuntime::new(profile.clone(), Arc::new(RejectingResolver));
        let mut request = admitted(&profile).request().clone();
        request.backend = SandboxBackend::Qemu;
        let mut policy = SandboxAdmissionPolicy {
            policy_id: "qemu-policy".to_string(),
            policy_epoch: 1,
            allowed_backends: BTreeSet::from([SandboxBackend::Qemu]),
            allowed_runtime_digests: BTreeSet::from([profile.runtime_digest]),
            allowed_kernel_digests: BTreeSet::from([profile.kernel_digest]),
            allowed_rootfs_digests: BTreeSet::from([profile.rootfs_digest]),
            allowed_network_policy_hashes: BTreeSet::from([digest(5)]),
            max_resources: request.resources,
            max_argv: 8,
            max_argv_bytes: 1024,
            max_inputs: 4,
            max_input_bytes: 4096,
            max_outputs: 4,
            max_secret_leases: 0,
            allow_secret_leases: false,
            required_isolation: SandboxIsolationRequirements::STRICT,
        };
        policy.allowed_backends = BTreeSet::from([SandboxBackend::Qemu]);
        let qemu = admit_sandbox_execution(&policy, request).expect("qemu admission");
        assert!(matches!(
            runtime.validate_admitted_boundary(&qemu),
            Err(SandboxRuntimeError::BackendMismatch(SandboxBackend::Qemu))
        ));
    }

    #[test]
    fn launch_command_never_uses_a_shell() {
        let command = build_runner_command(
            Path::new("/opt/helix/bin/runner"),
            Path::new("/var/lib/helix/run"),
            Path::new("/var/lib/helix/run/request.json"),
        )
        .expect("command");
        assert_eq!(command.program, PathBuf::from("/opt/helix/bin/runner"));
        assert_eq!(command.args[0], OsString::from("--protocol-version"));
        assert!(!command.args.iter().any(|arg| arg == "-c"));
        assert!(!command.environment.contains_key(&OsString::from("PATH")));
    }

    #[test]
    fn strict_boundary_rejects_secret_leases() {
        let profile = profile();
        let runtime = FirecrackerSandboxRuntime::new(profile.clone(), Arc::new(RejectingResolver));
        let mut request = admitted(&profile).request().clone();
        request
            .secret_leases
            .push(helix_core::sandbox_execution::SandboxSecretLeaseRef {
                provider_id: "github".to_string(),
                purpose: "read".to_string(),
                lease_hash: digest(7),
            });
        // The pure core would need a policy that permits the lease. The runtime
        // boundary itself remains independently fail-closed.
        let result = runtime.validate_admitted_boundary(
            &AdmittedSandboxExecutionForTest::from_request(profile, request),
        );
        assert!(matches!(
            result,
            Err(SandboxRuntimeError::SecretLeasesUnsupported)
        ));
    }

    // Test-only wrapper obtains a real admitted value while allowing the unit
    // test to vary a field under a matching permissive policy.
    struct AdmittedSandboxExecutionForTest;

    impl AdmittedSandboxExecutionForTest {
        fn from_request(
            profile: FirecrackerRuntimeProfile,
            request: SandboxExecutionRequest,
        ) -> AdmittedSandboxExecution {
            let policy = SandboxAdmissionPolicy {
                policy_id: "permissive-test-policy".to_string(),
                policy_epoch: 1,
                allowed_backends: BTreeSet::from([request.backend]),
                allowed_runtime_digests: BTreeSet::from([profile.runtime_digest]),
                allowed_kernel_digests: BTreeSet::from([profile.kernel_digest]),
                allowed_rootfs_digests: BTreeSet::from([profile.rootfs_digest]),
                allowed_network_policy_hashes: BTreeSet::from([request.network.policy_hash]),
                max_resources: request.resources,
                max_argv: 8,
                max_argv_bytes: 1024,
                max_inputs: 4,
                max_input_bytes: 4096,
                max_outputs: 4,
                max_secret_leases: 4,
                allow_secret_leases: true,
                required_isolation: SandboxIsolationRequirements::STRICT,
            };
            admit_sandbox_execution(&policy, request).expect("test admission")
        }
    }
}

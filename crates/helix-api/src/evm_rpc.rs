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

//! EVM JSON-RPC client utilities for imperative shell side effects.

use helix_core::HelixError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Minimal EVM receipt shape used by Helix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmReceipt {
    /// Transaction hash.
    #[serde(rename = "transactionHash")]
    pub transaction_hash: Option<String>,
    /// Receipt status (`0x1` success, `0x0` revert).
    pub status: Option<String>,
    /// Block number as hex.
    #[serde(rename = "blockNumber")]
    pub block_number: Option<String>,
}

impl EvmReceipt {
    /// Returns receipt status if status field is present and valid.
    pub fn execution_success(&self) -> Option<bool> {
        match self.status.as_deref() {
            Some("0x1") => Some(true),
            Some("0x0") => Some(false),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct JsonRpcErrorPayload {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    result: Option<Value>,
    error: Option<JsonRpcErrorPayload>,
}

/// Imperative EVM JSON-RPC client.
pub struct EvmRpcClient {
    rpc_url: String,
    http: reqwest::Client,
}

impl EvmRpcClient {
    /// Creates a client for one RPC URL.
    pub fn new(rpc_url: String) -> Result<Self, HelixError> {
        validate_rpc_url(&rpc_url)?;
        Ok(Self {
            rpc_url,
            http: reqwest::Client::new(),
        })
    }

    /// Calls `eth_sendRawTransaction`.
    pub async fn send_raw_transaction(&self, raw_tx_hex: &str) -> Result<String, HelixError> {
        validate_hex_string(raw_tx_hex, "raw_tx_hex")?;
        let value = self
            .rpc_call(
                "eth_sendRawTransaction",
                vec![Value::String(raw_tx_hex.to_string())],
            )
            .await?;
        let tx_hash = value
            .as_str()
            .ok_or_else(|| HelixError::ExternalServiceError {
                service: "evm_rpc".to_string(),
                details: "eth_sendRawTransaction result is not a string".to_string(),
            })?;
        validate_tx_hash(tx_hash)?;
        Ok(tx_hash.to_string())
    }

    /// Calls `eth_getTransactionReceipt`.
    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &str,
    ) -> Result<Option<EvmReceipt>, HelixError> {
        validate_tx_hash(tx_hash)?;
        let value = self
            .rpc_call(
                "eth_getTransactionReceipt",
                vec![Value::String(tx_hash.to_string())],
            )
            .await?;
        if value.is_null() {
            return Ok(None);
        }
        let receipt: EvmReceipt = serde_json::from_value(value)?;
        Ok(Some(receipt))
    }

    async fn rpc_call(&self, method: &str, params: Vec<Value>) -> Result<Value, HelixError> {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let response = self
            .http
            .post(&self.rpc_url)
            .json(&payload)
            .send()
            .await
            .map_err(|err| HelixError::ExternalServiceError {
                service: "evm_rpc".to_string(),
                details: format!("http error: {err}"),
            })?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|err| HelixError::ExternalServiceError {
                service: "evm_rpc".to_string(),
                details: format!("body read error: {err}"),
            })?;
        if !status.is_success() {
            return Err(HelixError::ExternalServiceError {
                service: "evm_rpc".to_string(),
                details: format!("http status {status}: {body}"),
            });
        }

        let parsed: JsonRpcResponse = serde_json::from_str(&body)?;
        if let Some(err) = parsed.error {
            return Err(HelixError::ExternalServiceError {
                service: "evm_rpc".to_string(),
                details: format!("rpc error {}: {}", err.code, err.message),
            });
        }

        parsed
            .result
            .ok_or_else(|| HelixError::ExternalServiceError {
                service: "evm_rpc".to_string(),
                details: format!("rpc method {method} missing result"),
            })
    }
}

/// Produces deterministic tx hash for dry-run mode.
pub fn deterministic_dry_run_hash(raw_tx_hex: &str) -> Result<String, HelixError> {
    validate_hex_string(raw_tx_hex, "raw_tx_hex")?;
    let normalized = raw_tx_hex.trim().to_ascii_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(66);
    out.push_str("0x");
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    Ok(out)
}

/// Validates EVM tx hash format.
pub fn validate_tx_hash(tx_hash: &str) -> Result<(), HelixError> {
    if !is_prefixed_hex(tx_hash) || tx_hash.len() != 66 {
        return Err(HelixError::validation_error(
            "tx_hash",
            "expected 0x-prefixed 32-byte hash",
        ));
    }
    Ok(())
}

fn validate_hex_string(value: &str, field: &str) -> Result<(), HelixError> {
    if !is_prefixed_hex(value) || value.len() % 2 != 0 {
        return Err(HelixError::validation_error(
            field,
            "expected non-empty even-length 0x-prefixed hex string",
        ));
    }
    Ok(())
}

fn validate_rpc_url(rpc_url: &str) -> Result<(), HelixError> {
    let normalized = rpc_url.trim();
    if normalized.starts_with("https://") || normalized.starts_with("http://") {
        return Ok(());
    }
    Err(HelixError::validation_error(
        "rpc_url",
        "expected http:// or https:// URL",
    ))
}

fn is_prefixed_hex(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'0' || !matches!(bytes[1], b'x' | b'X') {
        return false;
    }
    bytes[2..].iter().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_hash_is_deterministic() {
        let a = deterministic_dry_run_hash("0xdeadbeef").unwrap();
        let b = deterministic_dry_run_hash("0xdeadbeef").unwrap();
        assert_eq!(a, b);
        assert!(a.starts_with("0x"));
        assert_eq!(a.len(), 66);
    }

    #[test]
    fn tx_hash_validation_rejects_bad_shape() {
        let err = validate_tx_hash("0x1234").unwrap_err();
        assert!(matches!(err, HelixError::ValidationError { .. }));
    }

    #[test]
    fn hex_validation_requires_prefix_and_even_length() {
        let bad = validate_hex_string("deadbeef", "raw_tx_hex").unwrap_err();
        assert!(matches!(bad, HelixError::ValidationError { .. }));

        let bad2 = validate_hex_string("0xabc", "raw_tx_hex").unwrap_err();
        assert!(matches!(bad2, HelixError::ValidationError { .. }));
    }
}

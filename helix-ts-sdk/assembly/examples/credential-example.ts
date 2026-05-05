import { logMessage as log } from "../host/log";
import { getCredential } from "../host/credential";
import { WasmHostErrorCode, ValueNotFoundError, BufferTooSmallError } from "../utils/errors";

// agent_main is the entry point for the agent
export function agent_main(): void {
  log("Credential example agent started.");

  const existingCredentialName = "myApiKey"; // Assume this credential might exist
  const nonExistentCredentialName = "nonExistentApiKey12345";

  // 1. Try to get an existing credential
  log("Attempting to get credential: " + existingCredentialName);
  let credentialValue: String | null = null;
  try {
    credentialValue = getCredential(existingCredentialName);
    if (credentialValue !== null) {
      log("Successfully retrieved credential '" + existingCredentialName + "'. Value: " + credentialValue);
      // For security, in a real agent, you might not log the actual credential value.
      // This is for demonstration purposes.
      if (credentialValue.length == 0) {
        log("Note: Credential value is an empty string.");
      }
    } else {
      // This case should ideally be handled by ValueNotFoundError,
      // but getCredential returns null if not found without throwing ValueNotFoundError directly.
      log("Credential '" + existingCredentialName + "' not found (returned null).");
    }
  } catch (e: any) {
    if (e instanceof ValueNotFoundError) {
      log("Credential '" + existingCredentialName + "' not found. Error: " + e.message);
    } else if (e instanceof BufferTooSmallError) {
      log("Buffer too small for credential '" + existingCredentialName + "'. Error: " + e.message);
    } else {
      log("An unexpected error occurred while getting credential '" + existingCredentialName + "': " + e.message);
    }
  }

  // 2. Try to get a non-existent credential
  log("Attempting to get non-existent credential: " + nonExistentCredentialName);
  try {
    credentialValue = getCredential(nonExistentCredentialName);
    if (credentialValue !== null) {
      log("Error: Retrieved a value for non-existent credential '" + nonExistentCredentialName + "'. Value: " + credentialValue);
    } else {
      log("Non-existent credential '" + nonExistentCredentialName + "' correctly reported as not found (returned null).");
    }
  } catch (e: any) {
     if (e instanceof ValueNotFoundError) {
      log("Non-existent credential '" + nonExistentCredentialName + "' correctly not found. Error: " + e.message);
    } else if (e instanceof BufferTooSmallError) {
      // This case is unlikely for a "not found" scenario but included for completeness
      log("Buffer too small for non-existent credential '" + nonExistentCredentialName + "'. Error: " + e.message);
    } else {
      log("An unexpected error occurred while getting non-existent credential '" + nonExistentCredentialName + "': " + e.message);
    }
  }

  log("Credential example agent finished.");
}
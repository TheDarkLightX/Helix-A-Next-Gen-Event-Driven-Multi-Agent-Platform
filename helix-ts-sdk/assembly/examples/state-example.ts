import { logMessage as log } from "../host/log";
import { getState, setState, deleteState } from "../host/state";
import { WasmHostErrorCode } from "../utils/errors";

// agent_main is the entry point for the agent
export function agent_main(): void {
  log("State example agent started.");

  const testKey = "myTestStateKey";
  let value: String | null;

  // 1. Try to get initial state
  log("Attempting to get state for key: " + testKey);
  value = getState(testKey);

  if (value === null) {
    log("No initial state found for key: " + testKey + ". Setting initial value.");
    const initialValue = "Hello from state example!";
    const setResult = setState(testKey, initialValue);
    if (setResult == true) {
      log("Successfully set initial state. Value: " + initialValue);
      value = initialValue;
    } else {
      log("Error setting initial state. Code: " + setResult.toString());
      return; // Exit if we can't set initial state
    }
  } else {
    log("Initial state found for key '" + testKey + "'. Value: " + value);
  }

  // 2. Update the state
  const updatedValue = "State has been updated! Timestamp: " + Date.now().toString();
  log("Attempting to update state for key: " + testKey + " to: " + updatedValue);
  const updateResult = setState(testKey, updatedValue);
  if (updateResult == true) {
    log("Successfully updated state.");
  } else {
    log("Error updating state. Code: " + updateResult.toString());
    // Continue to see if we can retrieve the old value or if it's corrupted
  }

  // 3. Get the updated state
  log("Attempting to get updated state for key: " + testKey);
  value = getState(testKey);
  if (value !== null) {
    log("Retrieved updated state. Value: " + value);
    if (value != updatedValue && updateResult == true) {
      log("WARNING: Retrieved value does not match the successfully set updated value!");
    }
  } else {
    log("Error: Could not retrieve state after update attempt for key: " + testKey);
  }

  // 4. Delete the state
  log("Attempting to delete state for key: " + testKey);
  const deleteResult = deleteState(testKey);
  if (deleteResult == true) {
    log("Successfully deleted state for key: " + testKey);
  } else {
    log("Error deleting state. Code: " + deleteResult.toString());
  }

  // 5. Try to get the deleted state
  log("Attempting to get state for key '" + testKey + "' after deletion.");
  value = getState(testKey);
  if (value === null) {
    log("State for key '" + testKey + "' successfully confirmed as deleted (value is null).");
  } else {
    log("Error: State for key '" + testKey + "' was found after deletion. Value: " + value);
  }

  log("State example agent finished.");
}
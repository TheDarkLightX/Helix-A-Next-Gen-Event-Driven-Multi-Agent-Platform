import { Map } from "assemblyscript/std/map";

/**
 * Represents the header of a Helix event.
 */
export interface EventHeader {
  /** Unique identifier for the event. */
  id: String;
  /** Timestamp of when the event occurred, in ISO 8601 format. */
  timestamp: String;
  /** Identifier of the agent that produced the event. */
  agent_id: String;
  /** Optional kind or type of the event. */
  kind: String | null;
}

/**
 * Represents a Helix event, including its header and payload.
 */
export interface Event<T = unknown> {
  /** The header of the event. */
  header: EventHeader;
  /** The payload of the event, can be any type. */
  payload: T;
}

/**
 * Represents the configuration for a Helix agent.
 */
export interface AgentConfig {
  /** Unique identifier for the agent. */
  id: String;
  /** Identifier of the profile this agent belongs to. */
  profile_id: String;
  /** Agent-specific configuration object. */
  config: Map<String, unknown>;
  /** The name of the agent. */
  name: String;
  /** The type of the agent (e.g., "rust", "wasm_rust", "wasm_ts"). */
  agent_type: String;
  /** The version of the agent. */
  version: String;
  // TODO: Add other fields as they become relevant from helix-core
}
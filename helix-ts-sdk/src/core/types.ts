/**
 * Represents the header of a Helix event.
 */
export interface EventHeader {
  /** Unique identifier for the event. */
  id: string;
  /** Timestamp of when the event occurred, in ISO 8601 format. */
  timestamp: string;
  /** Identifier of the agent that produced the event. */
  agent_id: string;
  /** Optional kind or type of the event. */
  kind?: string;
}

/**
 * Represents a Helix event, including its header and payload.
 */
export interface Event<T = any> {
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
  id: string;
  /** Identifier of the profile this agent belongs to. */
  profile_id: string;
  /** Agent-specific configuration object. */
  config: Record<string, any>;
  /** The name of the agent. */
  name: string;
  /** The type of the agent (e.g., "rust", "wasm_rust", "wasm_ts"). */
  agent_type: string;
  /** The version of the agent. */
  version: string;
  // TODO: Add other fields as they become relevant from helix-core
}
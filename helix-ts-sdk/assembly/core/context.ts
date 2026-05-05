import { AgentConfig } from "./types.ts";
import * as host from "../host/index.ts";

/**
 * Provides the context for a Helix agent, enabling interaction with the host environment.
 * This includes accessing configuration, logging, emitting events, and other host-provided services.
 */
export class AgentContext {
  private _agentConfig: AgentConfig | null = null;

  /**
   * Initializes a new instance of the AgentContext.
   * This is typically done by the SDK when an agent is instantiated.
   * The agent configuration can be pre-loaded or fetched lazily.
   *
   * @param initialConfig Optional initial agent configuration. If not provided,
   *                      it might be fetched on first access via `getConfigValue`.
   */
  constructor(initialConfig?: AgentConfig) {
    if (initialConfig) {
      this._agentConfig = initialConfig;
    }
  }

  /**
   * Logs a message to the host environment.
   * @param message The message to log.
   */
  log(message: string): void {
    host.logMessage(message);
  }

  /**
   * Emits an event to the host environment.
   * @param payload The payload of the event.
   * @param eventTypeOverride Optional string to override the default event type.
   */
  emitEvent(payload: any, eventTypeOverride?: string): void {
    host.emitEvent(payload, eventTypeOverride);
  }

  /**
   * Retrieves a configuration value for the current agent.
   * If the full agent config hasn't been loaded yet, this might trigger
   * fetching specific keys. For simplicity in this version, we assume
   * `getConfigValue` can fetch any key.
   *
   * @param key The configuration key.
   * @returns The configuration value, or null if not found.
   * @template T The expected type of the configuration value.
   */
  getConfigValue<T>(key: string): T | null {
    // If we have the full config loaded, we could check there first.
    // However, the host.getConfigValue directly calls the host, which is authoritative.
    return host.getConfigValue<T>(key);
  }

  /**
   * Gets the full agent configuration.
   * This might involve fetching it from the host if not already available.
   * For this initial version, it relies on `getConfigValue` for individual values
   * or the initially provided config. A dedicated host function to get the *entire*
   * config object might be more efficient if needed frequently.
   *
   * @returns The agent's configuration object, or null if unavailable.
   */
  get agentConfig(): AgentConfig | null {
    if (!this._agentConfig) {
      // Attempt to fetch a well-known key that might represent the whole config
      // or indicate that the agent should fetch its full config.
      // This is a placeholder for a more robust config loading strategy.
      // For now, we'll assume that if no initialConfig was provided,
      // the agent developer uses getConfigValue for specific needs.
      // A host function like `helix_get_self_config()` returning the whole AgentConfig
      // would be ideal here.
      //
      // Let's try to fetch 'id' and 'profile_id' to construct a partial config
      // if no full config was provided at construction.
      const id = this.getConfigValue<string>("id"); // Assuming 'id' is a config key for agent's own ID
      const profile_id = this.getConfigValue<string>("profile_id"); // Assuming 'profile_id' is a config key

      if (id && profile_id) {
         // This is a very simplified reconstruction.
         // Ideally, the host provides the full AgentConfig object.
        this._agentConfig = {
            id,
            profile_id,
            config: {}, // The 'config' field within AgentConfig is the map of custom values.
                        // We can't reconstruct this fully without fetching all keys.
            name: this.getConfigValue<string>("name") || "UnknownAgent",
            agent_type: this.getConfigValue<string>("agent_type") || "wasm_ts",
            version: this.getConfigValue<string>("version") || "0.0.0",
        };
      }
    }
    return this._agentConfig;
  }


  /**
   * Gets the current time from the host in milliseconds since UNIX epoch.
   * @returns Current time as a BigInt.
   */
  getTime(): bigint {
    return host.getTime();
  }

  // TODO: Add wrappers for other host functions as they are implemented in src/host/
  // e.g., getState, setState, getCredential, random
}
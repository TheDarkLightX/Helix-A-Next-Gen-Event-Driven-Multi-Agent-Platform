import {
  AbstractHelixAgent,
  SourceAgent,
  AgentConfig,
  Event,
} from "../src/mod.ts";

/**
 * A simple example of a Source Agent.
 * This agent retrieves a message from its configuration and the current time
 * from the host, then emits an event containing this information.
 */
export class SimpleSourceAgent extends AbstractHelixAgent implements SourceAgent {
  private messagePrefix: string = "Default message prefix";
  private intervalId: number | null = null;
  private eventCounter: number = 0;

  constructor(agentConfig: AgentConfig) {
    super(agentConfig);
    // Agent-specific initialization can happen here, using `this.context` if needed.
    // However, async operations should be in `init()`.
    const configuredMessage = this.context.getConfigValue<string>("messagePrefix");
    if (configuredMessage) {
      this.messagePrefix = configuredMessage;
    }
    this.context.log("SimpleSourceAgent: Constructor called.");
  }

  /**
   * Asynchronous initialization for the agent.
   * Called by the SDK after the agent is instantiated.
   */
  async init(): Promise<void> {
    this.context.log(`SimpleSourceAgent: Initializing with prefix: "${this.messagePrefix}"`);
    // Example: Set up a timer to emit events periodically.
    // Note: `setInterval` in a WASM context might behave differently or not be available
    // in the same way as in Node.js/browsers without specific WASI support for timers
    // that `deno compile` might provide or that the host needs to polyfill/enable.
    // For a true source agent, the `runSource` method would typically be called
    // by the Helix runtime on a schedule or trigger, not self-initiated by setInterval.
    // This setInterval is for demonstration of an active agent.
    // In a real scenario, `runSource` would be the main loop or poll.

    // For now, let's assume `runSource` is called periodically by the host.
    // We won't use setInterval here to keep it simple and host-driven.
    this.context.log("SimpleSourceAgent: init complete. Ready to run.");
  }

  /**
   * Main execution logic for the Source Agent.
   * Called by the Helix runtime.
   */
  async runSource(): Promise<void> {
    try {
      this.eventCounter++;
      const currentTime = this.context.getTime(); // Returns BigInt
      const eventPayload = {
        message: `${this.messagePrefix} - Event #${this.eventCounter}`,
        timestampFromHost: currentTime.toString(), // Convert BigInt to string for JSON
        agentId: this.context.agentConfig?.id || "unknown_source_agent",
      };

      this.context.log(
        `SimpleSourceAgent: Emitting event: ${JSON.stringify(eventPayload)}`,
      );
      this.context.emitEvent(eventPayload, "SimpleSourceEvent");
    } catch (e: any) {
      this.context.log(`SimpleSourceAgent: Error in runSource: ${e.message}`);
      // Optionally re-throw or handle as per agent's error strategy
      // throw e;
    }
  }

  /**
   * Cleanup logic for the agent.
   * This is not a standard lifecycle method in the current SDK plan but demonstrates
   * how an agent might clean up resources if it had any (e.g., clearing intervals).
   */
  stopAgent(): void {
    if (this.intervalId !== null) {
      clearInterval(this.intervalId);
      this.intervalId = null;
      this.context.log("SimpleSourceAgent: Stopped periodic event emission.");
    }
  }
}
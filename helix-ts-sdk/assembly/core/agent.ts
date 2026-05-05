import { AgentContext } from "./context.ts";
import { Event, AgentConfig } from "./types.ts";

/**
 * Base interface for all Helix agents developed with the TypeScript SDK.
 */
export interface HelixAgent {
  /**
   * The context providing access to host functionalities and agent configuration.
   * This will be injected by the SDK runtime.
   */
  readonly context: AgentContext;

  /**
   * Optional asynchronous initialization method for the agent.
   * This method is called by the SDK after the agent is instantiated and
   * the context is prepared. Agents can override this to perform setup tasks
   * like fetching initial state, validating configuration, etc.
   *
   * If this method throws an error, agent initialization is considered failed.
   */
  init?(): Promise<void>;
}

/**
 * Interface for a Source agent.
 * Source agents are responsible for generating events from external or internal sources.
 */
export interface SourceAgent extends HelixAgent {
  /**
   * Executes the primary logic of the Source agent.
   * This method is called by the Helix runtime to trigger data polling or event generation.
   * The agent should use `this.context.emitEvent()` to publish new events.
   *
   * If this method throws an error, it will be caught by the runtime and logged.
   * Depending on the runtime's configuration, the agent might be rescheduled or marked as failed.
   */
  runSource(): Promise<void>;
}

/**
 * Interface for an Action agent.
 * Action agents perform actions based on incoming events.
 */
export interface ActionAgent extends HelixAgent {
  /**
   * Executes the primary logic of the Action agent for a given event.
   * This method is called by the Helix runtime when an event is routed to this agent.
   *
   * @param event The event that triggered this action. The payload type `T` can be specified
   *              by the agent implementation if known.
   * @template T The expected type of the event payload.
   *
   * If this method throws an error, it will be caught by the runtime and logged.
   */
  runAction<T = any>(event: Event<T>): Promise<void>;
}

/**
 * Interface for a Transform agent.
 * Transform agents process an incoming event and may produce a new, modified event.
 */
export interface TransformAgent extends HelixAgent {
  /**
   * Executes the transformation logic of the agent for a given event.
   * This method is called by the Helix runtime when an event is routed to this agent
   * for transformation.
   *
   * @param event The event to transform. The payload type `T` can be specified
   *              by the agent implementation if known.
   * @template T The expected type of the incoming event payload.
   * @template U The expected type of the outgoing (transformed) event payload.
   * @returns A Promise that resolves to the transformed event, or `null` or `undefined`
   *          if the event should be filtered out (i.e., not propagated further).
   *          If a new event object is returned, its `agent_id` in the header should typically
   *          be updated to this agent's ID.
   *
   * If this method throws an error, it will be caught by the runtime and logged,
   * and the original event will likely not be propagated.
   */
  runTransform<T = any, U = any>(event: Event<T>): Promise<Event<U> | null | undefined>;
}

/**
 * Abstract base class providing common functionality for Helix agents.
 * Agent implementations can extend this class for convenience.
 */
export abstract class AbstractHelixAgent implements HelixAgent {
  readonly context: AgentContext;

  /**
   * Constructs a new AbstractHelixAgent.
   * @param agentConfig The configuration for this agent, typically provided by the host during instantiation.
   *                    The SDK will use this to create the AgentContext.
   * @param memory Pointer to the WASM linear memory, required for initializing memory utilities.
   *               This is a simplification; a more robust approach would involve the SDK
   *               managing memory initialization more centrally.
   * @param sharedBufferPtr Pointer to the pre-allocated shared buffer in WASM memory.
   * @param sharedBufferSize Size of the pre-allocated shared buffer.
   */
  constructor(agentConfig: AgentConfig) {
    this.context = new AgentContext(agentConfig);
    // Note: Memory initialization (`initWasmMemory` and `_helix_sdk_init_shared_buffer`)
    // needs to be handled carefully. It's typically done once per WASM module instance.
    // Placing it in the agent constructor might be too late or redundant if multiple agents
    // are in one module (though current plan is one agent per module).
    // This will be refined when implementing the `_helix_agent_init` export.
  }

  async init?(): Promise<void> {
    // Base implementation does nothing. Subclasses can override.
  }
}
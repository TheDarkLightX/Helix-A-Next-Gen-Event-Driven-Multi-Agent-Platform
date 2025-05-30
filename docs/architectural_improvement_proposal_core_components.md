# Architectural Improvement Proposal: Phased Implementation of Core Components

**1. Introduction**

The Helix project, as defined in `docs/specification.md`, aims to be a sophisticated, event-driven agent platform. Our analysis of the current codebase reveals that while foundational elements like `helix-core` (domain models) and basic event handling via `helix-runtime` and NATS are in place, several critical components are currently placeholders. These include the Rule Engine, the Agent SDK, and the WASM plugin runtime, which are essential for realizing the "smarter IFTTT" vision and plugin-based extensibility.

This proposal outlines a phased plan to implement these missing core components, focusing on building a robust and extensible architecture.

**2. Guiding Principles**

The implementation of these components should adhere to the following principles:

*   **Modularity:** Each component (Agent SDK, WASM Runtime, Rule Engine) should be a distinct crate with clear responsibilities and well-defined interfaces.
*   **Testability:** Emphasize comprehensive testing (unit, integration) from the outset for each component.
*   **Adherence to Specification:** Align with the goals and technical choices outlined in `docs/specification.md` (e.g., Rete-like engine, Cedar policy, WASM for plugins).
*   **Incremental Development:** Build functionality in stages, allowing for earlier integration and feedback.
*   **Extensibility:** Design components with future enhancements in mind (e.g., new agent types, more complex rule conditions).
*   **Security:** Integrate security considerations (sandboxing for WASM, policy checks) as components are developed, not just as an afterthought.

**3. Phased Implementation Plan**

We propose a four-phase approach:

**Phase 1: Core Agent SDK & Native Agent Execution**

*   **Goal:** Enable the development and execution of basic, in-process Rust agents. This lays the groundwork before introducing WASM complexity.
*   **Key Tasks & Components:**
    *   **`helix-agent-sdk` (`crates/helix-agent-sdk/`):**
        *   Define core agent traits (e.g., `SourceAgent`, `TransformAgent`, `ActionAgent`) building upon `helix-core::agent::Agent`.
        *   Implement procedural macros (e.g., `#[source_agent]`, `#[action_agent]`) to simplify agent creation in Rust. These macros would handle boilerplate for registration and lifecycle management.
        *   Define clear agent lifecycle methods (e.g., `init`, `start`, `stop`, `handle_event` or `execute`).
        *   Provide context objects to agents, allowing them to emit events (back to NATS via `helix-runtime`) and access their configuration.
    *   **`helix-runtime` (`crates/helix-runtime/`):**
        *   Enhance `AgentRunner` (`crates/helix-runtime/src/agent_runner.rs`) to:
            *   Load configurations for these native Rust agents.
            *   Instantiate and manage the lifecycle of these agents.
            *   Execute simple recipes (`helix-core::recipe::Recipe`) by orchestrating calls between native agents.
    *   **`helix-storage` (`crates/helix-storage/`):**
        *   Ensure agent configurations and basic recipe definitions (linking native agents) can be persisted and retrieved using `PostgresStateStore` (`crates/helix-storage/src/postgres_state_store.rs`).
*   **Outcome:** Developers can write and run simple, chained automation tasks using Rust agents executed directly by the runtime.

**Phase 2: WASM Plugin Runtime & SDK Extension**

*   **Goal:** Enable the execution of agents compiled to WebAssembly (WASM), fulfilling a key extensibility requirement.
*   **Key Tasks & Components:**
    *   **`helix-wasm` (`crates/helix-wasm/`):**
        *   Integrate `wasmtime` and `wasmtime-wasi` (uncomment and configure dependencies).
        *   Implement the WASM host environment:
            *   Define a clear set of host functions that WASM agents can import and call (e.g., `helix_emit_event`, `helix_get_config_value`, `helix_log_message`). These functions will bridge WASM guest calls to `helix-runtime` or `helix-core` functionalities.
            *   Implement robust sandboxing for WASM modules (filesystem access, network access, resource limits) as per the security model in the specification.
        *   Develop mechanisms for loading, instantiating, and managing the lifecycle of WASM modules.
    *   **`helix-agent-sdk` (`crates/helix-agent-sdk/`):**
        *   Extend the procedural macros to support compiling Rust agents to the `wasm32-wasi` target. This involves generating the necessary WASM export/import bindings.
        *   Define the serialization format (e.g., JSON, MessagePack) for data passed between the host and WASM agents (events, configuration).
        *   Begin planning for the TypeScript SDK (e.g., using Deno to compile TypeScript to WASM, and providing equivalent TypeScript bindings/decorators).
    *   **`helix-runtime` (`crates/helix-runtime/`):**
        *   Modify `AgentRunner` to delegate execution of WASM agents to `helix-wasm`. This involves passing event data to WASM agents and receiving output.
*   **Outcome:** Developers can write agents in Rust (and later TypeScript), compile them to WASM, and have Helix execute them securely as plugins.

**Phase 3: Rule Engine Core & Basic Integration**

*   **Goal:** Implement a foundational rule engine capable of matching events and triggering agent recipes.
*   **Key Tasks & Components:**
    *   **`helix-rule-engine` (`crates/helix-rule-engine/`):**
        *   **Initial Matcher:** Start with a straightforward event pattern matcher. This matcher would subscribe to relevant event streams from NATS (via `helix-runtime`).
            *   Define a rule structure (e.g., in YAML or JSON, stored in `helix-storage`) that specifies:
                *   Event patterns/conditions to match (e.g., event type, specific header values, simple payload field checks).
                *   The `Recipe` ID to execute upon a match.
        *   **Integration:**
            *   When a rule matches an incoming event, the rule engine will instruct `helix-runtime` (e.g., by publishing a "trigger recipe" command to an internal NATS subject, or via a direct internal API if co-located) to execute the associated recipe.
    *   **`helix-storage` (`crates/helix-storage/`):**
        *   Implement storage and retrieval for rule definitions.
    *   **`helix-api` (`crates/helix-api/`):**
        *   Provide basic API endpoints for creating, reading, updating, and deleting rule definitions.
*   **Outcome:** The core "event -> rule match -> recipe execution" loop is functional, enabling basic event-driven automation.

**Phase 4: Cedar Policy Integration & Advanced Rule Engine**

*   **Goal:** Integrate Cedar for robust policy enforcement and evolve the rule engine towards the specified Rete-like capabilities.
*   **Key Tasks & Components:**
    *   **`helix-security` (`crates/helix-security/`):**
        *   Integrate the `cedar-policy` crate (uncomment and configure).
        *   Develop the `Policy` module (`crates/helix-security/src/policies.rs`) to:
            *   Load and manage Cedar policies (likely stored via `helix-storage`).
            *   Provide an interface for other components (Rule Engine, Agent Runner, API) to authorize actions based on these policies (e.g., "Can this profile's agent X access data Y?", "Can recipe Z be triggered by event E?").
    *   **`helix-rule-engine` (`crates/helix-rule-engine/`):**
        *   **Policy Enforcement:** Before triggering a recipe, consult `helix-security` to ensure the action is permitted by Cedar policies.
        *   **Rete Evolution/Integration:**
            *   If a simpler matcher was built in Phase 3, begin refactoring it towards a Rete-like architecture for improved performance with many rules and incremental updates. This involves implementing or integrating concepts like alpha/beta nodes, join networks, and working memory.
            *   Alternatively, evaluate and integrate an existing Rust Rete library if one meets the project's needs and licensing.
        *   Support more complex rule conditions and potentially stateful rules.
    *   **`helix-api` (`crates/helix-api/`):**
        *   Expose endpoints for managing Cedar policies.
*   **Outcome:** Helix has a powerful, policy-controlled rule engine capable of complex event processing, aligning closely with the specification's vision.

**4. Cross-Cutting Concerns**

*   **Testing:** Rigorous unit and integration tests are paramount for each phase. As components mature, develop end-to-end test scenarios.
*   **Documentation:**
    *   Update `docs/specification.md` to reflect design decisions made during implementation.
    *   Create developer documentation for the Agent SDK.
    *   Document the Rule Engine's capabilities and rule definition format.
*   **Observability:** Ensure `tracing` is used effectively throughout these new components to provide insights into event flow, agent execution, and rule matching.
*   **Error Handling:** Implement robust error handling and reporting within and between components.

**5. Proposed High-Level Component Interaction Diagram (Post-Implementation)**

```mermaid
graph TD
    subgraph User/External
        ExternalWebhook[External Webhook/Event Source]
        UserCLI[User via CLI/UI]
    end

    subgraph HelixSystem [Helix System - Kubernetes Cluster]
        API[helix-api (Axum)]
        Runtime[helix-runtime]
        Core[helix-core]
        AgentSDK[helix-agent-sdk (Macros)]
        WASMR[helix-wasm (Wasmtime)]
        RuleEngine[helix-rule-engine (Rete+Cedar)]
        Security[helix-security (Cedar, Age)]
        Storage[helix-storage (Postgres, etc.)]
        LLM[helix-llm (Placeholder for now)]
        Embeddings[helix-embeddings (Placeholder for now)]
        NATS[NATS JetStream (Event Mesh)]

        UserCLI -- Manages Rules/Policies/Agents --> API
        ExternalWebhook -- Publishes Raw Events --> API

        API -- Ingests Events/Triggers --> NATS
        API -- CRUD Ops --> Storage
        API -- AuthZ Requests --> Security

        Runtime -- Consumes Events & Commands --> NATS
        Runtime -- Publishes Processed Events --> NATS
        Runtime -- Loads Agent Configs/Recipes --> Storage
        Runtime -- Executes Native Agents --> Core
        Runtime -- Delegates to WASM Runtime --> WASMR
        Runtime -- Requests Policy Checks --> Security

        WASMR -- Executes WASM Agents --> AgentSDKLibrariesUsedInGuest
        WASMR -- Host Functions Call --> Runtime 
        
        RuleEngine -- Consumes Events --> NATS
        RuleEngine -- Loads Rules --> Storage
        RuleEngine -- Requests Policy Checks --> Security
        RuleEngine -- Publishes "Trigger Recipe" Commands --> NATS

        Security -- Loads Policies/Credentials --> Storage

        Core-. Defines Domain Models .-> Runtime
        Core-. Defines Domain Models .-> AgentSDK
        Core-. Defines Domain Models .-> RuleEngine
        Core-. Defines Domain Models .-> Security
        Core-. Defines Domain Models .-> Storage
        Core-. Defines Domain Models .-> LLM
        Core-. Defines Domain Models .-> Embeddings
        
        AgentSDK -- Used by Devs to Build --> NativeAgent[Native Rust Agent]
        AgentSDK -- Used by Devs to Build --> WASMAgent[WASM Agent (Rust/TS)]
        
        NativeAgent -- Executed by --> Runtime
        WASMAgent -- Executed by --> WASMR
    end

    NativeAgent -- Interacts via SDK --> Core
    WASMAgent -- Interacts via SDK --> Core


    style HelixSystem fill:#f9f,stroke:#333,stroke-width:2px
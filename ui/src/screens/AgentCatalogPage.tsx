import { useEffect, useState } from "react";
import { DeterministicAgentSpec, fetchAgentCatalog } from "../lib/api";

const verificationCommands = [
  "./scripts/verify_esso_roi_agents.sh",
  "cargo test --manifest-path crates/helix-core/Cargo.toml --lib deterministic_agents",
  "cargo test --manifest-path crates/helix-core/Cargo.toml --lib deterministic_policy",
];

export function AgentCatalogPage() {
  const [agents, setAgents] = useState<DeterministicAgentSpec[]>([]);
  const [status, setStatus] = useState<string>("Loading deterministic agent catalog...");

  useEffect(() => {
    void (async () => {
      try {
        const catalog = await fetchAgentCatalog();
        setAgents(catalog);
        setStatus(`Loaded ${catalog.length} deterministic agents.`);
      } catch (error) {
        setStatus(`Failed to load agent catalog: ${(error as Error).message}`);
      }
    })();
  }, []);

  return (
    <section className="dashboard-grid">
      <article className="panel panel-hero">
        <p className="mono-label">Agent Catalog</p>
        <h2>High-ROI Deterministic State Machines</h2>
        <p>
          These kernels are pure and replayable. Each has an ESSO model for fail-closed verification.
        </p>
      </article>

      <article className="panel">
        <p className="mono-label">Verification Commands</p>
        <div className="command-stack">
          {verificationCommands.map((command) => (
            <code key={command} className="command-inline">
              {command}
            </code>
          ))}
        </div>
        <p className="status-line">{status}</p>
      </article>

      <article className="panel">
        <p className="mono-label">Implemented Agents</p>
        <div className="agent-grid">
          {agents.map((agent) => (
            <div key={agent.id} className="agent-card">
              <div className="agent-card-head">
                <h3>{agent.name}</h3>
                <span className="status-pill ok">Implemented</span>
              </div>
              <p>{agent.roi_rationale}</p>
              <p className="mono-detail">{agent.id}</p>
              <code className="command-inline">{agent.kernel_module}</code>
              <code className="command-inline">{agent.esso_model}</code>
            </div>
          ))}
        </div>
      </article>
    </section>
  );
}

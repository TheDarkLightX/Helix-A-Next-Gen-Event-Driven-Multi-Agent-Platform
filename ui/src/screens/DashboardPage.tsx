import { useEffect, useState } from "react";
import type { DeterministicAgentSpec } from "../lib/api";
import { fetchAgentCatalog, fetchAgentCatalogQuality } from "../lib/api";

const lanes = [
  { name: "Formal Kernel", status: "Healthy", note: "Finite-state transitions + invariants" },
  { name: "Imperative Shell", status: "Healthy", note: "Effects isolated behind execution port" },
  { name: "Rule Engine", status: "In Progress", note: "Event matching + recipe triggers" },
  { name: "WASM Runtime", status: "In Progress", note: "Host APIs and sandbox hardening" },
  { name: "Onchain Shell", status: "Healthy", note: "EVM JSON-RPC raw tx + receipt lifecycle" },
  { name: "Autopilot Guard", status: "Healthy", note: "LLM-operable control plane with fail-closed gating" },
];

const controls = [
  { title: "Run Formal Core Verification", command: "./scripts/verify_formal_core.sh" },
  { title: "Verify ROI Agent Models", command: "./scripts/verify_formal_agents.sh" },
  { title: "Apply Secure Onchain Template", command: "POST /api/v1/agents/templates/secure_onchain_executor" },
  { title: "Autopilot Status", command: "GET /api/v1/autopilot/status" },
  { title: "Core Kernel Tests", command: "cargo test --manifest-path crates/helix-core/Cargo.toml execution_kernel" },
  { title: "Onchain Dry Run", command: "POST /api/v1/onchain/send_raw with dry_run=true" },
  { title: "Launch UI", command: "cd ui && npm install && npm run dev" },
];

const fallbackAgents = [
  {
    name: "Dedup Window Agent",
    value: "Eliminates duplicate event storms before downstream work.",
  },
  {
    name: "Token Bucket Rate Limiter",
    value: "Prevents overload while preserving deterministic admission.",
  },
  {
    name: "Circuit Breaker Agent",
    value: "Fast-fails unstable dependencies with controlled recovery probes.",
  },
  {
    name: "Retry Budget Agent",
    value: "Bounds retries to stop runaway feedback loops and queue blowups.",
  },
  {
    name: "Approval Gate Agent",
    value: "Deterministic quorum gate for high-risk or privileged actions.",
  },
  {
    name: "Backpressure Controller Agent",
    value: "Classifies queue pressure into accept, throttle, or shed without randomness.",
  },
  {
    name: "SLA Deadline Agent",
    value: "Tracks deadline windows and emits deterministic expiry/completion transitions.",
  },
  {
    name: "DLQ Budget Agent",
    value: "Routes repeated failures to dead-letter queue after a fixed failure budget.",
  },
  {
    name: "Onchain Transaction Intent Agent",
    value: "Controls tx submit/receipt transitions deterministically before RPC side effects.",
  },
  {
    name: "Nonce Manager Agent",
    value: "Reserves, confirms, and reconciles nonces to avoid collision and replay drift.",
  },
  {
    name: "Fee Bidding Agent",
    value: "Produces bounded deterministic fee quotes with rejection-based bumping.",
  },
  {
    name: "Finality Reorg Guard Agent",
    value: "Gates settlement by confirmation depth and detects reorg conditions.",
  },
  {
    name: "Allowlist Policy Guard Agent",
    value: "Deterministically blocks unauthorized chain/contract/method tuples.",
  },
];

const featuredAgentOrder = [
  "dedup_window",
  "token_bucket",
  "circuit_breaker",
  "retry_budget",
  "approval_gate",
  "backpressure",
  "sla_deadline",
  "dlq_budget",
  "onchain_tx_intent",
  "nonce_manager",
  "fee_bidding",
  "finality_guard",
  "allowlist_guard",
  "symbolic_reasoning_gate",
  "expert_system_gate",
  "neuro_risk_gate",
  "neuro_symbolic_fusion_gate",
];

const reasoningModes = [
  {
    name: "KRR + Symbolic",
    note: "Forward chaining over finite facts/rules/triples with deterministic closure rounds.",
  },
  {
    name: "Expert System",
    note: "Weighted deterministic rule voting with explicit feature thresholds.",
  },
  {
    name: "Neural Risk",
    note: "Deterministic linear scoring model with fixed thresholds for allow/review/deny.",
  },
  {
    name: "Neuro-Symbolic",
    note: "Fail-closed symbolic entailment gate fused with bounded neural confidence.",
  },
];

function pickFeaturedAgents(catalog: DeterministicAgentSpec[]) {
  const ranked = featuredAgentOrder
    .map((id) => catalog.find((agent) => agent.id === id))
    .filter((agent): agent is DeterministicAgentSpec => Boolean(agent));

  if (ranked.length >= 10) {
    return ranked.map((agent) => ({
      name: agent.name,
      value: agent.roi_rationale,
    }));
  }

  return fallbackAgents;
}

export function DashboardPage() {
  const [agentClassCount, setAgentClassCount] = useState<number>(fallbackAgents.length);
  const [featuredAgents, setFeaturedAgents] = useState(fallbackAgents);
  const [categoryCoverage, setCategoryCoverage] = useState<number>(0);
  const [huginnBaseline, setHuginnBaseline] = useState<number>(68);
  const [baselineGap, setBaselineGap] = useState<number>(agentClassCount - 68);
  const superiorityRatio = (agentClassCount / Math.max(huginnBaseline, 1)).toFixed(2);
  const healthyLaneCount = lanes.filter((lane) => lane.status === "Healthy").length;

  useEffect(() => {
    void (async () => {
      try {
        const [catalog, quality] = await Promise.all([
          fetchAgentCatalog(),
          fetchAgentCatalogQuality(),
        ]);
        setAgentClassCount(catalog.length);
        setFeaturedAgents(pickFeaturedAgents(catalog));
        setCategoryCoverage(quality.expanded_categories);
        setHuginnBaseline(quality.huginn_baseline_agents);
        setBaselineGap(quality.total_agents - quality.huginn_baseline_agents);
      } catch {
        setAgentClassCount(fallbackAgents.length);
        setFeaturedAgents(fallbackAgents);
        setCategoryCoverage(0);
        setHuginnBaseline(68);
        setBaselineGap(fallbackAgents.length - 68);
      }
    })();
  }, []);

  return (
    <section className="dashboard-grid">
      <article className="panel panel-hero panel-span-12">
        <p className="mono-label">System Thesis</p>
        <h2>Formal Functional Core, Imperative Shell</h2>
        <p>
          Helix execution state is modeled as a deterministic finite machine with formal model
          parity. Runtime performs side effects only through explicit effect handlers.
        </p>
        <div className="metrics-grid">
          <div className="metric-card">
            <p className="metric-label">Runtime Lanes</p>
            <p className="metric-value">{lanes.length}</p>
          </div>
          <div className="metric-card">
            <p className="metric-label">Healthy Lanes</p>
            <p className="metric-value">{healthyLaneCount}</p>
          </div>
          <div className="metric-card">
            <p className="metric-label">Coverage Domains</p>
            <p className="metric-value">{categoryCoverage}</p>
          </div>
          <div className="metric-card">
            <p className="metric-label">Agent Classes</p>
            <p className="metric-value">{agentClassCount}</p>
          </div>
          <div className="metric-card">
            <p className="metric-label">Baseline Gap</p>
            <p className="metric-value">{baselineGap >= 0 ? `+${baselineGap}` : baselineGap}</p>
          </div>
          <div className="metric-card">
            <p className="metric-label">Baseline Ratio</p>
            <p className="metric-value">{superiorityRatio}x</p>
          </div>
        </div>
      </article>

      <article className="panel panel-span-6">
        <p className="mono-label">Runtime Lanes</p>
        <ul className="lane-list">
          {lanes.map((lane) => (
            <li key={lane.name} className="lane-row">
              <div>
                <h3>{lane.name}</h3>
                <p>{lane.note}</p>
              </div>
              <span className={`status-pill ${lane.status === "Healthy" ? "ok" : "warn"}`}>
                {lane.status}
              </span>
            </li>
          ))}
        </ul>
      </article>

      <article className="panel panel-span-6">
        <p className="mono-label">Reasoning Backends</p>
        <ul className="lane-list">
          {reasoningModes.map((mode) => (
            <li key={mode.name} className="lane-row">
              <div>
                <h3>{mode.name}</h3>
                <p>{mode.note}</p>
              </div>
              <span className="status-pill ok">Deterministic</span>
            </li>
          ))}
        </ul>
      </article>

      <article className="panel panel-span-6">
        <p className="mono-label">Control Commands</p>
        <div className="command-stack">
          {controls.map((item) => (
            <div key={item.title} className="command-row">
              <h3>{item.title}</h3>
              <code>{item.command}</code>
            </div>
          ))}
        </div>
      </article>

      <article className="panel panel-span-12">
        <p className="mono-label">Featured Agent Classes</p>
        <ul className="lane-list">
          {featuredAgents.map((agent) => (
            <li key={agent.name} className="lane-row">
              <div>
                <h3>{agent.name}</h3>
                <p>{agent.value}</p>
              </div>
              <span className="status-pill ok">Implemented</span>
            </li>
          ))}
        </ul>
      </article>
    </section>
  );
}

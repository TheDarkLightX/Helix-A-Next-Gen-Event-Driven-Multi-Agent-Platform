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
  {
    title: "Verify Core + Onchain Formal Models",
    command: "./scripts/verify_formal_core.sh",
  },
  { title: "Autopilot Status", command: "GET /api/v1/autopilot/status" },
  { title: "Core Kernel Tests", command: "cargo test --manifest-path crates/helix-core/Cargo.toml execution_kernel" },
  { title: "Onchain Dry Run", command: "POST /api/v1/onchain/send_raw with dry_run=true" },
  { title: "Launch UI", command: "cd ui && npm install && npm run dev" },
];

const roiAgents = [
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

export function DashboardPage() {
  const healthyLaneCount = lanes.filter((lane) => lane.status === "Healthy").length;

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
            <p className="metric-label">Control Commands</p>
            <p className="metric-value">{controls.length}</p>
          </div>
          <div className="metric-card">
            <p className="metric-label">ROI Agents</p>
            <p className="metric-value">{roiAgents.length}</p>
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
        <p className="mono-label">High ROI Deterministic Agents</p>
        <ul className="lane-list">
          {roiAgents.map((agent) => (
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

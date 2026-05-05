import { useEffect, useState } from "react";
import type { CaseQueueEntry, DeterministicAgentSpec } from "../lib/api";
import {
  fetchAgentCatalog,
  fetchAgentCatalogQuality,
  fetchCases,
  fetchIntelOverview,
  fetchMarketIntelOverview,
} from "../lib/api";

const lanes = [
  { name: "Formal Kernel", status: "NOMINAL", note: "FSM transitions + invariants" },
  { name: "Imperative Shell", status: "NOMINAL", note: "Effects behind execution port" },
  { name: "Source Registry", status: "NOMINAL", note: "Trust-scored collectors" },
  { name: "Evidence Pipeline", status: "DEGRADED", note: "High latency detected", classOverride: "warning" },
  { name: "Case Lifecycle", status: "NOMINAL", note: "Deterministic state flow" },
  { name: "Market Intel View", status: "NOMINAL", note: "Pricing & launch metrics" },
  { name: "Onchain Shell", status: "NOMINAL", note: "EVM intent side-effects" },
  { name: "Autopilot Guard", status: "NOMINAL", note: "LLM fail-closed gate" },
];

const controls = [
  { title: "VERIFY_CORE", command: "./scripts/verify_formal_core.sh" },
  { title: "VERIFY_AGENTS", command: "./scripts/verify_formal_agents.sh" },
  { title: "SYNC_SOURCES", command: "GET /api/v1/sources" },
  { title: "INGEST_EVIDENCE", command: "POST /api/v1/evidence/ingest" },
  { title: "POLL_CASES", command: "GET /api/v1/cases" },
  { title: "DRY_RUN", command: "POST /api/v1/onchain/send_raw ?dry_run=1" },
];

const fallbackAgents = [
  { name: "Dedup Window", value: "Dup stream suppression" },
  { name: "Token Bucket", value: "Rate limit admissions" },
  { name: "Circuit Breaker", value: "TRIPPED: Auth rate exceeded", classOverride: "danger" },
  { name: "Retry Budget", value: "Bounded loop control" },
  { name: "Approval Gate", value: "Quorum auth gate" },
  { name: "Backpressure", value: "Queue shedder" },
  { name: "SLA Deadline", value: "Track expiry ticks" },
  { name: "DLQ Budget", value: "Dead-letter router" },
  { name: "Tx Intent", value: "EVM submission intent" },
  { name: "Nonce Manager", value: "Collision / replay guard" },
  { name: "Fee Bidding", value: "Deterministic EVM bump" },
  { name: "Finality Guard", value: "Reorg / depth gate" },
  { name: "Allowlist Guard", value: "Contract access wall" },
];

const reasoningModes = [
  { name: "KRR / Symbolic", note: "Forward chaining closure" },
  { name: "Expert System", note: "Weighted rule voting" },
  { name: "Neural Risk", note: "Linear bounds check" },
  { name: "Neuro-Symbolic", note: "Fused logic gate" },
];

function pickFeaturedAgents(catalog: DeterministicAgentSpec[]) {
  if (catalog.length >= 10) {
    return catalog.slice(0, 13).map((agent) => ({
      name: agent.name,
      value: agent.roi_rationale,
      classOverride: undefined
    }));
  }
  return fallbackAgents as any[];
}

export function DashboardPage() {
  const [agentClassCount, setAgentClassCount] = useState<number>(fallbackAgents.length);
  const [featuredAgents, setFeaturedAgents] = useState(fallbackAgents);
  const [topCases, setTopCases] = useState<CaseQueueEntry[]>([]);
  const [huginnBaseline, setHuginnBaseline] = useState<number>(68);
  const [sourceCount, setSourceCount] = useState<number>(0);
  const [watchlistCount, setWatchlistCount] = useState<number>(0);
  const [evidenceCount, setEvidenceCount] = useState<number>(0);
  const [openCaseCount, setOpenCaseCount] = useState<number>(0);
  const [escalatedCaseCount, setEscalatedCaseCount] = useState<number>(0);
  const [trackedCompanyCount, setTrackedCompanyCount] = useState<number>(0);
  const [marketWatchlistCount, setMarketWatchlistCount] = useState<number>(0);

  const superiorityRatio = (agentClassCount / Math.max(huginnBaseline, 1)).toFixed(2);

  useEffect(() => {
    void (async () => {
      try {
        const [catalog, quality, overview, marketOverview, cases] = await Promise.all([
          fetchAgentCatalog(),
          fetchAgentCatalogQuality(),
          fetchIntelOverview(),
          fetchMarketIntelOverview(),
          fetchCases({ limit: 4 }),
        ]);
        setAgentClassCount(catalog.length);
        setFeaturedAgents(pickFeaturedAgents(catalog));
        setTopCases(cases.slice(0, 4));
        setHuginnBaseline(quality.huginn_baseline_agents);
        setSourceCount(overview.source_count);
        setWatchlistCount(overview.watchlist_count);
        setEvidenceCount(overview.evidence_count);
        setOpenCaseCount(overview.open_case_count);
        setEscalatedCaseCount(overview.escalated_case_count);
        setTrackedCompanyCount(marketOverview.tracked_company_count);
        setMarketWatchlistCount(marketOverview.market_watchlist_count);
      } catch {
        // Fallbacks stay defaults
      }
    })();
  }, []);

  return (
    <section className="hud-grid">

      {/* GLOBAL TELEMETRY BAR */}
      <article className="tac-panel span-12">
        <div className="panel-header">
          <span className="panel-title">SYS.OVERVIEW [GLOBAL_READOUT]</span>
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(140px, 1fr))', gap: '8px' }}>
          <div className="data-block">
            <span className="d-label">NET.SOURCES</span>
            <span className="d-value">{sourceCount}</span>
          </div>
          <div className="data-block">
            <span className="d-label">EVIDENCE.VOL</span>
            <span className="d-value">{evidenceCount}</span>
          </div>
          <div className="data-block">
            <span className="d-label">WATCH.RULES</span>
            <span className="d-value">{watchlistCount}</span>
          </div>
          <div className="data-block">
            <span className="d-label">CASES.OPEN</span>
            <span className="d-value highlight">{openCaseCount}</span>
          </div>
          <div className="data-block">
            <span className="d-label">CASES.ESCALATED</span>
            <span className="d-value danger">{escalatedCaseCount}</span>
          </div>
          <div className="data-block">
            <span className="d-label">AGENT.CLASSES</span>
            <span className="d-value">{agentClassCount}</span>
          </div>
          <div className="data-block">
            <span className="d-label">MKT.TRACKED</span>
            <span className="d-value">{trackedCompanyCount}</span>
          </div>
          <div className="data-block">
            <span className="d-label">SUPERIORITY_RX</span>
            <span className="d-value alert">{superiorityRatio}x</span>
          </div>
        </div>
      </article>

      {/* SYSTEM LANES */}
      <article className="tac-panel span-4">
        <div className="panel-header">
          <span className="panel-title">SYS.LANES [RTE]</span>
        </div>
        <div className="tac-list">
          {lanes.map((lane: any) => (
            <div key={lane.name} className={`tac-row ${lane.classOverride || 'healthy'}`}>
              <span className="row-primary">{lane.name}</span>
              <span className="row-secondary">[{lane.status}]</span>
            </div>
          ))}
        </div>
      </article>

      {/* REASONING BACKENDS */}
      <article className="tac-panel span-4">
        <div className="panel-header">
          <span className="panel-title">SYS.REASONING [MODES]</span>
        </div>
        <div className="tac-list">
          {reasoningModes.map((mode) => (
            <div key={mode.name} className="tac-row healthy">
              <span className="row-primary">{mode.name}</span>
              <span className="row-secondary">DETERMINISTIC</span>
            </div>
          ))}
        </div>
        <br />
        <p>Models are strictly gated. All evaluations must pass the explicit threshold boundaries without hidden prompt drift.</p>
      </article>

      {/* CONTROL EXEC */}
      <article className="tac-panel span-4">
        <div className="panel-header">
          <span className="panel-title">SYS.CMD [QUICK_EXEC]</span>
        </div>
        <div className="tac-list" style={{ gap: '8px' }}>
          {controls.map((item) => (
            <div key={item.title}>
              <span className="d-label" style={{ display: 'block', marginBottom: '2px' }}>&gt; {item.title}</span>
              <pre className="tac-term">{item.command}</pre>
            </div>
          ))}
        </div>
      </article>

      <article className="tac-panel span-12">
        <div className="panel-header">
          <span className="panel-title">SYS.CASES [TRIAGE_QUEUE]</span>
        </div>
        <div className="tac-list">
          {topCases.length > 0 ? (
            topCases.map((entry, index) => (
              <div key={entry.case.id} className="tac-row alert" style={{ flexDirection: "column", alignItems: "flex-start", gap: "4px" }}>
                <span className="row-primary">
                  #{index + 1} {entry.case.title}
                </span>
                <span className="row-secondary">
                  {entry.case.status} | {entry.severity} | score {entry.priority.total}
                </span>
                <span className="row-secondary">
                  {entry.watchlist_name} | latest {entry.latest_signal_at ?? "unknown"}
                </span>
              </div>
            ))
          ) : (
            <div className="tac-row healthy">
              <span className="row-primary">CASE.QUEUE</span>
              <span className="row-secondary">No ranked cases available</span>
            </div>
          )}
        </div>
      </article>

      {/* FEATURED AGENT SUBSTRATE */}
      <article className="tac-panel span-12">
        <div className="panel-header">
          <span className="panel-title">SYS.AGENTS [ROI_KERNELS]</span>
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))', gap: '8px' }}>
          {featuredAgents.map((agent: any) => (
            <div key={agent.name} className={`tac-row ${agent.classOverride || 'healthy'}`} style={{ flexDirection: 'column', alignItems: 'flex-start', borderLeftWidth: '4px' }}>
              <span className="row-primary">{agent.name}</span>
              <span className="row-secondary" style={{ marginTop: '4px' }}>{agent.value}</span>
            </div>
          ))}
        </div>
      </article>

    </section>
  );
}

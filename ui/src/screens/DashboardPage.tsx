import { useEffect, useState } from "react";
import type { CaseQueueEntry, DeterministicAgentSpec } from "../lib/api";
import {
  fetchAgentCatalog,
  fetchAgentCatalogQuality,
  fetchCases,
  fetchIntelOverview,
  fetchMarketIntelOverview,
} from "../lib/api";
import { Panel, StatCard, StatGrid, Badge } from "../components";

const lanes = [
  { name: "Formal Kernel", status: "NOMINAL", note: "FSM transitions + invariants", tone: "ok" },
  { name: "Imperative Shell", status: "NOMINAL", note: "Effects behind execution port", tone: "ok" },
  { name: "Source Registry", status: "NOMINAL", note: "Trust-scored collectors", tone: "ok" },
  { name: "Evidence Pipeline", status: "DEGRADED", note: "High latency detected", tone: "warn" },
  { name: "Case Lifecycle", status: "NOMINAL", note: "Deterministic state flow", tone: "ok" },
  { name: "Market Intel View", status: "NOMINAL", note: "Pricing & launch metrics", tone: "ok" },
  { name: "Onchain Shell", status: "NOMINAL", note: "EVM intent side-effects", tone: "ok" },
  { name: "Autopilot Guard", status: "NOMINAL", note: "LLM fail-closed gate", tone: "ok" },
] as const;

const controls = [
  { title: "Verify Core", command: "./scripts/verify_formal_core.sh" },
  { title: "Verify Agents", command: "./scripts/verify_formal_agents.sh" },
  { title: "Sync Sources", command: "GET /api/v1/sources" },
  { title: "Ingest Evidence", command: "POST /api/v1/evidence/ingest" },
  { title: "Poll Cases", command: "GET /api/v1/cases" },
  { title: "Dry Run Tx", command: "POST /api/v1/onchain/send_raw?dry_run=1" },
];

type AgentTone = "ok" | "danger";

type FeaturedAgent = { name: string; value: string; tone: AgentTone };

const fallbackAgents: FeaturedAgent[] = [
  { name: "Dedup Window", value: "Dup stream suppression", tone: "ok" },
  { name: "Token Bucket", value: "Rate limit admissions", tone: "ok" },
  { name: "Circuit Breaker", value: "Tripped: auth rate exceeded", tone: "danger" },
  { name: "Retry Budget", value: "Bounded loop control", tone: "ok" },
  { name: "Approval Gate", value: "Quorum auth gate", tone: "ok" },
  { name: "Backpressure", value: "Queue shedder", tone: "ok" },
  { name: "SLA Deadline", value: "Track expiry ticks", tone: "ok" },
  { name: "DLQ Budget", value: "Dead-letter router", tone: "ok" },
  { name: "Tx Intent", value: "EVM submission intent", tone: "ok" },
  { name: "Nonce Manager", value: "Collision / replay guard", tone: "ok" },
  { name: "Fee Bidding", value: "Deterministic EVM bump", tone: "ok" },
  { name: "Finality Guard", value: "Reorg / depth gate", tone: "ok" },
  { name: "Allowlist Guard", value: "Contract access wall", tone: "ok" },
];

const reasoningModes = [
  { name: "KRR / Symbolic", note: "Forward chaining closure" },
  { name: "Expert System", note: "Weighted rule voting" },
  { name: "Neural Risk", note: "Linear bounds check" },
  { name: "Neuro-Symbolic", note: "Fused logic gate" },
];

function pickFeaturedAgents(catalog: DeterministicAgentSpec[]): FeaturedAgent[] {
  if (catalog.length >= 10) {
    return catalog.slice(0, 13).map((agent) => ({
      name: agent.name,
      value: agent.roi_rationale,
      tone: "ok" as AgentTone,
    }));
  }
  return fallbackAgents;
}

export function DashboardPage() {
  const [agentClassCount, setAgentClassCount] = useState<number>(fallbackAgents.length);
  const [featuredAgents, setFeaturedAgents] = useState<typeof fallbackAgents>(fallbackAgents);
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
    <section className="hx-page-grid">
      <Panel hero span={12} eyebrow="Overview" title="System Readout">
        <p className="hx-description">
          Helix is a self-hosted intelligence desk with deterministic policy gates,
          replayable evidence, and guarded autopilot. LLMs propose; deterministic
          kernels decide.
        </p>
        <StatGrid>
          <StatCard label="Sources" value={sourceCount} />
          <StatCard label="Evidence" value={evidenceCount} />
          <StatCard label="Watchlists" value={watchlistCount} />
          <StatCard label="Open Cases" value={openCaseCount} tone="accent" />
          <StatCard label="Escalated" value={escalatedCaseCount} tone="danger" />
          <StatCard label="Agent Classes" value={agentClassCount} />
          <StatCard label="Market Tracked" value={trackedCompanyCount} />
          <StatCard label="vs Huginn" value={`${superiorityRatio}x`} tone="info" sublabel={`${huginnBaseline} baseline`} />
        </StatGrid>
      </Panel>

      <Panel span={4} eyebrow="Runtime" title="System Lanes">
        <div className="hx-list">
          {lanes.map((lane) => (
            <div key={lane.name} className={`hx-row hx-row--${lane.tone}`}>
              <div className="hx-row-stack">
                <span className="hx-row-primary">{lane.name}</span>
                <span className="hx-row-secondary">{lane.note}</span>
              </div>
              <Badge tone={lane.tone === "warn" ? "warn" : "ok"}>{lane.status}</Badge>
            </div>
          ))}
        </div>
      </Panel>

      <Panel span={4} eyebrow="Backends" title="Reasoning Modes">
        <div className="hx-list">
          {reasoningModes.map((mode) => (
            <div key={mode.name} className="hx-row hx-row--ok">
              <div className="hx-row-stack">
                <span className="hx-row-primary">{mode.name}</span>
                <span className="hx-row-secondary">{mode.note}</span>
              </div>
              <Badge tone="info">DETERMINISTIC</Badge>
            </div>
          ))}
        </div>
        <p className="hx-description" style={{ marginTop: "var(--hx-space-3)" }}>
          Models are strictly gated. All evaluations must pass explicit threshold
          boundaries without hidden prompt drift.
        </p>
      </Panel>

      <Panel span={4} eyebrow="Quick Exec" title="Commands">
        <div className="hx-list">
          {controls.map((item) => (
            <div key={item.title} className="hx-row">
              <div className="hx-row-stack">
                <span className="hx-row-primary">{item.title}</span>
                <code className="hx-mono-detail">{item.command}</code>
              </div>
            </div>
          ))}
        </div>
      </Panel>

      <Panel span={12} eyebrow="Triage" title="Case Queue" actions={<Badge tone="accent">{topCases.length} ranked</Badge>}>
        {topCases.length > 0 ? (
          <div className="hx-card-grid">
            {topCases.map((entry, index) => (
              <div key={entry.case.id} className="hx-card">
                <div className="hx-card-head">
                  <h3>#{index + 1} {entry.case.title}</h3>
                  <Badge tone={entry.severity === "critical" || entry.severity === "high" ? "danger" : entry.severity === "medium" ? "warn" : "ok"}>
                    {entry.severity}
                  </Badge>
                </div>
                <div className="hx-card-body">
                  <p className="hx-row-secondary">{entry.case.status} · score {entry.priority.total}</p>
                  <p className="hx-mono-detail">{entry.watchlist_name}</p>
                  <p className="hx-mono-detail">latest: {entry.latest_signal_at ?? "unknown"}</p>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="hx-table-empty">
            <p>No ranked cases available. The case queue is empty or the API is unreachable.</p>
          </div>
        )}
      </Panel>

      <Panel span={12} eyebrow="Substrate" title="Agent ROI Kernels" actions={<Badge tone="info">{agentClassCount} classes</Badge>}>
        <div className="hx-card-grid">
          {featuredAgents.map((agent) => (
            <div key={agent.name} className={`hx-card${agent.tone === "danger" ? " hx-row--danger" : ""}`} style={{ borderLeftWidth: "3px" }}>
              <div className="hx-card-head">
                <h3>{agent.name}</h3>
                {agent.tone === "danger" && <Badge tone="danger">TRIPPED</Badge>}
              </div>
              <p className="hx-row-secondary">{agent.value}</p>
            </div>
          ))}
        </div>
      </Panel>
    </section>
  );
}

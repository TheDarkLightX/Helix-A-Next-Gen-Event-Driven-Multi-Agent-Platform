import { useEffect, useState } from "react";
import {
  CaseCommand,
  CaseFile,
  CaseStatus,
  IntelDeskOverviewResponse,
  fetchCases,
  fetchIntelOverview,
  transitionCase,
} from "../lib/api";

function statusClass(status: CaseStatus) {
  return status === "escalated" || status === "closed" ? "warn" : "ok";
}

export function CasesPage() {
  const [overview, setOverview] = useState<IntelDeskOverviewResponse | null>(null);
  const [cases, setCases] = useState<CaseFile[]>([]);
  const [status, setStatus] = useState("Loading cases...");

  async function loadCases(message?: string) {
    try {
      const [overviewData, caseCatalog] = await Promise.all([fetchIntelOverview(), fetchCases()]);
      setOverview(overviewData);
      setCases(caseCatalog);
      setStatus(message ?? `Loaded ${caseCatalog.length} cases.`);
    } catch (error) {
      setStatus(`Failed to load cases: ${(error as Error).message}`);
    }
  }

  useEffect(() => {
    void loadCases();
  }, []);

  async function applyTransition(caseId: string, command: CaseCommand, label: string) {
    setStatus(`${label} ${caseId}...`);
    try {
      const transition = await transitionCase(caseId, command);
      await loadCases(`Case ${transition.case.id} -> ${transition.case.status}.`);
    } catch (error) {
      setStatus(`Case transition failed: ${(error as Error).message}`);
    }
  }

  return (
    <section className="dashboard-grid">
      <article className="panel panel-hero panel-span-12">
        <p className="mono-label">Cases</p>
        <h2>Dossiers and Escalations</h2>
        <p>
          Cases are deterministic dossiers created by watchlist hits. Operators can move them
          through monitoring, brief-ready, escalated, closed, and reopened states without bypassing
          the lifecycle kernel.
        </p>
      </article>

      <article className="panel panel-span-12">
        <p className="mono-label">Case Metrics</p>
        <div className="metrics-grid">
          <div className="metric-card">
            <p className="metric-label">Sources</p>
            <p className="metric-value">{overview?.source_count ?? 0}</p>
          </div>
          <div className="metric-card">
            <p className="metric-label">Watchlists</p>
            <p className="metric-value">{overview?.watchlist_count ?? 0}</p>
          </div>
          <div className="metric-card">
            <p className="metric-label">Evidence</p>
            <p className="metric-value">{overview?.evidence_count ?? 0}</p>
          </div>
          <div className="metric-card">
            <p className="metric-label">Claims</p>
            <p className="metric-value">{overview?.claim_count ?? 0}</p>
          </div>
          <div className="metric-card">
            <p className="metric-label">Open Cases</p>
            <p className="metric-value">{overview?.open_case_count ?? 0}</p>
          </div>
          <div className="metric-card">
            <p className="metric-label">Escalated</p>
            <p className="metric-value">{overview?.escalated_case_count ?? 0}</p>
          </div>
        </div>
        <p className="status-line">{status}</p>
      </article>

      <article className="panel panel-span-12">
        <p className="mono-label">Case Files</p>
        <div className="agent-grid">
          {cases.map((item) => (
            <div key={item.id} className="agent-card">
              <div className="agent-card-head">
                <h3>{item.title}</h3>
                <span className={`status-pill ${statusClass(item.status)}`}>{item.status}</span>
              </div>
              <p>{item.latest_reason}</p>
              <p className="mono-detail">{item.id}</p>
              <div className="command-stack">
                <code className="command-inline">watchlist_id: {item.watchlist_id}</code>
                <code className="command-inline">primary_entity: {item.primary_entity ?? "none"}</code>
                <code className="command-inline">evidence_ids: {item.evidence_ids.join(", ")}</code>
                <code className="command-inline">claim_ids: {item.claim_ids.join(", ")}</code>
                <code className="command-inline">
                  briefing_summary: {item.briefing_summary ?? "not attached"}
                </code>
              </div>
              <div className="button-row">
                <button
                  className="btn-secondary"
                  onClick={() =>
                    void applyTransition(item.id, { type: "mark_monitoring" }, "Marking")
                  }
                >
                  Monitoring
                </button>
                <button
                  className="btn-secondary"
                  onClick={() =>
                    void applyTransition(
                      item.id,
                      { type: "attach_brief", summary: `Analyst briefing attached for ${item.title}.` },
                      "Attaching brief to"
                    )
                  }
                >
                  Attach Brief
                </button>
                <button
                  className="btn-secondary"
                  onClick={() =>
                    void applyTransition(
                      item.id,
                      { type: "escalate", reason: `Escalated by operator for ${item.title}.` },
                      "Escalating"
                    )
                  }
                >
                  Escalate
                </button>
                <button
                  className="btn-secondary"
                  onClick={() => void applyTransition(item.id, { type: "close" }, "Closing")}
                >
                  Close
                </button>
                <button
                  className="btn-secondary"
                  onClick={() =>
                    void applyTransition(
                      item.id,
                      { type: "reopen", reason: `Reopened by operator for ${item.title}.` },
                      "Reopening"
                    )
                  }
                >
                  Reopen
                </button>
              </div>
            </div>
          ))}
        </div>
      </article>
    </section>
  );
}

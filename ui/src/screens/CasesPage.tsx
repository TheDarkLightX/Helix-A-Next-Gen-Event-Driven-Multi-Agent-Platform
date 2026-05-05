import { useEffect, useState } from "react";
import {
  CaseCommand,
  CaseQueueFilters,
  CaseQueueEntry,
  CaseStatus,
  IntelDeskOverviewResponse,
  PriorityBreakdown,
  WatchlistSeverity,
  fetchCases,
  fetchIntelOverview,
  transitionCase,
} from "../lib/api";
import {
  SavedCaseView,
  clearCasesWorkspaceState,
  loadCasesWorkspaceState,
  makeSavedViewId,
  saveCasesWorkspaceState,
} from "../lib/intelDeskWorkspace";

const DEFAULT_CASE_LIMIT = "25";
type CaseStatusSelection = CaseStatus | "all";
type CaseSeveritySelection = WatchlistSeverity | "all";

function statusClass(status: CaseStatus) {
  if (status === "escalated") return "danger";
  if (status === "closed") return "warn";
  return "ok";
}

function severityClass(severity: WatchlistSeverity) {
  if (severity === "critical") return "danger";
  if (severity === "high") return "warn";
  return "info";
}

function priorityLabel(priority: PriorityBreakdown) {
  return `score ${priority.total} | a${priority.attention_tier} s${priority.severity_tier} c${priority.corroboration_tier} cred=${priority.credibility_bps}`;
}

function describeFilters(filters: CaseQueueFilters) {
  const labels = [
    filters.status ? `status=${filters.status}` : null,
    filters.severity ? `severity=${filters.severity}` : null,
    filters.watchlist_id ? `watchlist=${filters.watchlist_id}` : null,
    filters.primary_entity ? `entity=${filters.primary_entity}` : null,
    filters.limit ? `limit=${filters.limit}` : null,
  ].filter(Boolean);
  return labels.length > 0 ? labels.join(" | ") : "unfiltered";
}

function caseStatusSelection(filters: CaseQueueFilters): CaseStatusSelection {
  return filters.status ?? "all";
}

function caseSeveritySelection(filters: CaseQueueFilters): CaseSeveritySelection {
  return filters.severity ?? "all";
}

function caseLimitSelection(filters: CaseQueueFilters): string {
  return filters.limit !== undefined ? String(filters.limit) : "all";
}

export function CasesPage() {
  const [initialWorkspace] = useState(() => loadCasesWorkspaceState({ limit: Number(DEFAULT_CASE_LIMIT) }));
  const [overview, setOverview] = useState<IntelDeskOverviewResponse | null>(null);
  const [cases, setCases] = useState<CaseQueueEntry[]>([]);
  const [status, setStatus] = useState("Loading cases...");
  const [statusFilter, setStatusFilter] = useState<CaseStatusSelection>(
    caseStatusSelection(initialWorkspace.filters)
  );
  const [severityFilter, setSeverityFilter] = useState<CaseSeveritySelection>(
    caseSeveritySelection(initialWorkspace.filters)
  );
  const [watchlistFilter, setWatchlistFilter] = useState(initialWorkspace.filters.watchlist_id ?? "");
  const [entityFilter, setEntityFilter] = useState(initialWorkspace.filters.primary_entity ?? "");
  const [limitFilter, setLimitFilter] = useState(caseLimitSelection(initialWorkspace.filters));
  const [savedViews, setSavedViews] = useState<SavedCaseView[]>(initialWorkspace.savedViews);
  const [activeViewId, setActiveViewId] = useState<string | null>(initialWorkspace.activeViewId);
  const [savedViewName, setSavedViewName] = useState("");

  function currentFilters(): CaseQueueFilters {
    return {
      status: statusFilter === "all" ? undefined : statusFilter,
      severity: severityFilter === "all" ? undefined : severityFilter,
      watchlist_id: watchlistFilter.trim() || undefined,
      primary_entity: entityFilter.trim() || undefined,
      limit: limitFilter === "all" ? undefined : Number(limitFilter),
    };
  }

  useEffect(() => {
    saveCasesWorkspaceState({
      filters: currentFilters(),
      savedViews,
      activeViewId,
    });
  }, [statusFilter, severityFilter, watchlistFilter, entityFilter, limitFilter, savedViews, activeViewId]);

  async function loadCases(message?: string, filters: CaseQueueFilters = currentFilters()) {
    try {
      const [overviewData, caseCatalog] = await Promise.all([
        fetchIntelOverview(),
        fetchCases(filters),
      ]);
      setOverview(overviewData);
      setCases(caseCatalog);
      setStatus(
        message ?? `Loaded ${caseCatalog.length} cases (${describeFilters(filters)}).`
      );
    } catch (error) {
      setStatus(`Failed to load cases: ${(error as Error).message}`);
    }
  }

  useEffect(() => {
    void loadCases(undefined, initialWorkspace.filters);
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

  function applyFilters() {
    void loadCases(undefined, currentFilters());
  }

  function resetFilters() {
    setStatusFilter("all");
    setSeverityFilter("all");
    setWatchlistFilter("");
    setEntityFilter("");
    setLimitFilter(DEFAULT_CASE_LIMIT);
    void loadCases("Reset case queue filters.", { limit: Number(DEFAULT_CASE_LIMIT) });
  }

  function saveCurrentView() {
    const name = savedViewName.trim();
    if (!name) {
      setStatus("Saved view name is required.");
      return;
    }

    const view: SavedCaseView = {
      id: makeSavedViewId(name),
      name,
      filters: currentFilters(),
    };

    setSavedViews((existing) => {
      const next = existing.filter((item) => item.id !== view.id);
      return [view, ...next].slice(0, 8);
    });
    setActiveViewId(view.id);
    setSavedViewName("");
    setStatus(`Saved case view '${name}'.`);
  }

  function applySavedView(view: SavedCaseView) {
    setActiveViewId(view.id);
    setStatusFilter(caseStatusSelection(view.filters));
    setSeverityFilter(caseSeveritySelection(view.filters));
    setWatchlistFilter(view.filters.watchlist_id ?? "");
    setEntityFilter(view.filters.primary_entity ?? "");
    setLimitFilter(caseLimitSelection(view.filters));
    void loadCases(`Applied saved view '${view.name}'.`, view.filters);
  }

  function deleteSavedView(view: SavedCaseView) {
    setSavedViews((existing) => existing.filter((item) => item.id !== view.id));
    if (activeViewId === view.id) setActiveViewId(null);
    setStatus(`Deleted saved view '${view.name}'.`);
  }

  function clearWorkspace() {
    clearCasesWorkspaceState();
    setStatusFilter("all");
    setSeverityFilter("all");
    setWatchlistFilter("");
    setEntityFilter("");
    setLimitFilter(DEFAULT_CASE_LIMIT);
    setSavedViews([]);
    setActiveViewId(null);
    setSavedViewName("");
    void loadCases("Cleared local case workspace state.", { limit: Number(DEFAULT_CASE_LIMIT) });
  }

  return (
    <section className="dashboard-grid">
      <article className="panel panel-hero panel-span-12">
        <p className="mono-label">Cases</p>
        <h2>Dossiers and Escalations</h2>
        <p>
          Cases are deterministic dossiers created by watchlist hits. Operators can move them
          through monitoring, brief-ready, escalated, closed, and reopened states without bypassing
          the lifecycle kernel. Queue order is now a deterministic mixed-radix priority, not
          incidental insertion order.
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
        <p className="mono-label">Queue Discipline</p>
        <div className="pill-row">
          <span className="info-pill">priority = attention &gt; severity &gt; corroboration &gt; freshness &gt; trust &gt; density</span>
          <span className="info-pill">ties break on latest signal, then case id</span>
          <span className="info-pill">top case: {cases[0]?.case.id ?? "none"}</span>
        </div>
      </article>

      <article className="panel panel-span-12">
        <p className="mono-label">Queue Filters</p>
        <div className="form-grid">
          <label className="field">
            <span>Status</span>
            <select
              value={statusFilter}
              onChange={(event) => setStatusFilter(event.target.value as CaseStatus | "all")}
            >
              <option value="all">all</option>
              <option value="open">open</option>
              <option value="monitoring">monitoring</option>
              <option value="brief_ready">brief_ready</option>
              <option value="escalated">escalated</option>
              <option value="closed">closed</option>
            </select>
          </label>
          <label className="field">
            <span>Severity</span>
            <select
              value={severityFilter}
              onChange={(event) =>
                setSeverityFilter(event.target.value as WatchlistSeverity | "all")
              }
            >
              <option value="all">all</option>
              <option value="critical">critical</option>
              <option value="high">high</option>
              <option value="medium">medium</option>
              <option value="low">low</option>
            </select>
          </label>
          <label className="field">
            <span>Watchlist Id</span>
            <input
              value={watchlistFilter}
              onChange={(event) => setWatchlistFilter(event.target.value)}
              placeholder="watch_pricing_competitors"
            />
          </label>
          <label className="field">
            <span>Primary Entity</span>
            <input
              value={entityFilter}
              onChange={(event) => setEntityFilter(event.target.value)}
              placeholder="orion dynamics"
            />
          </label>
          <label className="field">
            <span>Limit</span>
            <select
              value={limitFilter}
              onChange={(event) => setLimitFilter(event.target.value)}
            >
              <option value="10">10</option>
              <option value="25">25</option>
              <option value="50">50</option>
              <option value="100">100</option>
              <option value="all">all</option>
            </select>
          </label>
        </div>
        <div className="button-row">
          <button className="btn-secondary" type="button" onClick={applyFilters}>
            Apply Filters
          </button>
          <button className="btn-secondary" type="button" onClick={resetFilters}>
            Reset
          </button>
        </div>
      </article>

      <article className="panel panel-span-12">
        <p className="mono-label">Saved Views</p>
        <div className="form-grid">
          <label className="field field-full">
            <span>view_name</span>
            <input
              value={savedViewName}
              onChange={(event) => setSavedViewName(event.target.value)}
              placeholder="critical-watchlists"
            />
          </label>
        </div>
        <div className="button-row">
          <button className="btn-secondary" type="button" onClick={saveCurrentView}>
            Save Current View
          </button>
          <button className="btn-secondary" type="button" onClick={clearWorkspace}>
            Clear Local Workspace
          </button>
        </div>
        {savedViews.length === 0 ? (
          <p className="panel-note">No saved case views yet.</p>
        ) : (
          <div className="command-stack">
            {savedViews.map((view) => (
              <div key={view.id} className="command-row">
                <div className="agent-card-head">
                  <h3>{view.name}</h3>
                  <span className={`status-pill ${activeViewId === view.id ? "ok" : "info"}`}>
                    {activeViewId === view.id ? "active" : "saved"}
                  </span>
                </div>
                <code>{describeFilters(view.filters)}</code>
                <div className="button-row">
                  <button
                    className="btn-secondary"
                    type="button"
                    onClick={() => applySavedView(view)}
                  >
                    Apply
                  </button>
                  <button
                    className="btn-secondary"
                    type="button"
                    onClick={() => deleteSavedView(view)}
                  >
                    Delete
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </article>

      <article className="panel panel-span-12">
        <p className="mono-label">Case Files</p>
        <div className="agent-grid">
          {cases.map((entry, index) => (
            <div key={entry.case.id} className="agent-card">
              <div className="agent-card-head">
                <h3>
                  #{index + 1} {entry.case.title}
                </h3>
                <span className={`status-pill ${statusClass(entry.case.status)}`}>{entry.case.status}</span>
              </div>
              <p>{entry.case.latest_reason}</p>
              <div className="pill-row">
                <span className="info-pill">{priorityLabel(entry.priority)}</span>
                <span className={`status-pill ${severityClass(entry.severity)}`}>{entry.severity}</span>
                <span className="info-pill">{entry.watchlist_name}</span>
                <span className="info-pill">latest: {entry.latest_signal_at ?? "unknown"}</span>
              </div>
              <p className="mono-detail">{entry.case.id}</p>
              <div className="command-stack">
                <code className="command-inline">watchlist_id: {entry.case.watchlist_id}</code>
                <code className="command-inline">primary_entity: {entry.case.primary_entity ?? "none"}</code>
                <code className="command-inline">evidence_ids: {entry.case.evidence_ids.join(", ")}</code>
                <code className="command-inline">claim_ids: {entry.case.claim_ids.join(", ")}</code>
                <code className="command-inline">
                  briefing_summary: {entry.case.briefing_summary ?? "not attached"}
                </code>
              </div>
              <div className="button-row">
                <button
                  className="btn-secondary"
                  onClick={() =>
                    void applyTransition(entry.case.id, { type: "mark_monitoring" }, "Marking")
                  }
                >
                  Monitoring
                </button>
                <button
                  className="btn-secondary"
                  onClick={() =>
                    void applyTransition(
                      entry.case.id,
                      { type: "attach_brief", summary: `Analyst briefing attached for ${entry.case.title}.` },
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
                      entry.case.id,
                      { type: "escalate", reason: `Escalated by operator for ${entry.case.title}.` },
                      "Escalating"
                    )
                  }
                >
                  Escalate
                </button>
                <button
                  className="btn-secondary"
                  onClick={() => void applyTransition(entry.case.id, { type: "close" }, "Closing")}
                >
                  Close
                </button>
                <button
                  className="btn-secondary"
                  onClick={() =>
                    void applyTransition(
                      entry.case.id,
                      { type: "reopen", reason: `Reopened by operator for ${entry.case.title}.` },
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

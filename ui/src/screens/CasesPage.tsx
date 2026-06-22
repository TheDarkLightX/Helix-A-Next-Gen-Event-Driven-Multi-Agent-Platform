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
import {
  Badge,
  Button,
  FormField,
  FormGrid,
  Input,
  Panel,
  Select,
  StatCard,
  StatGrid,
  StatusLine,
  Tag,
} from "../components";

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
    <section className="hx-page-grid">
      <Panel hero span={12} eyebrow="Cases" title="Dossiers and Escalations">
        <p className="hx-description">
          Cases are deterministic dossiers created by watchlist hits. Operators can move them
          through monitoring, brief-ready, escalated, closed, and reopened states without bypassing
          the lifecycle kernel. Queue order is now a deterministic mixed-radix priority, not
          incidental insertion order.
        </p>
      </Panel>

      <Panel span={12} eyebrow="Case Metrics">
        <StatGrid>
          <StatCard label="Sources" value={overview?.source_count ?? 0} />
          <StatCard label="Watchlists" value={overview?.watchlist_count ?? 0} />
          <StatCard label="Evidence" value={overview?.evidence_count ?? 0} />
          <StatCard label="Claims" value={overview?.claim_count ?? 0} />
          <StatCard label="Open Cases" value={overview?.open_case_count ?? 0} tone="info" />
          <StatCard label="Escalated" value={overview?.escalated_case_count ?? 0} tone="danger" />
        </StatGrid>
        <StatusLine>{status}</StatusLine>
      </Panel>

      <Panel span={12} eyebrow="Queue Discipline">
        <div className="hx-tag-row">
          <Tag>priority = attention &gt; severity &gt; corroboration &gt; freshness &gt; trust &gt; density</Tag>
          <Tag>ties break on latest signal, then case id</Tag>
          <Tag>top case: {cases[0]?.case.id ?? "none"}</Tag>
        </div>
      </Panel>

      <Panel span={12} eyebrow="Queue Filters">
        <FormGrid>
          <FormField label="Status">
            <Select
              value={statusFilter}
              onChange={(event) => setStatusFilter(event.target.value as CaseStatus | "all")}
            >
              <option value="all">all</option>
              <option value="open">open</option>
              <option value="monitoring">monitoring</option>
              <option value="brief_ready">brief_ready</option>
              <option value="escalated">escalated</option>
              <option value="closed">closed</option>
            </Select>
          </FormField>
          <FormField label="Severity">
            <Select
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
            </Select>
          </FormField>
          <FormField label="Watchlist Id">
            <Input
              value={watchlistFilter}
              onChange={(event) => setWatchlistFilter(event.target.value)}
              placeholder="watch_pricing_competitors"
            />
          </FormField>
          <FormField label="Primary Entity">
            <Input
              value={entityFilter}
              onChange={(event) => setEntityFilter(event.target.value)}
              placeholder="orion dynamics"
            />
          </FormField>
          <FormField label="Limit">
            <Select
              value={limitFilter}
              onChange={(event) => setLimitFilter(event.target.value)}
            >
              <option value="10">10</option>
              <option value="25">25</option>
              <option value="50">50</option>
              <option value="100">100</option>
              <option value="all">all</option>
            </Select>
          </FormField>
        </FormGrid>
        <div className="hx-cluster">
          <Button variant="secondary" type="button" onClick={applyFilters}>
            Apply Filters
          </Button>
          <Button variant="secondary" type="button" onClick={resetFilters}>
            Reset
          </Button>
        </div>
      </Panel>

      <Panel span={12} eyebrow="Saved Views">
        <FormGrid>
          <FormField label="view_name" full>
            <Input
              value={savedViewName}
              onChange={(event) => setSavedViewName(event.target.value)}
              placeholder="critical-watchlists"
            />
          </FormField>
        </FormGrid>
        <div className="hx-cluster">
          <Button variant="secondary" type="button" onClick={saveCurrentView}>
            Save Current View
          </Button>
          <Button variant="secondary" type="button" onClick={clearWorkspace}>
            Clear Local Workspace
          </Button>
        </div>
        {savedViews.length === 0 ? (
          <p className="hx-table-empty">No saved case views yet.</p>
        ) : (
          <div className="hx-list">
            {savedViews.map((view) => (
              <div key={view.id} className="hx-row">
                <div className="hx-card-head">
                  <h3>{view.name}</h3>
                  <Badge tone={activeViewId === view.id ? "ok" : "info"}>
                    {activeViewId === view.id ? "active" : "saved"}
                  </Badge>
                </div>
                <code className="hx-mono-detail">{describeFilters(view.filters)}</code>
                <div className="hx-cluster">
                  <Button
                    variant="secondary"
                    type="button"
                    onClick={() => applySavedView(view)}
                  >
                    Apply
                  </Button>
                  <Button
                    variant="secondary"
                    type="button"
                    onClick={() => deleteSavedView(view)}
                  >
                    Delete
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}
      </Panel>

      <Panel span={12} eyebrow="Case Files">
        <div className="hx-card-grid">
          {cases.map((entry, index) => (
            <div key={entry.case.id} className="hx-card">
              <div className="hx-card-head">
                <h3>
                  #{index + 1} {entry.case.title}
                </h3>
                <Badge tone={statusClass(entry.case.status) as "ok" | "warn" | "danger"}>
                  {entry.case.status}
                </Badge>
              </div>
              <p className="hx-description">{entry.case.latest_reason}</p>
              <div className="hx-tag-row">
                <Tag>{priorityLabel(entry.priority)}</Tag>
                <Badge tone={severityClass(entry.severity) as "ok" | "warn" | "danger" | "info"}>
                  {entry.severity}
                </Badge>
                <Tag>{entry.watchlist_name}</Tag>
                <Tag>latest: {entry.latest_signal_at ?? "unknown"}</Tag>
              </div>
              <p className="hx-mono-detail">{entry.case.id}</p>
              <div className="hx-stack">
                <code className="hx-mono-detail">watchlist_id: {entry.case.watchlist_id}</code>
                <code className="hx-mono-detail">primary_entity: {entry.case.primary_entity ?? "none"}</code>
                <code className="hx-mono-detail">evidence_ids: {entry.case.evidence_ids.join(", ")}</code>
                <code className="hx-mono-detail">claim_ids: {entry.case.claim_ids.join(", ")}</code>
                <code className="hx-mono-detail">
                  briefing_summary: {entry.case.briefing_summary ?? "not attached"}
                </code>
              </div>
              <div className="hx-cluster">
                <Button
                  variant="secondary"
                  onClick={() =>
                    void applyTransition(entry.case.id, { type: "mark_monitoring" }, "Marking")
                  }
                >
                  Monitoring
                </Button>
                <Button
                  variant="secondary"
                  onClick={() =>
                    void applyTransition(
                      entry.case.id,
                      { type: "attach_brief", summary: `Analyst briefing attached for ${entry.case.title}.` },
                      "Attaching brief to"
                    )
                  }
                >
                  Attach Brief
                </Button>
                <Button
                  variant="secondary"
                  onClick={() =>
                    void applyTransition(
                      entry.case.id,
                      { type: "escalate", reason: `Escalated by operator for ${entry.case.title}.` },
                      "Escalating"
                    )
                  }
                >
                  Escalate
                </Button>
                <Button
                  variant="secondary"
                  onClick={() => void applyTransition(entry.case.id, { type: "close" }, "Closing")}
                >
                  Close
                </Button>
                <Button
                  variant="secondary"
                  onClick={() =>
                    void applyTransition(
                      entry.case.id,
                      { type: "reopen", reason: `Reopened by operator for ${entry.case.title}.` },
                      "Reopening"
                    )
                  }
                >
                  Reopen
                </Button>
              </div>
            </div>
          ))}
        </div>
      </Panel>
    </section>
  );
}

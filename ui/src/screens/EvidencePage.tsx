import { FormEvent, useEffect, useState } from "react";
import {
  CaseStatus,
  ClaimQueueEntry,
  ClaimQueueFilters,
  ClaimReviewStatus,
  EvidenceQueueEntry,
  EvidenceQueueFilters,
  IngestEvidenceResponse,
  PriorityBreakdown,
  ProposedClaim,
  SourceDefinition,
  WatchlistSeverity,
  fetchClaims,
  fetchEvidence,
  fetchSources,
  ingestEvidence,
  reviewClaim,
} from "../lib/api";
import {
  SavedEvidenceDeskView,
  clearEvidenceDeskWorkspaceState,
  loadEvidenceDeskWorkspaceState,
  makeSavedViewId,
  saveEvidenceDeskWorkspaceState,
} from "../lib/intelDeskWorkspace";

const DEFAULT_CLAIMS: ProposedClaim[] = [
  {
    subject: "alice north",
    predicate: "resigned_from",
    object: "orion dynamics",
    confidence_bps: 9100,
    rationale: "explicitly stated in the source",
  },
];

function parseCsv(value: string): string[] {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function reviewStatusClass(status: ClaimReviewStatus) {
  if (status === "corroborated") return "ok";
  if (status === "rejected") return "danger";
  return "warn";
}

function severityClass(severity: WatchlistSeverity | null) {
  if (severity === "critical") return "danger";
  if (severity === "high") return "warn";
  if (severity === "medium") return "info";
  return "ok";
}

function priorityLabel(priority: PriorityBreakdown) {
  return `score ${priority.total} | a${priority.attention_tier} s${priority.severity_tier} c${priority.corroboration_tier} cred=${priority.credibility_bps}`;
}

function evidenceFilterSummary(filters: EvidenceQueueFilters) {
  const labels = [
    filters.source_id ? `source=${filters.source_id}` : null,
    filters.tag ? `tag=${filters.tag}` : null,
    filters.entity ? `entity=${filters.entity}` : null,
    filters.linked_status ? `linked_status=${filters.linked_status}` : null,
    filters.min_trust !== undefined ? `min_trust=${filters.min_trust}` : null,
    filters.q ? `q=${filters.q}` : null,
    filters.limit ? `limit=${filters.limit}` : null,
  ].filter(Boolean);
  return labels.length > 0 ? labels.join(" | ") : "unfiltered";
}

function claimFilterSummary(filters: ClaimQueueFilters) {
  const labels = [
    filters.review_status ? `review_status=${filters.review_status}` : null,
    filters.subject ? `subject=${filters.subject}` : null,
    filters.predicate ? `predicate=${filters.predicate}` : null,
    filters.linked_status ? `linked_status=${filters.linked_status}` : null,
    filters.min_confidence_bps !== undefined
      ? `min_confidence=${filters.min_confidence_bps}`
      : null,
    filters.q ? `q=${filters.q}` : null,
    filters.limit ? `limit=${filters.limit}` : null,
  ].filter(Boolean);
  return labels.length > 0 ? labels.join(" | ") : "unfiltered";
}

const DEFAULT_DESK_LIMIT = "6";
type LinkedStatusSelection = CaseStatus | "all";
type ClaimReviewSelection = ClaimReviewStatus | "all";

function linkedStatusSelection(status?: CaseStatus): LinkedStatusSelection {
  return status ?? "all";
}

function claimReviewSelection(status?: ClaimReviewStatus): ClaimReviewSelection {
  return status ?? "all";
}

function deskLimitSelection(limit?: number): string {
  return limit !== undefined ? String(limit) : "all";
}

export function EvidencePage() {
  const [initialWorkspace] = useState(() =>
    loadEvidenceDeskWorkspaceState({
      evidenceFilters: { limit: Number(DEFAULT_DESK_LIMIT) },
      claimFilters: { limit: Number(DEFAULT_DESK_LIMIT) },
    })
  );
  const [sources, setSources] = useState<SourceDefinition[]>([]);
  const [evidence, setEvidence] = useState<EvidenceQueueEntry[]>([]);
  const [claims, setClaims] = useState<ClaimQueueEntry[]>([]);
  const [status, setStatus] = useState<string>("Loading evidence desk...");
  const [sourceId, setSourceId] = useState<string>("");
  const [title, setTitle] = useState<string>("Alice North resigned from Orion Dynamics");
  const [summary, setSummary] = useState<string>("Leadership movement at Orion Dynamics.");
  const [content, setContent] = useState<string>(
    "Alice North resigned after a short detention, according to the report."
  );
  const [url, setUrl] = useState<string>("https://example.org/report");
  const [observedAt, setObservedAt] = useState<string>("2026-03-06T12:00:00Z");
  const [tagsText, setTagsText] = useState<string>("leadership, security");
  const [entitiesText, setEntitiesText] = useState<string>("alice north, orion dynamics");
  const [claimsText, setClaimsText] = useState<string>(JSON.stringify(DEFAULT_CLAIMS, null, 2));
  const [lastResult, setLastResult] = useState<IngestEvidenceResponse | null>(null);
  const [evidenceSourceFilter, setEvidenceSourceFilter] = useState<string>(
    initialWorkspace.evidenceFilters.source_id ?? ""
  );
  const [evidenceTagFilter, setEvidenceTagFilter] = useState<string>(
    initialWorkspace.evidenceFilters.tag ?? ""
  );
  const [evidenceEntityFilter, setEvidenceEntityFilter] = useState<string>(
    initialWorkspace.evidenceFilters.entity ?? ""
  );
  const [evidenceLinkedStatusFilter, setEvidenceLinkedStatusFilter] =
    useState<LinkedStatusSelection>(
      linkedStatusSelection(initialWorkspace.evidenceFilters.linked_status)
    );
  const [evidenceMinTrustFilter, setEvidenceMinTrustFilter] = useState<string>(
    initialWorkspace.evidenceFilters.min_trust !== undefined
      ? String(initialWorkspace.evidenceFilters.min_trust)
      : ""
  );
  const [evidenceQueryFilter, setEvidenceQueryFilter] = useState<string>(
    initialWorkspace.evidenceFilters.q ?? ""
  );
  const [evidenceLimitFilter, setEvidenceLimitFilter] = useState<string>(
    deskLimitSelection(initialWorkspace.evidenceFilters.limit)
  );
  const [claimReviewFilter, setClaimReviewFilter] =
    useState<ClaimReviewSelection>(
      claimReviewSelection(initialWorkspace.claimFilters.review_status)
    );
  const [claimSubjectFilter, setClaimSubjectFilter] = useState<string>(
    initialWorkspace.claimFilters.subject ?? ""
  );
  const [claimPredicateFilter, setClaimPredicateFilter] = useState<string>(
    initialWorkspace.claimFilters.predicate ?? ""
  );
  const [claimLinkedStatusFilter, setClaimLinkedStatusFilter] =
    useState<LinkedStatusSelection>(
      linkedStatusSelection(initialWorkspace.claimFilters.linked_status)
    );
  const [claimMinConfidenceFilter, setClaimMinConfidenceFilter] = useState<string>(
    initialWorkspace.claimFilters.min_confidence_bps !== undefined
      ? String(initialWorkspace.claimFilters.min_confidence_bps)
      : ""
  );
  const [claimQueryFilter, setClaimQueryFilter] = useState<string>(
    initialWorkspace.claimFilters.q ?? ""
  );
  const [claimLimitFilter, setClaimLimitFilter] = useState<string>(
    deskLimitSelection(initialWorkspace.claimFilters.limit)
  );
  const [savedViews, setSavedViews] = useState<SavedEvidenceDeskView[]>(
    initialWorkspace.savedViews
  );
  const [activeViewId, setActiveViewId] = useState<string | null>(initialWorkspace.activeViewId);
  const [savedViewName, setSavedViewName] = useState("");

  function currentEvidenceFilters(): EvidenceQueueFilters {
    return {
      source_id: evidenceSourceFilter.trim() || undefined,
      tag: evidenceTagFilter.trim() || undefined,
      entity: evidenceEntityFilter.trim() || undefined,
      linked_status:
        evidenceLinkedStatusFilter === "all" ? undefined : evidenceLinkedStatusFilter,
      min_trust: evidenceMinTrustFilter.trim()
        ? Number(evidenceMinTrustFilter)
        : undefined,
      q: evidenceQueryFilter.trim() || undefined,
      limit:
        evidenceLimitFilter === "all" ? undefined : Number(evidenceLimitFilter),
    };
  }

  function currentClaimFilters(): ClaimQueueFilters {
    return {
      review_status: claimReviewFilter === "all" ? undefined : claimReviewFilter,
      subject: claimSubjectFilter.trim() || undefined,
      predicate: claimPredicateFilter.trim() || undefined,
      linked_status:
        claimLinkedStatusFilter === "all" ? undefined : claimLinkedStatusFilter,
      min_confidence_bps: claimMinConfidenceFilter.trim()
        ? Number(claimMinConfidenceFilter)
        : undefined,
      q: claimQueryFilter.trim() || undefined,
      limit: claimLimitFilter === "all" ? undefined : Number(claimLimitFilter),
    };
  }

  useEffect(() => {
    saveEvidenceDeskWorkspaceState({
      evidenceFilters: currentEvidenceFilters(),
      claimFilters: currentClaimFilters(),
      savedViews,
      activeViewId,
    });
  }, [
    evidenceSourceFilter,
    evidenceTagFilter,
    evidenceEntityFilter,
    evidenceLinkedStatusFilter,
    evidenceMinTrustFilter,
    evidenceQueryFilter,
    evidenceLimitFilter,
    claimReviewFilter,
    claimSubjectFilter,
    claimPredicateFilter,
    claimLinkedStatusFilter,
    claimMinConfidenceFilter,
    claimQueryFilter,
    claimLimitFilter,
    savedViews,
    activeViewId,
  ]);

  async function loadDesk(
    message?: string,
    evidenceFilters: EvidenceQueueFilters = currentEvidenceFilters(),
    claimFilters: ClaimQueueFilters = currentClaimFilters()
  ) {
    try {
      const [sourceItems, evidenceItems, claimItems] = await Promise.all([
        fetchSources(),
        fetchEvidence(evidenceFilters),
        fetchClaims(claimFilters),
      ]);
      setSources(sourceItems);
      setEvidence(evidenceItems);
      setClaims(claimItems);
      setSourceId((current) => current || sourceItems[0]?.id || "");
      setStatus(
        message ??
          `Loaded ${evidenceItems.length} evidence items (${evidenceFilterSummary(
            evidenceFilters
          )}) and ${claimItems.length} claims (${claimFilterSummary(claimFilters)}).`
      );
    } catch (error) {
      setStatus(`Failed to load evidence desk: ${(error as Error).message}`);
    }
  }

  useEffect(() => {
    void loadDesk(undefined, initialWorkspace.evidenceFilters, initialWorkspace.claimFilters);
  }, []);

  async function applyReviewStatus(claimId: string, nextStatus: ClaimReviewStatus) {
    setStatus(`Updating ${claimId} -> ${nextStatus}...`);
    try {
      await reviewClaim(claimId, nextStatus);
      await loadDesk(`Claim ${claimId} -> ${nextStatus}.`);
    } catch (error) {
      setStatus(`Claim review failed: ${(error as Error).message}`);
    }
  }

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setStatus("Ingesting evidence...");
    try {
      const response = await ingestEvidence({
        source_id: sourceId,
        title,
        summary,
        content,
        url,
        observed_at: observedAt,
        tags: parseCsv(tagsText),
        entity_labels: parseCsv(entitiesText),
        proposed_claims: JSON.parse(claimsText) as ProposedClaim[],
      });
      setLastResult(response);
      await loadDesk(
        `Ingested ${response.evidence.id}; created ${response.claims.length} claims, ${response.hits.length} hits, ${response.case_updates.length} case updates.`
      );
    } catch (error) {
      setStatus(`Ingest failed: ${(error as Error).message}`);
    }
  }

  function applyEvidenceFilters() {
    void loadDesk(undefined, currentEvidenceFilters(), currentClaimFilters());
  }

  function resetEvidenceFilters() {
    setEvidenceSourceFilter("");
    setEvidenceTagFilter("");
    setEvidenceEntityFilter("");
    setEvidenceLinkedStatusFilter("all");
    setEvidenceMinTrustFilter("");
    setEvidenceQueryFilter("");
    setEvidenceLimitFilter(DEFAULT_DESK_LIMIT);
    void loadDesk(
      "Reset evidence queue filters.",
      { limit: Number(DEFAULT_DESK_LIMIT) },
      currentClaimFilters()
    );
  }

  function applyClaimFilters() {
    void loadDesk(undefined, currentEvidenceFilters(), currentClaimFilters());
  }

  function resetClaimFilters() {
    setClaimReviewFilter("all");
    setClaimSubjectFilter("");
    setClaimPredicateFilter("");
    setClaimLinkedStatusFilter("all");
    setClaimMinConfidenceFilter("");
    setClaimQueryFilter("");
    setClaimLimitFilter(DEFAULT_DESK_LIMIT);
    void loadDesk(
      "Reset claim queue filters.",
      currentEvidenceFilters(),
      { limit: Number(DEFAULT_DESK_LIMIT) }
    );
  }

  function saveCurrentView() {
    const name = savedViewName.trim();
    if (!name) {
      setStatus("Saved view name is required.");
      return;
    }

    const view: SavedEvidenceDeskView = {
      id: makeSavedViewId(name),
      name,
      evidenceFilters: currentEvidenceFilters(),
      claimFilters: currentClaimFilters(),
    };

    setSavedViews((existing) => {
      const next = existing.filter((item) => item.id !== view.id);
      return [view, ...next].slice(0, 8);
    });
    setActiveViewId(view.id);
    setSavedViewName("");
    setStatus(`Saved evidence desk view '${name}'.`);
  }

  function applySavedView(view: SavedEvidenceDeskView) {
    setActiveViewId(view.id);
    setEvidenceSourceFilter(view.evidenceFilters.source_id ?? "");
    setEvidenceTagFilter(view.evidenceFilters.tag ?? "");
    setEvidenceEntityFilter(view.evidenceFilters.entity ?? "");
    setEvidenceLinkedStatusFilter(linkedStatusSelection(view.evidenceFilters.linked_status));
    setEvidenceMinTrustFilter(
      view.evidenceFilters.min_trust !== undefined ? String(view.evidenceFilters.min_trust) : ""
    );
    setEvidenceQueryFilter(view.evidenceFilters.q ?? "");
    setEvidenceLimitFilter(deskLimitSelection(view.evidenceFilters.limit));
    setClaimReviewFilter(claimReviewSelection(view.claimFilters.review_status));
    setClaimSubjectFilter(view.claimFilters.subject ?? "");
    setClaimPredicateFilter(view.claimFilters.predicate ?? "");
    setClaimLinkedStatusFilter(linkedStatusSelection(view.claimFilters.linked_status));
    setClaimMinConfidenceFilter(
      view.claimFilters.min_confidence_bps !== undefined
        ? String(view.claimFilters.min_confidence_bps)
        : ""
    );
    setClaimQueryFilter(view.claimFilters.q ?? "");
    setClaimLimitFilter(deskLimitSelection(view.claimFilters.limit));
    void loadDesk(`Applied saved view '${view.name}'.`, view.evidenceFilters, view.claimFilters);
  }

  function deleteSavedView(view: SavedEvidenceDeskView) {
    setSavedViews((existing) => existing.filter((item) => item.id !== view.id));
    if (activeViewId === view.id) setActiveViewId(null);
    setStatus(`Deleted saved view '${view.name}'.`);
  }

  function clearWorkspace() {
    clearEvidenceDeskWorkspaceState();
    setEvidenceSourceFilter("");
    setEvidenceTagFilter("");
    setEvidenceEntityFilter("");
    setEvidenceLinkedStatusFilter("all");
    setEvidenceMinTrustFilter("");
    setEvidenceQueryFilter("");
    setEvidenceLimitFilter(DEFAULT_DESK_LIMIT);
    setClaimReviewFilter("all");
    setClaimSubjectFilter("");
    setClaimPredicateFilter("");
    setClaimLinkedStatusFilter("all");
    setClaimMinConfidenceFilter("");
    setClaimQueryFilter("");
    setClaimLimitFilter(DEFAULT_DESK_LIMIT);
    setSavedViews([]);
    setActiveViewId(null);
    setSavedViewName("");
    void loadDesk(
      "Cleared local evidence workspace state.",
      { limit: Number(DEFAULT_DESK_LIMIT) },
      { limit: Number(DEFAULT_DESK_LIMIT) }
    );
  }

  return (
    <section className="dashboard-grid">
      <article className="panel panel-hero panel-span-12">
        <p className="mono-label">Evidence Pipeline</p>
        <h2>Ingest Signals, Rank Evidence, Triage Claims</h2>
        <p>
          Manual ingest stands in for collection jobs in this slice. Every submission becomes
          normalized evidence with provenance, bounded claims, watchlist hits, and case updates.
          Evidence and claims are now ranked through deterministic queue math instead of raw
          reverse-chronological lists.
        </p>
      </article>

      <article className="panel panel-span-6">
        <p className="mono-label">Ingest Evidence</p>
        <form className="form-grid" onSubmit={onSubmit}>
          <label className="field field-full">
            <span>source_id</span>
            <select value={sourceId} onChange={(e) => setSourceId(e.target.value)}>
              {sources.map((source) => (
                <option key={source.id} value={source.id}>
                  {source.name}
                </option>
              ))}
            </select>
          </label>

          <label className="field field-full">
            <span>title</span>
            <input value={title} onChange={(e) => setTitle(e.target.value)} />
          </label>

          <label className="field field-full">
            <span>summary</span>
            <textarea rows={3} value={summary} onChange={(e) => setSummary(e.target.value)} />
          </label>

          <label className="field field-full">
            <span>content</span>
            <textarea rows={6} value={content} onChange={(e) => setContent(e.target.value)} />
          </label>

          <label className="field field-full">
            <span>url</span>
            <input value={url} onChange={(e) => setUrl(e.target.value)} />
          </label>

          <label className="field">
            <span>observed_at</span>
            <input value={observedAt} onChange={(e) => setObservedAt(e.target.value)} />
          </label>

          <label className="field field-full">
            <span>tags</span>
            <input value={tagsText} onChange={(e) => setTagsText(e.target.value)} />
          </label>

          <label className="field field-full">
            <span>entity_labels</span>
            <input value={entitiesText} onChange={(e) => setEntitiesText(e.target.value)} />
          </label>

          <label className="field field-full">
            <span>proposed_claims</span>
            <textarea
              className="command-editor"
              rows={8}
              value={claimsText}
              onChange={(e) => setClaimsText(e.target.value)}
            />
          </label>

          <button className="btn-primary" type="submit">
            Ingest Evidence
          </button>
        </form>
        <p className="status-line">{status}</p>
      </article>

      <article className="panel panel-span-6">
        <p className="mono-label">Latest Ingest Result</p>
        {lastResult ? (
          <div className="stack-list">
            <div>
              <p className="panel-note">
                Evidence: <code>{lastResult.evidence.id}</code>
              </p>
              <p className="panel-note">
                Provenance hash: <code>{lastResult.evidence.provenance_hash}</code>
              </p>
            </div>

            <div>
              <p className="mono-label">Watchlist Hits</p>
              {lastResult.hits.length === 0 ? (
                <p className="panel-note">No watchlist conditions matched.</p>
              ) : (
                <div className="command-stack">
                  {lastResult.hits.map((hit) => (
                    <div key={`${hit.watchlist_id}-${hit.evidence_id}`} className="command-row">
                      <h3>{hit.watchlist_name}</h3>
                      <code>severity: {hit.severity}</code>
                      <code>reason: {hit.reason}</code>
                      <code>keywords: {hit.matched_keywords.join(", ") || "none"}</code>
                      <code>entities: {hit.matched_entities.join(", ") || "none"}</code>
                    </div>
                  ))}
                </div>
              )}
            </div>

            <div>
              <p className="mono-label">Case Updates</p>
              {lastResult.case_updates.length === 0 ? (
                <p className="panel-note">No case lifecycle changes were required.</p>
              ) : (
                <div className="command-stack">
                  {lastResult.case_updates.map((transition) => (
                    <div key={transition.case.id} className="command-row">
                      <h3>{transition.case.title}</h3>
                      <code>case_id: {transition.case.id}</code>
                      <code>status: {transition.case.status}</code>
                      <code>decision: {JSON.stringify(transition.decision)}</code>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        ) : (
          <p>No ingest run yet.</p>
        )}
      </article>

      <article className="panel panel-span-6">
        <p className="mono-label">Evidence Filters</p>
        <div className="form-grid">
          <label className="field">
            <span>source_id</span>
            <input
              value={evidenceSourceFilter}
              onChange={(e) => setEvidenceSourceFilter(e.target.value)}
              placeholder="rss_national_security"
            />
          </label>
          <label className="field">
            <span>tag</span>
            <input
              value={evidenceTagFilter}
              onChange={(e) => setEvidenceTagFilter(e.target.value)}
              placeholder="security"
            />
          </label>
          <label className="field">
            <span>entity</span>
            <input
              value={evidenceEntityFilter}
              onChange={(e) => setEvidenceEntityFilter(e.target.value)}
              placeholder="alice north"
            />
          </label>
          <label className="field">
            <span>linked_status</span>
            <select
              value={evidenceLinkedStatusFilter}
              onChange={(e) =>
                setEvidenceLinkedStatusFilter(e.target.value as CaseStatus | "all")
              }
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
            <span>min_trust</span>
            <input
              value={evidenceMinTrustFilter}
              onChange={(e) => setEvidenceMinTrustFilter(e.target.value)}
              placeholder="80"
            />
          </label>
          <label className="field field-full">
            <span>q</span>
            <input
              value={evidenceQueryFilter}
              onChange={(e) => setEvidenceQueryFilter(e.target.value)}
              placeholder="orion leadership resignation"
            />
          </label>
          <label className="field">
            <span>limit</span>
            <select
              value={evidenceLimitFilter}
              onChange={(e) => setEvidenceLimitFilter(e.target.value)}
            >
              <option value="6">6</option>
              <option value="10">10</option>
              <option value="25">25</option>
              <option value="50">50</option>
              <option value="100">100</option>
              <option value="all">all</option>
            </select>
          </label>
        </div>
        <div className="button-row">
          <button className="btn-secondary" type="button" onClick={applyEvidenceFilters}>
            Apply Evidence Filters
          </button>
          <button className="btn-secondary" type="button" onClick={resetEvidenceFilters}>
            Reset
          </button>
        </div>
      </article>

      <article className="panel panel-span-6">
        <p className="mono-label">Claim Filters</p>
        <div className="form-grid">
          <label className="field">
            <span>review_status</span>
            <select
              value={claimReviewFilter}
              onChange={(e) =>
                setClaimReviewFilter(e.target.value as ClaimReviewStatus | "all")
              }
            >
              <option value="all">all</option>
              <option value="needs_review">needs_review</option>
              <option value="corroborated">corroborated</option>
              <option value="rejected">rejected</option>
            </select>
          </label>
          <label className="field">
            <span>subject</span>
            <input
              value={claimSubjectFilter}
              onChange={(e) => setClaimSubjectFilter(e.target.value)}
              placeholder="alice north"
            />
          </label>
          <label className="field">
            <span>predicate</span>
            <input
              value={claimPredicateFilter}
              onChange={(e) => setClaimPredicateFilter(e.target.value)}
              placeholder="resigned_from"
            />
          </label>
          <label className="field">
            <span>linked_status</span>
            <select
              value={claimLinkedStatusFilter}
              onChange={(e) =>
                setClaimLinkedStatusFilter(e.target.value as CaseStatus | "all")
              }
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
            <span>min_confidence_bps</span>
            <input
              value={claimMinConfidenceFilter}
              onChange={(e) => setClaimMinConfidenceFilter(e.target.value)}
              placeholder="8500"
            />
          </label>
          <label className="field field-full">
            <span>q</span>
            <input
              value={claimQueryFilter}
              onChange={(e) => setClaimQueryFilter(e.target.value)}
              placeholder="leadership appointment"
            />
          </label>
          <label className="field">
            <span>limit</span>
            <select
              value={claimLimitFilter}
              onChange={(e) => setClaimLimitFilter(e.target.value)}
            >
              <option value="6">6</option>
              <option value="10">10</option>
              <option value="25">25</option>
              <option value="50">50</option>
              <option value="100">100</option>
              <option value="all">all</option>
            </select>
          </label>
        </div>
        <div className="button-row">
          <button className="btn-secondary" type="button" onClick={applyClaimFilters}>
            Apply Claim Filters
          </button>
          <button className="btn-secondary" type="button" onClick={resetClaimFilters}>
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
              onChange={(e) => setSavedViewName(e.target.value)}
              placeholder="high-trust-needs-review"
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
          <p className="panel-note">No saved evidence desk views yet.</p>
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
                <code>evidence: {evidenceFilterSummary(view.evidenceFilters)}</code>
                <code>claims: {claimFilterSummary(view.claimFilters)}</code>
                <div className="button-row">
                  <button
                    className="btn-secondary"
                    onClick={() => applySavedView(view)}
                    type="button"
                  >
                    Apply
                  </button>
                  <button
                    className="btn-secondary"
                    onClick={() => deleteSavedView(view)}
                    type="button"
                  >
                    Delete
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </article>

      <article className="panel panel-span-7">
        <p className="mono-label">Ranked Evidence Queue</p>
        <div className="agent-grid">
          {evidence.map((entry, index) => (
            <div key={entry.evidence.id} className="agent-card">
              <div className="agent-card-head">
                <h3>
                  #{index + 1} {entry.evidence.title}
                </h3>
                <span className={`status-pill ${severityClass(entry.max_linked_severity)}`}>
                  {entry.max_linked_severity ?? "unlinked"}
                </span>
              </div>
              <p>{entry.evidence.summary || entry.evidence.content.slice(0, 140)}</p>
              <div className="pill-row">
                <span className="info-pill">{priorityLabel(entry.priority)}</span>
                <span className="info-pill">{entry.source_name}</span>
                <span className="info-pill">trust: {entry.source_trust_score}</span>
                <span className="info-pill">observed: {entry.evidence.observed_at}</span>
                {entry.semantic_score_bps != null ? (
                  <span className="info-pill">semantic: {entry.semantic_score_bps}</span>
                ) : null}
              </div>
              <div className="pill-row">
                <span className="info-pill">linked_cases: {entry.linked_case_count}</span>
                <span className="info-pill">linked_claims: {entry.linked_claim_count}</span>
                <span className="info-pill">provenance: {entry.evidence.provenance_hash}</span>
              </div>
              <div className="pill-row">
                {entry.evidence.tags.map((tag) => (
                  <span key={tag} className="tag-chip">
                    {tag}
                  </span>
                ))}
              </div>
            </div>
          ))}
        </div>
      </article>

      <article className="panel panel-span-5">
        <p className="mono-label">Ranked Claim Queue</p>
        <div className="command-stack">
          {claims.map((entry) => (
            <div key={entry.claim.id} className="command-row">
              <div className="agent-card-head">
                <h3>
                  {entry.claim.subject} {entry.claim.predicate} {entry.claim.object}
                </h3>
                <span className={`status-pill ${reviewStatusClass(entry.claim.review_status)}`}>
                  {entry.claim.review_status}
                </span>
              </div>
              <div className="pill-row">
                <span className="info-pill">{priorityLabel(entry.priority)}</span>
                <span className={`status-pill ${severityClass(entry.max_linked_severity)}`}>
                  {entry.max_linked_severity ?? "unlinked"}
                </span>
                <span className="info-pill">{entry.source_name}</span>
                <span className="info-pill">trust: {entry.source_trust_score}</span>
                {entry.semantic_score_bps != null ? (
                  <span className="info-pill">semantic: {entry.semantic_score_bps}</span>
                ) : null}
              </div>
              <code>{entry.claim.id}</code>
              <code>confidence_bps: {entry.claim.confidence_bps}</code>
              <code>evidence: {entry.evidence_title}</code>
              <code>observed_at: {entry.evidence_observed_at}</code>
              <code>linked_case_count: {entry.linked_case_count}</code>
              <code>rationale: {entry.claim.rationale}</code>
              <div className="button-row">
                <button
                  className="btn-secondary"
                  onClick={() => void applyReviewStatus(entry.claim.id, "corroborated")}
                  type="button"
                >
                  Corroborate
                </button>
                <button
                  className="btn-secondary"
                  onClick={() => void applyReviewStatus(entry.claim.id, "rejected")}
                  type="button"
                >
                  Reject
                </button>
                <button
                  className="btn-secondary"
                  onClick={() => void applyReviewStatus(entry.claim.id, "needs_review")}
                  type="button"
                >
                  Reset
                </button>
              </div>
            </div>
          ))}
        </div>
      </article>
    </section>
  );
}

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
import {
  Badge,
  Button,
  EmptyState,
  FormField,
  FormGrid,
  Input,
  Panel,
  Select,
  StatusLine,
  Tag,
  Textarea,
} from "../components";

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
    <section className="hx-page-grid">
      <Panel
        hero
        span={12}
        eyebrow="Evidence Pipeline"
        title="Ingest Signals, Rank Evidence, Triage Claims"
      >
        <p className="hx-description">
          Manual ingest stands in for collection jobs in this slice. Every submission becomes
          normalized evidence with provenance, bounded claims, watchlist hits, and case updates.
          Evidence and claims are now ranked through deterministic queue math instead of raw
          reverse-chronological lists.
        </p>
      </Panel>

      <Panel span={6} eyebrow="Ingest Evidence">
        <form className="hx-form-grid" onSubmit={onSubmit}>
          <FormField label="source_id" full>
            <Select value={sourceId} onChange={(e) => setSourceId(e.target.value)}>
              {sources.map((source) => (
                <option key={source.id} value={source.id}>
                  {source.name}
                </option>
              ))}
            </Select>
          </FormField>

          <FormField label="title" full>
            <Input value={title} onChange={(e) => setTitle(e.target.value)} />
          </FormField>

          <FormField label="summary" full>
            <Textarea rows={3} value={summary} onChange={(e) => setSummary(e.target.value)} />
          </FormField>

          <FormField label="content" full>
            <Textarea rows={6} value={content} onChange={(e) => setContent(e.target.value)} />
          </FormField>

          <FormField label="url" full>
            <Input value={url} onChange={(e) => setUrl(e.target.value)} />
          </FormField>

          <FormField label="observed_at">
            <Input value={observedAt} onChange={(e) => setObservedAt(e.target.value)} />
          </FormField>

          <FormField label="tags" full>
            <Input value={tagsText} onChange={(e) => setTagsText(e.target.value)} />
          </FormField>

          <FormField label="entity_labels" full>
            <Input value={entitiesText} onChange={(e) => setEntitiesText(e.target.value)} />
          </FormField>

          <FormField label="proposed_claims" full>
            <Textarea rows={8} value={claimsText} onChange={(e) => setClaimsText(e.target.value)} />
          </FormField>

          <Button variant="primary" type="submit">
            Ingest Evidence
          </Button>
        </form>
        <StatusLine>{status}</StatusLine>
      </Panel>

      <Panel span={6} eyebrow="Latest Ingest Result">
        {lastResult ? (
          <div className="hx-stack">
            <div className="hx-card-section">
              <p className="hx-mono-detail">
                Evidence: <code>{lastResult.evidence.id}</code>
              </p>
              <p className="hx-mono-detail">
                Provenance hash: <code>{lastResult.evidence.provenance_hash}</code>
              </p>
            </div>

            <div className="hx-card-section">
              <p className="hx-eyebrow">Watchlist Hits</p>
              {lastResult.hits.length === 0 ? (
                <p className="hx-description">No watchlist conditions matched.</p>
              ) : (
                <div className="hx-list">
                  {lastResult.hits.map((hit) => (
                    <div key={`${hit.watchlist_id}-${hit.evidence_id}`} className="hx-row">
                      <h3>{hit.watchlist_name}</h3>
                      <code className="hx-mono-detail">severity: {hit.severity}</code>
                      <code className="hx-mono-detail">reason: {hit.reason}</code>
                      <code className="hx-mono-detail">
                        keywords: {hit.matched_keywords.join(", ") || "none"}
                      </code>
                      <code className="hx-mono-detail">
                        entities: {hit.matched_entities.join(", ") || "none"}
                      </code>
                    </div>
                  ))}
                </div>
              )}
            </div>

            <div className="hx-card-section">
              <p className="hx-eyebrow">Case Updates</p>
              {lastResult.case_updates.length === 0 ? (
                <p className="hx-description">No case lifecycle changes were required.</p>
              ) : (
                <div className="hx-list">
                  {lastResult.case_updates.map((transition) => (
                    <div key={transition.case.id} className="hx-row">
                      <h3>{transition.case.title}</h3>
                      <code className="hx-mono-detail">case_id: {transition.case.id}</code>
                      <code className="hx-mono-detail">status: {transition.case.status}</code>
                      <code className="hx-mono-detail">
                        decision: {JSON.stringify(transition.decision)}
                      </code>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        ) : (
          <EmptyState title="No ingest run yet." />
        )}
      </Panel>

      <Panel span={6} eyebrow="Evidence Filters">
        <FormGrid>
          <FormField label="source_id">
            <Input
              value={evidenceSourceFilter}
              onChange={(e) => setEvidenceSourceFilter(e.target.value)}
              placeholder="rss_national_security"
            />
          </FormField>
          <FormField label="tag">
            <Input
              value={evidenceTagFilter}
              onChange={(e) => setEvidenceTagFilter(e.target.value)}
              placeholder="security"
            />
          </FormField>
          <FormField label="entity">
            <Input
              value={evidenceEntityFilter}
              onChange={(e) => setEvidenceEntityFilter(e.target.value)}
              placeholder="alice north"
            />
          </FormField>
          <FormField label="linked_status">
            <Select
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
            </Select>
          </FormField>
          <FormField label="min_trust">
            <Input
              value={evidenceMinTrustFilter}
              onChange={(e) => setEvidenceMinTrustFilter(e.target.value)}
              placeholder="80"
            />
          </FormField>
          <FormField label="q" full>
            <Input
              value={evidenceQueryFilter}
              onChange={(e) => setEvidenceQueryFilter(e.target.value)}
              placeholder="orion leadership resignation"
            />
          </FormField>
          <FormField label="limit">
            <Select
              value={evidenceLimitFilter}
              onChange={(e) => setEvidenceLimitFilter(e.target.value)}
            >
              <option value="6">6</option>
              <option value="10">10</option>
              <option value="25">25</option>
              <option value="50">50</option>
              <option value="100">100</option>
              <option value="all">all</option>
            </Select>
          </FormField>
        </FormGrid>
        <div className="hx-cluster">
          <Button variant="secondary" type="button" onClick={applyEvidenceFilters}>
            Apply Evidence Filters
          </Button>
          <Button variant="secondary" type="button" onClick={resetEvidenceFilters}>
            Reset
          </Button>
        </div>
      </Panel>

      <Panel span={6} eyebrow="Claim Filters">
        <FormGrid>
          <FormField label="review_status">
            <Select
              value={claimReviewFilter}
              onChange={(e) =>
                setClaimReviewFilter(e.target.value as ClaimReviewStatus | "all")
              }
            >
              <option value="all">all</option>
              <option value="needs_review">needs_review</option>
              <option value="corroborated">corroborated</option>
              <option value="rejected">rejected</option>
            </Select>
          </FormField>
          <FormField label="subject">
            <Input
              value={claimSubjectFilter}
              onChange={(e) => setClaimSubjectFilter(e.target.value)}
              placeholder="alice north"
            />
          </FormField>
          <FormField label="predicate">
            <Input
              value={claimPredicateFilter}
              onChange={(e) => setClaimPredicateFilter(e.target.value)}
              placeholder="resigned_from"
            />
          </FormField>
          <FormField label="linked_status">
            <Select
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
            </Select>
          </FormField>
          <FormField label="min_confidence_bps">
            <Input
              value={claimMinConfidenceFilter}
              onChange={(e) => setClaimMinConfidenceFilter(e.target.value)}
              placeholder="8500"
            />
          </FormField>
          <FormField label="q" full>
            <Input
              value={claimQueryFilter}
              onChange={(e) => setClaimQueryFilter(e.target.value)}
              placeholder="leadership appointment"
            />
          </FormField>
          <FormField label="limit">
            <Select
              value={claimLimitFilter}
              onChange={(e) => setClaimLimitFilter(e.target.value)}
            >
              <option value="6">6</option>
              <option value="10">10</option>
              <option value="25">25</option>
              <option value="50">50</option>
              <option value="100">100</option>
              <option value="all">all</option>
            </Select>
          </FormField>
        </FormGrid>
        <div className="hx-cluster">
          <Button variant="secondary" type="button" onClick={applyClaimFilters}>
            Apply Claim Filters
          </Button>
          <Button variant="secondary" type="button" onClick={resetClaimFilters}>
            Reset
          </Button>
        </div>
      </Panel>

      <Panel span={12} eyebrow="Saved Views">
        <FormGrid>
          <FormField label="view_name" full>
            <Input
              value={savedViewName}
              onChange={(e) => setSavedViewName(e.target.value)}
              placeholder="high-trust-needs-review"
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
          <p className="hx-description">No saved evidence desk views yet.</p>
        ) : (
          <div className="hx-list">
            {savedViews.map((view) => (
              <div key={view.id} className="hx-card">
                <div className="hx-card-head">
                  <h3>{view.name}</h3>
                  <Badge tone={activeViewId === view.id ? "ok" : "info"}>
                    {activeViewId === view.id ? "active" : "saved"}
                  </Badge>
                </div>
                <code className="hx-mono-detail">
                  evidence: {evidenceFilterSummary(view.evidenceFilters)}
                </code>
                <code className="hx-mono-detail">
                  claims: {claimFilterSummary(view.claimFilters)}
                </code>
                <div className="hx-cluster">
                  <Button variant="secondary" onClick={() => applySavedView(view)} type="button">
                    Apply
                  </Button>
                  <Button variant="secondary" onClick={() => deleteSavedView(view)} type="button">
                    Delete
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}
      </Panel>

      <Panel span={7} eyebrow="Ranked Evidence Queue">
        <div className="hx-card-grid">
          {evidence.map((entry, index) => (
            <div key={entry.evidence.id} className="hx-card">
              <div className="hx-card-head">
                <h3>
                  #{index + 1} {entry.evidence.title}
                </h3>
                <Badge tone={severityClass(entry.max_linked_severity) as "ok" | "warn" | "danger" | "info"}>
                  {entry.max_linked_severity ?? "unlinked"}
                </Badge>
              </div>
              <p className="hx-description">
                {entry.evidence.summary || entry.evidence.content.slice(0, 140)}
              </p>
              <div className="hx-tag-row">
                <Tag>{priorityLabel(entry.priority)}</Tag>
                <Tag>{entry.source_name}</Tag>
                <Tag>trust: {entry.source_trust_score}</Tag>
                <Tag>observed: {entry.evidence.observed_at}</Tag>
                {entry.semantic_score_bps != null ? (
                  <Tag>semantic: {entry.semantic_score_bps}</Tag>
                ) : null}
              </div>
              <div className="hx-tag-row">
                <Tag>linked_cases: {entry.linked_case_count}</Tag>
                <Tag>linked_claims: {entry.linked_claim_count}</Tag>
                <Tag>provenance: {entry.evidence.provenance_hash}</Tag>
              </div>
              <div className="hx-tag-row">
                {entry.evidence.tags.map((tag) => (
                  <Tag key={tag}>{tag}</Tag>
                ))}
              </div>
            </div>
          ))}
        </div>
      </Panel>

      <Panel span={5} eyebrow="Ranked Claim Queue">
        <div className="hx-list">
          {claims.map((entry) => (
            <div key={entry.claim.id} className="hx-row">
              <div className="hx-card-head">
                <h3>
                  {entry.claim.subject} {entry.claim.predicate} {entry.claim.object}
                </h3>
                <Badge tone={reviewStatusClass(entry.claim.review_status) as "ok" | "warn" | "danger"}>
                  {entry.claim.review_status}
                </Badge>
              </div>
              <div className="hx-tag-row">
                <Tag>{priorityLabel(entry.priority)}</Tag>
                <Badge tone={severityClass(entry.max_linked_severity) as "ok" | "warn" | "danger" | "info"}>
                  {entry.max_linked_severity ?? "unlinked"}
                </Badge>
                <Tag>{entry.source_name}</Tag>
                <Tag>trust: {entry.source_trust_score}</Tag>
                {entry.semantic_score_bps != null ? (
                  <Tag>semantic: {entry.semantic_score_bps}</Tag>
                ) : null}
              </div>
              <code className="hx-mono-detail">{entry.claim.id}</code>
              <code className="hx-mono-detail">confidence_bps: {entry.claim.confidence_bps}</code>
              <code className="hx-mono-detail">evidence: {entry.evidence_title}</code>
              <code className="hx-mono-detail">observed_at: {entry.evidence_observed_at}</code>
              <code className="hx-mono-detail">linked_case_count: {entry.linked_case_count}</code>
              <code className="hx-mono-detail">rationale: {entry.claim.rationale}</code>
              <div className="hx-cluster">
                <Button
                  variant="secondary"
                  onClick={() => void applyReviewStatus(entry.claim.id, "corroborated")}
                  type="button"
                >
                  Corroborate
                </Button>
                <Button
                  variant="secondary"
                  onClick={() => void applyReviewStatus(entry.claim.id, "rejected")}
                  type="button"
                >
                  Reject
                </Button>
                <Button
                  variant="secondary"
                  onClick={() => void applyReviewStatus(entry.claim.id, "needs_review")}
                  type="button"
                >
                  Reset
                </Button>
              </div>
            </div>
          ))}
        </div>
      </Panel>
    </section>
  );
}

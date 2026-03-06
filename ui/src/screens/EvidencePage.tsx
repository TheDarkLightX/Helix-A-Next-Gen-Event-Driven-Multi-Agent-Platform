import { FormEvent, useEffect, useMemo, useState } from "react";
import {
  ClaimRecord,
  ClaimReviewStatus,
  EvidenceItem,
  IngestEvidenceResponse,
  ProposedClaim,
  SourceDefinition,
  fetchClaims,
  fetchEvidence,
  fetchSources,
  ingestEvidence,
  reviewClaim,
} from "../lib/api";

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

export function EvidencePage() {
  const [sources, setSources] = useState<SourceDefinition[]>([]);
  const [evidence, setEvidence] = useState<EvidenceItem[]>([]);
  const [claims, setClaims] = useState<ClaimRecord[]>([]);
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

  async function loadDesk(message?: string) {
    try {
      const [sourceItems, evidenceItems, claimItems] = await Promise.all([
        fetchSources(),
        fetchEvidence(),
        fetchClaims(),
      ]);
      setSources(sourceItems);
      setEvidence(evidenceItems);
      setClaims(claimItems);
      setSourceId((current) => current || sourceItems[0]?.id || "");
      setStatus(
        message ?? `Loaded ${evidenceItems.length} evidence items and ${claimItems.length} claims.`
      );
    } catch (error) {
      setStatus(`Failed to load evidence desk: ${(error as Error).message}`);
    }
  }

  useEffect(() => {
    void loadDesk();
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
      await loadDesk();
      setStatus(
        `Ingested ${response.evidence.id}; created ${response.claims.length} claims, ${response.hits.length} hits, ${response.case_updates.length} case updates.`
      );
    } catch (error) {
      setStatus(`Ingest failed: ${(error as Error).message}`);
    }
  }

  const recentClaims = useMemo(() => claims.slice().reverse().slice(0, 6), [claims]);

  return (
    <section className="dashboard-grid">
      <article className="panel panel-hero panel-span-12">
        <p className="mono-label">Evidence Pipeline</p>
        <h2>Ingest Signals, Materialize Claims, Open Cases</h2>
        <p>
          Manual ingest stands in for collection jobs in this slice. Every submission becomes
          normalized evidence with provenance, bounded claims, watchlist hits, and case updates.
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

      <article className="panel panel-span-7">
        <p className="mono-label">Recent Evidence</p>
        <div className="agent-grid">
          {evidence
            .slice()
            .reverse()
            .slice(0, 6)
            .map((item) => (
              <div key={item.id} className="agent-card">
                <div className="agent-card-head">
                  <h3>{item.title}</h3>
                  <span className="status-pill ok">{item.source_id}</span>
                </div>
                <p>{item.summary || item.content.slice(0, 140)}</p>
                <p className="mono-detail">{item.id}</p>
                <div className="pill-row">
                  <span className="info-pill">observed: {item.observed_at}</span>
                </div>
                <div className="pill-row">
                  {item.tags.map((tag) => (
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
        <p className="mono-label">Recent Claims</p>
        <div className="command-stack">
          {recentClaims.map((claim) => (
            <div key={claim.id} className="command-row">
              <div className="agent-card-head">
                <h3>
                  {claim.subject} {claim.predicate} {claim.object}
                </h3>
                <span className={`status-pill ${reviewStatusClass(claim.review_status)}`}>
                  {claim.review_status}
                </span>
              </div>
              <code>{claim.id}</code>
              <code>confidence_bps: {claim.confidence_bps}</code>
              <code>rationale: {claim.rationale}</code>
              <div className="button-row">
                <button
                  className="btn-secondary"
                  onClick={() => void applyReviewStatus(claim.id, "corroborated")}
                  type="button"
                >
                  Corroborate
                </button>
                <button
                  className="btn-secondary"
                  onClick={() => void applyReviewStatus(claim.id, "rejected")}
                  type="button"
                >
                  Reject
                </button>
                <button
                  className="btn-secondary"
                  onClick={() => void applyReviewStatus(claim.id, "needs_review")}
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

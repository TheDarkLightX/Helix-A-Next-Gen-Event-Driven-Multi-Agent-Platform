import { FormEvent, useEffect, useState } from "react";
import { SourceDefinition, SourceKind, collectSource, createSource, fetchSources } from "../lib/api";

const SOURCE_KIND_OPTIONS: { value: SourceKind; label: string }[] = [
  { value: "rss_feed", label: "RSS Feed" },
  { value: "website_diff", label: "Website Diff" },
  { value: "json_api", label: "JSON API" },
  { value: "webhook_ingest", label: "Webhook Ingest" },
  { value: "email_digest", label: "Email Digest" },
  { value: "file_import", label: "File Import" },
];
const DEFAULT_PROFILE_ID = "50000000-0000-0000-0000-000000000010";

function parseCsv(value: string): string[] {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

export function SourcesPage() {
  const [sources, setSources] = useState<SourceDefinition[]>([]);
  const [status, setStatus] = useState<string>("Loading source registry...");
  const [profileId, setProfileId] = useState<string>(DEFAULT_PROFILE_ID);
  const [name, setName] = useState<string>("");
  const [description, setDescription] = useState<string>("");
  const [kind, setKind] = useState<SourceKind>("rss_feed");
  const [endpointUrl, setEndpointUrl] = useState<string>("");
  const [credentialId, setCredentialId] = useState<string>("");
  const [credentialHeaderName, setCredentialHeaderName] = useState<string>("Authorization");
  const [credentialHeaderPrefix, setCredentialHeaderPrefix] = useState<string>("Bearer");
  const [cadence, setCadence] = useState<number>(30);
  const [trustScore, setTrustScore] = useState<number>(75);
  const [enabled, setEnabled] = useState<boolean>(true);
  const [tagsText, setTagsText] = useState<string>("osint, monitoring");
  const [collectingSourceId, setCollectingSourceId] = useState<string | null>(null);

  async function loadSources() {
    try {
      const items = await fetchSources();
      setSources(items);
      setStatus(`Loaded ${items.length} sources.`);
    } catch (error) {
      setStatus(`Failed to load sources: ${(error as Error).message}`);
    }
  }

  useEffect(() => {
    void loadSources();
  }, []);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setStatus("Creating source...");
    try {
      const created = await createSource({
        profile_id: profileId.trim() || DEFAULT_PROFILE_ID,
        name,
        description,
        kind,
        endpoint_url: endpointUrl.trim() || null,
        credential_id: credentialId.trim() || null,
        credential_header_name: credentialHeaderName.trim() || "Authorization",
        credential_header_prefix: credentialHeaderPrefix.trim() || null,
        cadence_minutes: cadence,
        trust_score: trustScore,
        enabled,
        tags: parseCsv(tagsText),
      });
      setSources((prev) => [...prev, created].sort((left, right) => left.id.localeCompare(right.id)));
      setName("");
      setDescription("");
      setKind("rss_feed");
      setEndpointUrl("");
      setCredentialId("");
      setCredentialHeaderName("Authorization");
      setCredentialHeaderPrefix("Bearer");
      setCadence(30);
      setTrustScore(75);
      setEnabled(true);
      setTagsText("osint, monitoring");
      setStatus(`Created ${created.name}.`);
    } catch (error) {
      setStatus(`Create failed: ${(error as Error).message}`);
    }
  }

  async function onCollect(source: SourceDefinition) {
    setCollectingSourceId(source.id);
    setStatus(`Collecting ${source.name}...`);
    try {
      const response = await collectSource(source.id, {
        observed_at: new Date().toISOString(),
        max_items: 10,
      });
      const caseUpdates = response.results.reduce(
        (total, result) => total + result.case_updates.length,
        0
      );
      setStatus(
        `Collected ${response.collected_count} item(s) from ${source.name}; ${response.duplicate_count} duplicate(s), ${caseUpdates} case update(s).`
      );
    } catch (error) {
      setStatus(`Collect failed: ${(error as Error).message}`);
    } finally {
      setCollectingSourceId(null);
    }
  }

  return (
    <section className="dashboard-grid">
      <article className="panel panel-hero panel-span-12">
        <p className="mono-label">Source Registry</p>
        <h2>Self-Hosted Collection With Explicit Trust Boundaries</h2>
        <p>
          Register collection adapters, bound their cadence, and assign deterministic trust scores
          before evidence enters the desk.
        </p>
      </article>

      <article className="panel panel-span-5">
        <p className="mono-label">Create Source</p>
        <form className="form-grid" onSubmit={onSubmit}>
          <label className="field field-full">
            <span>profile_id</span>
            <input value={profileId} onChange={(e) => setProfileId(e.target.value)} />
          </label>

          <label className="field field-full">
            <span>name</span>
            <input value={name} onChange={(e) => setName(e.target.value)} />
          </label>

          <label className="field field-full">
            <span>description</span>
            <textarea
              rows={4}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </label>

          <label className="field">
            <span>kind</span>
            <select value={kind} onChange={(e) => setKind(e.target.value as SourceKind)}>
              {SOURCE_KIND_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>

          <label className="field field-full">
            <span>endpoint_url</span>
            <input value={endpointUrl} onChange={(e) => setEndpointUrl(e.target.value)} />
          </label>

          <label className="field field-full">
            <span>credential_id</span>
            <input value={credentialId} onChange={(e) => setCredentialId(e.target.value)} />
          </label>

          <label className="field">
            <span>credential_header_name</span>
            <input
              value={credentialHeaderName}
              onChange={(e) => setCredentialHeaderName(e.target.value)}
            />
          </label>

          <label className="field">
            <span>credential_header_prefix</span>
            <input
              value={credentialHeaderPrefix}
              onChange={(e) => setCredentialHeaderPrefix(e.target.value)}
            />
          </label>

          <label className="field">
            <span>cadence_minutes</span>
            <input
              type="number"
              min={1}
              max={1440}
              value={cadence}
              onChange={(e) => setCadence(Number(e.target.value))}
            />
          </label>

          <label className="field">
            <span>trust_score</span>
            <input
              type="number"
              min={0}
              max={100}
              value={trustScore}
              onChange={(e) => setTrustScore(Number(e.target.value))}
            />
          </label>

          <label className="field field-full">
            <span>tags</span>
            <input value={tagsText} onChange={(e) => setTagsText(e.target.value)} />
          </label>

          <label className="field checkbox-field field-full">
            <input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} />
            <span>enabled</span>
          </label>

          <button className="btn-primary" type="submit">
            Register Source
          </button>
        </form>
        <p className="status-line">{status}</p>
      </article>

      <article className="panel panel-span-7">
        <p className="mono-label">Active Sources</p>
        <div className="agent-grid">
          {sources.map((source) => (
            <div key={source.id} className="agent-card">
              <div className="agent-card-head">
                <h3>{source.name}</h3>
                <span className={`status-pill ${source.enabled ? "ok" : "warn"}`}>
                  {source.enabled ? "enabled" : "paused"}
                </span>
              </div>
              <p>{source.description}</p>
              <p className="mono-detail">{source.id}</p>
              <p className="mono-detail">profile: {source.profile_id}</p>
              {source.endpoint_url ? <p className="mono-detail">{source.endpoint_url}</p> : null}
              <div className="pill-row">
                <span className="info-pill">kind: {source.kind}</span>
                <span className="info-pill">cadence: {source.cadence_minutes}m</span>
                <span className="info-pill">trust: {source.trust_score}</span>
                {source.credential_id ? (
                  <span className="info-pill">
                    credential: {source.credential_id} via {source.credential_header_name}
                  </span>
                ) : null}
              </div>
              <div className="pill-row">
                {source.tags.map((tag) => (
                  <span key={tag} className="tag-chip">
                    {tag}
                  </span>
                ))}
              </div>
              <button
                className="btn-secondary"
                type="button"
                disabled={!source.endpoint_url || collectingSourceId === source.id}
                onClick={() => void onCollect(source)}
              >
                {collectingSourceId === source.id ? "Collecting" : "Collect Now"}
              </button>
            </div>
          ))}
        </div>
      </article>
    </section>
  );
}

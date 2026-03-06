import { FormEvent, useEffect, useState } from "react";
import { SourceDefinition, SourceKind, createSource, fetchSources } from "../lib/api";

const SOURCE_KIND_OPTIONS: { value: SourceKind; label: string }[] = [
  { value: "rss_feed", label: "RSS Feed" },
  { value: "website_diff", label: "Website Diff" },
  { value: "json_api", label: "JSON API" },
  { value: "webhook_ingest", label: "Webhook Ingest" },
  { value: "email_digest", label: "Email Digest" },
  { value: "file_import", label: "File Import" },
];

function parseCsv(value: string): string[] {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

export function SourcesPage() {
  const [sources, setSources] = useState<SourceDefinition[]>([]);
  const [status, setStatus] = useState<string>("Loading source registry...");
  const [name, setName] = useState<string>("");
  const [description, setDescription] = useState<string>("");
  const [kind, setKind] = useState<SourceKind>("rss_feed");
  const [cadence, setCadence] = useState<number>(30);
  const [trustScore, setTrustScore] = useState<number>(75);
  const [enabled, setEnabled] = useState<boolean>(true);
  const [tagsText, setTagsText] = useState<string>("osint, monitoring");

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
        name,
        description,
        kind,
        cadence_minutes: cadence,
        trust_score: trustScore,
        enabled,
        tags: parseCsv(tagsText),
      });
      setSources((prev) => [...prev, created].sort((left, right) => left.id.localeCompare(right.id)));
      setName("");
      setDescription("");
      setKind("rss_feed");
      setCadence(30);
      setTrustScore(75);
      setEnabled(true);
      setTagsText("osint, monitoring");
      setStatus(`Created ${created.name}.`);
    } catch (error) {
      setStatus(`Create failed: ${(error as Error).message}`);
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
              <div className="pill-row">
                <span className="info-pill">kind: {source.kind}</span>
                <span className="info-pill">cadence: {source.cadence_minutes}m</span>
                <span className="info-pill">trust: {source.trust_score}</span>
              </div>
              <div className="pill-row">
                {source.tags.map((tag) => (
                  <span key={tag} className="tag-chip">
                    {tag}
                  </span>
                ))}
              </div>
            </div>
          ))}
        </div>
      </article>
    </section>
  );
}

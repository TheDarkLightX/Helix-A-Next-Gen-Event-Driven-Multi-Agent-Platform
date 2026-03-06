import { FormEvent, useEffect, useState } from "react";
import {
  Watchlist,
  WatchlistSeverity,
  createWatchlist,
  fetchWatchlists,
} from "../lib/api";

const SEVERITY_OPTIONS: WatchlistSeverity[] = ["low", "medium", "high", "critical"];

function parseCsv(value: string): string[] {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function severityClass(severity: WatchlistSeverity) {
  if (severity === "critical" || severity === "high") return "danger";
  if (severity === "medium") return "warn";
  return "ok";
}

export function WatchlistsPage() {
  const [watchlists, setWatchlists] = useState<Watchlist[]>([]);
  const [status, setStatus] = useState<string>("Loading watchlists...");
  const [name, setName] = useState<string>("");
  const [description, setDescription] = useState<string>("");
  const [keywordsText, setKeywordsText] = useState<string>("resigned, appointed");
  const [entitiesText, setEntitiesText] = useState<string>("alice north, orion dynamics");
  const [minTrust, setMinTrust] = useState<number>(60);
  const [severity, setSeverity] = useState<WatchlistSeverity>("high");
  const [enabled, setEnabled] = useState<boolean>(true);

  async function loadWatchlists() {
    try {
      const items = await fetchWatchlists();
      setWatchlists(items);
      setStatus(`Loaded ${items.length} watchlists.`);
    } catch (error) {
      setStatus(`Failed to load watchlists: ${(error as Error).message}`);
    }
  }

  useEffect(() => {
    void loadWatchlists();
  }, []);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setStatus("Creating watchlist...");
    try {
      const watchlist = await createWatchlist({
        name,
        description,
        keywords: parseCsv(keywordsText),
        entities: parseCsv(entitiesText),
        min_source_trust: minTrust,
        severity,
        enabled,
      });
      setWatchlists((prev) => [...prev, watchlist].sort((left, right) => left.id.localeCompare(right.id)));
      setName("");
      setDescription("");
      setKeywordsText("resigned, appointed");
      setEntitiesText("alice north, orion dynamics");
      setMinTrust(60);
      setSeverity("high");
      setEnabled(true);
      setStatus(`Created ${watchlist.name}.`);
    } catch (error) {
      setStatus(`Create failed: ${(error as Error).message}`);
    }
  }

  return (
    <section className="dashboard-grid">
      <article className="panel panel-hero panel-span-12">
        <p className="mono-label">Watchlists</p>
        <h2>Deterministic Match Rules For What Matters</h2>
        <p>
          Model explicit keywords, entities, source-trust floors, and severity so the desk opens
          work only when bounded conditions are met.
        </p>
      </article>

      <article className="panel panel-span-5">
        <p className="mono-label">Create Watchlist</p>
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

          <label className="field field-full">
            <span>keywords</span>
            <input value={keywordsText} onChange={(e) => setKeywordsText(e.target.value)} />
          </label>

          <label className="field field-full">
            <span>entities</span>
            <input value={entitiesText} onChange={(e) => setEntitiesText(e.target.value)} />
          </label>

          <label className="field">
            <span>min_source_trust</span>
            <input
              type="number"
              min={0}
              max={100}
              value={minTrust}
              onChange={(e) => setMinTrust(Number(e.target.value))}
            />
          </label>

          <label className="field">
            <span>severity</span>
            <select value={severity} onChange={(e) => setSeverity(e.target.value as WatchlistSeverity)}>
              {SEVERITY_OPTIONS.map((value) => (
                <option key={value} value={value}>
                  {value}
                </option>
              ))}
            </select>
          </label>

          <label className="field checkbox-field field-full">
            <input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} />
            <span>enabled</span>
          </label>

          <button className="btn-primary" type="submit">
            Create Watchlist
          </button>
        </form>
        <p className="status-line">{status}</p>
      </article>

      <article className="panel panel-span-7">
        <p className="mono-label">Configured Watchlists</p>
        <div className="agent-grid">
          {watchlists.map((watchlist) => (
            <div key={watchlist.id} className="agent-card">
              <div className="agent-card-head">
                <h3>{watchlist.name}</h3>
                <span className={`status-pill ${severityClass(watchlist.severity)}`}>{watchlist.severity}</span>
              </div>
              <p>{watchlist.description}</p>
              <p className="mono-detail">{watchlist.id}</p>
              <div className="pill-row">
                <span className="info-pill">min trust: {watchlist.min_source_trust}</span>
                <span className="info-pill">enabled: {watchlist.enabled ? "yes" : "no"}</span>
              </div>
              <div className="stack-list">
                <div>
                  <p className="mono-label">keywords</p>
                  <div className="pill-row">
                    {watchlist.keywords.map((keyword) => (
                      <span key={keyword} className="tag-chip">
                        {keyword}
                      </span>
                    ))}
                  </div>
                </div>
                <div>
                  <p className="mono-label">entities</p>
                  <div className="pill-row">
                    {watchlist.entities.map((entity) => (
                      <span key={entity} className="tag-chip">
                        {entity}
                      </span>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          ))}
        </div>
      </article>
    </section>
  );
}

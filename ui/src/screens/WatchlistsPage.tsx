import { FormEvent, useEffect, useState } from "react";
import {
  Watchlist,
  WatchlistSeverity,
  createWatchlist,
  fetchWatchlists,
} from "../lib/api";
import {
  Panel,
  FormField,
  Input,
  Textarea,
  Select,
  CheckboxField,
  FormGrid,
  Button,
  Badge,
  Tag,
  StatusLine,
  severityTone,
} from "../components";

const SEVERITY_OPTIONS: WatchlistSeverity[] = ["low", "medium", "high", "critical"];

function parseCsv(value: string): string[] {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
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
    <section className="hx-page-grid">
      <Panel hero span={12} eyebrow="Watchlists" title="Deterministic Match Rules For What Matters">
        <p className="hx-description">
          Model explicit keywords, entities, source-trust floors, and severity so the
          desk opens work only when bounded conditions are met.
        </p>
      </Panel>

      <Panel span={5} eyebrow="Create" title="New Watchlist">
        <form className="hx-form-grid" onSubmit={onSubmit}>
          <FormField label="Name" full>
            <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Executive departures" />
          </FormField>

          <FormField label="Description" full>
            <Textarea rows={3} value={description} onChange={(e) => setDescription(e.target.value)} placeholder="What this watchlist tracks and why" />
          </FormField>

          <FormField label="Keywords" full hint="Comma-separated">
            <Input value={keywordsText} onChange={(e) => setKeywordsText(e.target.value)} />
          </FormField>

          <FormField label="Entities" full hint="Comma-separated">
            <Input value={entitiesText} onChange={(e) => setEntitiesText(e.target.value)} />
          </FormField>

          <FormField label="Min source trust" hint="0–100">
            <Input
              type="number"
              min={0}
              max={100}
              value={minTrust}
              onChange={(e) => setMinTrust(Number(e.target.value))}
            />
          </FormField>

          <FormField label="Severity">
            <Select value={severity} onChange={(e) => setSeverity(e.target.value as WatchlistSeverity)}>
              {SEVERITY_OPTIONS.map((value) => (
                <option key={value} value={value}>{value}</option>
              ))}
            </Select>
          </FormField>

          <CheckboxField label="Enabled" checked={enabled} onChange={setEnabled} full />

          <FormField label="" full>
            <Button type="submit">Create Watchlist</Button>
          </FormField>
        </form>
        <StatusLine>{status}</StatusLine>
      </Panel>

      <Panel span={7} eyebrow="Configured" title="Watchlists" actions={<Badge tone="accent">{watchlists.length}</Badge>}>
        {watchlists.length === 0 ? (
          <div className="hx-table-empty"><p>No watchlists configured yet.</p></div>
        ) : (
          <div className="hx-card-grid">
            {watchlists.map((watchlist) => (
              <div key={watchlist.id} className="hx-card">
                <div className="hx-card-head">
                  <h3>{watchlist.name}</h3>
                  <Badge tone={severityTone(watchlist.severity)}>{watchlist.severity}</Badge>
                </div>
                <p className="hx-row-secondary">{watchlist.description}</p>
                <p className="hx-mono-detail">{watchlist.id}</p>
                <div className="hx-tag-row">
                  <Tag>min trust: {watchlist.min_source_trust}</Tag>
                  <Tag>enabled: {watchlist.enabled ? "yes" : "no"}</Tag>
                </div>
                <div className="hx-card-section">
                  <span className="hx-eyebrow">Keywords</span>
                  <div className="hx-tag-row">
                    {watchlist.keywords.map((keyword) => (
                      <Tag key={keyword}>{keyword}</Tag>
                    ))}
                  </div>
                </div>
                <div className="hx-card-section">
                  <span className="hx-eyebrow">Entities</span>
                  <div className="hx-tag-row">
                    {watchlist.entities.map((entity) => (
                      <Tag key={entity}>{entity}</Tag>
                    ))}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </Panel>
    </section>
  );
}

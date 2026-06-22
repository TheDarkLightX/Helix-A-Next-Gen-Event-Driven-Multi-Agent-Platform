import { FormEvent, useEffect, useState } from "react";
import { SourceDefinition, SourceKind, collectSource, createSource, fetchSources } from "../lib/api";
import {
  Panel,
  FormField,
  Input,
  Textarea,
  Select,
  CheckboxField,
  Button,
  Badge,
  Tag,
  StatusLine,
} from "../components";

const SOURCE_KIND_OPTIONS: { value: SourceKind; label: string }[] = [
  { value: "rss_feed", label: "RSS Feed" },
  { value: "website_diff", label: "Website Diff" },
  { value: "json_api", label: "JSON API" },
  { value: "webhook_ingest", label: "Webhook Ingest" },
  { value: "email_digest", label: "Email Digest" },
  { value: "file_import", label: "File Import" },
];
const DEFAULT_PROFILE_ID = "50000000-0000-0000-0000-000000000010";

function supportsPullCollection(source: SourceDefinition): boolean {
  return source.kind === "rss_feed" || source.kind === "website_diff" || source.kind === "json_api";
}

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
    <section className="hx-page-grid">
      <Panel hero span={12} eyebrow="Source Registry" title="Self-Hosted Collection With Explicit Trust Boundaries">
        <p className="hx-description">
          Register collection adapters, bound their cadence, and assign deterministic
          trust scores before evidence enters the desk.
        </p>
      </Panel>

      <Panel span={5} eyebrow="Create" title="Register Source">
        <form className="hx-form-grid" onSubmit={onSubmit}>
          <FormField label="Profile ID" full>
            <Input value={profileId} onChange={(e) => setProfileId(e.target.value)} />
          </FormField>

          <FormField label="Name" full>
            <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. TechCrunch RSS" />
          </FormField>

          <FormField label="Description" full>
            <Textarea rows={3} value={description} onChange={(e) => setDescription(e.target.value)} placeholder="What this source collects" />
          </FormField>

          <FormField label="Kind">
            <Select value={kind} onChange={(e) => setKind(e.target.value as SourceKind)}>
              {SOURCE_KIND_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>{option.label}</option>
              ))}
            </Select>
          </FormField>

          <FormField label="Cadence (min)" hint="1–1440">
            <Input type="number" min={1} max={1440} value={cadence} onChange={(e) => setCadence(Number(e.target.value))} />
          </FormField>

          <FormField label="Endpoint URL" full>
            <Input value={endpointUrl} onChange={(e) => setEndpointUrl(e.target.value)} placeholder="https://..." />
          </FormField>

          <FormField label="Credential ID" full>
            <Input value={credentialId} onChange={(e) => setCredentialId(e.target.value)} placeholder="optional" />
          </FormField>

          <FormField label="Header name">
            <Input value={credentialHeaderName} onChange={(e) => setCredentialHeaderName(e.target.value)} />
          </FormField>

          <FormField label="Header prefix">
            <Input value={credentialHeaderPrefix} onChange={(e) => setCredentialHeaderPrefix(e.target.value)} />
          </FormField>

          <FormField label="Trust score" hint="0–100">
            <Input type="number" min={0} max={100} value={trustScore} onChange={(e) => setTrustScore(Number(e.target.value))} />
          </FormField>

          <FormField label="Tags" full hint="Comma-separated">
            <Input value={tagsText} onChange={(e) => setTagsText(e.target.value)} />
          </FormField>

          <CheckboxField label="Enabled" checked={enabled} onChange={setEnabled} full />

          <FormField label="" full>
            <Button type="submit">Register Source</Button>
          </FormField>
        </form>
        <StatusLine>{status}</StatusLine>
      </Panel>

      <Panel span={7} eyebrow="Active" title="Sources" actions={<Badge tone="accent">{sources.length}</Badge>}>
        {sources.length === 0 ? (
          <div className="hx-table-empty"><p>No sources registered yet.</p></div>
        ) : (
          <div className="hx-card-grid">
            {sources.map((source) => (
              <div key={source.id} className="hx-card">
                <div className="hx-card-head">
                  <h3>{source.name}</h3>
                  <Badge tone={source.enabled ? "ok" : "warn"}>
                    {source.enabled ? "enabled" : "paused"}
                  </Badge>
                </div>
                <p className="hx-row-secondary">{source.description}</p>
                <p className="hx-mono-detail">{source.id}</p>
                <p className="hx-mono-detail">profile: {source.profile_id}</p>
                {source.endpoint_url && <p className="hx-mono-detail">{source.endpoint_url}</p>}
                {source.kind === "webhook_ingest" && (
                  <p className="hx-mono-detail">webhook: /api/v1/sources/{source.id}/webhook</p>
                )}
                {source.kind === "file_import" && (
                  <p className="hx-mono-detail">file import: /api/v1/sources/{source.id}/import</p>
                )}
                <div className="hx-tag-row">
                  <Tag>kind: {source.kind}</Tag>
                  <Tag>cadence: {source.cadence_minutes}m</Tag>
                  <Tag>trust: {source.trust_score}</Tag>
                  {source.credential_id && (
                    <Tag>credential: {source.credential_id}</Tag>
                  )}
                </div>
                <div className="hx-tag-row">
                  {source.tags.map((tag) => (
                    <Tag key={tag}>{tag}</Tag>
                  ))}
                </div>
                <Button
                  variant="secondary"
                  type="button"
                  disabled={
                    !supportsPullCollection(source) ||
                    !source.endpoint_url ||
                    collectingSourceId === source.id
                  }
                  onClick={() => void onCollect(source)}
                >
                  {supportsPullCollection(source)
                    ? collectingSourceId === source.id
                      ? "Collecting..."
                      : "Collect Now"
                    : "Push Ingest"}
                </Button>
              </div>
            ))}
          </div>
        )}
      </Panel>
    </section>
  );
}

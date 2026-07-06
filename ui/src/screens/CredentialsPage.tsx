import { FormEvent, useEffect, useMemo, useState } from "react";
import {
  CredentialMetadataEntry,
  deleteCredential,
  fetchCredentials,
  upsertCredential,
} from "../lib/api";
import {
  Panel,
  FormField,
  Input,
  Textarea,
  Select,
  Button,
  Badge,
  Tag,
  StatusLine,
} from "../components";

const DEFAULT_PROFILE_ID = "50000000-0000-0000-0000-000000000010";
const DEFAULT_METADATA = JSON.stringify({ provider: "github", scope: "repo:read" }, null, 2);

function parseMetadata(value: string): Record<string, string> {
  const trimmed = value.trim();
  if (!trimmed) return {};
  const parsed = JSON.parse(trimmed) as unknown;
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("metadata must be a JSON object");
  }
  const metadata: Record<string, string> = {};
  for (const [key, item] of Object.entries(parsed)) {
    if (typeof item !== "string") {
      throw new Error(`metadata.${key} must be a string`);
    }
    metadata[key] = item;
  }
  return metadata;
}

function createdLabel(entry: CredentialMetadataEntry): string {
  return entry.updated_at === entry.created_at
    ? `created: ${entry.created_at}`
    : `updated: ${entry.updated_at}`;
}

export function CredentialsPage() {
  const [profileId, setProfileId] = useState(DEFAULT_PROFILE_ID);
  const [credentials, setCredentials] = useState<CredentialMetadataEntry[]>([]);
  const [persistenceEnabled, setPersistenceEnabled] = useState(false);
  const [status, setStatus] = useState("Loading credential vault...");
  const [name, setName] = useState("GitHub Token");
  const [kind, setKind] = useState("api_key");
  const [secret, setSecret] = useState("");
  const [metadataText, setMetadataText] = useState(DEFAULT_METADATA);
  const [submitting, setSubmitting] = useState(false);
  const [deletingId, setDeletingId] = useState<string | null>(null);

  const metadataKeys = useMemo(() => {
    try {
      return Object.keys(parseMetadata(metadataText));
    } catch {
      return [];
    }
  }, [metadataText]);

  async function loadCredentials(nextProfileId = profileId) {
    setStatus("Loading credential vault...");
    try {
      const response = await fetchCredentials(nextProfileId.trim());
      setCredentials(response.credentials);
      setPersistenceEnabled(response.persistence_enabled);
      setStatus(
        response.persistence_enabled
          ? `Loaded ${response.credentials.length} redacted credential record(s).`
          : "Credential vault is disabled because DATABASE_URL is not configured."
      );
    } catch (error) {
      setStatus(`Failed to load credential vault: ${(error as Error).message}`);
    }
  }

  useEffect(() => {
    void loadCredentials(DEFAULT_PROFILE_ID);
  }, []);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setSubmitting(true);
    setStatus("Saving encrypted credential...");
    try {
      const saved = await upsertCredential({
        profile_id: profileId.trim(),
        name: name.trim(),
        kind: kind.trim(),
        secret,
        metadata: parseMetadata(metadataText),
      });
      setSecret("");
      if (saved) {
        setCredentials((current) =>
          [saved, ...current.filter((entry) => entry.id !== saved.id)].sort((left, right) =>
            left.name.localeCompare(right.name) || left.id.localeCompare(right.id)
          )
        );
        setPersistenceEnabled(true);
        setStatus(`Saved ${saved.name}; secret accepted and redacted.`);
      } else {
        setStatus("Credential vault did not return a record.");
      }
    } catch (error) {
      setStatus(`Save failed: ${(error as Error).message}`);
    } finally {
      setSubmitting(false);
    }
  }

  async function onDelete(entry: CredentialMetadataEntry) {
    setDeletingId(entry.id);
    setStatus(`Deleting ${entry.name}...`);
    try {
      const response = await deleteCredential(entry.profile_id, entry.id);
      if (response.deleted) {
        setCredentials((current) => current.filter((item) => item.id !== entry.id));
        setPersistenceEnabled(response.persistence_enabled);
        setStatus(`Deleted ${entry.name}.`);
      } else {
        setStatus(`Delete did not remove ${entry.name}.`);
      }
    } catch (error) {
      setStatus(`Delete failed: ${(error as Error).message}`);
    } finally {
      setDeletingId(null);
    }
  }

  return (
    <section className="hx-page-grid">
      <Panel hero span={12} eyebrow="Credential Vault" title="Encrypted Access Material With Redacted Operator Views">
        <p className="hx-description">
          Store connector credentials behind the same profile boundary used by recipes,
          agents, and deterministic automation.
        </p>
      </Panel>

      <Panel
        span={5}
        eyebrow="Vault Write"
        title="Save Credential"
        actions={<Badge tone={persistenceEnabled ? "ok" : "warn"}>{persistenceEnabled ? "durable" : "disabled"}</Badge>}
      >
        <form className="hx-form-grid" onSubmit={onSubmit}>
          <FormField label="Profile ID" full>
            <Input value={profileId} onChange={(e) => setProfileId(e.target.value)} />
          </FormField>

          <FormField label="Name">
            <Input value={name} onChange={(e) => setName(e.target.value)} />
          </FormField>

          <FormField label="Kind">
            <Select value={kind} onChange={(e) => setKind(e.target.value)}>
              <option value="api_key">api_key</option>
              <option value="oauth2">oauth2</option>
              <option value="bearer_token">bearer_token</option>
              <option value="webhook_secret">webhook_secret</option>
            </Select>
          </FormField>

          <FormField label="Secret" full>
            <Input
              type="password"
              autoComplete="off"
              value={secret}
              onChange={(e) => setSecret(e.target.value)}
              placeholder="paste secret"
            />
          </FormField>

          <FormField label="Metadata JSON" full>
            <Textarea rows={5} value={metadataText} onChange={(e) => setMetadataText(e.target.value)} />
          </FormField>

          <div className="hx-cluster" style={{ gridColumn: "1 / -1" }}>
            <Button type="submit" disabled={submitting}>
              {submitting ? "Saving..." : "Save Credential"}
            </Button>
            <Button variant="secondary" type="button" onClick={() => void loadCredentials(profileId)}>
              Refresh
            </Button>
          </div>
        </form>

        <div className="hx-tag-row" style={{ marginTop: "var(--hx-space-3)" }}>
          {metadataKeys.length === 0 ? (
            <Tag>metadata: none</Tag>
          ) : (
            metadataKeys.map((key) => (
              <Tag key={key}>metadata: {key}</Tag>
            ))
          )}
        </div>
        <StatusLine>{status}</StatusLine>
      </Panel>

      <Panel
        span={7}
        eyebrow="Stored"
        title="Redacted Credentials"
        actions={<Badge tone="info">{credentials.length} record(s)</Badge>}
      >
        <StatusLine>profile: {profileId || "unset"}</StatusLine>
        {credentials.length === 0 ? (
          <div className="hx-table-empty"><p>No credential metadata is available for this profile.</p></div>
        ) : (
          <div className="hx-card-grid">
            {credentials.map((entry) => (
              <div key={entry.id} className="hx-card">
                <div className="hx-card-head">
                  <h3>{entry.name}</h3>
                  <Badge tone="ok">redacted</Badge>
                </div>
                <p className="hx-mono-detail">{entry.id}</p>
                <div className="hx-tag-row">
                  <Tag>kind: {entry.kind}</Tag>
                  <Tag>{createdLabel(entry)}</Tag>
                </div>
                <div className="hx-tag-row">
                  {Object.keys(entry.metadata).length === 0 ? (
                    <Tag>metadata: none</Tag>
                  ) : (
                    Object.entries(entry.metadata).map(([key, value]) => (
                      <Tag key={key}>{key}: {value}</Tag>
                    ))
                  )}
                </div>
                <Button
                  variant="danger"
                  type="button"
                  disabled={deletingId === entry.id}
                  onClick={() => void onDelete(entry)}
                >
                  {deletingId === entry.id ? "Deleting..." : "Delete"}
                </Button>
              </div>
            ))}
          </div>
        )}
      </Panel>
    </section>
  );
}

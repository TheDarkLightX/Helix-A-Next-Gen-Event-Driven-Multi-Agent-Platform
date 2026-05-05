import { FormEvent, useEffect, useMemo, useState } from "react";
import {
  CredentialMetadataEntry,
  deleteCredential,
  fetchCredentials,
  upsertCredential,
} from "../lib/api";

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
    <section className="dashboard-grid">
      <article className="panel panel-hero panel-span-12">
        <p className="mono-label">Credential Vault</p>
        <h2>Encrypted Access Material With Redacted Operator Views</h2>
        <p>
          Store connector credentials behind the same profile boundary used by recipes, agents, and
          deterministic automation.
        </p>
      </article>

      <article className="panel panel-span-5">
        <div className="panel-toolbar">
          <div>
            <p className="mono-label">Vault Write</p>
            <p className="status-line">{status}</p>
          </div>
          <span className={`status-pill ${persistenceEnabled ? "ok" : "warn"}`}>
            {persistenceEnabled ? "durable" : "disabled"}
          </span>
        </div>

        <form className="form-grid" onSubmit={onSubmit}>
          <label className="field field-full">
            <span>profile_id</span>
            <input value={profileId} onChange={(event) => setProfileId(event.target.value)} />
          </label>

          <label className="field">
            <span>name</span>
            <input value={name} onChange={(event) => setName(event.target.value)} />
          </label>

          <label className="field">
            <span>kind</span>
            <select value={kind} onChange={(event) => setKind(event.target.value)}>
              <option value="api_key">api_key</option>
              <option value="oauth2">oauth2</option>
              <option value="bearer_token">bearer_token</option>
              <option value="webhook_secret">webhook_secret</option>
            </select>
          </label>

          <label className="field field-full">
            <span>secret</span>
            <input
              type="password"
              autoComplete="off"
              value={secret}
              onChange={(event) => setSecret(event.target.value)}
            />
          </label>

          <label className="field field-full">
            <span>metadata_json</span>
            <textarea
              rows={5}
              value={metadataText}
              onChange={(event) => setMetadataText(event.target.value)}
            />
          </label>

          <div className="button-row field-full">
            <button className="btn-primary" type="submit" disabled={submitting}>
              {submitting ? "Saving" : "Save Credential"}
            </button>
            <button
              className="btn-secondary"
              type="button"
              onClick={() => void loadCredentials(profileId)}
            >
              Refresh
            </button>
          </div>
        </form>

        <div className="pill-row">
          {metadataKeys.length === 0 ? (
            <span className="info-pill">metadata: none</span>
          ) : (
            metadataKeys.map((key) => (
              <span key={key} className="info-pill">
                metadata: {key}
              </span>
            ))
          )}
        </div>
      </article>

      <article className="panel panel-span-7">
        <div className="panel-toolbar">
          <div>
            <p className="mono-label">Redacted Credentials</p>
            <p className="status-line">profile: {profileId || "unset"}</p>
          </div>
          <span className="status-pill info">{credentials.length} record(s)</span>
        </div>

        <div className="agent-grid">
          {credentials.length === 0 ? (
            <p className="panel-note">No credential metadata is available for this profile.</p>
          ) : (
            credentials.map((entry) => (
              <div key={entry.id} className="agent-card">
                <div className="agent-card-head">
                  <h3>{entry.name}</h3>
                  <span className="status-pill ok">redacted</span>
                </div>
                <p className="mono-detail">{entry.id}</p>
                <div className="pill-row">
                  <span className="info-pill">kind: {entry.kind}</span>
                  <span className="info-pill">{createdLabel(entry)}</span>
                </div>
                <div className="pill-row">
                  {Object.keys(entry.metadata).length === 0 ? (
                    <span className="tag-chip">metadata: none</span>
                  ) : (
                    Object.entries(entry.metadata).map(([key, value]) => (
                      <span key={key} className="tag-chip">
                        {key}: {value}
                      </span>
                    ))
                  )}
                </div>
                <button
                  className="btn-secondary"
                  type="button"
                  disabled={deletingId === entry.id}
                  onClick={() => void onDelete(entry)}
                >
                  {deletingId === entry.id ? "Deleting" : "Delete"}
                </button>
              </div>
            ))
          )}
        </div>
      </article>
    </section>
  );
}

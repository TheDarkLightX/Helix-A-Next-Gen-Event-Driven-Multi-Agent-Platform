import { useEffect, useState } from "react";
import { AuditLogEntry, fetchAuditLog } from "../lib/api";

function metadataPreview(metadata: unknown): string {
  try {
    return JSON.stringify(metadata, null, 2);
  } catch {
    return "{}";
  }
}

export function AuditPage() {
  const [entries, setEntries] = useState<AuditLogEntry[]>([]);
  const [persistenceEnabled, setPersistenceEnabled] = useState<boolean>(false);
  const [status, setStatus] = useState<string>("Loading audit timeline...");

  async function loadAuditLog() {
    try {
      const response = await fetchAuditLog(100);
      setEntries(response.entries);
      setPersistenceEnabled(response.persistence_enabled);
      setStatus(
        response.persistence_enabled
          ? `Loaded ${response.entries.length} durable audit event(s).`
          : "Audit persistence is disabled because DATABASE_URL is not configured."
      );
    } catch (error) {
      setStatus(`Failed to load audit timeline: ${(error as Error).message}`);
    }
  }

  useEffect(() => {
    void loadAuditLog();
  }, []);

  return (
    <section className="dashboard-grid">
      <article className="panel panel-hero panel-span-12">
        <p className="mono-label">Audit Timeline</p>
        <h2>Durable Operator Decisions</h2>
        <p>
          Review persisted source collection, evidence, policy, and autopilot guard decisions from
          the Postgres audit log.
        </p>
      </article>

      <article className="panel panel-span-12">
        <div className="panel-toolbar">
          <div>
            <p className="mono-label">Audit Store</p>
            <p className="status-line">{status}</p>
          </div>
          <div className="toolbar-actions">
            <span className={`status-pill ${persistenceEnabled ? "ok" : "warn"}`}>
              {persistenceEnabled ? "durable" : "in-memory mode"}
            </span>
            <button className="btn-secondary" type="button" onClick={() => void loadAuditLog()}>
              Refresh
            </button>
          </div>
        </div>

        <div className="command-list">
          {entries.length === 0 ? (
            <p className="panel-note">No audit records are available for the current store.</p>
          ) : (
            entries.map((entry) => (
              <div key={entry.id} className="command-row">
                <div>
                  <div className="command-row-head">
                    <h3>{entry.action}</h3>
                    <span className={`status-pill ${entry.decision === "allow" ? "ok" : "warn"}`}>
                      {entry.decision}
                    </span>
                  </div>
                  <p>{entry.resource}</p>
                  <div className="pill-row">
                    <span className="info-pill">subject: {entry.subject}</span>
                    <span className="info-pill">created: {entry.created_at}</span>
                    {entry.reason ? <span className="info-pill">reason: {entry.reason}</span> : null}
                  </div>
                  <pre className="json-block">{metadataPreview(entry.metadata)}</pre>
                </div>
              </div>
            ))
          )}
        </div>
      </article>
    </section>
  );
}

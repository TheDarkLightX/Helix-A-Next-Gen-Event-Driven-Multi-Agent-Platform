import { useEffect, useState } from "react";
import { AuditLogEntry, fetchAuditLog } from "../lib/api";
import { Panel, Badge, Button, StatusLine, CodeBlock, Tag } from "../components";

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
    <section className="hx-page-grid">
      <Panel hero span={12} eyebrow="Audit Timeline" title="Durable Operator Decisions">
        <p className="hx-description">
          Review persisted source collection, evidence, policy, and autopilot guard
          decisions from the Postgres audit log.
        </p>
      </Panel>

      <Panel
        span={12}
        eyebrow="Store"
        title="Audit Log"
        actions={
          <>
            <Badge tone={persistenceEnabled ? "ok" : "warn"}>
              {persistenceEnabled ? "durable" : "in-memory"}
            </Badge>
            <Button variant="secondary" type="button" onClick={() => void loadAuditLog()}>
              Refresh
            </Button>
          </>
        }
      >
        <StatusLine>{status}</StatusLine>
        {entries.length === 0 ? (
          <div className="hx-table-empty"><p>No audit records available for the current store.</p></div>
        ) : (
          <div className="hx-list">
            {entries.map((entry) => (
              <div key={entry.id} className="hx-card">
                <div className="hx-card-head">
                  <h3>{entry.action}</h3>
                  <Badge tone={entry.decision === "allow" ? "ok" : "warn"}>{entry.decision}</Badge>
                </div>
                <p className="hx-row-secondary">{entry.resource}</p>
                <div className="hx-tag-row">
                  <Tag>subject: {entry.subject}</Tag>
                  <Tag>created: {entry.created_at}</Tag>
                  {entry.reason && <Tag>reason: {entry.reason}</Tag>}
                </div>
                <CodeBlock label="metadata" maxHeight="180px">{metadataPreview(entry.metadata)}</CodeBlock>
              </div>
            ))}
          </div>
        )}
      </Panel>
    </section>
  );
}

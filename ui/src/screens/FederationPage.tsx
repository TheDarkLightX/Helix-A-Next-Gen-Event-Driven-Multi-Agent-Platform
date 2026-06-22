import { FormEvent, useEffect, useState } from "react";
import {
  PeerDesk,
  DispatchLogEntry,
  FederationOverview,
  fetchFederationOverview,
  fetchPeers,
  upsertPeer,
  deletePeer,
  fetchDispatchLog,
  manualBroadcast,
} from "../lib/api";
import {
  Panel,
  StatCard,
  StatGrid,
  Badge,
  Button,
  FormField,
  Input,
  Textarea,
  CheckboxField,
  DataTable,
  EmptyState,
  LoadingState,
  ErrorState,
  StatusLine,
} from "../components";

type BadgeTone = "default" | "ok" | "warn" | "danger" | "info" | "neutral" | "accent";

function parseCsv(value: string): string[] {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function dispatchStatusTone(status: string): BadgeTone {
  switch (status) {
    case "delivered":
      return "ok";
    case "failed":
      return "danger";
    case "unreachable":
      return "warn";
    default:
      return "neutral";
  }
}

function trustTone(score: number): BadgeTone {
  if (score >= 80) return "ok";
  if (score >= 50) return "warn";
  return "danger";
}

export function FederationPage() {
  const [overview, setOverview] = useState<FederationOverview | null>(null);
  const [peers, setPeers] = useState<PeerDesk[]>([]);
  const [logEntries, setLogEntries] = useState<DispatchLogEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [statusMsg, setStatusMsg] = useState("");

  // Form state for peer upsert
  const [editingPeer, setEditingPeer] = useState<PeerDesk | null>(null);
  const [peerId, setPeerId] = useState("");
  const [peerName, setPeerName] = useState("");
  const [peerUrl, setPeerUrl] = useState("");
  const [peerToken, setPeerToken] = useState("");
  const [peerTrust, setPeerTrust] = useState("80");
  const [peerEnabled, setPeerEnabled] = useState(true);
  const [peerTags, setPeerTags] = useState("");

  // Broadcast form
  const [broadcastTitle, setBroadcastTitle] = useState("");
  const [broadcastSummary, setBroadcastSummary] = useState("");
  const [broadcastContent, setBroadcastContent] = useState("");
  const [broadcastTags, setBroadcastTags] = useState("");

  async function loadAll() {
    setLoading(true);
    setError(null);
    try {
      const [ov, p, log] = await Promise.all([
        fetchFederationOverview(),
        fetchPeers(),
        fetchDispatchLog(50),
      ]);
      setOverview(ov);
      setPeers(p);
      setLogEntries(log);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load federation data");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadAll();
  }, []);

  function resetPeerForm() {
    setEditingPeer(null);
    setPeerId("");
    setPeerName("");
    setPeerUrl("");
    setPeerToken("");
    setPeerTrust("80");
    setPeerEnabled(true);
    setPeerTags("");
  }

  function startEditPeer(peer: PeerDesk) {
    setEditingPeer(peer);
    setPeerId(peer.id);
    setPeerName(peer.name);
    setPeerUrl(peer.endpoint_url);
    setPeerToken(peer.auth_token);
    setPeerTrust(String(peer.trust_score));
    setPeerEnabled(peer.enabled);
    setPeerTags(peer.tags.join(", "));
  }

  async function handlePeerSubmit(e: FormEvent) {
    e.preventDefault();
    setStatusMsg("");
    try {
      await upsertPeer({
        id: peerId.trim(),
        name: peerName.trim(),
        endpoint_url: peerUrl.trim(),
        auth_token: peerToken.trim(),
        trust_score: parseInt(peerTrust, 10) || 0,
        enabled: peerEnabled,
        tags: parseCsv(peerTags),
      });
      setStatusMsg(editingPeer ? "Peer updated." : "Peer registered.");
      resetPeerForm();
      await loadAll();
    } catch (err) {
      setStatusMsg(err instanceof Error ? err.message : "Failed to save peer");
    }
  }

  async function handleDeletePeer(peerId: string) {
    if (!confirm(`Remove peer "${peerId}"?`)) return;
    setStatusMsg("");
    try {
      await deletePeer(peerId);
      setStatusMsg(`Peer ${peerId} removed.`);
      await loadAll();
    } catch (err) {
      setStatusMsg(err instanceof Error ? err.message : "Failed to delete peer");
    }
  }

  async function handleBroadcast(e: FormEvent) {
    e.preventDefault();
    setStatusMsg("");
    try {
      const result = await manualBroadcast({
        title: broadcastTitle.trim(),
        summary: broadcastSummary.trim(),
        content: broadcastContent.trim(),
        tags: parseCsv(broadcastTags),
      });
      setStatusMsg(`Broadcast sent to ${result.dispatch_count} peer(s).`);
      setBroadcastTitle("");
      setBroadcastSummary("");
      setBroadcastContent("");
      setBroadcastTags("");
      await loadAll();
    } catch (err) {
      setStatusMsg(err instanceof Error ? err.message : "Failed to broadcast");
    }
  }

  if (loading) return <LoadingState title="Loading federation status..." />;
  if (error)
    return (
      <ErrorState
        title="Failed to load federation data"
        description={error}
      >
        <Button onClick={loadAll}>Retry</Button>
      </ErrorState>
    );

  const fedStatus = overview?.status;

  return (
    <div className="hx-page">
      <header className="hx-page-header">
        <h1>Federation</h1>
        <p className="hx-page-subtitle">
          Network Helix intelligence desks together. Share intelligence, coordinate cases, and
          swarm investigations across trusted peers.
        </p>
      </header>

      {fedStatus && (
        <StatGrid>
          <StatCard
            label="Desk ID"
            value={fedStatus.desk_id}
          />
          <StatCard
            label="Peers"
            value={String(fedStatus.peer_count)}
            sublabel={`${fedStatus.enabled_peer_count} enabled`}
          />
          <StatCard
            label="Total Dispatches"
            value={String(fedStatus.total_dispatches)}
          />
          <StatCard
            label="Federation"
            value={fedStatus.federation_enabled ? "Active" : "Inactive"}
            tone={fedStatus.federation_enabled ? "ok" : "default"}
          />
        </StatGrid>
      )}

      {fedStatus && (
        <Panel title="Federation Status">
          <div className="hx-stat-row">
            <span>Successful dispatches</span>
            <Badge tone="ok">{fedStatus.successful_dispatches}</Badge>
          </div>
          <div className="hx-stat-row">
            <span>Failed dispatches</span>
            <Badge tone="danger">{fedStatus.failed_dispatches}</Badge>
          </div>
          <div className="hx-stat-row">
            <span>Federation enabled</span>
            <Badge tone={fedStatus.federation_enabled ? "ok" : "neutral"}>
              {fedStatus.federation_enabled ? "Yes" : "No"}
            </Badge>
          </div>
        </Panel>
      )}

      <Panel title={editingPeer ? "Edit Peer Desk" : "Register Peer Desk"}>
        <form onSubmit={handlePeerSubmit} className="hx-form">
          <FormField label="Peer ID" hint="Unique slug (lowercase, digits, hyphens)">
            <Input
              value={peerId}
              onChange={(e) => setPeerId(e.target.value)}
              placeholder="desk-beta"
              disabled={!!editingPeer}
              required
            />
          </FormField>
          <FormField label="Name">
            <Input
              value={peerName}
              onChange={(e) => setPeerName(e.target.value)}
              placeholder="Beta Desk"
              required
            />
          </FormField>
          <FormField label="Endpoint URL" hint="Base URL of the remote Helix instance">
            <Input
              value={peerUrl}
              onChange={(e) => setPeerUrl(e.target.value)}
              placeholder="https://helix-beta.example.com"
              required
            />
          </FormField>
          <FormField label="Auth Token" hint="Bearer token for outbound dispatches">
            <Input
              type="password"
              value={peerToken}
              onChange={(e) => setPeerToken(e.target.value)}
              placeholder="secret-token-..."
              required
            />
          </FormField>
          <FormField label="Trust Score (0-100)">
            <Input
              type="number"
              min={0}
              max={100}
              value={peerTrust}
              onChange={(e) => setPeerTrust(e.target.value)}
              required
            />
          </FormField>
          <FormField label="Tags (comma-separated)">
            <Input
              value={peerTags}
              onChange={(e) => setPeerTags(e.target.value)}
              placeholder="osint, market"
            />
          </FormField>
          <CheckboxField
            label="Enabled"
            checked={peerEnabled}
            onChange={setPeerEnabled}
          />
          <div className="hx-form-actions">
            <Button type="submit">{editingPeer ? "Update Peer" : "Register Peer"}</Button>
            {editingPeer && (
              <Button type="button" variant="ghost" onClick={resetPeerForm}>
                Cancel
              </Button>
            )}
          </div>
        </form>
      </Panel>

      <Panel title={`Peer Desks (${peers.length})`}>
        {peers.length === 0 ? (
          <EmptyState title="No peer desks registered" description="Add one above to start swarming." />
        ) : (
          <DataTable
            columns={[
              {
                key: "id",
                header: "ID",
                render: (p) => <code>{p.id}</code>,
              },
              {
                key: "name",
                header: "Name",
                render: (p) => p.name,
              },
              {
                key: "endpoint",
                header: "Endpoint",
                render: (p) => <code>{p.endpoint_url}</code>,
              },
              {
                key: "trust",
                header: "Trust",
                render: (p) => <Badge tone={trustTone(p.trust_score)}>{p.trust_score}</Badge>,
              },
              {
                key: "enabled",
                header: "Status",
                render: (p) => (
                  <Badge tone={p.enabled ? "ok" : "neutral"}>
                    {p.enabled ? "Enabled" : "Disabled"}
                  </Badge>
                ),
              },
              {
                key: "actions",
                header: "Actions",
                render: (p) => (
                  <div className="hx-row-gap">
                    <Button
                      type="button"
                      variant="ghost"
                      onClick={() => startEditPeer(p)}
                    >
                      Edit
                    </Button>
                    <Button
                      type="button"
                      variant="danger"
                      onClick={() => handleDeletePeer(p.id)}
                    >
                      Remove
                    </Button>
                  </div>
                ),
              },
            ]}
            rows={peers}
            rowKey={(p) => p.id}
            emptyMessage="No peers registered"
          />
        )}
      </Panel>

      <Panel title="Manual Broadcast">
        <p className="hx-form-hint">
          Send an intelligence bulletin to all enabled peers. The payload is delivered as a
          CloudEvents-formatted federation event.
        </p>
        <form onSubmit={handleBroadcast} className="hx-form">
          <FormField label="Title">
            <Input
              value={broadcastTitle}
              onChange={(e) => setBroadcastTitle(e.target.value)}
              placeholder="Intelligence bulletin title"
              required
            />
          </FormField>
          <FormField label="Summary">
            <Textarea
              value={broadcastSummary}
              onChange={(e) => setBroadcastSummary(e.target.value)}
              placeholder="Brief summary of the intelligence..."
              rows={2}
              required
            />
          </FormField>
          <FormField label="Content">
            <Textarea
              value={broadcastContent}
              onChange={(e) => setBroadcastContent(e.target.value)}
              placeholder="Full details to share with peer desks..."
              rows={4}
              required
            />
          </FormField>
          <FormField label="Tags (comma-separated)">
            <Input
              value={broadcastTags}
              onChange={(e) => setBroadcastTags(e.target.value)}
              placeholder="osint, priority"
            />
          </FormField>
          <div className="hx-form-actions">
            <Button type="submit" disabled={!fedStatus?.federation_enabled}>
              Broadcast to Peers
            </Button>
          </div>
        </form>
      </Panel>

      <Panel title={`Dispatch Log (${logEntries.length})`}>
        {logEntries.length === 0 ? (
          <EmptyState
            title="No outbound dispatches yet"
            description="Federation events will appear here when intelligence is shared with peers."
          />
        ) : (
          <DataTable
            columns={[
              {
                key: "dispatched_at",
                header: "Timestamp",
                render: (e) => <code>{e.dispatched_at}</code>,
                width: "180px",
              },
              {
                key: "event_kind",
                header: "Event Type",
                render: (e) => <code>{e.event_kind}</code>,
              },
              {
                key: "event_title",
                header: "Title",
                render: (e) => e.event_title,
              },
              {
                key: "peer_id",
                header: "Peer",
                render: (e) => <code>{e.peer_id}</code>,
              },
              {
                key: "status",
                header: "Status",
                render: (e) => <Badge tone={dispatchStatusTone(e.status)}>{e.status}</Badge>,
              },
              {
                key: "latency",
                header: "Latency",
                render: (e) => `${e.latency_ms}ms`,
                width: "80px",
              },
              {
                key: "details",
                header: "Details",
                render: (e) =>
                  e.error_message ? (
                    <span className="hx-text-muted">{e.error_message}</span>
                  ) : e.http_status ? (
                    <span className="hx-text-muted">HTTP {e.http_status}</span>
                  ) : (
                    "—"
                  ),
              },
            ]}
            rows={logEntries}
            rowKey={(e) => e.id}
            emptyMessage="No dispatches logged"
          />
        )}
      </Panel>

      {statusMsg && <StatusLine>{statusMsg}</StatusLine>}
    </div>
  );
}

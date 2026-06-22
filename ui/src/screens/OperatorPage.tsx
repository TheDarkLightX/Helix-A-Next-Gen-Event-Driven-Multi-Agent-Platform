import { FormEvent, useEffect, useState } from "react";
import {
  OperatorStatus,
  AutopilotMode,
  OperatorActionScope,
  DeskOperatorConfig,
  OperatorStatusResponse,
  OperatorActivityEntry,
  OperatorSession,
  SessionKind,
  SessionRole,
  ConfirmationRequest,
  fetchOperatorStatus,
  fetchOperatorConfig,
  updateOperatorConfig,
  startOperator,
  stopOperator,
  pauseOperator,
  fetchOperatorActivity,
  joinSession,
  leaveSession,
  listSessions,
  sessionHeartbeat,
  listConfirmations,
  confirmProposal,
  denyProposal,
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
  Select,
  DataTable,
  EmptyState,
  LoadingState,
  ErrorState,
  StatusLine,
} from "../components";

type BadgeTone = "default" | "ok" | "warn" | "danger" | "info" | "neutral" | "accent";
type StatTone = "default" | "accent" | "ok" | "warn" | "danger" | "info";

function statusTone(status: OperatorStatus): BadgeTone {
  switch (status) {
    case "running":
      return "ok";
    case "paused":
      return "warn";
    case "stopped":
    default:
      return "default";
  }
}

function statusStatTone(status: OperatorStatus): StatTone {
  return statusTone(status) as StatTone;
}

function modeTone(mode: AutopilotMode): BadgeTone {
  switch (mode) {
    case "auto":
      return "ok";
    case "assist":
      return "info";
    case "off":
    default:
      return "default";
  }
}

function modeStatTone(mode: AutopilotMode): StatTone {
  return modeTone(mode) as StatTone;
}

function actionTone(entry: OperatorActivityEntry): BadgeTone {
  if (entry.allowed) return "ok";
  return "danger";
}

function sessionStatusTone(s: OperatorSession): BadgeTone {
  switch (s.status) {
    case "active": return "ok";
    case "idle": return "warn";
    case "disconnected": return "danger";
    default: return "neutral";
  }
}

function kindTone(k: SessionKind): BadgeTone {
  return k === "ai" ? "accent" : "info";
}

function confirmationTone(c: ConfirmationRequest): BadgeTone {
  switch (c.status) {
    case "confirmed": return "ok";
    case "denied": return "danger";
    case "expired": return "warn";
    case "pending": default: return "info";
  }
}

export function OperatorPage() {
  const [status, setStatus] = useState<OperatorStatusResponse | null>(null);
  const [config, setConfig] = useState<DeskOperatorConfig | null>(null);
  const [activity, setActivity] = useState<OperatorActivityEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [statusMsg, setStatusMsg] = useState("");

  // CoPilot state
  const [sessions, setSessions] = useState<OperatorSession[]>([]);
  const [humanCount, setHumanCount] = useState(0);
  const [aiCount, setAiCount] = useState(0);
  const [confirmations, setConfirmations] = useState<ConfirmationRequest[]>([]);
  const [mySession, setMySession] = useState<OperatorSession | null>(null);
  const [joinName, setJoinName] = useState("");
  const [joinKind, setJoinKind] = useState<SessionKind>("human");
  const [joinRole, setJoinRole] = useState<SessionRole>("operator");
  const [joinLocation, setJoinLocation] = useState("");

  // Editable config form state
  const [editEnabled, setEditEnabled] = useState(false);
  const [editMode, setEditMode] = useState<AutopilotMode>("assist");
  const [editScope, setEditScope] = useState<OperatorActionScope>("intelligence");
  const [editInterval, setEditInterval] = useState("60");
  const [editMaxActions, setEditMaxActions] = useState("3");
  const [editMaxCtx, setEditMaxCtx] = useState("10");
  const [editModel, setEditModel] = useState("gpt-4o-mini");
  const [editRules, setEditRules] = useState("");
  const [editDispatch, setEditDispatch] = useState(false);
  const [editLogDenied, setEditLogDenied] = useState(true);

  async function loadAll() {
    setLoading(true);
    setError(null);
    try {
      const [st, cfg, act, sess, confs] = await Promise.all([
        fetchOperatorStatus(),
        fetchOperatorConfig(),
        fetchOperatorActivity(50),
        listSessions(),
        listConfirmations(),
      ]);
      setStatus(st);
      setConfig(cfg);
      setActivity(act);
      setSessions(sess.sessions);
      setHumanCount(sess.human_count);
      setAiCount(sess.ai_count);
      setConfirmations(confs.pending);
      // Sync form state with loaded config
      setEditEnabled(cfg.enabled);
      setEditMode(cfg.autopilot_mode);
      setEditScope(cfg.action_scope);
      setEditInterval(String(cfg.loop_interval_secs));
      setEditMaxActions(String(cfg.max_actions_per_cycle));
      setEditMaxCtx(String(cfg.max_context_items));
      setEditModel(cfg.model);
      setEditRules(cfg.rules_text);
      setEditDispatch(cfg.dispatch_to_peers);
      setEditLogDenied(cfg.log_denied_proposals);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load operator data");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadAll();
  }, []);

  async function handleConfigSubmit(e: FormEvent) {
    e.preventDefault();
    setStatusMsg("");
    try {
      const updated = await updateOperatorConfig({
        enabled: editEnabled,
        autopilot_mode: editMode,
        action_scope: editScope,
        loop_interval_secs: parseInt(editInterval, 10) || 60,
        max_actions_per_cycle: parseInt(editMaxActions, 10) || 3,
        max_context_items: parseInt(editMaxCtx, 10) || 10,
        model: editModel.trim(),
        rules_text: editRules,
        dispatch_to_peers: editDispatch,
        log_denied_proposals: editLogDenied,
      });
      setConfig(updated);
      setStatusMsg("Configuration saved.");
      // Refresh status to reflect any mode changes
      const st = await fetchOperatorStatus();
      setStatus(st);
    } catch (err) {
      setStatusMsg(err instanceof Error ? err.message : "Failed to save config");
    }
  }

  async function handleStart() {
    setStatusMsg("");
    try {
      await startOperator();
      const st = await fetchOperatorStatus();
      setStatus(st);
      setStatusMsg("Operator started.");
    } catch (err) {
      setStatusMsg(err instanceof Error ? err.message : "Failed to start operator");
    }
  }

  async function handleStop() {
    setStatusMsg("");
    try {
      await stopOperator();
      const st = await fetchOperatorStatus();
      setStatus(st);
      setStatusMsg("Operator stopped.");
    } catch (err) {
      setStatusMsg(err instanceof Error ? err.message : "Failed to stop operator");
    }
  }

  async function handlePause() {
    setStatusMsg("");
    try {
      await pauseOperator();
      const st = await fetchOperatorStatus();
      setStatus(st);
      setStatusMsg("Operator paused.");
    } catch (err) {
      setStatusMsg(err instanceof Error ? err.message : "Failed to pause operator");
    }
  }

  // ---- CoPilot handlers ----

  async function handleJoin(e: FormEvent) {
    e.preventDefault();
    setStatusMsg("");
    try {
      const session = await joinSession({
        display_name: joinName.trim(),
        kind: joinKind,
        role: joinRole,
        location: joinLocation.trim() || undefined,
      });
      setMySession(session);
      setStatusMsg(`Joined as ${session.display_name}.`);
      setJoinName("");
      setJoinLocation("");
      await loadAll();
    } catch (err) {
      setStatusMsg(err instanceof Error ? err.message : "Failed to join");
    }
  }

  async function handleLeave() {
    if (!mySession) return;
    setStatusMsg("");
    try {
      await leaveSession(mySession.id);
      setMySession(null);
      setStatusMsg("Left the desk.");
      await loadAll();
    } catch (err) {
      setStatusMsg(err instanceof Error ? err.message : "Failed to leave");
    }
  }

  async function handleConfirm(confirmationId: string) {
    if (!mySession) {
      setStatusMsg("You must join the desk first to confirm proposals.");
      return;
    }
    setStatusMsg("");
    try {
      await confirmProposal(confirmationId, mySession.id);
      setStatusMsg("Proposal confirmed.");
      await loadAll();
    } catch (err) {
      setStatusMsg(err instanceof Error ? err.message : "Failed to confirm");
    }
  }

  async function handleDeny(confirmationId: string) {
    if (!mySession) {
      setStatusMsg("You must join the desk first to deny proposals.");
      return;
    }
    setStatusMsg("");
    try {
      await denyProposal(confirmationId, mySession.id);
      setStatusMsg("Proposal denied.");
      await loadAll();
    } catch (err) {
      setStatusMsg(err instanceof Error ? err.message : "Failed to deny");
    }
  }

  // Heartbeat effect — keep our session alive while connected
  useEffect(() => {
    if (!mySession) return;
    const interval = setInterval(async () => {
      try {
        await sessionHeartbeat(mySession.id);
      } catch {
        // Silent fail — heartbeat will retry
      }
    }, 15000); // 15 second heartbeat
    return () => clearInterval(interval);
  }, [mySession]);

  if (loading) return <LoadingState title="Loading operator status..." />;
  if (error)
    return (
      <ErrorState title="Failed to load operator data" description={error}>
        <Button onClick={loadAll}>Retry</Button>
      </ErrorState>
    );

  return (
    <div className="hx-page">
      <header className="hx-page-header">
        <h1>Desk Operator</h1>
        <p className="hx-page-subtitle">
          LLM-driven autonomous intelligence desk operator. The operator runs an
          observe-think-act loop within deterministic guardrails — the LLM never
          bypasses the guard.
        </p>
      </header>

      {status && (
        <StatGrid>
          <StatCard
            label="Status"
            value={status.status}
            tone={statusStatTone(status.status)}
          />
          <StatCard
            label="Autopilot Mode"
            value={status.autopilot_mode}
            tone={modeStatTone(status.autopilot_mode)}
          />
          <StatCard
            label="Cycles"
            value={String(status.cycle_count)}
            sublabel={`${status.activity_log_count} log entries`}
          />
          <StatCard
            label="Model"
            value={status.model}
            sublabel={`${status.loop_interval_secs}s interval`}
          />
        </StatGrid>
      )}

      <Panel title="Operator Controls">
        <div className="hx-stat-row">
          <span>Current status</span>
          <Badge tone={status ? statusTone(status.status) : "neutral"}>
            {status?.status ?? "unknown"}
          </Badge>
        </div>
        <div className="hx-stat-row">
          <span>Enabled in config</span>
          <Badge tone={status?.enabled ? "ok" : "neutral"}>
            {status?.enabled ? "Yes" : "No"}
          </Badge>
        </div>
        <div className="hx-stat-row">
          <span>Action scope</span>
          <Badge tone="info">{status?.action_scope ?? "—"}</Badge>
        </div>
        <div style={{ display: "flex", gap: "0.5rem", marginTop: "1rem" }}>
          <Button
            onClick={handleStart}
            disabled={status?.status === "running"}
          >
            Start
          </Button>
          <Button
            onClick={handlePause}
            disabled={status?.status !== "running"}
          >
            Pause
          </Button>
          <Button
            onClick={handleStop}
            disabled={status?.status === "stopped"}
          >
            Stop
          </Button>
        </div>
      </Panel>

      <Panel title="Operator Configuration">
        <form onSubmit={handleConfigSubmit} className="hx-form">
          <CheckboxField
            label="Enabled — master switch for the operator loop"
            checked={editEnabled}
            onChange={setEditEnabled}
          />
          <FormField label="Autopilot Mode" hint="Off=deny all, Assist=require confirmation, Auto=execute within guardrails">
            <Select
              value={editMode}
              onChange={(e) => setEditMode(e.target.value as AutopilotMode)}
            >
              <option value="off">Off</option>
              <option value="assist">Assist</option>
              <option value="auto">Auto</option>
            </Select>
          </FormField>
          <FormField label="Action Scope" hint="Observe-only, Intelligence, Policy, or Full authority">
            <Select
              value={editScope}
              onChange={(e) => setEditScope(e.target.value as OperatorActionScope)}
            >
              <option value="observe_only">Observe Only</option>
              <option value="intelligence">Intelligence</option>
              <option value="policy">Policy</option>
              <option value="full">Full</option>
            </Select>
          </FormField>
          <FormField label="Loop Interval (seconds)" hint="Minimum 5, maximum 3600">
            <Input
              type="number"
              value={editInterval}
              onChange={(e) => setEditInterval(e.target.value)}
              min={5}
              max={3600}
              required
            />
          </FormField>
          <FormField label="Max Actions Per Cycle" hint="Maximum 10">
            <Input
              type="number"
              value={editMaxActions}
              onChange={(e) => setEditMaxActions(e.target.value)}
              min={1}
              max={10}
              required
            />
          </FormField>
          <FormField label="Max Context Items" hint="Maximum 20 items in LLM prompt">
            <Input
              type="number"
              value={editMaxCtx}
              onChange={(e) => setEditMaxCtx(e.target.value)}
              min={1}
              max={20}
              required
            />
          </FormField>
          <FormField label="LLM Model" hint="Model identifier (e.g. gpt-4o-mini)">
            <Input
              value={editModel}
              onChange={(e) => setEditModel(e.target.value)}
              placeholder="gpt-4o-mini"
              required
            />
          </FormField>
          <FormField label="Rules Text" hint="Natural language instructions for the operator (max 4096 chars)">
            <Textarea
              value={editRules}
              onChange={(e) => setEditRules(e.target.value)}
              rows={6}
              placeholder="Prioritize cases about entity X. Escalate when trust score drops below 50..."
            />
          </FormField>
          <CheckboxField
            label="Dispatch to Federation Peers — share operator decisions with federation peers"
            checked={editDispatch}
            onChange={setEditDispatch}
          />
          <CheckboxField
            label="Log Denied Proposals — record denied proposals in the activity log for audit"
            checked={editLogDenied}
            onChange={setEditLogDenied}
          />
          <div style={{ marginTop: "1rem" }}>
            <Button type="submit">Save Configuration</Button>
          </div>
        </form>
      </Panel>

      <Panel title="Activity Log">
        {activity.length === 0 ? (
          <EmptyState
            title="No activity yet"
            description="Operator decisions will appear here once the loop runs."
          />
        ) : (
          <DataTable
            rowKey={(entry) => entry.id}
            columns={[
              { key: "cycle", header: "Cycle", render: (e) => String(e.cycle) },
              { key: "action_type", header: "Action", render: (e) => e.action_type },
              { key: "rationale", header: "Rationale", render: (e) => e.rationale },
              {
                key: "allowed",
                header: "Decision",
                render: (e) => (
                  <Badge tone={actionTone(e)}>
                    {e.allowed ? "Allowed" : "Denied"}
                  </Badge>
                ),
              },
              {
                key: "denial_reason",
                header: "Reason",
                render: (e) => e.denial_reason ?? "—",
              },
              { key: "timestamp", header: "Timestamp", render: (e) => e.timestamp },
            ]}
            rows={activity}
          />
        )}
      </Panel>

      {/* CoPilot Mode: Multi-participant collaboration */}

      <Panel title="CoPilot — Join the Desk">
        {mySession ? (
          <div>
            <div className="hx-stat-row">
              <span>You are connected as</span>
              <Badge tone={kindTone(mySession.kind)}>
                {mySession.display_name} ({mySession.kind})
              </Badge>
            </div>
            <div className="hx-stat-row">
              <span>Role</span>
              <Badge tone="info">{mySession.role}</Badge>
            </div>
            <div className="hx-stat-row">
              <span>Status</span>
              <Badge tone={sessionStatusTone(mySession)}>{mySession.status}</Badge>
            </div>
            <div style={{ marginTop: "1rem" }}>
              <Button onClick={handleLeave}>Leave Desk</Button>
            </div>
          </div>
        ) : (
          <form onSubmit={handleJoin} className="hx-form">
            <FormField label="Display Name" hint="Your name as it appears to other operators">
              <Input
                value={joinName}
                onChange={(e) => setJoinName(e.target.value)}
                placeholder="Alice"
                required
              />
            </FormField>
            <FormField label="Kind" hint="Human or AI copilot">
              <Select
                value={joinKind}
                onChange={(e) => setJoinKind(e.target.value as SessionKind)}
              >
                <option value="human">Human</option>
                <option value="ai">AI Copilot</option>
              </Select>
            </FormField>
            <FormField label="Role" hint="Viewer=observe only, Operator=can confirm, Admin=full control">
              <Select
                value={joinRole}
                onChange={(e) => setJoinRole(e.target.value as SessionRole)}
              >
                <option value="viewer">Viewer</option>
                <option value="operator">Operator</option>
                <option value="admin">Admin</option>
              </Select>
            </FormField>
            <FormField label="Location" hint="Where you're operating from (optional)">
              <Input
                value={joinLocation}
                onChange={(e) => setJoinLocation(e.target.value)}
                placeholder="Tokyo"
              />
            </FormField>
            <div style={{ marginTop: "1rem" }}>
              <Button type="submit">Join Desk</Button>
            </div>
          </form>
        )}
      </Panel>

      <Panel title={`CoPilot — Active Participants (${sessions.length})`}>
        {sessions.length === 0 ? (
          <EmptyState
            title="No one connected"
            description="Join the desk to start collaborating. Humans and AI copilots can work together in real time."
          />
        ) : (
          <DataTable
            rowKey={(s) => s.id}
            columns={[
              { key: "display_name", header: "Name", render: (s) => s.display_name },
              {
                key: "kind",
                header: "Kind",
                render: (s) => <Badge tone={kindTone(s.kind)}>{s.kind}</Badge>,
              },
              { key: "role", header: "Role", render: (s) => s.role },
              {
                key: "status",
                header: "Status",
                render: (s) => (
                  <Badge tone={sessionStatusTone(s)}>{s.status}</Badge>
                ),
              },
              { key: "location", header: "Location", render: (s) => s.location ?? "—" },
              { key: "joined_at", header: "Joined", render: (s) => s.joined_at },
            ]}
            rows={sessions}
          />
        )}
      </Panel>

      <Panel title={`Confirmation Queue — Pending AI Proposals (${confirmations.length})`}>
        {confirmations.length === 0 ? (
          <EmptyState
            title="No pending confirmations"
            description="When the AI operator proposes actions in assist mode, they will appear here for human review."
          />
        ) : (
          <DataTable
            rowKey={(c) => c.id}
            columns={[
              { key: "cycle", header: "Cycle", render: (c) => String(c.cycle) },
              {
                key: "action_type",
                header: "Action",
                render: (c) => c.proposal.type,
              },
              { key: "rationale", header: "Rationale", render: (c) => c.rationale },
              {
                key: "status",
                header: "Status",
                render: (c) => (
                  <Badge tone={confirmationTone(c)}>{c.status}</Badge>
                ),
              },
              {
                key: "actions",
                header: "Review",
                render: (c) =>
                  c.status === "pending" ? (
                    <div style={{ display: "flex", gap: "0.5rem" }}>
                      <Button onClick={() => handleConfirm(c.id)}>Confirm</Button>
                      <Button onClick={() => handleDeny(c.id)}>Deny</Button>
                    </div>
                  ) : (
                    <span>
                      {c.resolved_by_name ? `by ${c.resolved_by_name}` : "—"}
                    </span>
                  ),
              },
            ]}
            rows={confirmations}
          />
        )}
      </Panel>

      {statusMsg && <StatusLine>{statusMsg}</StatusLine>}
    </div>
  );
}

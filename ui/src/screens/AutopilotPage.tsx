import { FormEvent, useEffect, useState } from "react";
import {
  AutopilotGuardConfig,
  AutopilotReviewKind,
  AutopilotReviewExportPacketResponse,
  AutopilotReviewQueueEntry,
  AutopilotReviewQueueFilters,
  PriorityBreakdown,
  executeAutopilot,
  fetchAutopilotReviewExportPacket,
  fetchAutopilotReviewQueue,
  fetchAutopilotStatus,
  proposeAutopilot,
  proposeAutopilotFromReviewItem,
  updateAutopilotConfig,
} from "../lib/api";
import {
  SavedAutopilotReviewView,
  clearAutopilotWorkspaceState,
  loadAutopilotWorkspaceState,
  saveAutopilotWorkspaceState,
} from "../lib/autopilotWorkspace";

const DEFAULT_POLICY_ACTION = JSON.stringify(
  {
    confirmed_by_human: true,
    action: {
      type: "policy_simulation",
      commands: [
        { type: "nonce_reserve" },
        { type: "fee_quote", urgent: true },
        { type: "allowlist_evaluate", chain_id: 1, contract_tag: 55, method_tag: 3735928559 },
      ],
    },
  },
  null,
  2
);

const DEFAULT_ONCHAIN_ACTION = JSON.stringify(
  {
    confirmed_by_human: true,
    action: {
      type: "onchain_broadcast",
      request: {
        rpc_url: "https://rpc.ankr.com/eth",
        raw_tx_hex: "0xdeadbeef",
        await_receipt: false,
        dry_run: true,
        max_poll_rounds: 3,
        poll_interval_ms: 200,
      },
    },
  },
  null,
  2
);

const DEFAULT_REVIEW_LIMIT = "12";
type ReviewKindSelection = AutopilotReviewKind | "all";

function priorityLabel(priority: PriorityBreakdown) {
  return `score ${priority.total} | a${priority.attention_tier} s${priority.severity_tier} c${priority.corroboration_tier} cred=${priority.credibility_bps}`;
}

function reviewStateClass(item: AutopilotReviewQueueEntry) {
  if (item.case_status === "escalated" || item.severity === "critical") return "danger";
  if (
    item.case_status === "brief_ready" ||
    item.claim_review_status === "needs_review" ||
    item.severity === "high"
  ) {
    return "warn";
  }
  return "info";
}

function reviewStateLabel(item: AutopilotReviewQueueEntry) {
  return item.case_status ?? item.claim_review_status ?? item.severity ?? "queued";
}

function describeReviewFilters(filters: AutopilotReviewQueueFilters) {
  const labels = [
    filters.kind ? `kind=${filters.kind}` : null,
    filters.limit ? `limit=${filters.limit}` : null,
  ].filter(Boolean);
  return labels.length > 0 ? labels.join(" | ") : "unfiltered";
}

function reviewKindSelection(filters: AutopilotReviewQueueFilters): ReviewKindSelection {
  return filters.kind ?? "all";
}

function reviewLimitSelection(filters: AutopilotReviewQueueFilters): string {
  return filters.limit !== undefined ? String(filters.limit) : "all";
}

function savedViewId(name: string): string {
  const normalized = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return normalized || "view";
}

export function AutopilotPage() {
  const [initialWorkspace] = useState(() =>
    loadAutopilotWorkspaceState({
      policyActionText: DEFAULT_POLICY_ACTION,
      onchainActionText: DEFAULT_ONCHAIN_ACTION,
    })
  );

  const [config, setConfig] = useState<AutopilotGuardConfig | null>(null);
  const [statusLine, setStatusLine] = useState<string>("Loading autopilot status...");
  const [statsLine, setStatsLine] = useState<string>("");
  const [policyGoal, setPolicyGoal] = useState<string>(initialWorkspace.policyGoal);
  const [onchainGoal, setOnchainGoal] = useState<string>(initialWorkspace.onchainGoal);
  const [policyActionText, setPolicyActionText] = useState<string>(
    initialWorkspace.policyActionText
  );
  const [onchainActionText, setOnchainActionText] = useState<string>(
    initialWorkspace.onchainActionText
  );
  const [policyResult, setPolicyResult] = useState<string>("");
  const [onchainResult, setOnchainResult] = useState<string>("");
  const [reviewQueue, setReviewQueue] = useState<AutopilotReviewQueueEntry[]>([]);
  const [reviewStatus, setReviewStatus] = useState<string>("Loading review queue...");
  const [reviewKindFilter, setReviewKindFilter] = useState<ReviewKindSelection>(
    reviewKindSelection(initialWorkspace.reviewFilters)
  );
  const [reviewLimitFilter, setReviewLimitFilter] = useState<string>(
    reviewLimitSelection(initialWorkspace.reviewFilters)
  );
  const [savedViews, setSavedViews] = useState<SavedAutopilotReviewView[]>(
    initialWorkspace.savedViews
  );
  const [activeViewId, setActiveViewId] = useState<string | null>(initialWorkspace.activeViewId);
  const [savedViewName, setSavedViewName] = useState<string>("");
  const [lastReviewExport, setLastReviewExport] =
    useState<AutopilotReviewExportPacketResponse | null>(null);

  function currentReviewFilters(): AutopilotReviewQueueFilters {
    return {
      kind: reviewKindFilter === "all" ? undefined : reviewKindFilter,
      limit: reviewLimitFilter === "all" ? undefined : Number(reviewLimitFilter),
    };
  }

  const activeView = savedViews.find((view) => view.id === activeViewId) ?? null;

  useEffect(() => {
    saveAutopilotWorkspaceState({
      reviewFilters: currentReviewFilters(),
      policyGoal,
      onchainGoal,
      policyActionText,
      onchainActionText,
      savedViews,
      activeViewId,
    });
  }, [
    activeViewId,
    onchainActionText,
    onchainGoal,
    policyActionText,
    policyGoal,
    reviewKindFilter,
    reviewLimitFilter,
    savedViews,
  ]);

  async function refreshStatus(message: string = "Autopilot status loaded.") {
    try {
      const status = await fetchAutopilotStatus();
      setConfig(status.config);
      setStatusLine(message);
      setStatsLine(`evaluations=${status.stats.evaluations}, denied=${status.stats.denied}`);
    } catch (error) {
      setStatusLine(`Failed to load autopilot status: ${(error as Error).message}`);
    }
  }

  async function loadReviewQueue(
    message?: string,
    filters: AutopilotReviewQueueFilters = currentReviewFilters()
  ) {
    try {
      const items = await fetchAutopilotReviewQueue(filters);
      setReviewQueue(items);
      setReviewStatus(
        message ?? `Loaded ${items.length} review items (${describeReviewFilters(filters)}).`
      );
    } catch (error) {
      setReviewStatus(`Failed to load review queue: ${(error as Error).message}`);
    }
  }

  useEffect(() => {
    const filters = currentReviewFilters();
    void (async () => {
      await Promise.all([refreshStatus(), loadReviewQueue(undefined, filters)]);
    })();
  }, []);

  async function onSaveConfig(event: FormEvent) {
    event.preventDefault();
    if (!config) return;
    setStatusLine("Saving autopilot config...");
    try {
      const updated = await updateAutopilotConfig(config);
      setConfig(updated.config);
      setStatusLine("Autopilot config saved.");
      setStatsLine(`evaluations=${updated.stats.evaluations}, denied=${updated.stats.denied}`);
    } catch (error) {
      setStatusLine(`Save failed: ${(error as Error).message}`);
    }
  }

  async function onRunPolicyAction() {
    setPolicyResult("Executing policy autopilot action...");
    try {
      const parsed = JSON.parse(policyActionText);
      const response = await executeAutopilot(parsed);
      setPolicyResult(JSON.stringify(response, null, 2));
      await refreshStatus("Autopilot status refreshed after policy execution.");
    } catch (error) {
      setPolicyResult(`Execution failed: ${(error as Error).message}`);
    }
  }

  async function onProposePolicyAction() {
    if (!policyGoal.trim()) {
      setPolicyResult("Proposal goal is required.");
      return;
    }

    setPolicyResult("Requesting policy proposal...");
    try {
      const result = await proposeAutopilot({ goal: policyGoal, kind: "policy_simulation" });
      if (!result.ok) {
        setPolicyResult(JSON.stringify({ status: result.status, ...result.error }, null, 2));
        return;
      }

      setPolicyActionText(
        JSON.stringify(
          { confirmed_by_human: true, action: result.response.action },
          null,
          2
        )
      );
      setPolicyResult(JSON.stringify(result.response, null, 2));
    } catch (error) {
      setPolicyResult(`Proposal failed: ${(error as Error).message}`);
    }
  }

  async function onRunOnchainAction() {
    setOnchainResult("Executing onchain autopilot action...");
    try {
      const parsed = JSON.parse(onchainActionText);
      const response = await executeAutopilot(parsed);
      setOnchainResult(JSON.stringify(response, null, 2));
      await refreshStatus("Autopilot status refreshed after onchain execution.");
    } catch (error) {
      setOnchainResult(`Execution failed: ${(error as Error).message}`);
    }
  }

  async function onProposeOnchainAction() {
    if (!onchainGoal.trim()) {
      setOnchainResult("Proposal goal is required.");
      return;
    }

    setOnchainResult("Requesting onchain proposal...");
    let hints: { rpc_url?: string; raw_tx_hex?: string; dry_run?: boolean } = {};
    try {
      const parsed = JSON.parse(onchainActionText);
      const req = parsed?.action?.request;
      if (typeof req?.rpc_url === "string") hints.rpc_url = req.rpc_url;
      if (typeof req?.raw_tx_hex === "string") hints.raw_tx_hex = req.raw_tx_hex;
      if (typeof req?.dry_run === "boolean") hints.dry_run = req.dry_run;
    } catch {
      // Ignore parse failures and let the proposer decide.
    }

    try {
      const result = await proposeAutopilot({
        goal: onchainGoal,
        kind: "onchain_broadcast",
        ...hints,
      });
      if (!result.ok) {
        setOnchainResult(JSON.stringify({ status: result.status, ...result.error }, null, 2));
        return;
      }

      setOnchainActionText(
        JSON.stringify(
          { confirmed_by_human: true, action: result.response.action },
          null,
          2
        )
      );
      setOnchainResult(JSON.stringify(result.response, null, 2));
    } catch (error) {
      setOnchainResult(`Proposal failed: ${(error as Error).message}`);
    }
  }

  function applyReviewFilters() {
    void loadReviewQueue(undefined, currentReviewFilters());
  }

  function resetReviewFilters() {
    setActiveViewId(null);
    setReviewKindFilter("all");
    setReviewLimitFilter(DEFAULT_REVIEW_LIMIT);
    void loadReviewQueue("Reset review queue filters.", { limit: Number(DEFAULT_REVIEW_LIMIT) });
  }

  function useGoalHint(item: AutopilotReviewQueueEntry) {
    setPolicyGoal(item.goal_hint);
    setPolicyResult(`Loaded policy goal hint from ${item.kind} ${item.item_id}.`);
  }

  async function draftPolicyFromReviewItem(item: AutopilotReviewQueueEntry) {
    setPolicyResult(`Drafting policy proposal from ${item.kind} ${item.item_id}...`);
    try {
      const response = await proposeAutopilotFromReviewItem({
        review_kind: item.kind,
        item_id: item.item_id,
        kind: "policy_simulation",
      });
      setPolicyGoal(response.review_item.goal_hint);
      setPolicyActionText(
        JSON.stringify(
          { confirmed_by_human: true, action: response.proposal.action },
          null,
          2
        )
      );
      setPolicyResult(JSON.stringify(response, null, 2));
    } catch (error) {
      setPolicyResult(`Review-item proposal failed: ${(error as Error).message}`);
    }
  }

  async function exportReviewPacket(item: AutopilotReviewQueueEntry) {
    setReviewStatus(`Exporting packet for ${item.kind} ${item.item_id}...`);
    try {
      const packet = await fetchAutopilotReviewExportPacket(item.kind, item.item_id);
      setLastReviewExport(packet);
      setReviewStatus(`Exported packet ${packet.packet_id}.`);
    } catch (error) {
      setReviewStatus(`Review export failed: ${(error as Error).message}`);
    }
  }

  function saveCurrentView() {
    const name = savedViewName.trim();
    if (!name) {
      setReviewStatus("Saved view name is required.");
      return;
    }

    const view: SavedAutopilotReviewView = {
      id: savedViewId(name),
      name,
      filters: currentReviewFilters(),
    };
    setSavedViews((existing) => {
      const next = existing.filter((item) => item.id !== view.id);
      return [view, ...next].slice(0, 8);
    });
    setActiveViewId(view.id);
    setSavedViewName("");
    setReviewStatus(`Saved review view '${name}'.`);
  }

  function applySavedView(view: SavedAutopilotReviewView) {
    setActiveViewId(view.id);
    setReviewKindFilter(reviewKindSelection(view.filters));
    setReviewLimitFilter(reviewLimitSelection(view.filters));
    void loadReviewQueue(`Applied saved view '${view.name}'.`, view.filters);
  }

  function deleteSavedView(view: SavedAutopilotReviewView) {
    setSavedViews((existing) => existing.filter((item) => item.id !== view.id));
    if (activeViewId === view.id) {
      setActiveViewId(null);
    }
    setReviewStatus(`Deleted saved view '${view.name}'.`);
  }

  function clearWorkspace() {
    clearAutopilotWorkspaceState();
    setPolicyGoal("");
    setOnchainGoal("");
    setPolicyActionText(DEFAULT_POLICY_ACTION);
    setOnchainActionText(DEFAULT_ONCHAIN_ACTION);
    setPolicyResult("");
    setOnchainResult("");
    setReviewKindFilter("all");
    setReviewLimitFilter(DEFAULT_REVIEW_LIMIT);
    setSavedViews([]);
    setActiveViewId(null);
    setSavedViewName("");
    void loadReviewQueue("Cleared local workspace state.", { limit: Number(DEFAULT_REVIEW_LIMIT) });
  }

  return (
    <section className="dashboard-grid">
      <article className="panel panel-hero panel-span-12">
        <p className="mono-label">Autopilot</p>
        <h2>LLM-Operable Helix With Deterministic Guardrails</h2>
        <p>
          Use assist or auto mode to let an LLM operate Helix through bounded actions, while policy
          and on-chain commands remain fail-closed. The review queue below is deterministic: cases,
          claims, and evidence are merged into one guarded worklist before any proposal is drafted.
        </p>
      </article>

      <article className="panel panel-span-4">
        <p className="mono-label">Autopilot Config</p>
        <form className="form-grid" onSubmit={onSaveConfig}>
          {config ? (
            <>
              <label className="field">
                <span>mode</span>
                <select
                  value={config.mode}
                  onChange={(e) =>
                    setConfig({
                      ...config,
                      mode: e.target.value as AutopilotGuardConfig["mode"],
                    })
                  }
                >
                  <option value="off">off</option>
                  <option value="assist">assist</option>
                  <option value="auto">auto</option>
                </select>
              </label>

              <label className="field checkbox-field">
                <input
                  type="checkbox"
                  checked={config.allow_onchain}
                  onChange={(e) => setConfig({ ...config, allow_onchain: e.target.checked })}
                />
                <span>allow_onchain</span>
              </label>

              <label className="field checkbox-field">
                <input
                  type="checkbox"
                  checked={config.require_onchain_confirmation}
                  onChange={(e) =>
                    setConfig({ ...config, require_onchain_confirmation: e.target.checked })
                  }
                />
                <span>require_onchain_confirmation</span>
              </label>

              <label className="field checkbox-field">
                <input
                  type="checkbox"
                  checked={config.require_onchain_dry_run}
                  onChange={(e) =>
                    setConfig({ ...config, require_onchain_dry_run: e.target.checked })
                  }
                />
                <span>require_onchain_dry_run</span>
              </label>

              <label className="field">
                <span>max_policy_commands</span>
                <input
                  type="number"
                  min={1}
                  value={config.max_policy_commands}
                  onChange={(e) =>
                    setConfig({ ...config, max_policy_commands: Number(e.target.value) })
                  }
                />
              </label>

              <button className="btn-primary" type="submit">
                Save Autopilot Config
              </button>
            </>
          ) : (
            <p>Loading...</p>
          )}
        </form>
        <p className="status-line">{statusLine}</p>
        <p className="status-line">{statsLine}</p>
      </article>

      <article className="panel panel-span-4">
        <p className="mono-label">Policy Action</p>
        <label className="field">
          <span>proposal_goal</span>
          <input
            type="text"
            value={policyGoal}
            onChange={(e) => setPolicyGoal(e.target.value)}
            placeholder="e.g. simulate allowlist and fee quote before broadcast"
          />
        </label>
        <div className="button-row">
          <button className="btn-secondary" onClick={onProposePolicyAction}>
            Propose Policy Action
          </button>
        </div>
        <textarea
          className="command-editor"
          rows={16}
          value={policyActionText}
          onChange={(e) => setPolicyActionText(e.target.value)}
        />
        <div className="button-row">
          <button className="btn-primary" onClick={onRunPolicyAction}>
            Run Policy Action
          </button>
          <button className="btn-secondary" onClick={() => setPolicyActionText(DEFAULT_POLICY_ACTION)}>
            Reset Example
          </button>
        </div>
        <pre className="json-output">{policyResult}</pre>
      </article>

      <article className="panel panel-span-4">
        <p className="mono-label">Onchain Action</p>
        <label className="field">
          <span>proposal_goal</span>
          <input
            type="text"
            value={onchainGoal}
            onChange={(e) => setOnchainGoal(e.target.value)}
            placeholder="e.g. propose a safe dry-run broadcast payload"
          />
        </label>
        <div className="button-row">
          <button className="btn-secondary" onClick={onProposeOnchainAction}>
            Propose Onchain Action
          </button>
        </div>
        <textarea
          className="command-editor"
          rows={16}
          value={onchainActionText}
          onChange={(e) => setOnchainActionText(e.target.value)}
        />
        <div className="button-row">
          <button className="btn-primary" onClick={onRunOnchainAction}>
            Run Onchain Action
          </button>
          <button className="btn-secondary" onClick={() => setOnchainActionText(DEFAULT_ONCHAIN_ACTION)}>
            Reset Example
          </button>
        </div>
        <pre className="json-output">{onchainResult}</pre>
      </article>

      <article className="panel panel-span-5">
        <p className="mono-label">Review Filters</p>
        <div className="pill-row">
          <span className="info-pill">
            priority = attention &gt; severity &gt; corroboration &gt; freshness &gt; trust &gt;
            density
          </span>
          <span className="info-pill">ties break on latest signal, then kind, then item id</span>
          <span className="info-pill">active_view: {activeView?.name ?? "none"}</span>
        </div>
        <div className="form-grid">
          <label className="field">
            <span>kind</span>
            <select
              value={reviewKindFilter}
              onChange={(event) => {
                setActiveViewId(null);
                setReviewKindFilter(event.target.value as ReviewKindSelection);
              }}
            >
              <option value="all">all</option>
              <option value="case">case</option>
              <option value="claim">claim</option>
              <option value="evidence">evidence</option>
            </select>
          </label>
          <label className="field">
            <span>limit</span>
            <select
              value={reviewLimitFilter}
              onChange={(event) => {
                setActiveViewId(null);
                setReviewLimitFilter(event.target.value);
              }}
            >
              <option value="6">6</option>
              <option value="12">12</option>
              <option value="25">25</option>
              <option value="50">50</option>
              <option value="100">100</option>
              <option value="all">all</option>
            </select>
          </label>
        </div>
        <div className="button-row">
          <button className="btn-secondary" type="button" onClick={applyReviewFilters}>
            Apply Filters
          </button>
          <button className="btn-secondary" type="button" onClick={resetReviewFilters}>
            Reset
          </button>
        </div>
        <p className="status-line">{reviewStatus}</p>
      </article>

      <article className="panel panel-span-7">
        <p className="mono-label">Saved Views</p>
        <label className="field">
          <span>view_name</span>
          <input
            type="text"
            value={savedViewName}
            onChange={(event) => setSavedViewName(event.target.value)}
            placeholder="e.g. claim-review-focus"
          />
        </label>
        <div className="button-row">
          <button className="btn-secondary" type="button" onClick={saveCurrentView}>
            Save Current View
          </button>
          <button className="btn-secondary" type="button" onClick={clearWorkspace}>
            Clear Workspace
          </button>
        </div>
        <div className="agent-grid">
          {savedViews.length > 0 ? (
            savedViews.map((view) => (
              <div key={view.id} className="agent-card">
                <div className="agent-card-head">
                  <h3>{view.name}</h3>
                  <span className={`status-pill ${activeViewId === view.id ? "ok" : "info"}`}>
                    {activeViewId === view.id ? "active" : "saved"}
                  </span>
                </div>
                <div className="pill-row">
                  <span className="info-pill">{describeReviewFilters(view.filters)}</span>
                  <span className="info-pill">view_id: {view.id}</span>
                </div>
                <div className="button-row">
                  <button
                    className="btn-primary"
                    type="button"
                    onClick={() => applySavedView(view)}
                  >
                    Apply View
                  </button>
                  <button
                    className="btn-secondary"
                    type="button"
                    onClick={() => deleteSavedView(view)}
                  >
                    Delete
                  </button>
                </div>
              </div>
            ))
          ) : (
            <div className="agent-card">
              <div className="agent-card-head">
                <h3>No Saved Views</h3>
                <span className="status-pill info">local</span>
              </div>
              <p>
                Save the current review filters to return to the same ranked autopilot worklist
                after a reload.
              </p>
            </div>
          )}
        </div>
      </article>

      <article className="panel panel-span-12">
        <p className="mono-label">Review Queue</p>
        <div className="pill-row">
          <span className="info-pill">top item: {reviewQueue[0]?.item_id ?? "none"}</span>
          <span className="info-pill">filters: {describeReviewFilters(currentReviewFilters())}</span>
        </div>
        <div className="agent-grid">
          {reviewQueue.map((item, index) => (
            <div key={`${item.kind}:${item.item_id}`} className="agent-card">
              <div className="agent-card-head">
                <h3>
                  #{index + 1} {item.title}
                </h3>
                <span className={`status-pill ${reviewStateClass(item)}`}>
                  {reviewStateLabel(item)}
                </span>
              </div>
              <p>{item.summary}</p>
              <div className="pill-row">
                <span className="info-pill">{item.kind}</span>
                <span className="info-pill">{priorityLabel(item.priority)}</span>
                <span className="info-pill">context: {item.context_label}</span>
                <span className="info-pill">route: {item.route}</span>
              </div>
              <div className="pill-row">
                <span className="info-pill">latest: {item.latest_signal_at ?? "unknown"}</span>
                <span className="info-pill">item_id: {item.item_id}</span>
                {item.severity ? (
                  <span className={`status-pill ${reviewStateClass(item)}`}>{item.severity}</span>
                ) : null}
              </div>
              <code className="command-inline">{item.goal_hint}</code>
              <div className="button-row">
                <button
                  className="btn-primary"
                  type="button"
                  onClick={() => void draftPolicyFromReviewItem(item)}
                >
                  Draft Policy Proposal
                </button>
                <button className="btn-secondary" type="button" onClick={() => useGoalHint(item)}>
                  Use Goal Hint
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() => void exportReviewPacket(item)}
                >
                  Export Packet
                </button>
              </div>
            </div>
          ))}
        </div>
      </article>

      <article className="panel panel-span-12">
        <p className="mono-label">Latest Review Export</p>
        {lastReviewExport ? (
          <pre className="json-output">{JSON.stringify(lastReviewExport, null, 2)}</pre>
        ) : (
          <p>No review export packet has been generated in this session.</p>
        )}
      </article>
    </section>
  );
}

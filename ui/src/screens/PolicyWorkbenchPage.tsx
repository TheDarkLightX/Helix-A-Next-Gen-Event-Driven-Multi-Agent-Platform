import { FormEvent, useEffect, useMemo, useState } from "react";
import {
  DeterministicPolicyConfig,
  PolicyCommand,
  PolicyStepResult,
  fetchPolicyConfig,
  simulatePolicy,
  updatePolicyConfig,
} from "../lib/api";

const DEFAULT_COMMANDS: PolicyCommand[] = [
  { type: "nonce_reserve" },
  { type: "nonce_reserve" },
  { type: "nonce_confirm", nonce: 0 },
  { type: "fee_quote", urgent: false },
  { type: "fee_rejected" },
  { type: "fee_quote", urgent: true },
  { type: "allowlist_evaluate", chain_id: 1, contract_tag: 55, method_tag: 0xdeadbeef },
  { type: "allowlist_evaluate", chain_id: 1, contract_tag: 99, method_tag: 0xdeadbeef },
  { type: "finality_observe_depth", depth: 1 },
  { type: "finality_observe_depth", depth: 3 },
  { type: "start_sla_window" },
  { type: "tick" },
  { type: "request", fingerprint: 11, cost: 1 },
  { type: "request", fingerprint: 11, cost: 1 },
  { type: "enqueue_backpressure", count: 3 },
  { type: "enqueue_backpressure", count: 3 },
  { type: "tick" },
  { type: "request", fingerprint: 11, cost: 1 },
  { type: "failure" },
  { type: "failure" },
  { type: "failure" },
  { type: "complete_sla_window" },
  { type: "request", fingerprint: 22, cost: 1 },
];

function prettyCommand(c: PolicyCommand): string {
  switch (c.type) {
    case "request":
      return `request(fp=${c.fingerprint}, cost=${c.cost})`;
    case "enqueue_backpressure":
      return `enqueue_backpressure(count=${c.count})`;
    case "dequeue_backpressure":
      return `dequeue_backpressure(count=${c.count})`;
    case "nonce_confirm":
      return `nonce_confirm(${c.nonce})`;
    case "nonce_reconcile":
      return `nonce_reconcile(${c.chain_next_nonce})`;
    case "fee_update_base_fee":
      return `fee_update_base_fee(${c.base_fee})`;
    case "fee_quote":
      return `fee_quote(urgent=${c.urgent})`;
    case "finality_observe_depth":
      return `finality_observe_depth(${c.depth})`;
    case "allowlist_evaluate":
      return `allowlist_evaluate(chain=${c.chain_id},contract=${c.contract_tag},method=${c.method_tag})`;
    default:
      return c.type;
  }
}

export function PolicyWorkbenchPage() {
  const [config, setConfig] = useState<DeterministicPolicyConfig | null>(null);
  const [configStatus, setConfigStatus] = useState<string>("Loading config...");
  const [commandsText, setCommandsText] = useState<string>(
    JSON.stringify(DEFAULT_COMMANDS, null, 2)
  );
  const [steps, setSteps] = useState<PolicyStepResult[]>([]);
  const [simulateStatus, setSimulateStatus] = useState<string>("");

  useEffect(() => {
    void (async () => {
      try {
        const loaded = await fetchPolicyConfig();
        setConfig(loaded);
        setConfigStatus("Config loaded from API.");
      } catch (error) {
        setConfigStatus(`Failed to load config: ${(error as Error).message}`);
      }
    })();
  }, []);

  async function onSaveConfig(event: FormEvent) {
    event.preventDefault();
    if (!config) return;
    setConfigStatus("Saving...");
    try {
      const saved = await updatePolicyConfig(config);
      setConfig(saved);
      setConfigStatus("Config saved.");
    } catch (error) {
      setConfigStatus(`Save failed: ${(error as Error).message}`);
    }
  }

  async function onSimulate() {
    setSimulateStatus("Running simulation...");
    try {
      const commands = JSON.parse(commandsText) as PolicyCommand[];
      const output = await simulatePolicy(commands);
      setSteps(output);
      setSimulateStatus(`Simulation completed (${output.length} steps).`);
    } catch (error) {
      setSimulateStatus(`Simulation failed: ${(error as Error).message}`);
    }
  }

  const finalSnapshot = useMemo(() => (steps.length > 0 ? steps[steps.length - 1].snapshot : null), [steps]);

  return (
    <section className="dashboard-grid">
      <article className="panel panel-hero">
        <p className="mono-label">Policy Workbench</p>
        <h2>Deterministic Controls + Replayable Simulation</h2>
        <p>
          Edit policy parameters, run deterministic command sequences, and inspect final snapshots.
        </p>
      </article>

      <article className="panel">
        <p className="mono-label">Policy Config</p>
        <form onSubmit={onSaveConfig} className="form-grid">
          {config ? (
            <>
              {(
                [
                  "dedup_window_ticks",
                  "rate_max_tokens",
                  "rate_refill_per_tick",
                  "breaker_failure_threshold",
                  "breaker_open_duration_ticks",
                  "retry_budget",
                  "approval_quorum",
                  "approval_reviewers",
                  "backpressure_soft_limit",
                  "backpressure_hard_limit",
                  "sla_deadline_ticks",
                  "dlq_max_consecutive_failures",
                  "nonce_start",
                  "nonce_max_in_flight",
                  "fee_base_fee",
                  "fee_priority_fee",
                  "fee_bump_bps",
                  "fee_max_fee_cap",
                  "finality_required_depth",
                  "allowlist_chain_id",
                  "allowlist_contract_tag",
                  "allowlist_method_tag",
                ] as const
              ).map((key) => (
                <label key={key} className="field">
                  <span>{key}</span>
                  <input
                    type="number"
                    min={0}
                    value={config[key]}
                    onChange={(e) =>
                      setConfig({
                        ...config,
                        [key]: Number(e.target.value),
                      })
                    }
                  />
                </label>
              ))}
              <button type="submit" className="btn-primary">
                Save Config
              </button>
            </>
          ) : (
            <p>Loading...</p>
          )}
        </form>
        <p className="status-line">{configStatus}</p>
      </article>

      <article className="panel">
        <p className="mono-label">Simulation Commands</p>
        <textarea
          className="command-editor"
          value={commandsText}
          onChange={(e) => setCommandsText(e.target.value)}
          rows={14}
        />
        <div className="button-row">
          <button className="btn-primary" onClick={onSimulate}>
            Run Simulation
          </button>
          <button
            className="btn-secondary"
            onClick={() => setCommandsText(JSON.stringify(DEFAULT_COMMANDS, null, 2))}
          >
            Reset Example
          </button>
        </div>
        <p className="status-line">{simulateStatus}</p>
      </article>

      <article className="panel">
        <p className="mono-label">Simulation Output</p>
        <div className="table-wrap">
          <table className="result-table">
            <thead>
              <tr>
                <th>#</th>
                <th>Command</th>
                <th>Decision</th>
                <th>Rate Tokens</th>
                <th>Queue Depth</th>
                <th>Breaker</th>
                <th>Retry Left</th>
                <th>DLQ Failures</th>
                <th>SLA</th>
                <th>Nonce</th>
                <th>Fee</th>
                <th>Finality</th>
                <th>Allowlist</th>
              </tr>
            </thead>
            <tbody>
              {steps.map((step, idx) => (
                <tr key={`${idx}-${step.decision.kind}`}>
                  <td>{idx + 1}</td>
                  <td>{prettyCommand(step.command)}</td>
                  <td>
                    {step.decision.kind}
                    {step.decision.reason ? ` (${step.decision.reason})` : ""}
                    {step.decision.decision ? ` (${step.decision.decision})` : ""}
                    {step.decision.status ? ` (${step.decision.status})` : ""}
                    {step.decision.route ? ` (${step.decision.route})` : ""}
                    {step.decision.outcome ? ` (${step.decision.outcome})` : ""}
                    {step.decision.quoted !== undefined ? ` (quoted=${step.decision.quoted})` : ""}
                    {step.decision.state ? ` (${step.decision.state})` : ""}
                    {step.decision.remaining_depth !== undefined
                      ? ` (remaining_depth=${step.decision.remaining_depth})`
                      : ""}
                    {step.decision.nonce !== undefined ? ` (nonce=${step.decision.nonce})` : ""}
                    {step.decision.next_nonce !== undefined
                      ? ` (next_nonce=${step.decision.next_nonce})`
                      : ""}
                    {step.decision.max_fee !== undefined
                      ? ` (max_fee=${step.decision.max_fee}, priority=${step.decision.max_priority_fee}, rejects=${step.decision.rejection_count})`
                      : ""}
                  </td>
                  <td>{step.snapshot.rate_tokens}</td>
                  <td>{step.snapshot.queue_depth}</td>
                  <td>{step.snapshot.breaker_phase}</td>
                  <td>{step.snapshot.retry_remaining}</td>
                  <td>{step.snapshot.dlq_consecutive_failures}</td>
                  <td>
                    {step.snapshot.sla_active
                      ? `${step.snapshot.sla_expired ? "expired" : "active"}:${step.snapshot.sla_remaining_ticks}`
                      : "idle"}
                  </td>
                  <td>
                    next={step.snapshot.nonce_next}, in_flight={step.snapshot.nonce_in_flight}
                  </td>
                  <td>rejections={step.snapshot.fee_rejection_count}</td>
                  <td>
                    depth={step.snapshot.finality_observed_depth}, finalized=
                    {String(step.snapshot.finality_finalized)}, reorg=
                    {String(step.snapshot.finality_reorg_detected)}
                  </td>
                  <td>paused={String(step.snapshot.allowlist_paused)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {finalSnapshot && (
          <p className="status-line">
            Final snapshot: tokens={finalSnapshot.rate_tokens}, queue={finalSnapshot.queue_depth},
            breaker={finalSnapshot.breaker_phase}, retry={finalSnapshot.retry_remaining},
            dlq_failures={finalSnapshot.dlq_consecutive_failures}, sla=
            {finalSnapshot.sla_active
              ? `${finalSnapshot.sla_expired ? "expired" : "active"}:${finalSnapshot.sla_remaining_ticks}`
              : "idle"}
            , nonce_next={finalSnapshot.nonce_next}, nonce_in_flight={finalSnapshot.nonce_in_flight}
            , fee_rejections={finalSnapshot.fee_rejection_count}
            , finality_depth={finalSnapshot.finality_observed_depth}, finalized=
            {String(finalSnapshot.finality_finalized)}, reorg=
            {String(finalSnapshot.finality_reorg_detected)}, allowlist_paused=
            {String(finalSnapshot.allowlist_paused)}
          </p>
        )}
      </article>
    </section>
  );
}

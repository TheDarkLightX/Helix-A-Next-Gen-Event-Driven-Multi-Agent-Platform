import { FormEvent, ReactNode, useEffect, useMemo, useState } from "react";
import {
  DeterministicPolicyConfig,
  PolicyCommand,
  PolicyStepResult,
  fetchPolicyConfig,
  simulatePolicy,
  updatePolicyConfig,
} from "../lib/api";
import {
  Panel,
  FormField,
  Input,
  Textarea,
  Button,
  StatusLine,
  DataTable,
  LoadingState,
} from "../components";

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

type StepRow = { idx: number; step: PolicyStepResult };

type StepColumn = {
  key: string;
  header: string;
  render: (row: StepRow) => ReactNode;
  width?: string;
};

const OUTPUT_COLUMNS: StepColumn[] = [
  { key: "index", header: "#", render: (row) => row.idx + 1 },
  { key: "command", header: "Command", render: (row) => prettyCommand(row.step.command) },
  {
    key: "decision",
    header: "Decision",
    render: (row) => {
      const d = row.step.decision;
      return (
        <>
          {d.kind}
          {d.reason ? ` (${d.reason})` : ""}
          {d.decision ? ` (${d.decision})` : ""}
          {d.status ? ` (${d.status})` : ""}
          {d.route ? ` (${d.route})` : ""}
          {d.outcome ? ` (${d.outcome})` : ""}
          {d.quoted !== undefined ? ` (quoted=${d.quoted})` : ""}
          {d.state ? ` (${d.state})` : ""}
          {d.remaining_depth !== undefined ? ` (remaining_depth=${d.remaining_depth})` : ""}
          {d.nonce !== undefined ? ` (nonce=${d.nonce})` : ""}
          {d.next_nonce !== undefined ? ` (next_nonce=${d.next_nonce})` : ""}
          {d.max_fee !== undefined
            ? ` (max_fee=${d.max_fee}, priority=${d.max_priority_fee}, rejects=${d.rejection_count})`
            : ""}
        </>
      );
    },
  },
  { key: "rate_tokens", header: "Rate Tokens", render: (row) => row.step.snapshot.rate_tokens },
  { key: "queue_depth", header: "Queue Depth", render: (row) => row.step.snapshot.queue_depth },
  { key: "breaker", header: "Breaker", render: (row) => row.step.snapshot.breaker_phase },
  { key: "retry", header: "Retry Left", render: (row) => row.step.snapshot.retry_remaining },
  {
    key: "dlq",
    header: "DLQ Failures",
    render: (row) => row.step.snapshot.dlq_consecutive_failures,
  },
  {
    key: "sla",
    header: "SLA",
    render: (row) =>
      row.step.snapshot.sla_active
        ? `${row.step.snapshot.sla_expired ? "expired" : "active"}:${row.step.snapshot.sla_remaining_ticks}`
        : "idle",
  },
  {
    key: "nonce",
    header: "Nonce",
    render: (row) =>
      `next=${row.step.snapshot.nonce_next}, in_flight=${row.step.snapshot.nonce_in_flight}`,
  },
  {
    key: "fee",
    header: "Fee",
    render: (row) => `rejections=${row.step.snapshot.fee_rejection_count}`,
  },
  {
    key: "finality",
    header: "Finality",
    render: (row) =>
      `depth=${row.step.snapshot.finality_observed_depth}, finalized=${String(
        row.step.snapshot.finality_finalized
      )}, reorg=${String(row.step.snapshot.finality_reorg_detected)}`,
  },
  {
    key: "allowlist",
    header: "Allowlist",
    render: (row) => `paused=${String(row.step.snapshot.allowlist_paused)}`,
  },
];

const CONFIG_KEYS = [
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
] as const;

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

  const finalSnapshot = useMemo(
    () => (steps.length > 0 ? steps[steps.length - 1].snapshot : null),
    [steps]
  );

  const stepRows: StepRow[] = useMemo(
    () => steps.map((step, idx) => ({ idx, step })),
    [steps]
  );

  return (
    <section className="hx-page-grid">
      <Panel hero span={12} eyebrow="Policy Workbench" title="Deterministic Controls + Replayable Simulation">
        <p className="hx-description">
          Edit policy parameters, run deterministic command sequences, and inspect final snapshots.
        </p>
      </Panel>

      <Panel span={6} eyebrow="Policy Config" title="Parameters">
        {config ? (
          <form className="hx-form-grid" onSubmit={onSaveConfig}>
            {CONFIG_KEYS.map((key) => (
              <FormField key={key} label={key}>
                <Input
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
              </FormField>
            ))}
            <div className="hx-cluster" style={{ gridColumn: "1 / -1" }}>
              <Button type="submit">Save Config</Button>
            </div>
          </form>
        ) : (
          <LoadingState title="Loading config..." />
        )}
        <StatusLine>{configStatus}</StatusLine>
      </Panel>

      <Panel span={6} eyebrow="Simulation Commands" title="Replay Sequence">
        <FormField label="Command JSON" full>
          <Textarea rows={14} value={commandsText} onChange={(e) => setCommandsText(e.target.value)} />
        </FormField>
        <div className="hx-cluster">
          <Button type="button" onClick={onSimulate}>
            Run Simulation
          </Button>
          <Button
            variant="secondary"
            type="button"
            onClick={() => setCommandsText(JSON.stringify(DEFAULT_COMMANDS, null, 2))}
          >
            Reset Example
          </Button>
        </div>
        <StatusLine>{simulateStatus}</StatusLine>
      </Panel>

      <Panel span={12} eyebrow="Simulation Output" title="Step Trace">
        <DataTable
          columns={OUTPUT_COLUMNS}
          rows={stepRows}
          rowKey={(row) => `${row.idx}-${row.step.decision.kind}`}
          emptyMessage="Run a simulation to inspect the deterministic step trace."
        />
        {finalSnapshot && (
          <StatusLine>
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
          </StatusLine>
        )}
      </Panel>
    </section>
  );
}

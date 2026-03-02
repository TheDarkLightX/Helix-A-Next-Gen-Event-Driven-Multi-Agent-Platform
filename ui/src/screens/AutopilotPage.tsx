import { FormEvent, useEffect, useState } from "react";
import {
  AutopilotGuardConfig,
  executeAutopilot,
  fetchAutopilotStatus,
  updateAutopilotConfig,
} from "../lib/api";

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

export function AutopilotPage() {
  const [config, setConfig] = useState<AutopilotGuardConfig | null>(null);
  const [statusLine, setStatusLine] = useState<string>("Loading autopilot status...");
  const [statsLine, setStatsLine] = useState<string>("");
  const [policyActionText, setPolicyActionText] = useState<string>(DEFAULT_POLICY_ACTION);
  const [onchainActionText, setOnchainActionText] = useState<string>(DEFAULT_ONCHAIN_ACTION);
  const [policyResult, setPolicyResult] = useState<string>("");
  const [onchainResult, setOnchainResult] = useState<string>("");

  useEffect(() => {
    void (async () => {
      try {
        const status = await fetchAutopilotStatus();
        setConfig(status.config);
        setStatusLine("Autopilot status loaded.");
        setStatsLine(`evaluations=${status.stats.evaluations}, denied=${status.stats.denied}`);
      } catch (error) {
        setStatusLine(`Failed to load autopilot status: ${(error as Error).message}`);
      }
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
      const status = await fetchAutopilotStatus();
      setStatsLine(`evaluations=${status.stats.evaluations}, denied=${status.stats.denied}`);
    } catch (error) {
      setPolicyResult(`Execution failed: ${(error as Error).message}`);
    }
  }

  async function onRunOnchainAction() {
    setOnchainResult("Executing onchain autopilot action...");
    try {
      const parsed = JSON.parse(onchainActionText);
      const response = await executeAutopilot(parsed);
      setOnchainResult(JSON.stringify(response, null, 2));
      const status = await fetchAutopilotStatus();
      setStatsLine(`evaluations=${status.stats.evaluations}, denied=${status.stats.denied}`);
    } catch (error) {
      setOnchainResult(`Execution failed: ${(error as Error).message}`);
    }
  }

  return (
    <section className="dashboard-grid">
      <article className="panel panel-hero">
        <p className="mono-label">Autopilot</p>
        <h2>LLM-Operable Helix With Deterministic Guardrails</h2>
        <p>
          Use assist or auto mode to let an LLM operate Helix through bounded actions, while policy
          and on-chain commands remain fail-closed.
        </p>
      </article>

      <article className="panel">
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

      <article className="panel">
        <p className="mono-label">Policy Action</p>
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

      <article className="panel">
        <p className="mono-label">Onchain Action</p>
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
    </section>
  );
}

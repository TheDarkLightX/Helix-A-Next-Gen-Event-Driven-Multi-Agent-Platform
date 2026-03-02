import { useEffect, useMemo, useState } from "react";
import {
  applyAgentTemplate,
  DeterministicAgentSpec,
  DeterministicAgentTemplate,
  fetchAgentCatalog,
  fetchAgentTemplates,
} from "../lib/api";

const verificationCommands = [
  "./scripts/verify_esso_roi_agents.sh",
  "cargo test --manifest-path crates/helix-core/Cargo.toml --lib deterministic_agents",
  "cargo test --manifest-path crates/helix-core/Cargo.toml --lib deterministic_policy",
];

export function AgentCatalogPage() {
  const [agents, setAgents] = useState<DeterministicAgentSpec[]>([]);
  const [templates, setTemplates] = useState<DeterministicAgentTemplate[]>([]);
  const [selectedTemplateId, setSelectedTemplateId] = useState<string>("");
  const [status, setStatus] = useState<string>("Loading deterministic agent catalog...");
  const [templateStatus, setTemplateStatus] = useState<string>("Loading templates...");
  const [applyStatus, setApplyStatus] = useState<string>("");

  useEffect(() => {
    void (async () => {
      try {
        const [catalog, templateCatalog] = await Promise.all([
          fetchAgentCatalog(),
          fetchAgentTemplates(),
        ]);
        setAgents(catalog);
        setTemplates(templateCatalog);
        if (templateCatalog.length > 0) {
          setSelectedTemplateId(templateCatalog[0].id);
        }
        setStatus(`Loaded ${catalog.length} deterministic agents.`);
        setTemplateStatus(`Loaded ${templateCatalog.length} deployment templates.`);
      } catch (error) {
        const message = (error as Error).message;
        setStatus(`Failed to load agent catalog: ${message}`);
        setTemplateStatus(`Failed to load templates: ${message}`);
      }
    })();
  }, []);

  const selectedTemplate = useMemo(
    () => templates.find((template) => template.id === selectedTemplateId) ?? null,
    [selectedTemplateId, templates]
  );

  async function copyToClipboard(label: string, value: string) {
    if (!navigator.clipboard) {
      setTemplateStatus("Clipboard is not available in this browser context.");
      return;
    }

    try {
      await navigator.clipboard.writeText(value);
      setTemplateStatus(`${label} copied to clipboard.`);
    } catch (error) {
      setTemplateStatus(`Failed to copy ${label.toLowerCase()}: ${(error as Error).message}`);
    }
  }

  async function applySelectedTemplate() {
    if (!selectedTemplate) {
      return;
    }

    setApplyStatus(`Applying ${selectedTemplate.name}...`);
    try {
      const response = await applyAgentTemplate(selectedTemplate.id, true);
      const stepCount = response.bootstrap_steps?.length ?? 0;
      setApplyStatus(
        `Applied ${response.template.name}. Policy config updated. Bootstrap simulation steps: ${stepCount}.`
      );
    } catch (error) {
      setApplyStatus(`Template apply failed: ${(error as Error).message}`);
    }
  }

  return (
    <section className="dashboard-grid">
      <article className="panel panel-hero">
        <p className="mono-label">Agent Catalog</p>
        <h2>High-ROI Deterministic State Machines</h2>
        <p>
          These kernels are pure and replayable. Each has an ESSO model for fail-closed verification.
        </p>
      </article>

      <article className="panel">
        <p className="mono-label">Verification Commands</p>
        <div className="command-stack">
          {verificationCommands.map((command) => (
            <code key={command} className="command-inline">
              {command}
            </code>
          ))}
        </div>
        <p className="status-line">{status}</p>
      </article>

      <article className="panel">
        <p className="mono-label">Implemented Agents</p>
        <div className="agent-grid">
          {agents.map((agent) => (
            <div key={agent.id} className="agent-card">
              <div className="agent-card-head">
                <h3>{agent.name}</h3>
                <span className="status-pill ok">Implemented</span>
              </div>
              <p>{agent.roi_rationale}</p>
              <p className="mono-detail">{agent.id}</p>
              <code className="command-inline">{agent.kernel_module}</code>
              <code className="command-inline">{agent.esso_model}</code>
            </div>
          ))}
        </div>
      </article>

      <article className="panel">
        <p className="mono-label">Deployment Templates</p>
        <div className="agent-grid">
          {templates.map((template) => (
            <div key={template.id} className="agent-card">
              <div className="agent-card-head">
                <h3>{template.name}</h3>
                <span className="status-pill ok">Template</span>
              </div>
              <p>{template.summary}</p>
              <p className="mono-detail">{template.id}</p>
              <p>Use for: {template.recommended_for}</p>
              <code className="command-inline">
                required_agents=[{template.required_agents.join(", ")}]
              </code>
              <div className="button-row">
                <button
                  className={template.id === selectedTemplateId ? "btn-primary" : "btn-secondary"}
                  onClick={() => setSelectedTemplateId(template.id)}
                >
                  {template.id === selectedTemplateId ? "Selected" : "Select Template"}
                </button>
              </div>
            </div>
          ))}
        </div>
        <p className="status-line">{templateStatus}</p>
      </article>

      <article className="panel">
        <p className="mono-label">Template Details</p>
        {selectedTemplate ? (
          <>
            <h3>{selectedTemplate.name}</h3>
            <p>{selectedTemplate.summary}</p>
            <p className="status-line">{selectedTemplate.recommended_for}</p>

            <p className="mono-label">Config JSON</p>
            <pre className="json-output">{JSON.stringify(selectedTemplate.config, null, 2)}</pre>

            <p className="mono-label">Bootstrap Commands</p>
            <pre className="json-output">
              {JSON.stringify(selectedTemplate.bootstrap_commands, null, 2)}
            </pre>

            <div className="button-row">
              <button
                className="btn-secondary"
                onClick={() =>
                  void copyToClipboard("Config JSON", JSON.stringify(selectedTemplate.config, null, 2))
                }
              >
                Copy Config
              </button>
              <button
                className="btn-secondary"
                onClick={() =>
                  void copyToClipboard(
                    "Bootstrap Commands",
                    JSON.stringify(selectedTemplate.bootstrap_commands, null, 2)
                  )
                }
              >
                Copy Commands
              </button>
              <button className="btn-primary" onClick={() => void applySelectedTemplate()}>
                Apply Template
              </button>
            </div>
            <p className="status-line">{applyStatus}</p>
          </>
        ) : (
          <p>Select a template to inspect deterministic config and bootstrap commands.</p>
        )}
      </article>
    </section>
  );
}

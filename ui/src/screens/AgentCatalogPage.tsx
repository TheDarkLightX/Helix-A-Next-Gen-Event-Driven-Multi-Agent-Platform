import { useEffect, useMemo, useState } from "react";
import {
  applyAgentTemplate,
  DeterministicAgentSpec,
  DeterministicAgentTemplate,
  fetchAgentCatalog,
  fetchAgentTemplates,
} from "../lib/api";
import {
  Panel,
  Button,
  Badge,
  StatusLine,
  CodeBlock,
  Tag,
} from "../components";

const verificationCommands = [
  "cargo test --manifest-path crates/helix-api/Cargo.toml",
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
    if (!selectedTemplate) return;
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
    <section className="hx-page-grid">
      <Panel hero span={12} eyebrow="Agent Catalog" title="High-ROI Deterministic State Machines">
        <p className="hx-description">
          These kernels are pure and replayable. Each includes formal model coverage
          for fail-closed verification.
        </p>
      </Panel>

      <Panel span={5} eyebrow="Verify" title="Verification Commands">
        <div className="hx-list">
          {verificationCommands.map((command) => (
            <div key={command} className="hx-row">
              <code className="hx-mono-detail">{command}</code>
            </div>
          ))}
        </div>
        <StatusLine>{status}</StatusLine>
      </Panel>

      <Panel span={7} eyebrow="Implemented" title="Agents" actions={<Badge tone="accent">{agents.length}</Badge>}>
        {agents.length === 0 ? (
          <div className="hx-table-empty"><p>No agents loaded.</p></div>
        ) : (
          <div className="hx-card-grid">
            {agents.map((agent) => (
              <div key={agent.id} className="hx-card">
                <div className="hx-card-head">
                  <h3>{agent.name}</h3>
                  <Badge tone="ok">Implemented</Badge>
                </div>
                <p className="hx-row-secondary">{agent.roi_rationale}</p>
                <p className="hx-mono-detail">{agent.id}</p>
                <div className="hx-tag-row">
                  <Tag>{agent.kernel_module}</Tag>
                  <Tag>formal model: available</Tag>
                </div>
              </div>
            ))}
          </div>
        )}
      </Panel>

      <Panel span={6} eyebrow="Deploy" title="Templates" actions={<Badge tone="info">{templates.length}</Badge>}>
        {templates.length === 0 ? (
          <div className="hx-table-empty"><p>No templates loaded.</p></div>
        ) : (
          <div className="hx-card-grid">
            {templates.map((template) => (
              <div key={template.id} className="hx-card">
                <div className="hx-card-head">
                  <h3>{template.name}</h3>
                  <Badge tone={template.id === selectedTemplateId ? "accent" : "neutral"}>Template</Badge>
                </div>
                <p className="hx-row-secondary">{template.summary}</p>
                <p className="hx-mono-detail">{template.id}</p>
                <p className="hx-row-secondary">Use for: {template.recommended_for}</p>
                <div className="hx-tag-row">
                  <Tag>required: {template.required_agents.join(", ")}</Tag>
                </div>
                <Button
                  variant={template.id === selectedTemplateId ? "primary" : "secondary"}
                  onClick={() => setSelectedTemplateId(template.id)}
                >
                  {template.id === selectedTemplateId ? "Selected" : "Select"}
                </Button>
              </div>
            ))}
          </div>
        )}
        <StatusLine>{templateStatus}</StatusLine>
      </Panel>

      <Panel span={6} eyebrow="Details" title="Template Config">
        {selectedTemplate ? (
          <div className="hx-list">
            <div className="hx-card-head">
              <h3>{selectedTemplate.name}</h3>
            </div>
            <p className="hx-row-secondary">{selectedTemplate.summary}</p>
            <StatusLine>{selectedTemplate.recommended_for}</StatusLine>
            <CodeBlock label="Config JSON">{JSON.stringify(selectedTemplate.config, null, 2)}</CodeBlock>
            <CodeBlock label="Bootstrap Commands">{JSON.stringify(selectedTemplate.bootstrap_commands, null, 2)}</CodeBlock>
            <div className="hx-cluster">
              <Button variant="secondary" onClick={() => void copyToClipboard("Config JSON", JSON.stringify(selectedTemplate.config, null, 2))}>
                Copy Config
              </Button>
              <Button variant="secondary" onClick={() => void copyToClipboard("Bootstrap Commands", JSON.stringify(selectedTemplate.bootstrap_commands, null, 2))}>
                Copy Commands
              </Button>
              <Button onClick={() => void applySelectedTemplate()}>Apply Template</Button>
            </div>
            <StatusLine>{applyStatus}</StatusLine>
          </div>
        ) : (
          <div className="hx-table-empty"><p>Select a template to inspect deterministic config and bootstrap commands.</p></div>
        )}
      </Panel>
    </section>
  );
}

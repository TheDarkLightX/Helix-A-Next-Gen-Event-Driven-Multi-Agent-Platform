import { FormEvent, useEffect, useMemo, useState } from "react";
import {
  AutomationRule,
  AutomationRuleEvaluationEntry,
  RecipeRunEntry,
  RecipeTriggerPlan,
  RuleOperator,
  evaluateAutomationRules,
  fetchAutomationRuleEvaluations,
  fetchAutomationRules,
  fetchRecipeRuns,
  runRecipeTriggerPlan,
  upsertAutomationRule,
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
  Select,
  CheckboxField,
  StatusLine,
  Tag,
  CodeBlock,
} from "../components";

const OPERATOR_OPTIONS: { value: RuleOperator; label: string }[] = [
  { value: "equals", label: "equals" },
  { value: "contains", label: "contains" },
  { value: "starts_with", label: "starts_with" },
  { value: "greater_than_or_equals", label: "greater_than_or_equals" },
  { value: "exists", label: "exists" },
  { value: "regex_matches", label: "regex_matches" },
];

const DEFAULT_EVENT = JSON.stringify(
  {
    source: "intel",
    type: "intel.case.opened",
    data: {
      severity: "critical",
      case_id: "case_demo_001",
    },
  },
  null,
  2
);

function nextRuleId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return "70000000-0000-4000-8000-000000000001";
}

function parseLiteral(value: string): unknown {
  const trimmed = value.trim();
  if (trimmed === "") return "";
  try {
    return JSON.parse(trimmed);
  } catch {
    return value;
  }
}

function formatJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return "{}";
  }
}

function describeCondition(condition: unknown): string {
  if (!condition || typeof condition !== "object") return "unreadable condition";
  const record = condition as Record<string, unknown>;
  const directField = typeof record.field === "string" ? record.field : null;
  const directOperator = typeof record.operator === "string" ? record.operator : null;
  if (directField && directOperator) return `${directField} ${directOperator}`;

  const nested = record.field;
  if (nested && typeof nested === "object") {
    const fieldCondition = nested as Record<string, unknown>;
    const field = typeof fieldCondition.field === "string" ? fieldCondition.field : "field";
    const operator =
      typeof fieldCondition.operator === "string" ? fieldCondition.operator : "operator";
    return `${field} ${operator}`;
  }

  const logical = ["and", "or", "not"].find((key) => key in record);
  return logical ? logical : "composite condition";
}

function actionTarget(rule: AutomationRule): string {
  const action = rule.actions[0];
  if (!action) return "no action";
  if (action.recipe_id) return `recipe_id: ${action.recipe_id}`;
  if (action.recipe_name) return `recipe_name: ${action.recipe_name}`;
  return "missing target";
}

export function RulesPage() {
  const [rules, setRules] = useState<AutomationRule[]>([]);
  const [persistenceEnabled, setPersistenceEnabled] = useState(false);
  const [status, setStatus] = useState("Loading automation rules...");
  const [name, setName] = useState("Critical case brief");
  const [field, setField] = useState("event.data.severity");
  const [operator, setOperator] = useState<RuleOperator>("equals");
  const [literal, setLiteral] = useState('"critical"');
  const [recipeId, setRecipeId] = useState("70000000-0000-4000-8000-000000000002");
  const [parameterName, setParameterName] = useState("case_id");
  const [parameterPath, setParameterPath] = useState("event.data.case_id");
  const [enabled, setEnabled] = useState(true);
  const [eventJson, setEventJson] = useState(DEFAULT_EVENT);
  const [plans, setPlans] = useState<RecipeTriggerPlan[]>([]);
  const [lastEvaluationId, setLastEvaluationId] = useState<number | null>(null);
  const [evaluations, setEvaluations] = useState<AutomationRuleEvaluationEntry[]>([]);
  const [historyPersistenceEnabled, setHistoryPersistenceEnabled] = useState(false);
  const [historyStatus, setHistoryStatus] = useState("Loading evaluation history...");
  const [runs, setRuns] = useState<RecipeRunEntry[]>([]);
  const [runPersistenceEnabled, setRunPersistenceEnabled] = useState(false);
  const [runStatus, setRunStatus] = useState("Loading recipe runs...");

  const enabledCount = useMemo(() => rules.filter((rule) => rule.enabled !== false).length, [rules]);

  async function loadRules() {
    try {
      const response = await fetchAutomationRules();
      setRules(response.rules);
      setPersistenceEnabled(response.persistence_enabled);
      setStatus(
        response.persistence_enabled
          ? `Loaded ${response.rules.length} durable rule(s).`
          : `Loaded ${response.rules.length} in-memory rule(s).`
      );
    } catch (error) {
      setStatus(`Failed to load rules: ${(error as Error).message}`);
    }
  }

  useEffect(() => {
    void loadRules();
    void loadEvaluationHistory();
    void loadRecipeRuns();
  }, []);

  async function loadEvaluationHistory() {
    try {
      const response = await fetchAutomationRuleEvaluations(25);
      setEvaluations(response.entries);
      setHistoryPersistenceEnabled(response.persistence_enabled);
      setHistoryStatus(
        response.persistence_enabled
          ? `Loaded ${response.entries.length} durable evaluation(s).`
          : "Evaluation history is disabled because DATABASE_URL is not configured."
      );
    } catch (error) {
      setHistoryStatus(`Failed to load evaluation history: ${(error as Error).message}`);
    }
  }

  async function loadRecipeRuns() {
    try {
      const response = await fetchRecipeRuns(25);
      setRuns(response.entries);
      setRunPersistenceEnabled(response.persistence_enabled);
      setRunStatus(
        response.persistence_enabled
          ? `Loaded ${response.entries.length} durable recipe run(s).`
          : "Recipe run history is disabled because DATABASE_URL is not configured."
      );
    } catch (error) {
      setRunStatus(`Failed to load recipe runs: ${(error as Error).message}`);
    }
  }

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setStatus("Saving automation rule...");

    const rule: AutomationRule = {
      id: nextRuleId(),
      name,
      version: "1.0.0",
      enabled,
      tags: ["operator", "automation"],
      metadata: {},
      condition: {
        field,
        operator,
        value: operator === "exists" ? undefined : parseLiteral(literal),
      },
      actions: [
        {
          type: "trigger_recipe",
          recipe_id: recipeId.trim(),
          parameters: {
            [parameterName.trim() || "case_id"]: { from_event: parameterPath.trim() },
            mode: { literal: "prepare_brief" },
          },
        },
      ],
    };

    try {
      const saved = await upsertAutomationRule(rule);
      setRules((current) =>
        [...current.filter((existing) => existing.id !== saved.id), saved].sort((left, right) =>
          left.id.localeCompare(right.id)
        )
      );
      setStatus(`Saved ${saved.name}.`);
    } catch (error) {
      setStatus(`Save failed: ${(error as Error).message}`);
    }
  }

  async function onEvaluate() {
    setStatus("Evaluating rule set...");
    try {
      const parsed = JSON.parse(eventJson) as { source?: unknown; type?: unknown; data?: unknown };
      if (typeof parsed.source !== "string" || typeof parsed.type !== "string") {
        setStatus("Evaluate failed: event source and type must be strings.");
        return;
      }
      const result = await evaluateAutomationRules({
        source: parsed.source,
        type: parsed.type,
        data: parsed.data,
      });
      setPlans(result.trigger_plans);
      setLastEvaluationId(result.evaluation?.id ?? null);
      if (result.evaluation) {
        setEvaluations((current) => [
          result.evaluation as AutomationRuleEvaluationEntry,
          ...current.filter((entry) => entry.id !== result.evaluation?.id),
        ]);
        setHistoryPersistenceEnabled(true);
      }
      setStatus(
        `Evaluated ${result.rule_count} rule(s); ${result.trigger_plans.length} trigger plan(s).`
      );
    } catch (error) {
      setStatus(`Evaluate failed: ${(error as Error).message}`);
    }
  }

  async function onRunPlan(plan: RecipeTriggerPlan) {
    setRunStatus("Running trigger plan...");
    try {
      const result = await runRecipeTriggerPlan(plan, lastEvaluationId);
      if (result.run) {
        setRuns((current) => [
          result.run as RecipeRunEntry,
          ...current.filter((entry) => entry.id !== result.run?.id),
        ]);
        setRunPersistenceEnabled(result.persistence_enabled);
        setRunStatus(
          `${result.run.status === "completed" ? "Completed" : "Failed"} recipe run ${result.run.id}.`
        );
      } else {
        setRunPersistenceEnabled(false);
        setRunStatus("Recipe run history is disabled because DATABASE_URL is not configured.");
      }
    } catch (error) {
      setRunStatus(`Run failed: ${(error as Error).message}`);
    }
  }

  return (
    <section className="hx-page-grid">
      <Panel hero span={12} eyebrow="Automation Rules" title="Event Rules With Durable Trigger Plans">
        <p className="hx-description">
          Store deterministic match rules, evaluate CloudEvents, and inspect the recipe trigger
          packets before automation reaches the runtime.
        </p>
      </Panel>

      <Panel
        span={4}
        eyebrow="Rule Status"
        title="Workspace Metrics"
        actions={
          <Button variant="secondary" type="button" onClick={() => void loadRules()}>
            Refresh
          </Button>
        }
      >
        <StatGrid>
          <StatCard label="rules" value={rules.length} tone="info" />
          <StatCard label="enabled" value={enabledCount} tone="ok" />
        </StatGrid>
        <div className="hx-tag-row">
          <Badge tone={persistenceEnabled ? "ok" : "warn"}>
            {persistenceEnabled ? "durable" : "in-memory"}
          </Badge>
        </div>
        <StatusLine>{status}</StatusLine>
      </Panel>

      <Panel span={8} eyebrow="Create Rule" title="Rule Builder">
        <form className="hx-form-grid" onSubmit={onSubmit}>
          <FormField label="name" full>
            <Input value={name} onChange={(event) => setName(event.target.value)} />
          </FormField>

          <FormField label="match_field">
            <Input value={field} onChange={(event) => setField(event.target.value)} />
          </FormField>

          <FormField label="operator">
            <Select
              value={operator}
              onChange={(event) => setOperator(event.target.value as RuleOperator)}
            >
              {OPERATOR_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </Select>
          </FormField>

          <FormField label="value_json">
            <Input
              disabled={operator === "exists"}
              value={literal}
              onChange={(event) => setLiteral(event.target.value)}
            />
          </FormField>

          <FormField label="recipe_id">
            <Input value={recipeId} onChange={(event) => setRecipeId(event.target.value)} />
          </FormField>

          <FormField label="parameter">
            <Input
              value={parameterName}
              onChange={(event) => setParameterName(event.target.value)}
            />
          </FormField>

          <FormField label="from_event">
            <Input
              value={parameterPath}
              onChange={(event) => setParameterPath(event.target.value)}
            />
          </FormField>

          <CheckboxField
            label="enabled"
            checked={enabled}
            onChange={(checked) => setEnabled(checked)}
            full
          />

          <div className="hx-cluster" style={{ gridColumn: "1 / -1" }}>
            <Button type="submit">Save Rule</Button>
          </div>
        </form>
      </Panel>

      <Panel
        span={6}
        eyebrow="Rule Catalog"
        title="Stored Definitions"
        actions={<Badge tone="info">{rules.length} rule(s)</Badge>}
      >
        <StatusLine>{rules.length} stored rule definition(s).</StatusLine>
        {rules.length === 0 ? (
          <div className="hx-table-empty">
            <p>No automation rules are stored in the current workspace.</p>
          </div>
        ) : (
          <div className="hx-list">
            {rules.map((rule) => (
              <div key={rule.id} className="hx-row">
                <div className="hx-row-stack">
                  <div className="hx-row-primary">
                    <h3>{rule.name}</h3>
                    <Badge tone={rule.enabled !== false ? "ok" : "warn"}>
                      {rule.enabled !== false ? "enabled" : "paused"}
                    </Badge>
                  </div>
                  <p className="hx-mono-detail">{rule.id}</p>
                  <div className="hx-tag-row">
                    <Tag>{describeCondition(rule.condition)}</Tag>
                    <Tag>{actionTarget(rule)}</Tag>
                  </div>
                </div>
                <CodeBlock>{formatJson(rule.actions)}</CodeBlock>
              </div>
            ))}
          </div>
        )}
      </Panel>

      <Panel
        span={6}
        eyebrow="Evaluate Event"
        title="Trigger Plan Preview"
        actions={
          <Button variant="secondary" type="button" onClick={() => void onEvaluate()}>
            Evaluate
          </Button>
        }
      >
        <StatusLine>{plans.length} trigger plan(s) from latest evaluation.</StatusLine>
        <FormField label="event_json" full>
          <Textarea
            rows={9}
            value={eventJson}
            onChange={(event) => setEventJson(event.target.value)}
          />
        </FormField>
        {plans.length === 0 ? (
          <div className="hx-table-empty">
            <p>No trigger plans have been produced.</p>
          </div>
        ) : (
          <div className="hx-list">
            {plans.map((plan) => (
              <div
                key={`${plan.rule_id}:${plan.action_id ?? plan.recipe_id ?? plan.recipe_name}`}
                className="hx-row hx-row--info"
              >
                <div className="hx-row-stack">
                  <div className="hx-row-primary">
                    <h3>{plan.rule_name}</h3>
                    <div className="hx-cluster">
                      <Badge tone="info">trigger</Badge>
                      <Button
                        variant="secondary"
                        type="button"
                        onClick={() => void onRunPlan(plan)}
                      >
                        Run
                      </Button>
                    </div>
                  </div>
                  <div className="hx-tag-row">
                    {plan.recipe_id ? <Tag>recipe_id: {plan.recipe_id}</Tag> : null}
                    {plan.recipe_name ? <Tag>recipe_name: {plan.recipe_name}</Tag> : null}
                  </div>
                </div>
                <CodeBlock>{formatJson(plan.parameters)}</CodeBlock>
              </div>
            ))}
          </div>
        )}
      </Panel>

      <Panel
        span={12}
        eyebrow="Evaluation History"
        title="Persisted Evaluations"
        actions={
          <div className="hx-cluster">
            <Badge tone={historyPersistenceEnabled ? "ok" : "warn"}>
              {historyPersistenceEnabled ? "durable" : "not persisted"}
            </Badge>
            <Button
              variant="secondary"
              type="button"
              onClick={() => void loadEvaluationHistory()}
            >
              Refresh
            </Button>
          </div>
        }
      >
        <StatusLine>{historyStatus}</StatusLine>
        {evaluations.length === 0 ? (
          <div className="hx-table-empty">
            <p>No rule evaluations have been persisted.</p>
          </div>
        ) : (
          <div className="hx-list">
            {evaluations.map((entry) => (
              <div key={entry.id} className="hx-row">
                <div className="hx-row-stack">
                  <div className="hx-row-primary">
                    <h3>{entry.event_type}</h3>
                    <Badge tone={entry.trigger_plan_count > 0 ? "ok" : "warn"}>
                      {entry.trigger_plan_count} plan(s)
                    </Badge>
                  </div>
                  <p className="hx-mono-detail">{entry.event_id}</p>
                  <div className="hx-tag-row">
                    <Tag>source: {entry.event_source}</Tag>
                    <Tag>rules: {entry.rule_count}</Tag>
                    <Tag>created: {entry.created_at}</Tag>
                  </div>
                </div>
                <CodeBlock>
                  {formatJson({
                    event: entry.event,
                    trigger_plans: entry.trigger_plans,
                  })}
                </CodeBlock>
              </div>
            ))}
          </div>
        )}
      </Panel>

      <Panel
        span={12}
        eyebrow="Recipe Run History"
        title="Automation Executions"
        actions={
          <div className="hx-cluster">
            <Badge tone={runPersistenceEnabled ? "ok" : "warn"}>
              {runPersistenceEnabled ? "durable" : "not persisted"}
            </Badge>
            <Button variant="secondary" type="button" onClick={() => void loadRecipeRuns()}>
              Refresh
            </Button>
          </div>
        }
      >
        <StatusLine>{runStatus}</StatusLine>
        {runs.length === 0 ? (
          <div className="hx-table-empty">
            <p>No recipe runs have been persisted.</p>
          </div>
        ) : (
          <div className="hx-list">
            {runs.map((entry) => (
              <div key={entry.id} className="hx-row">
                <div className="hx-row-stack">
                  <div className="hx-row-primary">
                    <h3>
                      {entry.resolved_recipe_name ??
                        entry.requested_recipe_name ??
                        "unresolved recipe"}
                    </h3>
                    <Badge tone={entry.status === "completed" ? "ok" : "warn"}>
                      {entry.status}
                    </Badge>
                  </div>
                  <p className="hx-mono-detail">run {entry.id}</p>
                  <div className="hx-tag-row">
                    {entry.evaluation_id ? (
                      <Tag>evaluation: {entry.evaluation_id}</Tag>
                    ) : null}
                    {entry.resolved_recipe_id ? (
                      <Tag>recipe_id: {entry.resolved_recipe_id}</Tag>
                    ) : null}
                    <Tag>agents: {entry.started_agent_ids.length}</Tag>
                    <Tag>created: {entry.created_at}</Tag>
                  </div>
                </div>
                {entry.error ? <StatusLine>reason: {entry.error}</StatusLine> : null}
                <CodeBlock>
                  {formatJson({
                    trigger_plan: entry.trigger_plan,
                    parameters: entry.parameters,
                    started_agent_ids: entry.started_agent_ids,
                    emitted_events: entry.emitted_events,
                    state_snapshots: entry.state_snapshots,
                  })}
                </CodeBlock>
              </div>
            ))}
          </div>
        )}
      </Panel>
    </section>
  );
}

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
    <section className="dashboard-grid">
      <article className="panel panel-hero panel-span-12">
        <p className="mono-label">Automation Rules</p>
        <h2>Event Rules With Durable Trigger Plans</h2>
        <p>
          Store deterministic match rules, evaluate CloudEvents, and inspect the recipe trigger
          packets before automation reaches the runtime.
        </p>
      </article>

      <article className="panel panel-span-4">
        <p className="mono-label">Rule Status</p>
        <div className="metrics-grid">
          <div className="metric-card">
            <span className="metric-label">rules</span>
            <strong className="metric-value">{rules.length}</strong>
          </div>
          <div className="metric-card">
            <span className="metric-label">enabled</span>
            <strong className="metric-value">{enabledCount}</strong>
          </div>
        </div>
        <div className="pill-row">
          <span className={`status-pill ${persistenceEnabled ? "ok" : "warn"}`}>
            {persistenceEnabled ? "durable" : "in-memory"}
          </span>
          <button className="btn-secondary" type="button" onClick={() => void loadRules()}>
            Refresh
          </button>
        </div>
        <p className="status-line">{status}</p>
      </article>

      <article className="panel panel-span-8">
        <p className="mono-label">Create Rule</p>
        <form className="form-grid" onSubmit={onSubmit}>
          <label className="field field-full">
            <span>name</span>
            <input value={name} onChange={(event) => setName(event.target.value)} />
          </label>

          <label className="field">
            <span>match_field</span>
            <input value={field} onChange={(event) => setField(event.target.value)} />
          </label>

          <label className="field">
            <span>operator</span>
            <select
              value={operator}
              onChange={(event) => setOperator(event.target.value as RuleOperator)}
            >
              {OPERATOR_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>

          <label className="field">
            <span>value_json</span>
            <input
              disabled={operator === "exists"}
              value={literal}
              onChange={(event) => setLiteral(event.target.value)}
            />
          </label>

          <label className="field">
            <span>recipe_id</span>
            <input value={recipeId} onChange={(event) => setRecipeId(event.target.value)} />
          </label>

          <label className="field">
            <span>parameter</span>
            <input
              value={parameterName}
              onChange={(event) => setParameterName(event.target.value)}
            />
          </label>

          <label className="field">
            <span>from_event</span>
            <input
              value={parameterPath}
              onChange={(event) => setParameterPath(event.target.value)}
            />
          </label>

          <label className="field checkbox-field">
            <input
              type="checkbox"
              checked={enabled}
              onChange={(event) => setEnabled(event.target.checked)}
            />
            <span>enabled</span>
          </label>

          <button className="btn-primary" type="submit">
            Save Rule
          </button>
        </form>
      </article>

      <article className="panel panel-span-6">
        <div className="panel-toolbar">
          <div>
            <p className="mono-label">Rule Catalog</p>
            <p className="status-line">{rules.length} stored rule definition(s).</p>
          </div>
        </div>
        <div className="command-list">
          {rules.length === 0 ? (
            <p className="panel-note">No automation rules are stored in the current workspace.</p>
          ) : (
            rules.map((rule) => (
              <div key={rule.id} className="command-row">
                <div className="command-row-head">
                  <h3>{rule.name}</h3>
                  <span className={`status-pill ${rule.enabled !== false ? "ok" : "warn"}`}>
                    {rule.enabled !== false ? "enabled" : "paused"}
                  </span>
                </div>
                <p className="mono-detail">{rule.id}</p>
                <div className="pill-row">
                  <span className="info-pill">{describeCondition(rule.condition)}</span>
                  <span className="info-pill">{actionTarget(rule)}</span>
                </div>
                <pre className="json-block">{formatJson(rule.actions)}</pre>
              </div>
            ))
          )}
        </div>
      </article>

      <article className="panel panel-span-6">
        <div className="panel-toolbar">
          <div>
            <p className="mono-label">Evaluate Event</p>
            <p className="status-line">{plans.length} trigger plan(s) from latest evaluation.</p>
          </div>
          <button className="btn-secondary" type="button" onClick={() => void onEvaluate()}>
            Evaluate
          </button>
        </div>
        <label className="field field-full">
          <span>event_json</span>
          <textarea
            rows={9}
            value={eventJson}
            onChange={(event) => setEventJson(event.target.value)}
          />
        </label>
        <div className="command-list rule-plan-list">
          {plans.length === 0 ? (
            <p className="panel-note">No trigger plans have been produced.</p>
          ) : (
            plans.map((plan) => (
              <div key={`${plan.rule_id}:${plan.action_id ?? plan.recipe_id ?? plan.recipe_name}`} className="command-row">
                <div className="command-row-head">
                  <h3>{plan.rule_name}</h3>
                  <div className="toolbar-actions">
                    <span className="status-pill info">trigger</span>
                    <button
                      className="btn-secondary"
                      type="button"
                      onClick={() => void onRunPlan(plan)}
                    >
                      Run
                    </button>
                  </div>
                </div>
                <div className="pill-row">
                  {plan.recipe_id ? <span className="info-pill">recipe_id: {plan.recipe_id}</span> : null}
                  {plan.recipe_name ? (
                    <span className="info-pill">recipe_name: {plan.recipe_name}</span>
                  ) : null}
                </div>
                <pre className="json-block">{formatJson(plan.parameters)}</pre>
              </div>
            ))
          )}
        </div>
      </article>

      <article className="panel panel-span-12">
        <div className="panel-toolbar">
          <div>
            <p className="mono-label">Evaluation History</p>
            <p className="status-line">{historyStatus}</p>
          </div>
          <div className="toolbar-actions">
            <span className={`status-pill ${historyPersistenceEnabled ? "ok" : "warn"}`}>
              {historyPersistenceEnabled ? "durable" : "not persisted"}
            </span>
            <button
              className="btn-secondary"
              type="button"
              onClick={() => void loadEvaluationHistory()}
            >
              Refresh
            </button>
          </div>
        </div>
        <div className="command-list">
          {evaluations.length === 0 ? (
            <p className="panel-note">No rule evaluations have been persisted.</p>
          ) : (
            evaluations.map((entry) => (
              <div key={entry.id} className="command-row">
                <div className="command-row-head">
                  <h3>{entry.event_type}</h3>
                  <span className={`status-pill ${entry.trigger_plan_count > 0 ? "ok" : "warn"}`}>
                    {entry.trigger_plan_count} plan(s)
                  </span>
                </div>
                <p className="mono-detail">{entry.event_id}</p>
                <div className="pill-row">
                  <span className="info-pill">source: {entry.event_source}</span>
                  <span className="info-pill">rules: {entry.rule_count}</span>
                  <span className="info-pill">created: {entry.created_at}</span>
                </div>
                <pre className="json-block">
                  {formatJson({
                    event: entry.event,
                    trigger_plans: entry.trigger_plans,
                  })}
                </pre>
              </div>
            ))
          )}
        </div>
      </article>

      <article className="panel panel-span-12">
        <div className="panel-toolbar">
          <div>
            <p className="mono-label">Recipe Run History</p>
            <p className="status-line">{runStatus}</p>
          </div>
          <div className="toolbar-actions">
            <span className={`status-pill ${runPersistenceEnabled ? "ok" : "warn"}`}>
              {runPersistenceEnabled ? "durable" : "not persisted"}
            </span>
            <button className="btn-secondary" type="button" onClick={() => void loadRecipeRuns()}>
              Refresh
            </button>
          </div>
        </div>
        <div className="command-list">
          {runs.length === 0 ? (
            <p className="panel-note">No recipe runs have been persisted.</p>
          ) : (
            runs.map((entry) => (
              <div key={entry.id} className="command-row">
                <div className="command-row-head">
                  <h3>{entry.resolved_recipe_name ?? entry.requested_recipe_name ?? "unresolved recipe"}</h3>
                  <span className={`status-pill ${entry.status === "completed" ? "ok" : "warn"}`}>
                    {entry.status}
                  </span>
                </div>
                <p className="mono-detail">run {entry.id}</p>
                <div className="pill-row">
                  {entry.evaluation_id ? (
                    <span className="info-pill">evaluation: {entry.evaluation_id}</span>
                  ) : null}
                  {entry.resolved_recipe_id ? (
                    <span className="info-pill">recipe_id: {entry.resolved_recipe_id}</span>
                  ) : null}
                  <span className="info-pill">agents: {entry.started_agent_ids.length}</span>
                  <span className="info-pill">created: {entry.created_at}</span>
                </div>
                {entry.error ? <p className="status-line">reason: {entry.error}</p> : null}
                <pre className="json-block">
                  {formatJson({
                    trigger_plan: entry.trigger_plan,
                    parameters: entry.parameters,
                    started_agent_ids: entry.started_agent_ids,
                    emitted_events: entry.emitted_events,
                    state_snapshots: entry.state_snapshots,
                  })}
                </pre>
              </div>
            ))
          )}
        </div>
      </article>
    </section>
  );
}

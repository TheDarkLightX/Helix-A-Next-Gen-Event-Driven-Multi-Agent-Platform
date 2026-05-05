DO $$
BEGIN
  CREATE TYPE agent_runtime_type AS ENUM ('native', 'wasm');
EXCEPTION
  WHEN duplicate_object THEN NULL;
END
$$;

CREATE TABLE IF NOT EXISTS agent_configurations (
  id uuid PRIMARY KEY,
  profile_id uuid NOT NULL,
  name text,
  agent_kind text NOT NULL,
  agent_runtime agent_runtime_type NOT NULL DEFAULT 'native',
  wasm_module_path text,
  config_data jsonb NOT NULL DEFAULT '{}'::jsonb,
  credential_ids uuid[] NOT NULL DEFAULT '{}',
  enabled boolean NOT NULL DEFAULT true,
  dependencies uuid[] NOT NULL DEFAULT '{}',
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_agent_configurations_profile
  ON agent_configurations (profile_id);

CREATE TABLE IF NOT EXISTS recipes (
  id uuid PRIMARY KEY,
  profile_id uuid NOT NULL,
  name text NOT NULL,
  description text,
  trigger jsonb,
  graph_definition jsonb NOT NULL,
  enabled boolean NOT NULL DEFAULT true,
  version text,
  tags text[] NOT NULL DEFAULT '{}',
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_recipes_profile
  ON recipes (profile_id);

CREATE TABLE IF NOT EXISTS automation_rules (
  id uuid PRIMARY KEY,
  name text NOT NULL,
  enabled boolean NOT NULL DEFAULT true,
  record jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_automation_rules_enabled
  ON automation_rules (enabled);

CREATE TABLE IF NOT EXISTS automation_rule_evaluations (
  id bigserial PRIMARY KEY,
  event_id uuid NOT NULL,
  event_type text NOT NULL,
  event_source text NOT NULL,
  event jsonb NOT NULL,
  rule_count integer NOT NULL,
  trigger_plan_count integer NOT NULL,
  trigger_plans jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_automation_rule_evaluations_created
  ON automation_rule_evaluations (created_at DESC);

CREATE INDEX IF NOT EXISTS idx_automation_rule_evaluations_event_type
  ON automation_rule_evaluations (event_type);

CREATE TABLE IF NOT EXISTS recipe_runs (
  id bigserial PRIMARY KEY,
  evaluation_id bigint REFERENCES automation_rule_evaluations(id) ON DELETE SET NULL,
  rule_id uuid NOT NULL,
  action_id text,
  requested_recipe_id uuid,
  requested_recipe_name text,
  resolved_recipe_id uuid,
  resolved_recipe_name text,
  trigger_plan jsonb NOT NULL,
  parameters jsonb NOT NULL,
  status text NOT NULL,
  started_agent_ids uuid[] NOT NULL DEFAULT '{}',
  error text,
  created_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE recipe_runs
  ADD COLUMN IF NOT EXISTS emitted_events jsonb NOT NULL DEFAULT '[]'::jsonb;

ALTER TABLE recipe_runs
  ADD COLUMN IF NOT EXISTS state_snapshots jsonb NOT NULL DEFAULT '{}'::jsonb;

CREATE INDEX IF NOT EXISTS idx_recipe_runs_created
  ON recipe_runs (created_at DESC);

CREATE INDEX IF NOT EXISTS idx_recipe_runs_status
  ON recipe_runs (status);

CREATE INDEX IF NOT EXISTS idx_recipe_runs_resolved_recipe
  ON recipe_runs (resolved_recipe_id);

CREATE TABLE IF NOT EXISTS credentials (
  id uuid PRIMARY KEY,
  profile_id uuid NOT NULL,
  name text NOT NULL,
  kind text NOT NULL,
  encrypted_data text NOT NULL,
  metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (profile_id, name)
);

CREATE INDEX IF NOT EXISTS idx_credentials_profile
  ON credentials (profile_id);

CREATE TABLE IF NOT EXISTS agent_states (
  profile_id uuid NOT NULL,
  agent_id uuid NOT NULL,
  data jsonb NOT NULL DEFAULT '{}'::jsonb,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (profile_id, agent_id)
);

CREATE TABLE IF NOT EXISTS intel_sources (
  id text PRIMARY KEY,
  record jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS intel_watchlists (
  id text PRIMARY KEY,
  record jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS intel_evidence (
  id text PRIMARY KEY,
  record jsonb NOT NULL,
  source_id text,
  observed_at text,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_intel_evidence_source
  ON intel_evidence (source_id);

CREATE TABLE IF NOT EXISTS intel_claims (
  id text PRIMARY KEY,
  record jsonb NOT NULL,
  evidence_id text,
  review_status text,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_intel_claims_evidence
  ON intel_claims (evidence_id);

CREATE TABLE IF NOT EXISTS intel_cases (
  id text PRIMARY KEY,
  record jsonb NOT NULL,
  status text,
  watchlist_id text,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_intel_cases_status
  ON intel_cases (status);

CREATE TABLE IF NOT EXISTS policy_config_snapshots (
  id bigserial PRIMARY KEY,
  config jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS autopilot_guard_snapshots (
  id bigserial PRIMARY KEY,
  config jsonb NOT NULL,
  stats jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS audit_log (
  id bigserial PRIMARY KEY,
  subject text NOT NULL,
  action text NOT NULL,
  resource text NOT NULL,
  decision text NOT NULL,
  reason text,
  metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
  created_at timestamptz NOT NULL DEFAULT now()
);

export type DeterministicPolicyConfig = {
  dedup_window_ticks: number;
  rate_max_tokens: number;
  rate_refill_per_tick: number;
  breaker_failure_threshold: number;
  breaker_open_duration_ticks: number;
  retry_budget: number;
  approval_quorum: number;
  approval_reviewers: number;
  backpressure_soft_limit: number;
  backpressure_hard_limit: number;
  sla_deadline_ticks: number;
  dlq_max_consecutive_failures: number;
  nonce_start: number;
  nonce_max_in_flight: number;
  fee_base_fee: number;
  fee_priority_fee: number;
  fee_bump_bps: number;
  fee_max_fee_cap: number;
  finality_required_depth: number;
  allowlist_chain_id: number;
  allowlist_contract_tag: number;
  allowlist_method_tag: number;
};

export type PolicyCommand =
  | { type: "tick" }
  | { type: "request"; fingerprint: number; cost: number }
  | { type: "success" }
  | { type: "failure" }
  | { type: "retry" }
  | { type: "reset_retry" }
  | { type: "approve" }
  | { type: "reject" }
  | { type: "reset_approvals" }
  | { type: "start_sla_window" }
  | { type: "complete_sla_window" }
  | { type: "reset_sla_window" }
  | { type: "enqueue_backpressure"; count: number }
  | { type: "dequeue_backpressure"; count: number }
  | { type: "reset_dlq" }
  | { type: "nonce_reserve" }
  | { type: "nonce_confirm"; nonce: number }
  | { type: "nonce_reconcile"; chain_next_nonce: number }
  | { type: "fee_update_base_fee"; base_fee: number }
  | { type: "fee_quote"; urgent: boolean }
  | { type: "fee_rejected" }
  | { type: "fee_confirmed" }
  | { type: "finality_observe_depth"; depth: number }
  | { type: "finality_mark_reorg" }
  | { type: "finality_reset" }
  | { type: "allowlist_evaluate"; chain_id: number; contract_tag: number; method_tag: number }
  | { type: "allowlist_pause" }
  | { type: "allowlist_resume" };

export type PolicyStepResult = {
  command: PolicyCommand;
  decision: {
    kind: string;
    reason?: string;
    allowed?: boolean;
    decision?: string;
    status?: string;
    route?: string;
    outcome?: string;
    quoted?: boolean;
    nonce?: number;
    next_nonce?: number;
    max_fee?: number;
    max_priority_fee?: number;
    rejection_count?: number;
    state?: string;
    remaining_depth?: number;
  };
  snapshot: {
    rate_tokens: number;
    breaker_phase: string;
    retry_remaining: number;
    queue_depth: number;
    dlq_consecutive_failures: number;
    sla_active: boolean;
    sla_remaining_ticks: number;
    sla_expired: boolean;
    nonce_next: number;
    nonce_in_flight: number;
    fee_rejection_count: number;
    finality_observed_depth: number;
    finality_finalized: boolean;
    finality_reorg_detected: boolean;
    allowlist_paused: boolean;
  };
};

export type DeterministicAgentSpec = {
  id: string;
  name: string;
  roi_rationale: string;
  kernel_module: string;
  formal_model: string;
};

export type DeterministicAgentTemplate = {
  id: string;
  name: string;
  summary: string;
  recommended_for: string;
  required_agents: string[];
  config: DeterministicPolicyConfig;
  bootstrap_commands: PolicyCommand[];
};

export type AgentCatalogQuality = {
  total_agents: number;
  foundational_agents: number;
  expanded_agents: number;
  expanded_categories: number;
  temporal_inputs: number;
  temporal_decisions: number;
  huginn_baseline_agents: number;
  exceeds_huginn: boolean;
};

export type ApplyAgentTemplateResponse = {
  template: DeterministicAgentTemplate;
  config: DeterministicPolicyConfig;
  bootstrap_steps: PolicyStepResult[] | null;
};

export type AutopilotMode = "off" | "assist" | "auto";

export type AutopilotGuardConfig = {
  mode: AutopilotMode;
  allow_onchain: boolean;
  require_onchain_confirmation: boolean;
  require_onchain_dry_run: boolean;
  max_policy_commands: number;
};

export type AutopilotStatusResponse = {
  config: AutopilotGuardConfig;
  stats: {
    evaluations: number;
    denied: number;
  };
};

export type OnchainBroadcastRequest = {
  rpc_url: string;
  raw_tx_hex: string;
  await_receipt?: boolean;
  max_poll_rounds?: number;
  poll_interval_ms?: number;
  dry_run?: boolean;
};

export type AutopilotExecuteRequest = {
  confirmed_by_human: boolean;
  action:
    | { type: "policy_simulation"; commands: PolicyCommand[] }
    | { type: "onchain_broadcast"; request: OnchainBroadcastRequest };
};

export type AutopilotExecuteResponse = {
  allowed: boolean;
  reason: string | null;
  requires_confirmation: boolean;
  result: unknown | null;
};

export type AutopilotProposeKind = "policy_simulation" | "onchain_broadcast";

export type AutopilotProposeRequest = {
  goal: string;
  kind: AutopilotProposeKind;
  rpc_url?: string | null;
  raw_tx_hex?: string | null;
  dry_run?: boolean | null;
};

export type AutopilotProposeResponse = {
  model: string;
  raw: string;
  action: AutopilotExecuteRequest["action"];
  guard_preview: {
    action_class: unknown;
    decision_unconfirmed: unknown;
    decision_confirmed: unknown;
  };
};

export type AutopilotProposeErrorResponse = {
  error: string;
  model: string | null;
  raw: string | null;
};

export type AutopilotProposeResult =
  | { ok: true; response: AutopilotProposeResponse }
  | { ok: false; status: number; error: AutopilotProposeErrorResponse };

export type OnchainBroadcastResponse = {
  phase: "Idle" | "Submitting" | "PendingReceipt" | "Confirmed" | "Reverted" | "Failed";
  tx_hash: string | null;
  poll_rounds: number;
  max_poll_rounds: number;
  receipt: {
    transactionHash?: string;
    status?: string;
    blockNumber?: string;
  } | null;
};

export type OnchainReceiptResponse = {
  found: boolean;
  receipt: {
    transactionHash?: string;
    status?: string;
    blockNumber?: string;
  } | null;
};

export type SourceKind =
  | "rss_feed"
  | "website_diff"
  | "json_api"
  | "webhook_ingest"
  | "email_digest"
  | "file_import";

export type WatchlistSeverity = "low" | "medium" | "high" | "critical";

export type ClaimReviewStatus = "needs_review" | "corroborated" | "rejected";

export type SourceDefinition = {
  id: string;
  name: string;
  description: string;
  kind: SourceKind;
  cadence_minutes: number;
  trust_score: number;
  enabled: boolean;
  tags: string[];
};

export type Watchlist = {
  id: string;
  name: string;
  description: string;
  keywords: string[];
  entities: string[];
  min_source_trust: number;
  severity: WatchlistSeverity;
  enabled: boolean;
};

export type ProposedClaim = {
  subject: string;
  predicate: string;
  object: string;
  confidence_bps: number;
  rationale?: string | null;
};

export type EvidenceItem = {
  id: string;
  source_id: string;
  title: string;
  summary: string;
  content: string;
  url: string | null;
  observed_at: string;
  tags: string[];
  entity_labels: string[];
  provenance_hash: string;
};

export type ClaimRecord = {
  id: string;
  evidence_id: string;
  subject: string;
  predicate: string;
  object: string;
  confidence_bps: number;
  review_status: ClaimReviewStatus;
  rationale: string;
};

export type WatchlistHit = {
  watchlist_id: string;
  watchlist_name: string;
  evidence_id: string;
  severity: WatchlistSeverity;
  matched_keywords: string[];
  matched_entities: string[];
  reason: string;
};

export type CaseStatus = "open" | "monitoring" | "brief_ready" | "escalated" | "closed";

export type CaseFile = {
  id: string;
  title: string;
  watchlist_id: string;
  status: CaseStatus;
  primary_entity: string | null;
  evidence_ids: string[];
  claim_ids: string[];
  latest_reason: string;
  briefing_summary: string | null;
};

export type CaseCommand =
  | { type: "mark_monitoring" }
  | { type: "attach_brief"; summary: string }
  | { type: "escalate"; reason: string }
  | { type: "close" }
  | { type: "reopen"; reason: string };

export type CaseTransition = {
  case: CaseFile;
  decision:
    | { kind: "opened" }
    | { kind: "updated" }
    | { kind: "status_changed"; status: CaseStatus }
    | { kind: "denied"; reason: string };
};

export type IntelDeskOverviewResponse = {
  source_count: number;
  watchlist_count: number;
  evidence_count: number;
  claim_count: number;
  open_case_count: number;
  escalated_case_count: number;
};

export type MarketIntelThemeCard = {
  theme_id: string;
  name: string;
  summary: string;
  watchlist_count: number;
  evidence_count: number;
  active_case_count: number;
  escalated_case_count: number;
  top_entities: string[];
};

export type MarketIntelCompanyCard = {
  company: string;
  mention_count: number;
  claim_count: number;
  active_case_count: number;
  themes: string[];
  latest_signal_at: string | null;
};

export type MarketIntelPlaybook = {
  id: string;
  name: string;
  objective: string;
  signals: string[];
};

export type MarketIntelCaseBrief = {
  case_id: string;
  title: string;
  company: string | null;
  theme_id: string;
  theme_name: string;
  status: CaseStatus;
  latest_signal_at: string | null;
  evidence_count: number;
  claim_count: number;
  attached_to_case: boolean;
  summary: string;
  key_claims: string[];
  recommended_actions: string[];
};

export type MarketIntelOverviewResponse = {
  market_source_count: number;
  market_watchlist_count: number;
  tracked_company_count: number;
  active_case_count: number;
  theme_cards: MarketIntelThemeCard[];
  company_cards: MarketIntelCompanyCard[];
  case_briefs: MarketIntelCaseBrief[];
  playbooks: MarketIntelPlaybook[];
};

export type GenerateMarketIntelBriefRequest = {
  attach_to_case: boolean;
};

export type GenerateMarketIntelBriefResponse = {
  briefing: MarketIntelCaseBrief;
  transition: CaseTransition | null;
};

export type CreateSourceRequest = {
  name: string;
  description: string;
  kind: SourceKind;
  cadence_minutes: number;
  trust_score: number;
  enabled: boolean;
  tags: string[];
};

export type CreateWatchlistRequest = {
  name: string;
  description: string;
  keywords: string[];
  entities: string[];
  min_source_trust: number;
  severity: WatchlistSeverity;
  enabled: boolean;
};

export type IngestEvidenceRequest = {
  source_id: string;
  title: string;
  summary: string;
  content: string;
  url?: string | null;
  observed_at: string;
  tags: string[];
  entity_labels: string[];
  proposed_claims: ProposedClaim[];
};

export type IngestEvidenceResponse = {
  duplicate: boolean;
  evidence: EvidenceItem;
  claims: ClaimRecord[];
  hits: WatchlistHit[];
  case_updates: CaseTransition[];
};

const API_BASE = import.meta.env.VITE_API_BASE_URL ?? "http://127.0.0.1:3000";

async function parseOrThrow<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const text = await response.text();
    throw new Error(text || `HTTP ${response.status}`);
  }
  return response.json() as Promise<T>;
}

export async function fetchPolicyConfig(): Promise<DeterministicPolicyConfig> {
  const response = await fetch(`${API_BASE}/api/v1/policy/config`);
  const payload = await parseOrThrow<{ config: DeterministicPolicyConfig }>(response);
  return payload.config;
}

export async function updatePolicyConfig(
  config: DeterministicPolicyConfig
): Promise<DeterministicPolicyConfig> {
  const response = await fetch(`${API_BASE}/api/v1/policy/config`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ config }),
  });
  const payload = await parseOrThrow<{ config: DeterministicPolicyConfig }>(response);
  return payload.config;
}

export async function simulatePolicy(commands: PolicyCommand[]): Promise<PolicyStepResult[]> {
  const response = await fetch(`${API_BASE}/api/v1/policy/simulate`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ commands }),
  });
  const payload = await parseOrThrow<{ steps: PolicyStepResult[] }>(response);
  return payload.steps;
}

export async function fetchAgentCatalog(): Promise<DeterministicAgentSpec[]> {
  const response = await fetch(`${API_BASE}/api/v1/agents`);
  const payload = await parseOrThrow<{ agents: DeterministicAgentSpec[] }>(response);
  return payload.agents;
}

export async function fetchAgentCatalogQuality(): Promise<AgentCatalogQuality> {
  const response = await fetch(`${API_BASE}/api/v1/agents/quality`);
  const payload = await parseOrThrow<{ quality: AgentCatalogQuality }>(response);
  return payload.quality;
}

export async function fetchAgentTemplates(): Promise<DeterministicAgentTemplate[]> {
  const response = await fetch(`${API_BASE}/api/v1/agents/templates`);
  const payload = await parseOrThrow<{ templates: DeterministicAgentTemplate[] }>(response);
  return payload.templates;
}

export async function fetchAgentTemplate(templateId: string): Promise<DeterministicAgentTemplate> {
  const response = await fetch(`${API_BASE}/api/v1/agents/templates/${encodeURIComponent(templateId)}`);
  const payload = await parseOrThrow<{ template: DeterministicAgentTemplate }>(response);
  return payload.template;
}

export async function applyAgentTemplate(
  templateId: string,
  runBootstrapSimulation: boolean = true
): Promise<ApplyAgentTemplateResponse> {
  const response = await fetch(`${API_BASE}/api/v1/agents/templates/${encodeURIComponent(templateId)}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ run_bootstrap_simulation: runBootstrapSimulation }),
  });
  return parseOrThrow<ApplyAgentTemplateResponse>(response);
}

export async function sendRawOnchainTransaction(
  request: OnchainBroadcastRequest
): Promise<OnchainBroadcastResponse> {
  const response = await fetch(`${API_BASE}/api/v1/onchain/send_raw`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });
  return parseOrThrow<OnchainBroadcastResponse>(response);
}

export async function fetchOnchainReceipt(
  rpc_url: string,
  tx_hash: string
): Promise<OnchainReceiptResponse> {
  const response = await fetch(`${API_BASE}/api/v1/onchain/receipt`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ rpc_url, tx_hash }),
  });
  return parseOrThrow<OnchainReceiptResponse>(response);
}

export async function fetchAutopilotStatus(): Promise<AutopilotStatusResponse> {
  const response = await fetch(`${API_BASE}/api/v1/autopilot/status`);
  return parseOrThrow<AutopilotStatusResponse>(response);
}

export async function updateAutopilotConfig(
  config: AutopilotGuardConfig
): Promise<AutopilotStatusResponse> {
  const response = await fetch(`${API_BASE}/api/v1/autopilot/config`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ config }),
  });
  return parseOrThrow<AutopilotStatusResponse>(response);
}

export async function executeAutopilot(
  request: AutopilotExecuteRequest
): Promise<AutopilotExecuteResponse> {
  const response = await fetch(`${API_BASE}/api/v1/autopilot/execute`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });
  return parseOrThrow<AutopilotExecuteResponse>(response);
}

export async function proposeAutopilot(
  request: AutopilotProposeRequest
): Promise<AutopilotProposeResult> {
  const response = await fetch(`${API_BASE}/api/v1/autopilot/propose`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });

  const text = await response.text();
  let payload: unknown = null;
  try {
    payload = text ? JSON.parse(text) : null;
  } catch {
    payload = { error: text || `HTTP ${response.status}`, model: null, raw: null };
  }

  if (response.ok) {
    return { ok: true, response: payload as AutopilotProposeResponse };
  }

  return {
    ok: false,
    status: response.status,
    error: payload as AutopilotProposeErrorResponse,
  };
}

export async function fetchIntelOverview(): Promise<IntelDeskOverviewResponse> {
  const response = await fetch(`${API_BASE}/api/v1/intel/overview`);
  return parseOrThrow<IntelDeskOverviewResponse>(response);
}

export async function fetchMarketIntelOverview(): Promise<MarketIntelOverviewResponse> {
  const response = await fetch(`${API_BASE}/api/v1/market-intel/overview`);
  return parseOrThrow<MarketIntelOverviewResponse>(response);
}

export async function generateMarketIntelBrief(
  caseId: string,
  request: GenerateMarketIntelBriefRequest
): Promise<GenerateMarketIntelBriefResponse> {
  const response = await fetch(
    `${API_BASE}/api/v1/market-intel/cases/${encodeURIComponent(caseId)}/brief`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    }
  );
  return parseOrThrow<GenerateMarketIntelBriefResponse>(response);
}

export async function fetchSources(): Promise<SourceDefinition[]> {
  const response = await fetch(`${API_BASE}/api/v1/sources`);
  const payload = await parseOrThrow<{ sources: SourceDefinition[] }>(response);
  return payload.sources;
}

export async function createSource(request: CreateSourceRequest): Promise<SourceDefinition> {
  const response = await fetch(`${API_BASE}/api/v1/sources`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });
  const payload = await parseOrThrow<{ source: SourceDefinition }>(response);
  return payload.source;
}

export async function fetchWatchlists(): Promise<Watchlist[]> {
  const response = await fetch(`${API_BASE}/api/v1/watchlists`);
  const payload = await parseOrThrow<{ watchlists: Watchlist[] }>(response);
  return payload.watchlists;
}

export async function createWatchlist(request: CreateWatchlistRequest): Promise<Watchlist> {
  const response = await fetch(`${API_BASE}/api/v1/watchlists`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });
  const payload = await parseOrThrow<{ watchlist: Watchlist }>(response);
  return payload.watchlist;
}

export async function fetchEvidence(): Promise<EvidenceItem[]> {
  const response = await fetch(`${API_BASE}/api/v1/evidence`);
  const payload = await parseOrThrow<{ evidence: EvidenceItem[] }>(response);
  return payload.evidence;
}

export async function ingestEvidence(request: IngestEvidenceRequest): Promise<IngestEvidenceResponse> {
  const response = await fetch(`${API_BASE}/api/v1/evidence/ingest`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request),
  });
  return parseOrThrow<IngestEvidenceResponse>(response);
}

export async function fetchClaims(): Promise<ClaimRecord[]> {
  const response = await fetch(`${API_BASE}/api/v1/claims`);
  const payload = await parseOrThrow<{ claims: ClaimRecord[] }>(response);
  return payload.claims;
}

export async function reviewClaim(
  claimId: string,
  status: ClaimReviewStatus
): Promise<ClaimRecord> {
  const response = await fetch(`${API_BASE}/api/v1/claims/${encodeURIComponent(claimId)}/review`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ status }),
  });
  const payload = await parseOrThrow<{ claim: ClaimRecord }>(response);
  return payload.claim;
}

export async function fetchCases(): Promise<CaseFile[]> {
  const response = await fetch(`${API_BASE}/api/v1/cases`);
  const payload = await parseOrThrow<{ cases: CaseFile[] }>(response);
  return payload.cases;
}

export async function transitionCase(
  caseId: string,
  command: CaseCommand
): Promise<CaseTransition> {
  const response = await fetch(
    `${API_BASE}/api/v1/cases/${encodeURIComponent(caseId)}/transition`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ command }),
    }
  );
  const payload = await parseOrThrow<{ transition: CaseTransition }>(response);
  return payload.transition;
}

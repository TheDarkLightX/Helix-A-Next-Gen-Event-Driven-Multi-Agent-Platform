import {
  CaseQueueFilters,
  CaseStatus,
  ClaimQueueFilters,
  ClaimReviewStatus,
  EvidenceQueueFilters,
  WatchlistSeverity,
} from "./api";

const CASES_WORKSPACE_STORAGE_KEY = "helix.cases.workspace.v1";
const EVIDENCE_DESK_WORKSPACE_STORAGE_KEY = "helix.evidence.workspace.v1";
const MAX_SAVED_VIEWS = 8;

const VALID_CASE_STATUSES: ReadonlySet<string> = new Set([
  "open",
  "monitoring",
  "brief_ready",
  "escalated",
  "closed",
]);
const VALID_SEVERITIES: ReadonlySet<string> = new Set(["low", "medium", "high", "critical"]);
const VALID_CLAIM_REVIEW_STATUSES: ReadonlySet<string> = new Set([
  "needs_review",
  "corroborated",
  "rejected",
]);

export type SavedCaseView = {
  id: string;
  name: string;
  filters: CaseQueueFilters;
};

export type CasesWorkspaceState = {
  filters: CaseQueueFilters;
  savedViews: SavedCaseView[];
  activeViewId: string | null;
};

export type SavedEvidenceDeskView = {
  id: string;
  name: string;
  evidenceFilters: EvidenceQueueFilters;
  claimFilters: ClaimQueueFilters;
};

export type EvidenceDeskWorkspaceState = {
  evidenceFilters: EvidenceQueueFilters;
  claimFilters: ClaimQueueFilters;
  savedViews: SavedEvidenceDeskView[];
  activeViewId: string | null;
};

type RawCasesWorkspaceState = Partial<CasesWorkspaceState>;
type RawEvidenceDeskWorkspaceState = Partial<EvidenceDeskWorkspaceState>;

function isCaseStatus(value: unknown): value is CaseStatus {
  return typeof value === "string" && VALID_CASE_STATUSES.has(value);
}

function isSeverity(value: unknown): value is WatchlistSeverity {
  return typeof value === "string" && VALID_SEVERITIES.has(value);
}

function isClaimReviewStatus(value: unknown): value is ClaimReviewStatus {
  return typeof value === "string" && VALID_CLAIM_REVIEW_STATUSES.has(value);
}

function normalizedLimit(value: unknown): number | undefined {
  if (typeof value !== "number" || !Number.isInteger(value)) return undefined;
  if (value < 1 || value > 100) return undefined;
  return value;
}

function normalizedPercent(value: unknown): number | undefined {
  if (typeof value !== "number" || !Number.isFinite(value)) return undefined;
  if (value < 0 || value > 100) return undefined;
  return value;
}

function normalizedConfidenceBps(value: unknown): number | undefined {
  if (typeof value !== "number" || !Number.isInteger(value)) return undefined;
  if (value < 0 || value > 10_000) return undefined;
  return value;
}

function normalizedString(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function savedViewId(name: string): string {
  const normalized = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return normalized || "view";
}

function normalizeCaseFilters(value: unknown): CaseQueueFilters {
  const candidate = typeof value === "object" && value !== null ? value : {};
  return {
    status: isCaseStatus((candidate as { status?: unknown }).status)
      ? (candidate as { status?: CaseStatus }).status
      : undefined,
    severity: isSeverity((candidate as { severity?: unknown }).severity)
      ? (candidate as { severity?: WatchlistSeverity }).severity
      : undefined,
    watchlist_id: normalizedString((candidate as { watchlist_id?: unknown }).watchlist_id).trim() ||
      undefined,
    primary_entity:
      normalizedString((candidate as { primary_entity?: unknown }).primary_entity).trim() ||
      undefined,
    limit: normalizedLimit((candidate as { limit?: unknown }).limit),
  };
}

function normalizeEvidenceFilters(value: unknown): EvidenceQueueFilters {
  const candidate = typeof value === "object" && value !== null ? value : {};
  return {
    source_id: normalizedString((candidate as { source_id?: unknown }).source_id).trim() || undefined,
    tag: normalizedString((candidate as { tag?: unknown }).tag).trim() || undefined,
    entity: normalizedString((candidate as { entity?: unknown }).entity).trim() || undefined,
    linked_status: isCaseStatus((candidate as { linked_status?: unknown }).linked_status)
      ? (candidate as { linked_status?: CaseStatus }).linked_status
      : undefined,
    min_trust: normalizedPercent((candidate as { min_trust?: unknown }).min_trust),
    q: normalizedString((candidate as { q?: unknown }).q).trim() || undefined,
    limit: normalizedLimit((candidate as { limit?: unknown }).limit),
  };
}

function normalizeClaimFilters(value: unknown): ClaimQueueFilters {
  const candidate = typeof value === "object" && value !== null ? value : {};
  return {
    review_status: isClaimReviewStatus((candidate as { review_status?: unknown }).review_status)
      ? (candidate as { review_status?: ClaimReviewStatus }).review_status
      : undefined,
    subject: normalizedString((candidate as { subject?: unknown }).subject).trim() || undefined,
    predicate:
      normalizedString((candidate as { predicate?: unknown }).predicate).trim() || undefined,
    linked_status: isCaseStatus((candidate as { linked_status?: unknown }).linked_status)
      ? (candidate as { linked_status?: CaseStatus }).linked_status
      : undefined,
    min_confidence_bps: normalizedConfidenceBps(
      (candidate as { min_confidence_bps?: unknown }).min_confidence_bps
    ),
    q: normalizedString((candidate as { q?: unknown }).q).trim() || undefined,
    limit: normalizedLimit((candidate as { limit?: unknown }).limit),
  };
}

function normalizeCaseSavedViews(value: unknown): SavedCaseView[] {
  if (!Array.isArray(value)) return [];
  const normalized = value
    .map((item) => {
      if (typeof item !== "object" || item === null) return null;
      const name = normalizedString((item as { name?: unknown }).name).trim();
      const id = normalizedString((item as { id?: unknown }).id).trim() || savedViewId(name);
      if (!name) return null;
      return {
        id,
        name,
        filters: normalizeCaseFilters((item as { filters?: unknown }).filters),
      };
    })
    .filter((item): item is SavedCaseView => item !== null);

  const unique = new Map<string, SavedCaseView>();
  for (const view of normalized) {
    if (!unique.has(view.id)) unique.set(view.id, view);
  }
  return Array.from(unique.values()).slice(0, MAX_SAVED_VIEWS);
}

function normalizeEvidenceSavedViews(value: unknown): SavedEvidenceDeskView[] {
  if (!Array.isArray(value)) return [];
  const normalized = value
    .map((item) => {
      if (typeof item !== "object" || item === null) return null;
      const name = normalizedString((item as { name?: unknown }).name).trim();
      const id = normalizedString((item as { id?: unknown }).id).trim() || savedViewId(name);
      if (!name) return null;
      return {
        id,
        name,
        evidenceFilters: normalizeEvidenceFilters(
          (item as { evidenceFilters?: unknown }).evidenceFilters
        ),
        claimFilters: normalizeClaimFilters((item as { claimFilters?: unknown }).claimFilters),
      };
    })
    .filter((item): item is SavedEvidenceDeskView => item !== null);

  const unique = new Map<string, SavedEvidenceDeskView>();
  for (const view of normalized) {
    if (!unique.has(view.id)) unique.set(view.id, view);
  }
  return Array.from(unique.values()).slice(0, MAX_SAVED_VIEWS);
}

export function defaultCasesWorkspaceState(defaultFilters: CaseQueueFilters): CasesWorkspaceState {
  return {
    filters: normalizeCaseFilters(defaultFilters),
    savedViews: [],
    activeViewId: null,
  };
}

export function loadCasesWorkspaceState(defaultFilters: CaseQueueFilters): CasesWorkspaceState {
  const fallback = defaultCasesWorkspaceState(defaultFilters);
  if (typeof window === "undefined") return fallback;

  try {
    const raw = window.localStorage.getItem(CASES_WORKSPACE_STORAGE_KEY);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw) as RawCasesWorkspaceState;
    const savedViews = normalizeCaseSavedViews(parsed.savedViews);
    const activeViewId = normalizedString(parsed.activeViewId).trim() || null;
    return {
      filters: normalizeCaseFilters(parsed.filters),
      savedViews,
      activeViewId:
        activeViewId && savedViews.some((view) => view.id === activeViewId) ? activeViewId : null,
    };
  } catch {
    return fallback;
  }
}

export function saveCasesWorkspaceState(state: CasesWorkspaceState): void {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(
    CASES_WORKSPACE_STORAGE_KEY,
    JSON.stringify({
      filters: normalizeCaseFilters(state.filters),
      savedViews: normalizeCaseSavedViews(state.savedViews),
      activeViewId: state.activeViewId,
    })
  );
}

export function clearCasesWorkspaceState(): void {
  if (typeof window === "undefined") return;
  window.localStorage.removeItem(CASES_WORKSPACE_STORAGE_KEY);
}

export function defaultEvidenceDeskWorkspaceState(
  defaults: Pick<EvidenceDeskWorkspaceState, "evidenceFilters" | "claimFilters">
): EvidenceDeskWorkspaceState {
  return {
    evidenceFilters: normalizeEvidenceFilters(defaults.evidenceFilters),
    claimFilters: normalizeClaimFilters(defaults.claimFilters),
    savedViews: [],
    activeViewId: null,
  };
}

export function loadEvidenceDeskWorkspaceState(
  defaults: Pick<EvidenceDeskWorkspaceState, "evidenceFilters" | "claimFilters">
): EvidenceDeskWorkspaceState {
  const fallback = defaultEvidenceDeskWorkspaceState(defaults);
  if (typeof window === "undefined") return fallback;

  try {
    const raw = window.localStorage.getItem(EVIDENCE_DESK_WORKSPACE_STORAGE_KEY);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw) as RawEvidenceDeskWorkspaceState;
    const savedViews = normalizeEvidenceSavedViews(parsed.savedViews);
    const activeViewId = normalizedString(parsed.activeViewId).trim() || null;

    return {
      evidenceFilters: normalizeEvidenceFilters(parsed.evidenceFilters),
      claimFilters: normalizeClaimFilters(parsed.claimFilters),
      savedViews,
      activeViewId:
        activeViewId && savedViews.some((view) => view.id === activeViewId) ? activeViewId : null,
    };
  } catch {
    return fallback;
  }
}

export function saveEvidenceDeskWorkspaceState(state: EvidenceDeskWorkspaceState): void {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(
    EVIDENCE_DESK_WORKSPACE_STORAGE_KEY,
    JSON.stringify({
      evidenceFilters: normalizeEvidenceFilters(state.evidenceFilters),
      claimFilters: normalizeClaimFilters(state.claimFilters),
      savedViews: normalizeEvidenceSavedViews(state.savedViews),
      activeViewId: state.activeViewId,
    })
  );
}

export function clearEvidenceDeskWorkspaceState(): void {
  if (typeof window === "undefined") return;
  window.localStorage.removeItem(EVIDENCE_DESK_WORKSPACE_STORAGE_KEY);
}

export function makeSavedViewId(name: string): string {
  return savedViewId(name);
}

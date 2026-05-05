import { AutopilotReviewKind, AutopilotReviewQueueFilters } from "./api";

const AUTOPILOT_WORKSPACE_STORAGE_KEY = "helix.autopilot.workspace.v1";
const VALID_REVIEW_KINDS: ReadonlySet<string> = new Set(["case", "claim", "evidence"]);
const MAX_SAVED_VIEWS = 8;

export type SavedAutopilotReviewView = {
  id: string;
  name: string;
  filters: AutopilotReviewQueueFilters;
};

export type AutopilotWorkspaceState = {
  reviewFilters: AutopilotReviewQueueFilters;
  policyGoal: string;
  onchainGoal: string;
  policyActionText: string;
  onchainActionText: string;
  savedViews: SavedAutopilotReviewView[];
  activeViewId: string | null;
};

type RawAutopilotWorkspaceState = Partial<AutopilotWorkspaceState>;

function isReviewKind(value: unknown): value is AutopilotReviewKind {
  return typeof value === "string" && VALID_REVIEW_KINDS.has(value);
}

function normalizedLimit(value: unknown): number | undefined {
  if (typeof value !== "number" || !Number.isInteger(value)) return undefined;
  if (value < 1 || value > 100) return undefined;
  return value;
}

function normalizedString(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function normalizeFilters(value: unknown): AutopilotReviewQueueFilters {
  const candidate = typeof value === "object" && value !== null ? value : {};
  const maybeKind = (candidate as { kind?: unknown }).kind;
  const maybeLimit = (candidate as { limit?: unknown }).limit;
  return {
    kind: isReviewKind(maybeKind) ? maybeKind : undefined,
    limit: normalizedLimit(maybeLimit),
  };
}

function normalizeSavedViews(value: unknown): SavedAutopilotReviewView[] {
  if (!Array.isArray(value)) return [];
  const normalized = value
    .map((item) => {
      if (typeof item !== "object" || item === null) return null;
      const id = normalizedString((item as { id?: unknown }).id).trim();
      const name = normalizedString((item as { name?: unknown }).name).trim();
      const filters = normalizeFilters((item as { filters?: unknown }).filters);
      if (!id || !name) return null;
      return { id, name, filters };
    })
    .filter((item): item is SavedAutopilotReviewView => item !== null);

  const unique = new Map<string, SavedAutopilotReviewView>();
  for (const view of normalized) {
    if (!unique.has(view.id)) unique.set(view.id, view);
  }
  return Array.from(unique.values()).slice(0, MAX_SAVED_VIEWS);
}

export function defaultAutopilotWorkspaceState(
  defaults: Pick<AutopilotWorkspaceState, "policyActionText" | "onchainActionText">
): AutopilotWorkspaceState {
  return {
    reviewFilters: { limit: 12 },
    policyGoal: "",
    onchainGoal: "",
    policyActionText: defaults.policyActionText,
    onchainActionText: defaults.onchainActionText,
    savedViews: [],
    activeViewId: null,
  };
}

export function loadAutopilotWorkspaceState(
  defaults: Pick<AutopilotWorkspaceState, "policyActionText" | "onchainActionText">
): AutopilotWorkspaceState {
  const fallback = defaultAutopilotWorkspaceState(defaults);
  if (typeof window === "undefined") return fallback;

  try {
    const raw = window.localStorage.getItem(AUTOPILOT_WORKSPACE_STORAGE_KEY);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw) as RawAutopilotWorkspaceState;
    const savedViews = normalizeSavedViews(parsed.savedViews);
    const activeViewId = normalizedString(parsed.activeViewId).trim() || null;

    return {
      reviewFilters: normalizeFilters(parsed.reviewFilters),
      policyGoal: normalizedString(parsed.policyGoal),
      onchainGoal: normalizedString(parsed.onchainGoal),
      policyActionText: normalizedString(parsed.policyActionText) || defaults.policyActionText,
      onchainActionText: normalizedString(parsed.onchainActionText) || defaults.onchainActionText,
      savedViews,
      activeViewId:
        activeViewId && savedViews.some((view) => view.id === activeViewId) ? activeViewId : null,
    };
  } catch {
    return fallback;
  }
}

export function saveAutopilotWorkspaceState(state: AutopilotWorkspaceState): void {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(
    AUTOPILOT_WORKSPACE_STORAGE_KEY,
    JSON.stringify({
      ...state,
      reviewFilters: normalizeFilters(state.reviewFilters),
      savedViews: normalizeSavedViews(state.savedViews),
      activeViewId: state.activeViewId,
    })
  );
}

export function clearAutopilotWorkspaceState(): void {
  if (typeof window === "undefined") return;
  window.localStorage.removeItem(AUTOPILOT_WORKSPACE_STORAGE_KEY);
}

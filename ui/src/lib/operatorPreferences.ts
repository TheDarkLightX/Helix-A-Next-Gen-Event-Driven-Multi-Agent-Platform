const OPERATOR_PREFERENCES_STORAGE_KEY = "helix.operator.preferences.v1";

const VALID_LANDING_ROUTES = [
  "/",
  "/market-intel",
  "/autopilot",
  "/sources",
  "/evidence",
  "/watchlists",
  "/cases",
  "/agents",
  "/policies",
  "/credentials",
  "/rules",
  "/audit",
  "/onchain",
] as const;

export type OperatorLandingRoute = (typeof VALID_LANDING_ROUTES)[number];

export type OperatorPreferences = {
  sidebarCollapsed: boolean;
  defaultLandingRoute: OperatorLandingRoute;
};

type RawOperatorPreferences = Partial<OperatorPreferences>;

export const DEFAULT_OPERATOR_PREFERENCES: OperatorPreferences = {
  sidebarCollapsed: false,
  defaultLandingRoute: "/",
};

function isLandingRoute(value: unknown): value is OperatorLandingRoute {
  return typeof value === "string" && VALID_LANDING_ROUTES.includes(value as OperatorLandingRoute);
}

export function loadOperatorPreferences(): OperatorPreferences {
  if (typeof window === "undefined") return DEFAULT_OPERATOR_PREFERENCES;

  try {
    const raw = window.localStorage.getItem(OPERATOR_PREFERENCES_STORAGE_KEY);
    if (!raw) return DEFAULT_OPERATOR_PREFERENCES;
    const parsed = JSON.parse(raw) as RawOperatorPreferences;
    return {
      sidebarCollapsed: parsed.sidebarCollapsed === true,
      defaultLandingRoute: isLandingRoute(parsed.defaultLandingRoute)
        ? parsed.defaultLandingRoute
        : DEFAULT_OPERATOR_PREFERENCES.defaultLandingRoute,
    };
  } catch {
    return DEFAULT_OPERATOR_PREFERENCES;
  }
}

export function saveOperatorPreferences(preferences: OperatorPreferences): void {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(
    OPERATOR_PREFERENCES_STORAGE_KEY,
    JSON.stringify({
      sidebarCollapsed: preferences.sidebarCollapsed === true,
      defaultLandingRoute: isLandingRoute(preferences.defaultLandingRoute)
        ? preferences.defaultLandingRoute
        : DEFAULT_OPERATOR_PREFERENCES.defaultLandingRoute,
    })
  );
}

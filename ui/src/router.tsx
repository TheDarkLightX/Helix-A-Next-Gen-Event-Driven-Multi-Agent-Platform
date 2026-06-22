import { useEffect, useState } from "react";
import {
  Link,
  Outlet,
  createRootRoute,
  createRoute,
  createRouter,
  useNavigate,
} from "@tanstack/react-router";
import { AgentCatalogPage } from "./screens/AgentCatalogPage";
import { AuditPage } from "./screens/AuditPage";
import { AutopilotPage } from "./screens/AutopilotPage";
import { CasesPage } from "./screens/CasesPage";
import { CredentialsPage } from "./screens/CredentialsPage";
import { DashboardPage } from "./screens/DashboardPage";
import { EvidencePage } from "./screens/EvidencePage";
import { FederationPage } from "./screens/FederationPage";
import { MarketIntelPage } from "./screens/MarketIntelPage";
import { OnchainPage } from "./screens/OnchainPage";
import { PolicyWorkbenchPage } from "./screens/PolicyWorkbenchPage";
import { RulesPage } from "./screens/RulesPage";
import { SourcesPage } from "./screens/SourcesPage";
import { WatchlistsPage } from "./screens/WatchlistsPage";
import { clearApiToken, loadApiToken, saveApiToken } from "./lib/apiAuth";
import {
  OperatorLandingRoute,
  loadOperatorPreferences,
  saveOperatorPreferences,
} from "./lib/operatorPreferences";

const NAV_GROUPS = [
  {
    name: "Command",
    items: [
      { to: "/" as const, label: "Dashboard", icon: "◆" },
      { to: "/market-intel" as const, label: "Market Intel", icon: "◐" },
      { to: "/autopilot" as const, label: "Autopilot", icon: "◎" },
    ],
  },
  {
    name: "Intelligence",
    items: [
      { to: "/sources" as const, label: "Sources", icon: "▸" },
      { to: "/evidence" as const, label: "Evidence", icon: "▤" },
      { to: "/watchlists" as const, label: "Watchlists", icon: "⚑" },
    ],
  },
  {
    name: "Operations",
    items: [
      { to: "/cases" as const, label: "Cases", icon: "⊡" },
      { to: "/agents" as const, label: "Agents", icon: "⎌" },
      { to: "/policies" as const, label: "Policy", icon: "⌥" },
      { to: "/credentials" as const, label: "Credentials", icon: "⚿" },
      { to: "/rules" as const, label: "Automation", icon: "⟲" },
      { to: "/audit" as const, label: "Audit Log", icon: "≡" },
      { to: "/onchain" as const, label: "Onchain", icon: "⟡" },
    ],
  },
  {
    name: "Network",
    items: [
      { to: "/federation" as const, label: "Federation", icon: "⬡" },
    ],
  },
];

function RootLayout() {
  const navigate = useNavigate();
  const [preferences, setPreferences] = useState(() => loadOperatorPreferences());
  const [apiTokenInput, setApiTokenInput] = useState(() => loadApiToken());
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const hasApiToken = apiTokenInput.trim().length > 0;

  useEffect(() => {
    saveOperatorPreferences(preferences);
  }, [preferences]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    if (window.location.pathname !== "/") return;
    if (preferences.defaultLandingRoute === "/") return;
    void navigate({ to: preferences.defaultLandingRoute, replace: true });
  }, [navigate, preferences.defaultLandingRoute]);

  function toggleSidebarCollapsed() {
    setPreferences((current) => ({
      ...current,
      sidebarCollapsed: !current.sidebarCollapsed,
    }));
  }

  function updateDefaultLandingRoute(route: OperatorLandingRoute) {
    setPreferences((current) => ({
      ...current,
      defaultLandingRoute: route,
    }));
  }

  function updateApiToken(value: string) {
    setApiTokenInput(value);
    saveApiToken(value);
  }

  function resetApiToken() {
    setApiTokenInput("");
    clearApiToken();
  }

  return (
    <div className="hx-root">
      <div className="hx-bg-grid" />

      <div className="hx-layout">
        <header className="hx-topbar">
          <div className="hx-topbar-brand">
            <button
              type="button"
              className="hx-topbar-menu"
              aria-label={sidebarOpen ? "Close navigation" : "Open navigation"}
              aria-expanded={sidebarOpen}
              aria-controls="helix-sidebar"
              onClick={() => setSidebarOpen((open) => !open)}
            >
              MENU
            </button>
            <span className="hx-brand-icon">⎈</span>
            <span className="hx-brand-name">HELIX</span>
            <span className="hx-brand-version">v2.1 · Intelligence Desk</span>
            <button
              type="button"
              className="hx-topbar-collapse"
              aria-label={preferences.sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
              aria-pressed={preferences.sidebarCollapsed}
              onClick={toggleSidebarCollapsed}
            >
              {preferences.sidebarCollapsed ? "▸" : "◂"}
            </button>
          </div>
          <div className="hx-topbar-meta">
            <div className="hx-meta-chip">
              <span className="hx-meta-label">NET</span>
              <span className="hx-meta-value is-ok">SECURE</span>
            </div>
            <div className="hx-meta-chip">
              <span className="hx-meta-label">RPC</span>
              <span className="hx-meta-value">127.0.0.1:3000</span>
            </div>
            <div className="hx-meta-chip">
              <span className="hx-meta-label">SYNC</span>
              <span className="hx-meta-value is-ok">ONLINE</span>
            </div>
            <label className="hx-meta-auth">
              <span className="hx-meta-label">TOKEN</span>
              <input
                aria-label="API bearer token"
                className="hx-token-input"
                type="password"
                autoComplete="off"
                placeholder="bearer token"
                value={apiTokenInput}
                onChange={(event) => updateApiToken(event.target.value)}
              />
              {hasApiToken ? (
                <button type="button" className="hx-token-clear" onClick={resetApiToken}>
                  CLEAR
                </button>
              ) : (
                <span className="hx-meta-value">LOCAL</span>
              )}
            </label>
          </div>
        </header>

        <div
          className={`hx-main-grid${preferences.sidebarCollapsed ? " is-collapsed" : ""}`}
        >
          <aside
            id="helix-sidebar"
            className={`hx-sidebar${sidebarOpen ? " is-open" : ""}${
              preferences.sidebarCollapsed ? " is-collapsed" : ""
            }`}
          >
            <nav className="hx-nav">
              {NAV_GROUPS.map((group) => (
                <div key={group.name} className="hx-nav-group">
                  <span className="hx-nav-group-label">{group.name}</span>
                  <div className="hx-nav-group-items">
                    {group.items.map((item) => (
                      <Link
                        key={item.to}
                        to={item.to}
                        activeProps={{ className: "hx-nav-link active" }}
                        inactiveProps={{ className: "hx-nav-link" }}
                        title={item.label}
                        onClick={() => setSidebarOpen(false)}
                      >
                        <span className="hx-nav-icon">{item.icon}</span>
                        <span className="hx-nav-text">{item.label}</span>
                        <span className="hx-nav-caret">›</span>
                      </Link>
                    ))}
                  </div>
                </div>
              ))}
            </nav>
            <div className="hx-sidebar-footer">
              <label className="hx-sidebar-pref">
                <span className="hx-nav-group-label">Default Landing</span>
                <select
                  value={preferences.defaultLandingRoute}
                  onChange={(event) =>
                    updateDefaultLandingRoute(event.target.value as OperatorLandingRoute)
                  }
                >
                  <option value="/">Dashboard</option>
                  <option value="/market-intel">Market Intel</option>
                  <option value="/autopilot">Autopilot</option>
                  <option value="/sources">Sources</option>
                  <option value="/evidence">Evidence</option>
                  <option value="/watchlists">Watchlists</option>
                  <option value="/cases">Cases</option>
                  <option value="/agents">Agents</option>
                  <option value="/policies">Policy</option>
                  <option value="/credentials">Credentials</option>
                  <option value="/rules">Automation</option>
                  <option value="/audit">Audit Log</option>
                  <option value="/onchain">Onchain</option>
                </select>
              </label>
              <div className="hx-sidebar-status">
                <span className="hx-pulse-dot" />
                <span>SYS NOMINAL</span>
              </div>
            </div>
          </aside>

          {sidebarOpen ? (
            <button
              type="button"
              className="hx-sidebar-scrim"
              aria-label="Close navigation"
              onClick={() => setSidebarOpen(false)}
            />
          ) : null}

          <main className="hx-workspace">
            <div className="hx-workspace-frame">
              <Outlet />
            </div>
          </main>
        </div>
      </div>
    </div>
  );
}

const rootRoute = createRootRoute({
  component: RootLayout,
});

const dashboardRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: DashboardPage,
});

const policiesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/policies",
  component: PolicyWorkbenchPage,
});

const marketIntelRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/market-intel",
  component: MarketIntelPage,
});

const sourcesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/sources",
  component: SourcesPage,
});

const watchlistsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/watchlists",
  component: WatchlistsPage,
});

const evidenceRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/evidence",
  component: EvidencePage,
});

const casesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/cases",
  component: CasesPage,
});

const agentsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/agents",
  component: AgentCatalogPage,
});

const credentialsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/credentials",
  component: CredentialsPage,
});

const onchainRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/onchain",
  component: OnchainPage,
});

const auditRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/audit",
  component: AuditPage,
});

const rulesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/rules",
  component: RulesPage,
});

const autopilotRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/autopilot",
  component: AutopilotPage,
});

const federationRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/federation",
  component: FederationPage,
});

const routeTree = rootRoute.addChildren([
  dashboardRoute,
  marketIntelRoute,
  sourcesRoute,
  watchlistsRoute,
  evidenceRoute,
  casesRoute,
  policiesRoute,
  agentsRoute,
  credentialsRoute,
  rulesRoute,
  auditRoute,
  onchainRoute,
  autopilotRoute,
  federationRoute,
]);

export const router = createRouter({
  routeTree,
  defaultPreload: "intent",
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

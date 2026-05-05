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
    name: "Command & Control",
    items: [
      { to: "/" as const, label: "SYS.DASHBOARD", icon: "❖" },
      { to: "/market-intel" as const, label: "MARKET.INTEL", icon: "◒" },
      { to: "/autopilot" as const, label: "AUTOPILOT.AI", icon: "⌾" },
    ],
  },
  {
    name: "Data Substrate",
    items: [
      { to: "/sources" as const, label: "NET.SOURCES", icon: "⏚" },
      { to: "/evidence" as const, label: "RAW.EVIDENCE", icon: "▤" },
      { to: "/watchlists" as const, label: "WATCH.RULES", icon: "⎈" },
    ],
  },
  {
    name: "Execution & Logic",
    items: [
      { to: "/cases" as const, label: "ACTIVE.CASES", icon: "⊡" },
      { to: "/agents" as const, label: "AGENT.NODES", icon: "⎍" },
      { to: "/policies" as const, label: "POLICY.GATE", icon: "⍜" },
      { to: "/credentials" as const, label: "KEYS.VAULT", icon: "K" },
      { to: "/rules" as const, label: "AUTO.RULES", icon: "⟲" },
      { to: "/audit" as const, label: "AUDIT.LOG", icon: "⌁" },
      { to: "/onchain" as const, label: "EVM.SHELL", icon: "⟡" },
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
    <div className="tac-root">
      <div className="tac-scanlines"></div>
      <div className="tac-glow-orbs"></div>

      <div className="tac-layout">
        <header className="tac-topbar">
          <div className="topbar-brand">
            <button
              type="button"
              className="topbar-menu"
              aria-label={sidebarOpen ? "Close navigation" : "Open navigation"}
              aria-expanded={sidebarOpen}
              aria-controls="helix-sidebar"
              onClick={() => setSidebarOpen((open) => !open)}
            >
              NAV
            </button>
            <span className="brand-icon">⎈</span>
            <span className="brand-name">HELIX</span>
            <span className="brand-version">v2.0.4 - TACTICAL OSINT</span>
            <button
              type="button"
              className="topbar-collapse"
              aria-label={preferences.sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
              aria-pressed={preferences.sidebarCollapsed}
              onClick={toggleSidebarCollapsed}
            >
              {preferences.sidebarCollapsed ? "EXPAND" : "COLLAPSE"}
            </button>
          </div>
          <div className="topbar-telemetry">
            <div className="telemetry-block">
              <span className="t-label">NET</span>
              <span className="t-val ok">SECURE</span>
            </div>
            <div className="telemetry-block">
              <span className="t-label">RPC</span>
              <span className="t-val">127.0.0.1:3000</span>
            </div>
            <div className="telemetry-block">
              <span className="t-label">UI_SYNC</span>
              <span className="t-val ok">SYNCED</span>
            </div>
            <label className="telemetry-auth">
              <span className="t-label">API_AUTH</span>
              <input
                aria-label="API bearer token"
                className="topbar-token"
                type="password"
                autoComplete="off"
                placeholder="token"
                value={apiTokenInput}
                onChange={(event) => updateApiToken(event.target.value)}
              />
              {hasApiToken ? (
                <button type="button" className="topbar-token-clear" onClick={resetApiToken}>
                  CLEAR
                </button>
              ) : (
                <span className="t-val">LOCAL</span>
              )}
            </label>
          </div>
        </header>

        <div
          className={`tac-main-grid${preferences.sidebarCollapsed ? " sidebar-collapsed" : ""}`}
        >
          <aside
            id="helix-sidebar"
            className={`tac-sidebar${sidebarOpen ? " is-open" : ""}${
              preferences.sidebarCollapsed ? " is-collapsed" : ""
            }`}
          >
            <nav className="tac-nav">
              {NAV_GROUPS.map((group) => (
                <div key={group.name} className="nav-group">
                  <span className="group-label">[{group.name}]</span>
                  <div className="group-items">
                    {group.items.map((item) => (
                      <Link
                        key={item.to}
                        to={item.to}
                        activeProps={{ className: "tac-nav-link active" }}
                        inactiveProps={{ className: "tac-nav-link" }}
                        title={item.label}
                        onClick={() => setSidebarOpen(false)}
                      >
                        <span className="nav-icon">{item.icon}</span>
                        <span className="nav-text">{item.label}</span>
                        <span className="nav-caret">⟩</span>
                      </Link>
                    ))}
                  </div>
                </div>
              ))}
            </nav>
            <div className="sidebar-footer">
              <label className="sidebar-pref">
                <span className="group-label">[Default Landing]</span>
                <select
                  value={preferences.defaultLandingRoute}
                  onChange={(event) =>
                    updateDefaultLandingRoute(event.target.value as OperatorLandingRoute)
                  }
                >
                  <option value="/">SYS.DASHBOARD</option>
                  <option value="/market-intel">MARKET.INTEL</option>
                  <option value="/autopilot">AUTOPILOT.AI</option>
                  <option value="/sources">NET.SOURCES</option>
                  <option value="/evidence">RAW.EVIDENCE</option>
                  <option value="/watchlists">WATCH.RULES</option>
                  <option value="/cases">ACTIVE.CASES</option>
                  <option value="/agents">AGENT.NODES</option>
                  <option value="/policies">POLICY.GATE</option>
                  <option value="/credentials">KEYS.VAULT</option>
                  <option value="/rules">AUTO.RULES</option>
                  <option value="/audit">AUDIT.LOG</option>
                  <option value="/onchain">EVM.SHELL</option>
                </select>
              </label>
              <div className="status-indicator">
                <span className="pulse-dot ok"></span>
                <span>SYS_NOMINAL</span>
              </div>
            </div>
          </aside>

          {sidebarOpen ? (
            <button
              type="button"
              className="sidebar-scrim"
              aria-label="Close navigation"
              onClick={() => setSidebarOpen(false)}
            />
          ) : null}

          <main className="tac-workspace">
            <div className="workspace-frame">
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

import { Link, Outlet, createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { AgentCatalogPage } from "./screens/AgentCatalogPage";
import { AutopilotPage } from "./screens/AutopilotPage";
import { CasesPage } from "./screens/CasesPage";
import { DashboardPage } from "./screens/DashboardPage";
import { EvidencePage } from "./screens/EvidencePage";
import { MarketIntelPage } from "./screens/MarketIntelPage";
import { OnchainPage } from "./screens/OnchainPage";
import { PolicyWorkbenchPage } from "./screens/PolicyWorkbenchPage";
import { SourcesPage } from "./screens/SourcesPage";
import { WatchlistsPage } from "./screens/WatchlistsPage";

const NAV_ITEMS = [
  { to: "/" as const, label: "Dashboard", hint: "System posture and intelligence desk metrics" },
  { to: "/market-intel" as const, label: "Market Intel", hint: "Competitors, pricing, and launches" },
  { to: "/sources" as const, label: "Sources", hint: "Collection registry and trust posture" },
  { to: "/watchlists" as const, label: "Watchlists", hint: "Deterministic detection rules" },
  { to: "/evidence" as const, label: "Evidence", hint: "Ingest signals and inspect claims" },
  { to: "/cases" as const, label: "Cases", hint: "OSINT dossiers and lifecycle control" },
  { to: "/policies" as const, label: "Policies", hint: "Deterministic controls and replay" },
  { to: "/agents" as const, label: "Agents", hint: "ROI kernels and deployment templates" },
  { to: "/onchain" as const, label: "Onchain", hint: "EVM intent execution shell" },
  { to: "/autopilot" as const, label: "Autopilot", hint: "LLM-operated guarded actions" },
];

function RootLayout() {
  return (
    <div className="app-root">
      <div className="background-layer" />
      <div className="shell-layout">
        <aside className="side-rail">
          <div className="brand-block">
            <span className="brand-dot" />
            <div>
              <p className="brand-kicker">Helix</p>
              <h1 className="brand-title">Personal Intelligence Agency Console</h1>
            </div>
          </div>
          <p className="rail-note">
            Self-hosted intelligence desk for OSINT and market intelligence with deterministic
            kernels, provenance-first evidence, and guarded autonomy.
          </p>

          <nav className="route-nav">
            {NAV_ITEMS.map((item) => (
              <Link
                key={item.to}
                to={item.to}
                activeProps={{ className: "route-link route-link-active" }}
                inactiveProps={{ className: "route-link" }}
              >
                <span>{item.label}</span>
                <small>{item.hint}</small>
              </Link>
            ))}
          </nav>

          <div className="side-section">
            <p className="mono-label">Build Discipline</p>
            <ul className="rail-list">
              <li>Collection and claims stay provenance-linked and replayable</li>
              <li>Formal kernels gate case lifecycle and high-risk automation</li>
              <li>LLMs can propose; deterministic guards decide</li>
            </ul>
          </div>

          <div className="nav-chip">OSINT Desk / Fail Closed</div>
        </aside>

        <div className="workspace">
          <header className="workspace-topbar">
            <p className="topbar-title">Helix 2.0 Intelligence Desk</p>
            <div className="topbar-chips">
              <span className="topbar-chip">API 127.0.0.1:3000</span>
              <span className="topbar-chip">UI 127.0.0.1:5173</span>
            </div>
          </header>
          <main className="content-shell">
            <Outlet />
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

const onchainRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/onchain",
  component: OnchainPage,
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

import { Link, Outlet, createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { AgentCatalogPage } from "./screens/AgentCatalogPage";
import { AutopilotPage } from "./screens/AutopilotPage";
import { DashboardPage } from "./screens/DashboardPage";
import { OnchainPage } from "./screens/OnchainPage";
import { PolicyWorkbenchPage } from "./screens/PolicyWorkbenchPage";

const NAV_ITEMS = [
  { to: "/" as const, label: "Dashboard", hint: "System posture and runtime lanes" },
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
              <h1 className="brand-title">Correct-by-Construction Control Plane</h1>
            </div>
          </div>
          <p className="rail-note">
            Formal functional core. Imperative shell. Deterministic operations under explicit
            guardrails.
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
              <li>Deterministic simulation before effectful execution</li>
              <li>Formal model coverage for core and ROI agent kernels</li>
              <li>Fail-closed autopilot controls for LLM operation</li>
            </ul>
          </div>

          <div className="nav-chip">Formally Verified Kernel</div>
        </aside>

        <div className="workspace">
          <header className="workspace-topbar">
            <p className="topbar-title">Helix 1.0 Operator Console</p>
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

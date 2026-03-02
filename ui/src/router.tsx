import { Link, Outlet, createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { AgentCatalogPage } from "./screens/AgentCatalogPage";
import { AutopilotPage } from "./screens/AutopilotPage";
import { DashboardPage } from "./screens/DashboardPage";
import { OnchainPage } from "./screens/OnchainPage";
import { PolicyWorkbenchPage } from "./screens/PolicyWorkbenchPage";

function RootLayout() {
  return (
    <div className="app-root">
      <div className="background-layer" />
      <header className="top-nav">
        <div className="brand-block">
          <span className="brand-dot" />
          <div>
            <p className="brand-kicker">Helix</p>
            <h1 className="brand-title">Correct-by-Construction Control Plane</h1>
          </div>
        </div>
        <div className="top-nav-right">
          <nav className="route-nav">
            <Link to="/" className="route-link">
              Dashboard
            </Link>
            <Link to="/policies" className="route-link">
              Policies
            </Link>
            <Link to="/agents" className="route-link">
              Agents
            </Link>
            <Link to="/onchain" className="route-link">
              Onchain
            </Link>
            <Link to="/autopilot" className="route-link">
              Autopilot
            </Link>
          </nav>
          <div className="nav-chip">ESSO Verified Kernel</div>
        </div>
      </header>
      <main className="content-shell">
        <Outlet />
      </main>
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

import { useEffect, useState } from "react";
import {
  GenerateMarketIntelBriefResponse,
  MarketIntelCaseBrief,
  MarketIntelCompanyCard,
  MarketIntelOverviewResponse,
  MarketIntelPlaybook,
  MarketIntelThemeCard,
  fetchMarketIntelOverview,
  generateMarketIntelBrief,
} from "../lib/api";

function caseStatusClass(activeCaseCount: number, escalatedCaseCount?: number) {
  if ((escalatedCaseCount ?? 0) > 0) return "danger";
  if (activeCaseCount > 0) return "warn";
  return "ok";
}

function briefStatusClass(status: MarketIntelCaseBrief["status"], attachedToCase: boolean) {
  if (status === "escalated") return "danger";
  if (attachedToCase || status === "brief_ready") return "ok";
  return "warn";
}

function renderPlaybook(playbook: MarketIntelPlaybook) {
  return (
    <div key={playbook.id} className="agent-card">
      <div className="agent-card-head">
        <h3>{playbook.name}</h3>
        <span className="status-pill info">deterministic</span>
      </div>
      <p>{playbook.objective}</p>
      <div className="pill-row">
        {playbook.signals.map((signal) => (
          <span key={signal} className="tag-chip">
            {signal}
          </span>
        ))}
      </div>
    </div>
  );
}

function renderTheme(theme: MarketIntelThemeCard) {
  return (
    <div key={theme.theme_id} className="agent-card">
      <div className="agent-card-head">
        <h3>{theme.name}</h3>
        <span className={`status-pill ${caseStatusClass(theme.active_case_count, theme.escalated_case_count)}`}>
          {theme.active_case_count > 0 ? `${theme.active_case_count} active` : "watching"}
        </span>
      </div>
      <p>{theme.summary}</p>
      <div className="pill-row">
        <span className="info-pill">watchlists: {theme.watchlist_count}</span>
        <span className="info-pill">evidence: {theme.evidence_count}</span>
        <span className="info-pill">escalated: {theme.escalated_case_count}</span>
      </div>
      <div className="pill-row">
        {theme.top_entities.length > 0 ? (
          theme.top_entities.map((entity) => (
            <span key={entity} className="tag-chip">
              {entity}
            </span>
          ))
        ) : (
          <span className="info-pill">no tracked entities yet</span>
        )}
      </div>
    </div>
  );
}

function renderCompany(card: MarketIntelCompanyCard) {
  return (
    <div key={card.company} className="command-row">
      <div className="agent-card-head">
        <h3>{card.company}</h3>
        <span className={`status-pill ${caseStatusClass(card.active_case_count)}`}>
          {card.active_case_count > 0 ? `${card.active_case_count} cases` : "tracked"}
        </span>
      </div>
      <code>mentions: {card.mention_count}</code>
      <code>claims: {card.claim_count}</code>
      <code>latest_signal_at: {card.latest_signal_at ?? "none"}</code>
      <div className="pill-row">
        {card.themes.map((theme) => (
          <span key={theme} className="tag-chip">
            {theme}
          </span>
        ))}
      </div>
    </div>
  );
}

function renderCaseBrief(
  briefing: MarketIntelCaseBrief,
  onAttach: (caseId: string) => void
) {
  return (
    <div key={briefing.case_id} className="agent-card">
      <div className="agent-card-head">
        <h3>{briefing.title}</h3>
        <span className={`status-pill ${briefStatusClass(briefing.status, briefing.attached_to_case)}`}>
          {briefing.status}
        </span>
      </div>
      <p>{briefing.summary}</p>
      <div className="pill-row">
        <span className="info-pill">theme: {briefing.theme_name}</span>
        <span className="info-pill">company: {briefing.company ?? "unassigned"}</span>
        <span className="info-pill">evidence: {briefing.evidence_count}</span>
        <span className="info-pill">claims: {briefing.claim_count}</span>
      </div>
      <div className="pill-row">
        <span className="info-pill">
          latest_signal_at: {briefing.latest_signal_at ?? "unknown"}
        </span>
        <span className={`status-pill ${briefing.attached_to_case ? "ok" : "warn"}`}>
          {briefing.attached_to_case ? "brief attached" : "preview only"}
        </span>
      </div>
      <div className="stack-list">
        <div>
          <p className="mono-label">Key Claims</p>
          <div className="pill-row">
            {briefing.key_claims.map((claim) => (
              <span key={claim} className="tag-chip">
                {claim}
              </span>
            ))}
          </div>
        </div>
        <div>
          <p className="mono-label">Recommended Actions</p>
          <div className="pill-row">
            {briefing.recommended_actions.map((action) => (
              <span key={action} className="tag-chip">
                {action}
              </span>
            ))}
          </div>
        </div>
      </div>
      <div className="button-row">
        <button
          className="btn-secondary"
          type="button"
          onClick={() => onAttach(briefing.case_id)}
          disabled={briefing.attached_to_case}
        >
          {briefing.attached_to_case ? "Attached" : "Attach Brief"}
        </button>
      </div>
    </div>
  );
}

export function MarketIntelPage() {
  const [overview, setOverview] = useState<MarketIntelOverviewResponse | null>(null);
  const [status, setStatus] = useState<string>("Loading market intelligence desk...");
  const [lastBrief, setLastBrief] = useState<GenerateMarketIntelBriefResponse | null>(null);

  async function loadOverview(message?: string) {
    try {
      const response = await fetchMarketIntelOverview();
      setOverview(response);
      setStatus(
        message ??
          `Loaded ${response.market_source_count} market sources, ${response.market_watchlist_count} watchlists, and ${response.tracked_company_count} tracked companies.`
      );
    } catch (error) {
      setStatus(`Failed to load market intelligence desk: ${(error as Error).message}`);
    }
  }

  useEffect(() => {
    void loadOverview();
  }, []);

  async function attachBrief(caseId: string) {
    setStatus(`Attaching deterministic brief to ${caseId}...`);
    try {
      const response = await generateMarketIntelBrief(caseId, { attach_to_case: true });
      setLastBrief(response);
      await loadOverview(`Attached market brief to ${caseId}.`);
    } catch (error) {
      setStatus(`Failed to attach market brief: ${(error as Error).message}`);
    }
  }

  return (
    <section className="dashboard-grid">
      <article className="panel panel-hero panel-span-12">
        <p className="mono-label">Market Intelligence</p>
        <h2>Competitors, Pricing, Launches, and Channel Motion</h2>
        <p>
          Market intelligence runs on the same deterministic substrate as the OSINT desk: explicit
          sources, provenance-linked evidence, watchlists, cases, and guarded follow-up. The use
          case changes. The trust model does not.
        </p>
      </article>

      <article className="panel panel-span-12">
        <p className="mono-label">Market Coverage</p>
        <div className="metrics-grid">
          <div className="metric-card">
            <p className="metric-label">Market Sources</p>
            <p className="metric-value">{overview?.market_source_count ?? 0}</p>
          </div>
          <div className="metric-card">
            <p className="metric-label">Market Watchlists</p>
            <p className="metric-value">{overview?.market_watchlist_count ?? 0}</p>
          </div>
          <div className="metric-card">
            <p className="metric-label">Tracked Companies</p>
            <p className="metric-value">{overview?.tracked_company_count ?? 0}</p>
          </div>
          <div className="metric-card">
            <p className="metric-label">Active Cases</p>
            <p className="metric-value">{overview?.active_case_count ?? 0}</p>
          </div>
        </div>
        <p className="status-line">{status}</p>
      </article>

      <article className="panel panel-span-7">
        <p className="mono-label">Theme Coverage</p>
        <div className="agent-grid">{overview?.theme_cards.map(renderTheme)}</div>
      </article>

      <article className="panel panel-span-5">
        <p className="mono-label">Tracked Companies</p>
        <div className="command-stack">
          {overview?.company_cards.length ? (
            overview.company_cards.map(renderCompany)
          ) : (
            <p>No company signals yet. Register sources or ingest evidence to populate this view.</p>
          )}
        </div>
      </article>

      <article className="panel panel-span-12">
        <p className="mono-label">Active Case Briefs</p>
        <div className="agent-grid">
          {overview?.case_briefs.length ? (
            overview.case_briefs.map((briefing) => renderCaseBrief(briefing, attachBrief))
          ) : (
            <p>No market-intelligence cases are active yet.</p>
          )}
        </div>
      </article>

      <article className="panel panel-span-12">
        <p className="mono-label">Reference Playbooks</p>
        <div className="agent-grid">{overview?.playbooks.map(renderPlaybook)}</div>
      </article>

      <article className="panel panel-span-12">
        <p className="mono-label">Latest Attached Brief</p>
        {lastBrief ? (
          <pre className="json-output">{JSON.stringify(lastBrief, null, 2)}</pre>
        ) : (
          <p>No brief has been attached in this session.</p>
        )}
      </article>
    </section>
  );
}

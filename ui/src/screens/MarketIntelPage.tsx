import { useEffect, useState } from "react";
import {
  GenerateMarketIntelBriefResponse,
  MarketIntelCaseBrief,
  MarketIntelBriefExportPacketResponse,
  MarketIntelCompanyCard,
  MarketIntelOverviewResponse,
  MarketIntelPlaybook,
  MarketIntelThemeCard,
  PriorityBreakdown,
  fetchMarketIntelBriefExportPacket,
  fetchMarketIntelOverview,
  generateMarketIntelBrief,
} from "../lib/api";
import {
  Panel,
  StatCard,
  StatGrid,
  Badge,
  Button,
  Tag,
  StatusLine,
  CodeBlock,
} from "../components";

function caseStatusTone(activeCaseCount: number, escalatedCaseCount?: number) {
  if ((escalatedCaseCount ?? 0) > 0) return "danger" as const;
  if (activeCaseCount > 0) return "warn" as const;
  return "ok" as const;
}

function briefStatusTone(status: MarketIntelCaseBrief["status"], attachedToCase: boolean) {
  if (status === "escalated") return "danger" as const;
  if (attachedToCase || status === "brief_ready") return "ok" as const;
  return "warn" as const;
}

function priorityLabel(priority: PriorityBreakdown) {
  return `score ${priority.total} | a${priority.attention_tier} s${priority.severity_tier} c${priority.corroboration_tier} cred=${priority.credibility_bps}`;
}

function renderPlaybook(playbook: MarketIntelPlaybook) {
  return (
    <div key={playbook.id} className="hx-card">
      <div className="hx-card-head">
        <h3>{playbook.name}</h3>
        <Badge tone="info">deterministic</Badge>
      </div>
      <p className="hx-description">{playbook.objective}</p>
      <div className="hx-tag-row">
        {playbook.signals.map((signal) => (
          <Tag key={signal}>{signal}</Tag>
        ))}
      </div>
    </div>
  );
}

function renderTheme(theme: MarketIntelThemeCard) {
  return (
    <div key={theme.theme_id} className="hx-card">
      <div className="hx-card-head">
        <h3>{theme.name}</h3>
        <Badge tone={caseStatusTone(theme.active_case_count, theme.escalated_case_count)}>
          {theme.active_case_count > 0 ? `${theme.active_case_count} active` : "watching"}
        </Badge>
      </div>
      <p className="hx-description">{theme.summary}</p>
      <div className="hx-tag-row">
        <Tag>{priorityLabel(theme.priority)}</Tag>
        <Tag>watchlists: {theme.watchlist_count}</Tag>
        <Tag>evidence: {theme.evidence_count}</Tag>
        <Tag>escalated: {theme.escalated_case_count}</Tag>
      </div>
      <div className="hx-tag-row">
        {theme.top_entities.length > 0 ? (
          theme.top_entities.map((entity) => (
            <Tag key={entity}>{entity}</Tag>
          ))
        ) : (
          <Tag>no tracked entities yet</Tag>
        )}
      </div>
    </div>
  );
}

function renderCompany(card: MarketIntelCompanyCard) {
  return (
    <div key={card.company} className="hx-row hx-row-stack">
      <div className="hx-card-head">
        <h3>{card.company}</h3>
        <Badge tone={caseStatusTone(card.active_case_count)}>
          {card.active_case_count > 0 ? `${card.active_case_count} cases` : "tracked"}
        </Badge>
      </div>
      <code className="hx-mono-detail">mentions: {card.mention_count}</code>
      <code className="hx-mono-detail">claims: {card.claim_count}</code>
      <code className="hx-mono-detail">latest_signal_at: {card.latest_signal_at ?? "none"}</code>
      <div className="hx-tag-row">
        <Tag>{priorityLabel(card.priority)}</Tag>
        {card.themes.map((theme) => (
          <Tag key={theme}>{theme}</Tag>
        ))}
      </div>
    </div>
  );
}

function renderCaseBrief(
  briefing: MarketIntelCaseBrief,
  onAttach: (caseId: string) => void,
  onExport: (caseId: string) => void
) {
  return (
    <div key={briefing.case_id} className="hx-card">
      <div className="hx-card-head">
        <h3>{briefing.title}</h3>
        <Badge tone={briefStatusTone(briefing.status, briefing.attached_to_case)}>
          {briefing.status}
        </Badge>
      </div>
      <p className="hx-description">{briefing.summary}</p>
      <div className="hx-tag-row">
        <Tag>{priorityLabel(briefing.priority)}</Tag>
        <Tag>theme: {briefing.theme_name}</Tag>
        <Tag>company: {briefing.company ?? "unassigned"}</Tag>
        <Tag>evidence: {briefing.evidence_count}</Tag>
        <Tag>claims: {briefing.claim_count}</Tag>
      </div>
      <div className="hx-tag-row">
        <Tag>latest_signal_at: {briefing.latest_signal_at ?? "unknown"}</Tag>
        <Badge tone={briefing.attached_to_case ? "ok" : "warn"}>
          {briefing.attached_to_case ? "brief attached" : "preview only"}
        </Badge>
      </div>
      <div className="hx-stack">
        <div>
          <p className="hx-eyebrow">Key Claims</p>
          <div className="hx-tag-row">
            {briefing.key_claims.map((claim) => (
              <Tag key={claim}>{claim}</Tag>
            ))}
          </div>
        </div>
        <div>
          <p className="hx-eyebrow">Recommended Actions</p>
          <div className="hx-tag-row">
            {briefing.recommended_actions.map((action) => (
              <Tag key={action}>{action}</Tag>
            ))}
          </div>
        </div>
      </div>
      <div className="hx-cluster">
        <Button
          variant="secondary"
          type="button"
          onClick={() => onAttach(briefing.case_id)}
          disabled={briefing.attached_to_case}
        >
          {briefing.attached_to_case ? "Attached" : "Attach Brief"}
        </Button>
        <Button
          variant="secondary"
          type="button"
          onClick={() => onExport(briefing.case_id)}
        >
          Export Packet
        </Button>
      </div>
    </div>
  );
}

export function MarketIntelPage() {
  const [overview, setOverview] = useState<MarketIntelOverviewResponse | null>(null);
  const [status, setStatus] = useState<string>("Loading market intelligence desk...");
  const [lastBrief, setLastBrief] = useState<GenerateMarketIntelBriefResponse | null>(null);
  const [lastExportPacket, setLastExportPacket] =
    useState<MarketIntelBriefExportPacketResponse | null>(null);

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

  async function exportBrief(caseId: string) {
    setStatus(`Exporting deterministic brief packet for ${caseId}...`);
    try {
      const packet = await fetchMarketIntelBriefExportPacket(caseId);
      setLastExportPacket(packet);
      setStatus(`Exported market brief packet ${packet.packet_id}.`);
    } catch (error) {
      setStatus(`Failed to export market brief: ${(error as Error).message}`);
    }
  }

  return (
    <section className="hx-page-grid">
      <Panel hero span={12} eyebrow="Market Intelligence" title="Competitors, Pricing, Launches, and Channel Motion">
        <p className="hx-description">
          Market intelligence runs on the same deterministic substrate as the OSINT desk: explicit
          sources, provenance-linked evidence, watchlists, cases, and guarded follow-up. The use
          case changes. The trust model does not.
        </p>
      </Panel>

      <Panel span={12} eyebrow="Market Coverage" title="Coverage Metrics">
        <StatGrid>
          <StatCard label="Market Sources" value={overview?.market_source_count ?? 0} />
          <StatCard label="Market Watchlists" value={overview?.market_watchlist_count ?? 0} />
          <StatCard label="Tracked Companies" value={overview?.tracked_company_count ?? 0} />
          <StatCard label="Active Cases" value={overview?.active_case_count ?? 0} />
        </StatGrid>
        <StatusLine>{status}</StatusLine>
      </Panel>

      <Panel span={7} eyebrow="Theme Coverage" title="Themes">
        <div className="hx-card-grid">{overview?.theme_cards.map(renderTheme)}</div>
      </Panel>

      <Panel span={5} eyebrow="Tracked Companies" title="Companies">
        <div className="hx-list">
          {overview?.company_cards.length ? (
            overview.company_cards.map(renderCompany)
          ) : (
            <div className="hx-table-empty">
              <p>No company signals yet. Register sources or ingest evidence to populate this view.</p>
            </div>
          )}
        </div>
      </Panel>

      <Panel span={12} eyebrow="Active Case Briefs" title="Case Briefs">
        <div className="hx-card-grid">
          {overview?.case_briefs.length ? (
            overview.case_briefs.map((briefing) =>
              renderCaseBrief(briefing, attachBrief, exportBrief)
            )
          ) : (
            <div className="hx-table-empty">
              <p>No market-intelligence cases are active yet.</p>
            </div>
          )}
        </div>
      </Panel>

      <Panel span={12} eyebrow="Reference Playbooks" title="Playbooks">
        <div className="hx-card-grid">{overview?.playbooks.map(renderPlaybook)}</div>
      </Panel>

      <Panel span={12} eyebrow="Latest Attached Brief" title="Attached Brief">
        {lastBrief ? (
          <CodeBlock label="Brief JSON">{JSON.stringify(lastBrief, null, 2)}</CodeBlock>
        ) : (
          <div className="hx-table-empty">
            <p>No brief has been attached in this session.</p>
          </div>
        )}
      </Panel>

      <Panel span={12} eyebrow="Latest Export Packet" title="Export Packet">
        {lastExportPacket ? (
          <CodeBlock label="Packet JSON">{JSON.stringify(lastExportPacket, null, 2)}</CodeBlock>
        ) : (
          <div className="hx-table-empty">
            <p>No market brief export packet has been generated in this session.</p>
          </div>
        )}
      </Panel>
    </section>
  );
}

import { expect, Page, Route, test } from "@playwright/test";

type SourceKind =
  | "rss_feed"
  | "website_diff"
  | "json_api"
  | "webhook_ingest"
  | "email_digest"
  | "file_import";

type WatchlistSeverity = "low" | "medium" | "high" | "critical";
type ClaimReviewStatus = "needs_review" | "corroborated" | "rejected";
type CaseStatus = "open" | "monitoring" | "brief_ready" | "escalated" | "closed";

type SourceDefinition = {
  id: string;
  profile_id: string;
  name: string;
  description: string;
  kind: SourceKind;
  endpoint_url: string | null;
  credential_id: string | null;
  credential_header_name: string;
  credential_header_prefix: string | null;
  cadence_minutes: number;
  trust_score: number;
  enabled: boolean;
  tags: string[];
};

type Watchlist = {
  id: string;
  name: string;
  description: string;
  keywords: string[];
  entities: string[];
  min_source_trust: number;
  severity: WatchlistSeverity;
  enabled: boolean;
};

type ProposedClaim = {
  subject: string;
  predicate: string;
  object: string;
  confidence_bps: number;
  rationale?: string | null;
};

type EvidenceItem = {
  id: string;
  source_id: string;
  title: string;
  summary: string;
  content: string;
  url: string | null;
  observed_at: string;
  tags: string[];
  entity_labels: string[];
  provenance_hash: string;
};

type ClaimRecord = ProposedClaim & {
  id: string;
  evidence_id: string;
  review_status: ClaimReviewStatus;
  rationale: string;
};

type WatchlistHit = {
  watchlist_id: string;
  watchlist_name: string;
  evidence_id: string;
  severity: WatchlistSeverity;
  matched_keywords: string[];
  matched_entities: string[];
  reason: string;
};

type CaseFile = {
  id: string;
  title: string;
  watchlist_id: string;
  status: CaseStatus;
  primary_entity: string | null;
  evidence_ids: string[];
  claim_ids: string[];
  latest_reason: string;
  briefing_summary: string | null;
};

type CaseCommand =
  | { type: "mark_monitoring" }
  | { type: "attach_brief"; summary: string }
  | { type: "escalate"; reason: string }
  | { type: "close" }
  | { type: "reopen"; reason: string };

type IngestEvidenceRequest = {
  source_id: string;
  title: string;
  summary: string;
  content: string;
  url?: string | null;
  observed_at: string;
  tags: string[];
  entity_labels: string[];
  proposed_claims: ProposedClaim[];
};

type CredentialMetadataEntry = {
  id: string;
  profile_id: string;
  name: string;
  kind: string;
  metadata: Record<string, string>;
  created_at: string;
  updated_at: string;
};

type CredentialUpsertRequest = {
  profile_id: string;
  name: string;
  kind: string;
  secret: string;
  metadata?: Record<string, string>;
};

const API_ORIGIN = "http://127.0.0.1:3000";
const CORS_HEADERS = {
  "Access-Control-Allow-Headers": "authorization, content-type",
  "Access-Control-Allow-Methods": "GET, POST, PUT, DELETE, OPTIONS",
  "Access-Control-Allow-Origin": "*",
};

const priority = {
  total: 4_220,
  attention_tier: 4,
  severity_tier: 2,
  corroboration_tier: 1,
  credibility_bps: 9_100,
  freshness_tier: 3,
  trust_tier: 2,
  density_tier: 1,
};

function csvValues(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.map(String).filter(Boolean);
}

function slug(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 48);
}

function includesAny(haystack: string, needles: string[]): string[] {
  const lowerHaystack = haystack.toLowerCase();
  return needles.filter((needle) => lowerHaystack.includes(needle.toLowerCase()));
}

async function fulfill(route: Route, payload: unknown, status = 200) {
  await route.fulfill({
    status,
    contentType: "application/json",
    headers: CORS_HEADERS,
    body: JSON.stringify(payload),
  });
}

async function setupGoldenPathApi(page: Page) {
  const sources: SourceDefinition[] = [];
  const watchlists: Watchlist[] = [];
  const evidenceItems: EvidenceItem[] = [];
  const claims: ClaimRecord[] = [];
  const cases: CaseFile[] = [];
  const credentials: CredentialMetadataEntry[] = [];

  function sourceFor(id: string): SourceDefinition {
    const source = sources.find((item) => item.id === id);
    if (!source) throw new Error(`unknown source ${id}`);
    return source;
  }

  function evidenceEntry(evidence: EvidenceItem) {
    const source = sourceFor(evidence.source_id);
    return {
      evidence,
      source_name: source.name,
      source_trust_score: source.trust_score,
      priority,
      linked_case_count: cases.filter((item) => item.evidence_ids.includes(evidence.id)).length,
      linked_claim_count: claims.filter((claim) => claim.evidence_id === evidence.id).length,
      max_linked_severity: watchlists[0]?.severity ?? null,
      semantic_score_bps: null,
    };
  }

  function claimEntry(claim: ClaimRecord) {
    const evidence = evidenceItems.find((item) => item.id === claim.evidence_id);
    const source = evidence ? sourceFor(evidence.source_id) : null;
    return {
      claim,
      evidence_title: evidence?.title ?? "unknown evidence",
      evidence_observed_at: evidence?.observed_at ?? "unknown",
      source_name: source?.name ?? "unknown source",
      source_trust_score: source?.trust_score ?? 0,
      priority,
      linked_case_count: cases.filter((item) => item.claim_ids.includes(claim.id)).length,
      max_linked_severity: watchlists[0]?.severity ?? null,
      semantic_score_bps: null,
    };
  }

  function caseEntry(caseFile: CaseFile) {
    const watchlist = watchlists.find((item) => item.id === caseFile.watchlist_id);
    return {
      case: caseFile,
      watchlist_name: watchlist?.name ?? "unknown watchlist",
      severity: watchlist?.severity ?? "low",
      priority,
      latest_signal_at: "2026-03-06T12:00:00Z",
    };
  }

  function ingestEvidence(request: IngestEvidenceRequest) {
    const source = sourceFor(request.source_id);
    const evidence: EvidenceItem = {
      id: `evidence_${evidenceItems.length + 1}`,
      source_id: source.id,
      title: request.title,
      summary: request.summary,
      content: request.content,
      url: request.url ?? null,
      observed_at: request.observed_at,
      tags: request.tags,
      entity_labels: request.entity_labels,
      provenance_hash: `hash_${evidenceItems.length + 1}`,
    };
    evidenceItems.unshift(evidence);

    const createdClaims = request.proposed_claims.map((proposedClaim) => {
      const claim: ClaimRecord = {
        id: `claim_${claims.length + 1}`,
        evidence_id: evidence.id,
        subject: proposedClaim.subject,
        predicate: proposedClaim.predicate,
        object: proposedClaim.object,
        confidence_bps: proposedClaim.confidence_bps,
        review_status: "needs_review",
        rationale: proposedClaim.rationale ?? "operator supplied claim",
      };
      claims.unshift(claim);
      return claim;
    });

    const text = `${request.title} ${request.summary} ${request.content} ${request.entity_labels.join(" ")}`;
    const hits: WatchlistHit[] = watchlists
      .filter((watchlist) => watchlist.enabled && source.trust_score >= watchlist.min_source_trust)
      .map((watchlist) => ({
        watchlist,
        matched_keywords: includesAny(text, watchlist.keywords),
        matched_entities: includesAny(text, watchlist.entities),
      }))
      .filter((match) => match.matched_keywords.length > 0 || match.matched_entities.length > 0)
      .map((match) => ({
        watchlist_id: match.watchlist.id,
        watchlist_name: match.watchlist.name,
        evidence_id: evidence.id,
        severity: match.watchlist.severity,
        matched_keywords: match.matched_keywords,
        matched_entities: match.matched_entities,
        reason: `matched ${match.watchlist.name}`,
      }));

    const caseUpdates = hits.map((hit) => {
      const primaryEntity = request.entity_labels[0] ?? null;
      let caseFile = cases.find(
        (item) => item.watchlist_id === hit.watchlist_id && item.primary_entity === primaryEntity
      );
      const opened = !caseFile;
      if (!caseFile) {
        caseFile = {
          id: `case_${cases.length + 1}`,
          title: `${hit.watchlist_name}: ${primaryEntity ?? evidence.title}`,
          watchlist_id: hit.watchlist_id,
          status: "open",
          primary_entity: primaryEntity,
          evidence_ids: [],
          claim_ids: [],
          latest_reason: hit.reason,
          briefing_summary: null,
        };
        cases.unshift(caseFile);
      }
      if (!caseFile.evidence_ids.includes(evidence.id)) caseFile.evidence_ids.unshift(evidence.id);
      for (const claim of createdClaims) {
        if (!caseFile.claim_ids.includes(claim.id)) caseFile.claim_ids.unshift(claim.id);
      }
      caseFile.latest_reason = hit.reason;
      return {
        case: caseFile,
        decision: opened ? { kind: "opened" } : { kind: "updated" },
      };
    });

    return {
      duplicate: false,
      evidence,
      claims: createdClaims,
      hits,
      case_updates: caseUpdates,
    };
  }

  await page.addInitScript(() => {
    window.localStorage.clear();
    window.sessionStorage.clear();
  });

  await page.route(`${API_ORIGIN}/api/v1/**`, async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const path = url.pathname;

    if (request.method() === "OPTIONS") {
      await route.fulfill({ status: 204, headers: CORS_HEADERS });
      return;
    }

    if (path === "/api/v1/credentials" && request.method() === "GET") {
      const profileId = url.searchParams.get("profile_id");
      await fulfill(route, {
        persistence_enabled: true,
        credentials: credentials.filter((entry) => entry.profile_id === profileId),
      });
      return;
    }

    if (path === "/api/v1/credentials" && request.method() === "POST") {
      const body = request.postDataJSON() as CredentialUpsertRequest;
      if (!body.secret) {
        await fulfill(route, { error: "secret required" }, 400);
        return;
      }
      const existing = credentials.find(
        (entry) => entry.profile_id === body.profile_id && entry.name === body.name
      );
      const now = "2026-03-06 12:00:00+00";
      const credential: CredentialMetadataEntry = {
        id: existing?.id ?? "50000000-0000-0000-0000-000000000011",
        profile_id: body.profile_id,
        name: body.name,
        kind: body.kind,
        metadata: body.metadata ?? {},
        created_at: existing?.created_at ?? now,
        updated_at: now,
      };
      if (existing) {
        Object.assign(existing, credential);
      } else {
        credentials.unshift(credential);
      }
      await fulfill(route, { persistence_enabled: true, credential });
      return;
    }

    if (path.match(/^\/api\/v1\/credentials\/[^/]+\/[^/]+$/) && request.method() === "DELETE") {
      const [, , , , profileId, credentialId] = path.split("/");
      const index = credentials.findIndex(
        (entry) =>
          entry.profile_id === decodeURIComponent(profileId) &&
          entry.id === decodeURIComponent(credentialId)
      );
      if (index >= 0) credentials.splice(index, 1);
      await fulfill(route, { persistence_enabled: true, deleted: index >= 0 });
      return;
    }

    if (path === "/api/v1/sources" && request.method() === "GET") {
      await fulfill(route, { sources });
      return;
    }

    if (path === "/api/v1/sources" && request.method() === "POST") {
      const body = request.postDataJSON() as Partial<SourceDefinition>;
      const source: SourceDefinition = {
        id: `source_${slug(String(body.name ?? "golden")) || "golden"}`,
        profile_id: String(body.profile_id ?? "50000000-0000-0000-0000-000000000010"),
        name: String(body.name ?? "Golden Source"),
        description: String(body.description ?? ""),
        kind: (body.kind ?? "json_api") as SourceKind,
        endpoint_url: body.endpoint_url ? String(body.endpoint_url) : null,
        credential_id: body.credential_id ? String(body.credential_id) : null,
        credential_header_name: String(body.credential_header_name ?? "Authorization"),
        credential_header_prefix:
          body.credential_header_prefix === null
            ? null
            : String(body.credential_header_prefix ?? "Bearer"),
        cadence_minutes: Number(body.cadence_minutes ?? 15),
        trust_score: Number(body.trust_score ?? 90),
        enabled: body.enabled !== false,
        tags: csvValues(body.tags),
      };
      sources.splice(0, sources.length, source);
      await fulfill(route, { source });
      return;
    }

    if (path.match(/^\/api\/v1\/sources\/[^/]+\/collect$/) && request.method() === "POST") {
      const sourceId = decodeURIComponent(path.split("/")[4]);
      const source = sourceFor(sourceId);
      const result = ingestEvidence({
        source_id: source.id,
        title: "Alice North resigned from Orion Dynamics",
        summary: "Collection found a leadership signal.",
        content: "Alice North resigned from Orion Dynamics after a short detention.",
        url: source.endpoint_url,
        observed_at: "2026-03-06T12:00:00Z",
        tags: ["leadership", "security"],
        entity_labels: ["alice north", "orion dynamics"],
        proposed_claims: [
          {
            subject: "alice north",
            predicate: "resigned_from",
            object: "orion dynamics",
            confidence_bps: 9_100,
            rationale: "collected source stated the resignation",
          },
        ],
      });
      await fulfill(route, {
        source,
        fetched_url: source.endpoint_url ?? "",
        collected_count: 1,
        duplicate_count: 0,
        results: [result],
      });
      return;
    }

    if (path === "/api/v1/watchlists" && request.method() === "GET") {
      await fulfill(route, { watchlists });
      return;
    }

    if (path === "/api/v1/watchlists" && request.method() === "POST") {
      const body = request.postDataJSON() as Partial<Watchlist>;
      const watchlist: Watchlist = {
        id: `watch_${slug(String(body.name ?? "golden")) || "golden"}`,
        name: String(body.name ?? "Golden Watch"),
        description: String(body.description ?? ""),
        keywords: csvValues(body.keywords),
        entities: csvValues(body.entities),
        min_source_trust: Number(body.min_source_trust ?? 60),
        severity: (body.severity ?? "high") as WatchlistSeverity,
        enabled: body.enabled !== false,
      };
      watchlists.splice(0, watchlists.length, watchlist);
      await fulfill(route, { watchlist });
      return;
    }

    if (path === "/api/v1/evidence" && request.method() === "GET") {
      await fulfill(route, { evidence: evidenceItems.map(evidenceEntry) });
      return;
    }

    if (path === "/api/v1/evidence/ingest" && request.method() === "POST") {
      const result = ingestEvidence(request.postDataJSON() as IngestEvidenceRequest);
      await fulfill(route, result);
      return;
    }

    if (path === "/api/v1/claims" && request.method() === "GET") {
      await fulfill(route, { claims: claims.map(claimEntry) });
      return;
    }

    if (path.match(/^\/api\/v1\/claims\/[^/]+\/review$/) && request.method() === "POST") {
      const claimId = decodeURIComponent(path.split("/")[4]);
      const body = request.postDataJSON() as { status: ClaimReviewStatus };
      const claim = claims.find((item) => item.id === claimId);
      if (!claim) {
        await fulfill(route, { error: "claim not found" }, 404);
        return;
      }
      claim.review_status = body.status;
      await fulfill(route, { claim });
      return;
    }

    if (path === "/api/v1/intel/overview" && request.method() === "GET") {
      await fulfill(route, {
        source_count: sources.length,
        watchlist_count: watchlists.length,
        evidence_count: evidenceItems.length,
        claim_count: claims.length,
        open_case_count: cases.filter((item) => item.status !== "closed").length,
        escalated_case_count: cases.filter((item) => item.status === "escalated").length,
      });
      return;
    }

    if (path === "/api/v1/cases" && request.method() === "GET") {
      await fulfill(route, { cases: cases.map(caseEntry) });
      return;
    }

    if (path.match(/^\/api\/v1\/cases\/[^/]+\/transition$/) && request.method() === "POST") {
      const caseId = decodeURIComponent(path.split("/")[4]);
      const body = request.postDataJSON() as { command: CaseCommand };
      const caseFile = cases.find((item) => item.id === caseId);
      if (!caseFile) {
        await fulfill(route, { error: "case not found" }, 404);
        return;
      }

      if (body.command.type === "mark_monitoring") caseFile.status = "monitoring";
      if (body.command.type === "attach_brief") {
        caseFile.status = "brief_ready";
        caseFile.briefing_summary = body.command.summary;
      }
      if (body.command.type === "escalate") {
        caseFile.status = "escalated";
        caseFile.latest_reason = body.command.reason;
      }
      if (body.command.type === "close") caseFile.status = "closed";
      if (body.command.type === "reopen") {
        caseFile.status = "open";
        caseFile.latest_reason = body.command.reason;
      }

      await fulfill(route, {
        transition: {
          case: caseFile,
          decision: { kind: "status_changed", status: caseFile.status },
        },
      });
      return;
    }

    await fulfill(route, { error: `Unhandled test route: ${request.method()} ${path}` }, 404);
  });
}

test("operator can promote a collected signal into a reviewed case", async ({ page }) => {
  await setupGoldenPathApi(page);

  await page.goto("/credentials");
  await expect(
    page.getByRole("heading", { name: "Encrypted Access Material With Redacted Operator Views" })
  ).toBeVisible();
  await page.getByRole("textbox", { name: "secret" }).fill("super-secret-value");
  await page.getByRole("button", { name: "Save Credential" }).click();
  await expect(page.getByText("Saved GitHub Token; secret accepted and redacted.")).toBeVisible();
  await expect(page.getByRole("heading", { name: "GitHub Token" })).toBeVisible();
  await expect(page.locator("body")).not.toContainText("super-secret-value");
  await page.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByText("Deleted GitHub Token.")).toBeVisible();

  await page.goto("/sources");
  await page.getByLabel("profile_id").fill("50000000-0000-0000-0000-000000000010");
  await page.getByLabel("name", { exact: true }).fill("Golden Source");
  await page.getByLabel("description").fill("Deterministic source for E2E coverage.");
  await page.getByLabel("kind").selectOption("json_api");
  await page.getByLabel("endpoint_url").fill("https://example.test/feed.json");
  await page.getByLabel("credential_id").fill("50000000-0000-0000-0000-000000000011");
  await page.getByLabel("credential_header_name").fill("Authorization");
  await page.getByLabel("credential_header_prefix").fill("Bearer");
  await page.getByLabel("cadence_minutes").fill("15");
  await page.getByLabel("trust_score").fill("91");
  await page.getByLabel("tags").fill("leadership, security");
  await page.getByRole("button", { name: "Register Source" }).click();

  await expect(page.getByText("Created Golden Source.")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Golden Source" })).toBeVisible();

  await page.goto("/watchlists");
  await page.getByLabel("name").fill("Golden Watch");
  await page.getByLabel("description").fill("Leadership and security change signals.");
  await page.getByLabel("keywords").fill("resigned, appointed");
  await page.getByLabel("entities").fill("alice north, orion dynamics");
  await page.getByLabel("min_source_trust").fill("60");
  await page.getByLabel("severity").selectOption("high");
  await page.getByRole("button", { name: "Create Watchlist" }).click();

  await expect(page.getByText("Created Golden Watch.")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Golden Watch" })).toBeVisible();

  await page.goto("/sources");
  await page.getByRole("button", { name: "Collect Now" }).click();
  await expect(page.getByText(/Collected 1 item\(s\).*1 case update\(s\)\./)).toBeVisible();

  await page.goto("/evidence");
  await expect(
    page.getByRole("heading", { name: /Alice North resigned from Orion Dynamics/ })
  ).toBeVisible();
  await page.getByLabel("title").fill("Alice North appointed by Orion Dynamics");
  await page
    .getByLabel("summary")
    .fill("Follow-on leadership appointment at Orion Dynamics.");
  await page
    .getByLabel("content")
    .fill("Alice North was appointed by Orion Dynamics after the resignation report.");
  await page.getByLabel("url").fill("https://example.test/report");
  await page.getByRole("button", { name: "Ingest Evidence" }).click();

  await expect(
    page.getByText(/Ingested evidence_2; created 1 claims, 1 hits, 1 case updates\./)
  ).toBeVisible();
  await expect(page.getByText("Golden Watch").first()).toBeVisible();

  await page.getByRole("button", { name: "Corroborate" }).first().click();
  await expect(page.getByText(/Claim claim_\d+ -> corroborated\./)).toBeVisible();
  await expect(page.getByText("corroborated").first()).toBeVisible();

  await page.goto("/cases");
  await expect(page.getByRole("heading", { name: /Golden Watch: alice north/ })).toBeVisible();
  await page.getByRole("button", { name: "Attach Brief" }).first().click();

  await expect(page.getByText("Case case_1 -> brief_ready.")).toBeVisible();
  await expect(
    page.getByText(/briefing_summary: Analyst briefing attached for Golden Watch: alice north\./)
  ).toBeVisible();
});

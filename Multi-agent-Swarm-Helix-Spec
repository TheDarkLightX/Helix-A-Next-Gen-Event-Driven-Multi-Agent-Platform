🛠️ **Project Codename: “Helix” – A Next-Gen Event-Driven Agent Platform (Huginn 2.0+)**  
*(Everything below is directly paste-able into your architecture repo as `docs/specification.md`)*  

---

## 0 Vision & Goals
| Goal | KPI | Stretch |
|------|-----|---------|
| **Self-hosted personal event-automation** (like Huginn/IFTTT, but smarter) | <50 ms rule latency, 99.95 % uptime | Autoscale to 1 M agents on one cluster |
| **ML-augmented agents** *(LLM declarative rules + classic pipelines)* | ≥95 % correct trigger matching in benchmarks | Natural-language rule synthesis |
| **Modern DX & Ops** | Full IaC/k8s one-line deploy | Hot-reload plugins in <2 s |
| **Security first** | Zero CVE pipeline, OSS-audited | Formal-verified policy engine |

---

## 1 Tech-Stack Rationale

| Layer | Choice | Reason |
|-------|--------|--------|
| **Lang** | *Rust* for core runtime (determinism, perf, WASM hosting) + *TypeScript* for plugins/web | Safety + huge ecosystem |
| **Runtime** | Tokio async + NATS JetStream (event mesh) | Back-pressure, at-least-once, horizontal scale |
| **Rule Engine** | [`cedar-policy`](https://www.cedarpolicy.com) + Rete-like incremental matcher | Policy as data, micro-seconds match time |
| **Storage** | PostgreSQL 16 (metadata), Redpanda/Apache Kafka (streams), SurrealDB (graph of agents) | Strong SQL, high-throughput log, native graph queries |
| **Vector store** | Qdrant (embeddings for LLM context) | Rust client, ANN speed |
| **ML/LLM** | OpenAI / Anthropic adapters + local GGUF (via `llama.cpp`) | Hybrid SaaS/on-prem |
| **UI** | React 19 + TanStack Router, shadcn/ui, Tailwind | Modern PWA |
| **DevOps** | Helm + K8s, GitHub Actions, FluxCD | GitOps, multi-cloud |
| **Observability** | OpenTelemetry, Prometheus, Grafana Tempo | Unified traces & metrics |

---

## 2 Domain Model (Ubiquitous Language)

* **Agent** – encapsulated unit of behavior (`Source`, `Transformer`, `Action` mix-in traits).  
* **Event** – immutable JSON blob with headers (`id`, `ts`, `agent_id`, `kind`).  
* **Recipe** – DAG of Agents.  
* **Credential** – secret (API key, OAuth token) versioned & encrypted (age).  
* **Policy** – Cedar doc controlling data/agent access.  
* **Profile** – multi-tenant namespace (user or org).  

---

## 3 C4 View

### 3.1 Context
```
+──────────+       publish        +──────────────+
| External | ───────────────────▶ |  API Gateway |
|  Webhook |                      +─────┬────────+
+──────────+                            │REST/gRPC
                                         ▼
                       +─────────────────────────────+
                       |    Helix Cluster (k8s)      |
                       +─────────────────────────────+
```

### 3.2 Container
```
┌───────────────┐  WAL  ┌──────────────┐  CQRS  ┌─────────────┐
│  Event Hub    │──────▶│ Rule Engine  │───────▶│ Action Hub │
│ (NATS Stream) │◀──────│  (Rete+ML)   │        │  (Workers) │
└───────────────┘ ACK    └──────────────┘ RETRY  └─────────────┘
          ▲                 ▲    │               │
          │ ingest          │    └─ DSL/WASM ────┘
          │                 │
┌─────────┴───────┐  Graph  │
│  Recipe Store   │◀───────┘
└─────────────────┘
```

### 3.3 Component (Rule Engine)
* **Parser** – converts YAML/LLM text → AST.  
* **Optimizer** – static graph reductions, predicate pushdown.  
* **Matcher** – incremental Rete network (Rust, sled for alpha memories).  
* **Sidecar ML hooks** – embedding similarity, classification, GPT-function call.

### 3.4 Code
* `crates/helix-core/src/agent.rs`  
* `plugins/twitter_source.ts` (WASM compiled)  

---

## 4 Algorithms

| Problem | Algorithm | Complexity | Notes |
|---------|-----------|------------|-------|
| Rule matching | **Rete** w/ shared alpha | O(#events × #joins) reduced by discrimination | Incremental, memoized |
| Deduplication | HyperLogLog + Bloom | O(1) avg | 0.01 % FP |
| Schedule (cron/interval) | Hierarchical timing wheel | O(1) enqueue/dequeue | µs timers |
| Content similarity | SBERT embeddings + HNSW ANN | O(log N) query | Qdrant shard-aware |
| Rate limiting | Token bucket in Redis via `rate-limiter-flexible` | O(1) | Cluster-safe |

---

## 5 Agent SDK

```rust
#[agent(source)]
async fn github_issues(ctx: SourceContext) -> Result<()> {
  let issues = fetch_github(...).await?;
  for i in issues { ctx.emit(json!({ "title": i.title, ... })); }
}
```
* Macros generate WASM interface + JSON-schema.  
* Deterministic Rust compile target: `wasm32-wasi`.  
* Same macro set in TypeScript (`deno compile --unstable --target wasm`).  

---

## 6 AI-Augmented Features

1. **Natural-Language Recipe Builder**  
   *Prompt:* “When BTC price > 5 %, SMS me.”  
   *Pipeline:*
   - LLM parses intent → intermediate JSON spec (`trigger`, `condition`, `action`).  
   - Spec validated (Cedar policy) → auto-creates agents via SDK.  

2. **Self-healing Agents** – On exception, run GPT-4 “fixit” recipe:  
   *Input:* stack trace + agent source code.  
   *LLM Output:* patch diff + commit message.  
   *CI:* auto-runs unit & integration tests → auto-merge if green.  

3. **Anomaly Detection** – Time-series of event rates fed to Facebook Prophet; >3σ deviations raise alerts agent.

---

## 7 Security Model

* **Zero-trust:** All agent plugins run in Wasmtime sandbox (cap-sandboxed FS/net).  
* **Secrets:** Sealed box per profile using age-xsalsa20 + scrypt KDF.  
* **RBAC + ABAC:** Cedar policies enforce `profile_id` isolation; every API call includes JWT with scopes.  
* **Supply-chain:** Sigstore cosign verification on container and WASM modules.  
* **Smart-contract (if needed):** optional on-chain audit trail via Solana program logging CID of recipe DAG.

---

## 8 Testing & QA

| Level | Tool | Coverage Target |
|-------|------|-----------------|
| Unit | Rust `cargo test`, Vitest for TS | 90 % lines |
| Integration | `docker-compose up test-env` → run cucumber-rs | 90 % scenarios |
| E2E | Playwright + Testcontainers | Critical paths |
| Fuzz | `cargo fuzz`, Echidna for solidity module | 95 % branch |
| Static | Clippy + Rudra, ESLint, Semgrep | Zero High severity |
| Perf | Criterion.rs benchmarks on matcher | alert if >1.5× baseline |

CI matrix in GitHub Actions; mandatory green to merge.

---

## 9 DevOps & Release

* **GitFlow + Conventional Commits** → auto version via semantic-release.  
* **Helm charts** for prod; `helmfile sync` for staged envs.  
* Canary: blue/green NATS JetStream streams.  
* Observability: OTEL exporter sidecar in every pod; Jaeger + Grafana dashboards incl. per-rule latency.

---

## 10 Roadmap

| Phase | Milestone | Target |
|-------|-----------|--------|
| **MVP** | Core runtime, REST API, Web UI, 10 OSS plugins | Month 3 |
| **β** | LLM recipe builder, Wasm plugin SDK, multi-tenant RBAC | Month 6 |
| **v1.0** | Autoscale cluster, BYO-LLM, marketplace, mobile PWA | Month 10 |
| **v1.1** | Edge deploy (Wasm workers), on-chain audit, AI-self-healing | Month 14 |

---

## 11 Super-Concise LLM Prompt Rules (≤200 chars)

```
Role=HelixAgent
Given: Story+steps, output Gherkin only.
Then: Gen Rust/TS plugin code (Wasm), unit tests 90% cov.
Must pass clippy+ci. 1-retry auto-fix loop. Unique emoji each reply.
```

---

### End of Specification

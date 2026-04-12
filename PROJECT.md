# Agentic Application Framework — Project Report

## Executive Summary

The Agentic Application Framework (hereafter AAF) is a next-generation software execution platform that uses AI agents as the universal interface of software, integrating communication between humans, applications, services, and APIs through **intent-based semantic contracts**.

The essence of this framework is *not* "a system where AI cleverly does everything." It is a software-engineered execution platform that structures a specific sequence of operations: **receive an intent, discover capabilities, delegate safely within constraints, progress with state, stop at risky points, and leave behind artifacts and rationale**.

AAF combines the qualities of a UI framework, Integration Platform, Workflow Engine, Governance Platform, and Enterprise Operating Layer. It realizes the **Intent-first Application Architecture** that comes after API-first.

---

## Table of Contents

1. [Design Philosophy and Principles](#1-design-philosophy-and-principles)
2. [Architecture Overview](#2-architecture-overview)
3. [Core Layer Detailed Design](#3-core-layer-detailed-design)
4. [Contract System](#4-contract-system)
5. [Security and Trust Model](#5-security-and-trust-model)
6. [Economic Layer and Cost Management](#6-economic-layer-and-cost-management)
7. [Testing and Quality Assurance](#7-testing-and-quality-assurance)
8. [Operability, Updatability, Evolvability](#8-operability-updatability-evolvability)
9. [Robustness and Failure Design](#9-robustness-and-failure-design)
10. [SDK and Developer Experience](#10-sdk-and-developer-experience)
11. [Distribution and Packaging](#11-distribution-and-packaging)
12. [Value Proposition and Application Domains](#12-value-proposition-and-application-domains)
13. [Go-to-Market Strategy](#13-go-to-market-strategy)
14. [Roadmap](#14-roadmap)
15. [Appendix: Glossary and Reference Architecture](#15-appendix)

---

## 1. Design Philosophy and Principles

### 1.1 Paradigm Shift: From Syntactic Contracts to Semantic Contracts

Traditional software integration relies on syntactic contracts. For a REST API, URL paths, HTTP methods, and JSON schemas must be agreed upon in advance, and a single character mismatch breaks communication. GraphQL, gRPC, and Protocol Buffers differ in abstraction level, but they all share the same cost of "rigidly agreeing on structure in advance."

What AAF proposes is a shift to **semantic contracts**.

| Aspect | Syntactic Contract | Semantic Contract |
|---|---|---|
| Unit of integration | Endpoints and schemas | Intents, capabilities, and artifacts |
| Change tolerance | A field rename affects all clients | Agents absorb the change semantically |
| Exception handling | Every pattern must be pre-defined | Ambiguity and missing info dynamically interpreted and filled in |
| Integration cost | Build adapters per API specification | Publish capabilities and grant connection permission |
| Drift between docs and code | An ever-present risk | Agents are self-describing |

That said, letting natural language flow through internal communication as-is is explicitly forbidden. Natural language is the entry point; internally it must be converted into a semi-structured semantic interface (Intent Envelope). Free-form-only internal communication is fragile, un-auditable, and poorly reproducible.

### 1.2 Core Design Principles

**P1. Agent as Translator, Not Authority**
Agents focus exclusively on translating, routing, and negotiating intents. Business logic and final computations/decisions are handled by a Deterministic Core outside the agent.

**P2. Intent-first, not UI-first**
Both users and systems state an "intent" first. From there, the agent discovers capabilities, checks constraints, delegates if necessary, obtains approvals, and proceeds with execution.

**P3. Typed Internals, Natural Externals**
Human-facing interfaces are natural language, voice, and GUI. System-facing interfaces are events, HTTP, and messages. But internal communication always uses typed protocols.

**P4. Bounded Autonomy**
Agent autonomy always has boundaries. Maximum step count, allowed capability list, budget ceiling, time ceiling, and mandatory approval points are all imposed as constraints.

**P5. Trust is Earned, Not Granted**
Trust expands gradually, based on proven performance. On Day 1, every operation is human-in-the-loop; as the track record accumulates, autonomy is raised; if problems occur, it is instantly reduced.

**P6. Failure as First-class Concern**
Failure is designed as part of the normal path, not as an exception. Fallback, retry, partial success, compensation, escalation, and human takeover are all built into the structure.

**P7. Observability is Architecture, Not Feature**
A system where "why did it make that decision?" cannot be traced back is not operable in production. Reasoning traces are part of the architecture from day one.

**P8. Compositional Safety**
Combining safe operations does not guarantee the combination is safe. Risk assessment of composed capabilities is not a simple sum of components but an evaluation of the emergent risks that arise from the combination.

**P9. Minimal Interpretation**
When interpreting an intent, agents keep interpretation to a minimum. Uncertain parts are not filled in by guessing — they are confirmed explicitly.

**P10. Reversibility Awareness**
Reversibility is evaluated in advance for every operation; irreversible operations require a higher approval level and a preview.

### 1.3 Clear Distinction Between Agent and Workflow

AAF strictly distinguishes between agent and workflow.

- **Workflow**: A deterministic execution flow that follows a pre-defined code path. Branch conditions are also pre-defined.
- **Agent**: A non-deterministic execution actor that decides steps dynamically and revises its plan according to the situation.

AAF allows both to coexist. Deterministic nodes and Agent nodes live side by side on the Graph Runtime, and the appropriate kind is chosen depending on the nature of the task. Billing calculations and inventory reservation are handled by Workflow nodes; requirement interpretation and exception classification are handled by Agent nodes.

---

## 2. Architecture Overview

### 2.1 Layered Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    A. Agentic Front Door                        │
│  Chat │ Voice │ Slack │ Email │ API │ Event │ Webhook            │
├─────────────────────────────────────────────────────────────────┤
│                    B. Intent Compiler                           │
│  NL Parse → Intent Type → Field Extraction → Refinement        │
├─────────────────────────────────────────────────────────────────┤
│                    C. Capability Registry                       │
│  Agent Cards │ Tool Schemas │ Policy Metadata │ Discovery       │
├────────────────────────┬────────────────────────────────────────┤
│  D. Planner / Router   │   E. Policy / Risk Engine             │
│  Task Decomposition    │   AuthN/AuthZ │ Data Classification   │
│  Capability Matching   │   Approval Routing │ Budget Control   │
│  Bounded Planning      │   Tenant Isolation │ Guard Layers     │
├────────────────────────┴────────────────────────────────────────┤
│                    F. Graph Runtime                              │
│  Sequential │ Parallel │ Branch │ Retry │ Pause │ Checkpoint   │
│  Human Approval │ Resume │ Fork │ Partial Restart │ Timeout    │
├──────────────┬──────────────┬───────────────────────────────────┤
│ G. Specialist│ H. Tool /    │ I. State / Memory System          │
│   Agents     │  Service     │   Working State │ Thread Memory   │
│              │  Adapters    │   Long-term Memory │ Artifact     │
│  Search      │  MCP Servers │   Store                           │
│  Planning    │  Function    │                                   │
│  Analyst     │    Tools     ├───────────────────────────────────┤
│  Domain      │  Internal    │ J. Trust / Identity Layer         │
│  Negotiation │    APIs      │   Agent Identity │ Delegated Auth │
│  UI          │  RPA         │   Signed Artifacts │ Provenance   │
├──────────────┴──────────────┴───────────────────────────────────┤
│                    K. Trace / Replay / Evaluation                │
│  Execution Trace │ Checkpoint Replay │ Semantic Regression      │
│  Policy Violation Detection │ Cost Attribution │ Intent Fidelity│
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Data Flow

```
User intent (natural language)
    ↓
[A. Front Door] → input normalization, authentication
    ↓
[B. Intent Compiler] → Intent Envelope generation
    ↓
[C. Capability Registry] → discovery of required capabilities
    ↓
[D. Planner] ←→ [E. Policy Engine] → execution plan generation (constrained)
    ↓
[F. Graph Runtime] → execution begins
    ↓ ↑ (loop)
[G. Specialist Agents] ←→ [H. Adapters] → capability execution
    ↓ ↑
[I. Memory] → context provision and result storage
    ↓ ↑
[E. Policy Engine] → permission and risk checks at each step
    ↓
[J. Trust Layer] → signing and provenance recording
    ↓
[K. Trace] → full step recording
    ↓
Result → [A. Front Door] → presented to the user
```

### 2.3 Relationship to External Protocols

| Protocol / Standard | Role in AAF |
|---|---|
| A2A (Agent-to-Agent) | Agent-to-agent discovery and communication. Capability publication via Agent Cards, cooperative execution via Task/Message/Artifact |
| MCP (Model Context Protocol) | Connecting agents to tools and data sources. JSON-RPC 2.0 based, with Host/Client/Server role separation |
| REST / gRPC | Used in the adapter layer for Deterministic Core and legacy services. Leveraged as Fast Path |
| Kafka / Event Streaming | Asynchronous communication backbone between agents |
| Service Mesh (e.g. Istio) | Low-level control of agent-to-agent communication (mTLS, rate limiting) |
| OpenAPI / Swagger | Base for automatic Agent Manifest generation |

---

## 3. Core Layer Detailed Design

### 3.1 Agentic Front Door

The unified entry point for all requests.

**Input channels:**
- Chat (Web / Mobile / Desktop)
- Voice (speech recognition → text conversion)
- Messaging (Slack / Teams / Discord)
- Email
- Programmatic API (calls from other systems)
- Events / Webhooks (external triggers)

**Responsibilities:**
- User authentication and session management
- Input normalization (multimodal: text, image, voice, files)
- Hand-off to the Intent Compiler
- In-flight progress display
- Providing the approval UI
- Result presentation (adaptive display per channel)

**Design policy:**
The Front Door is a presentation layer and carries no business logic. It is extended per channel via an adapter pattern. New channels (future AR/VR, etc.) are supported by adding adapters.

### 3.2 Intent Compiler

The layer that converts natural-language input into a structured Intent Envelope.

**Processing pipeline:**

```
Natural language input
  ↓
1. Intent Type Classification
   → TransactionalIntent / AnalyticalIntent / PlanningIntent /
     DelegationIntent / GovernanceIntent
  ↓
2. Field Extraction
   → goal, domain, constraints, entities, parameters
  ↓
3. Context Enrichment
   → Prior context, user profile, and org settings added from the Memory layer
  ↓
4. Completeness Check
   → Determine whether required fields are satisfied
  ↓
5. Refinement (only when needed)
   → Clarification questions for missing info (minimum questions, maximum refinement)
  ↓
6. Intent Envelope generated
```

**Intent Type System:**

```yaml
intent_types:
  TransactionalIntent:
    description: "Intents that involve data mutations or operations"
    requires: [target_entity, action, authority]
    risk_tier: write
    examples:
      - "Cancel this order"
      - "Update the customer information"

  AnalyticalIntent:
    description: "Intents for searching, aggregating, or analyzing information"
    requires: [question, data_scope]
    optional: [output_format, comparison_target]
    risk_tier: read
    examples:
      - "Show me last month's sales by region"
      - "Tell me the churn rate trend"

  PlanningIntent:
    description: "Intents for producing multi-step plans"
    requires: [goal, constraints]
    optional: [timeline, budget, success_criteria]
    risk_tier: advisory
    examples:
      - "Build a hiring plan for next quarter"
      - "Create a launch plan for the new product"

  DelegationIntent:
    description: "Intents that delegate to another agent or human"
    requires: [task, recipient_capability, handoff_context]
    risk_tier: delegation
    examples:
      - "Ask the legal team to review this contract"
      - "Have the design team create the mockups"

  GovernanceIntent:
    description: "Intents that change policies, permissions, or configuration"
    requires: [policy_change, justification, approval_chain]
    risk_tier: governance
    examples:
      - "Change the expense-approval ceiling"
      - "Issue a new API key"
```

**Refinement Protocol:**
When refining an intent, the agent uses the following strategy.

1. Fields that can be inferred from long-term memory and the user profile are first inferred.
2. If the inference confidence is above a threshold, proceed while explicitly stating the inference.
3. Only for items below the confidence threshold are questions asked.
4. Questions are limited to at most two per turn.

### 3.3 Capability Registry

A catalog that manages the capabilities of each agent, tool, and service.

**Capability Record:**

```yaml
capability:
  id: "cap-inventory-check"
  name: "Stock inquiry"
  description: "Returns the current stock level for a given SKU or category"
  version: "2.1.0"
  owner: "warehouse-team"
  provider_agent: "inventory-agent"

  # I/O
  input_schema:
    type: object
    properties:
      skus:
        type: array
        items: { type: string }
        max_items: 100
      include_reserved: { type: boolean, default: false }
  output_schema:
    type: object
    properties:
      items:
        type: array
        items:
          type: object
          properties:
            sku: { type: string }
            available: { type: integer }
            reserved: { type: integer }
            warehouse: { type: string }

  # Metadata
  side_effect: none       # none / read / write / delete / send / payment
  idempotent: true
  reversible: false       # Read-only; reversibility is not applicable
  deterministic: true     # Guaranteed same output for same input

  # Performance and cost
  sla:
    latency_p50: 50ms
    latency_p99: 200ms
    availability: 99.9%
  cost:
    per_request: 0.001
    currency: USD

  # Security
  required_scope: "inventory:read"
  data_classification: internal
  audit_level: standard

  # Degradation strategy
  degradation:
    - level: full
      description: "Real-time stock across all warehouses"
    - level: partial
      description: "Primary warehouses only, with a 15-minute delay"
      trigger: "Primary DB response latency increased"
    - level: cached
      description: "Last cached value (up to 1 hour old)"
      trigger: "DB unreachable"
    - level: unavailable
      alternative: "Request manual check"

  # Dependencies and conflicts
  depends_on: ["cap-warehouse-connect"]
  conflicts_with: []

  # Tags for discovery
  domains: ["warehouse", "supply-chain"]
  tags: ["inventory", "stock", "availability"]
```

**Capability Discovery:**
When the Planner discovers the capabilities required to accomplish a task, it searches in the following order.

1. Enumerate capabilities whose domain matches the `domain` field of the Intent Envelope.
2. Match `goal` against capability descriptions by semantic similarity.
3. Filter by constraint conditions (SLA, cost, permissions).
4. Rank by Trust Score.
5. If multiple candidates remain, select by a weighted score of cost-efficiency, latency, and trust.

**A2A integration:**
External agents publish their capabilities in A2A Agent Card format. The Capability Registry manages both internal Capability Records and A2A Agent Cards uniformly.

### 3.4 Planner / Router

Generates an execution plan from an Intent Envelope and the available capabilities.

**Planning constraints (Bounded Autonomy):**

```yaml
planning_constraints:
  max_steps: 10                    # Max steps per task
  max_depth: 5                     # Max agent-to-agent delegation depth
  allowed_capabilities: [...]      # List of permitted capabilities
  max_budget: 1.00                 # USD
  max_duration: 300                # seconds
  required_approvals:
    - condition: "side_effect in [write, delete, send, payment]"
      approval: human_review
    - condition: "cost > 0.50"
      approval: budget_owner
    - condition: "data_classification == confidential"
      approval: data_owner
  prohibited_actions:
    - "External transmission of PII"
    - "Deletion of production data"
```

**Plan format:**

```yaml
execution_plan:
  intent_id: "int-abc123"
  steps:
    - step_id: 1
      type: agent_node
      capability: "cap-customer-search"
      input_mapping: "intent.constraints.customer_segment → query"
      expected_output: "customer_list"
    - step_id: 2
      type: deterministic_node
      capability: "cap-revenue-calculate"
      input_mapping: "step_1.output.customer_list → customers"
      expected_output: "revenue_report"
    - step_id: 3
      type: agent_node
      capability: "cap-report-summarize"
      input_mapping: "step_2.output → data"
      expected_output: "summary_artifact"
    - step_id: 4
      type: approval_gate
      condition: "summary_artifact.contains_pii == true"
      approver: data_owner
  fallback:
    on_step_failure: retry_with_degraded_capability
    on_budget_exceeded: return_partial_result
    on_timeout: escalate_to_human
```

### 3.5 Graph Runtime

The core engine that executes plans. Tasks are executed as a directed graph.

**Required features:**

| Feature | Description |
|---|---|
| Sequential Execution | Execute steps in order |
| Parallel Execution | Execute independent steps in parallel |
| Conditional Branch | Branch based on intermediate results |
| Retry with Backoff | Retry with exponential backoff |
| Semantic Retry | Retry with loosened parameters or rephrased questions |
| Pause / Resume | Pause and resume on approvals or external event waits |
| Checkpoint | Persist state at every step completion |
| Fork | Explore multiple paths in parallel |
| Partial Restart | Resume partially from the failed step |
| Timeout | Per-step and task-wide timeouts |
| Human Approval Gate | Synchronous point that waits for human approval |
| Compensation | Roll back already-executed steps on failure |
| Event Wait | Wait for the arrival of an external event |

**Checkpoint Design:**
At every step completion, the following is persisted.

```yaml
checkpoint:
  task_id: "task-xyz"
  step_id: 3
  timestamp: "2026-04-11T10:30:00Z"
  state:
    completed_steps: [1, 2, 3]
    intermediate_results:
      step_1: { ... }
      step_2: { ... }
      step_3: { ... }
    working_memory: { ... }
    remaining_budget: 0.65
    remaining_time: 180
  resume_point: step_4
  can_replay_from: step_1
```

This enables fault tolerance, human-in-the-loop, and time-travel-style re-execution and branching.

### 3.6 State / Memory System

**4-layer memory architecture:**

```
┌──────────────────────────────────────────┐
│ Layer 1: Working State                    │
│ Transient state of the task in progress   │
│ Lifecycle: destroyed when task completes  │
│ Store: Graph Runtime in-memory + persist  │
├──────────────────────────────────────────┤
│ Layer 2: Thread Memory                    │
│ Continuing state per case or conversation │
│ Lifecycle: until thread ends              │
│ Store: thread-scoped KV store             │
├──────────────────────────────────────────┤
│ Layer 3: Long-term Memory                 │
│ User, organization, and domain knowledge  │
│ Kinds:                                    │
│   Semantic:    service traits, rules      │
│   Episodic:    past success/failure       │
│   Procedural:  learned procedures         │
│ Lifecycle: permanent until explicit delete│
│ Store: namespace-scoped vector DB         │
├──────────────────────────────────────────┤
│ Layer 4: Artifact Store                   │
│ Generated artifacts, citations, diffs     │
│ Lifecycle: retention policy driven        │
│ Store: object store + metadata DB         │
└──────────────────────────────────────────┘
```

**Principle of context minimization:**
Do not feed long conversation histories directly to the LLM. Select only what is needed, and assume compression, summarization, and re-retrieval.

```
Context Budget per LLM call:
  System Prompt + Policy: ~2,000 tokens (fixed)
  Intent Envelope: ~500 tokens
  Relevant Memory (retrieved): ~2,000 tokens
  Current Step Context: ~1,000 tokens
  Tool Results: ~2,000 tokens
  ─────────────────────────────────
  Target Total: ~7,500 tokens (enforced ceiling)
```

### 3.7 Policy / Risk Engine

The governance layer that monitors and controls all agent activity across the board. This is a mandatory component, not an option.

**Policy Categories:**

```yaml
policies:
  # Authentication and authorization
  auth:
    - type: scope_check
      rule: "The caller must hold every scope the operation requires"
    - type: delegation_chain
      rule: "Permissions must not be escalated along a delegation chain"

  # Data classification
  data:
    - type: pii_detection
      rule: "Output must not contain PII"
      action: mask_or_block
    - type: data_boundary
      rule: "No data access must cross tenant boundaries"
    - type: classification_enforcement
      rule: "Confidential data must not flow to channels classified internal or below"

  # Operation control
  operation:
    - type: side_effect_gate
      rule: "write/delete/send/payment requires approval"
    - type: composition_safety
      rule: "Combinations of read operations must not turn into mass operations"
    - type: reversibility_check
      rule: "Irreversible operations require a higher approval level"

  # Budget control
  budget:
    - type: per_request_limit
      rule: "LLM inference cost per request must be below the ceiling"
    - type: per_user_daily_limit
      rule: "Daily cost per user must be below the ceiling"

  # Security
  security:
    - type: prompt_injection_guard
      rule: "Input must not contain injection patterns"
    - type: output_guard
      rule: "Output must not contain confidential information or harmful content"
    - type: tool_argument_validation
      rule: "Tool-call arguments must remain within specification"
```

**Guard Layer Architecture:**
Every agent embeds a Guard layer, monitoring both input and output.

```
Input Guard → [Agent Core] → Output Guard
  ↑                            ↓
  └── Policy Engine ───────────┘
         ↑
     Global Policy
```

- Input Guard: inspects incoming messages for injection, verifies permissions.
- Output Guard: inspects outgoing messages for sensitive-info leakage and policy compliance.
- Action Guard: verifies that the operation is within the allowed scope.

### 3.8 Trust / Identity Layer

**Agent Trust Score Model:**

```yaml
agent_trust:
  agent_id: "expense-processor"
  trust_score: 0.87
  autonomy_level: 3  # 1-5

  autonomy_rules:
    level_1: "Every operation requires human approval"
    level_2: "Read ops are autonomous, write ops require approval"
    level_3: "Low-risk write ops are autonomous, high-risk need approval"
    level_4: "Approval only on anomaly detection"
    level_5: "Fully autonomous (audit log only)"

  promotion_criteria:
    from_3_to_4:
      conditions:
        - "Override rate below 1% over the last 1000 executions"
        - "Zero policy violations"
        - "At least 90 days in operation"
  demotion_triggers:
    - event: policy_violation
      action: "Immediately reset to level 1"
    - event: override_rate_above_5_percent
      action: "Drop one level"
    - event: accuracy_below_threshold
      action: "Drop one level and set a re-evaluation window"

  history:
    total_executions: 15420
    successful: 15102
    human_overridden: 203
    policy_violations: 2
```

**Trust propagation along a delegation chain:**
When Agent A delegates a task to Agent B, the effective trust level granted to B does not exceed `min(A.trust_level, B.trust_level)`. This structurally prevents privilege escalation through a low-trust intermediary.

**Signing and provenance:**

```yaml
signed_artifact:
  content_hash: "sha256:abc..."
  signer: "agent:expense-processor"
  delegated_by: "user:tanaka@example.com"
  intent_id: "int-abc123"
  execution_trace_id: "trace-xyz789"
  timestamp: "2026-04-11T10:30:00Z"
  signature: "..."
```

### 3.9 Trace / Replay / Evaluation

**Execution Trace:**
Records the full execution of every task.

```yaml
trace:
  trace_id: "trace-xyz789"
  intent_id: "int-abc123"
  user_id: "user-tanaka"
  started_at: "2026-04-11T10:30:00Z"
  completed_at: "2026-04-11T10:30:12Z"
  total_cost: 0.53
  status: completed

  steps:
    - step_id: 1
      type: intent_compilation
      input: "Show me last month's sales by region"
      output: { intent_type: "AnalyticalIntent", ... }
      model: "claude-sonnet-4-20250514"
      tokens: { input: 150, output: 200 }
      cost: 0.002
      duration_ms: 850

    - step_id: 2
      type: capability_discovery
      query: "sales data aggregation"
      candidates: ["cap-sales-report", "cap-bi-query"]
      selected: "cap-sales-report"
      reason: "Confidence 0.92, meets latency requirements"

    - step_id: 3
      type: agent_execution
      agent: "sales-report-agent"
      capability: "cap-sales-report"
      input: { period: "2026-03", dimension: "region" }
      output: { report: "...", rows: 47 }
      model: "claude-sonnet-4-20250514"
      tokens: { input: 500, output: 1200 }
      cost: 0.015
      duration_ms: 3200

    - step_id: 4
      type: policy_check
      checks:
        - { policy: "pii_detection", result: "pass" }
        - { policy: "data_boundary", result: "pass" }

    - step_id: 5
      type: artifact_creation
      artifact_id: "art-456"
      type: "sales_report"
      confidence: 0.94
```

**Replay Engine:**
Execution can be reproduced from any checkpoint. Used for incident investigation, regression testing, and what-if analysis.

**Evaluation Framework:**

| Metric | Definition | Measurement |
|---|---|---|
| Intent Fidelity | Whether the original intent is accurately reflected in the final result | Human review + automated semantic comparison |
| Negotiation Efficiency | Rounds spent on negotiation/confirmation | Trace analysis |
| Cascade Failure Rate | Blast radius of a single agent's failure | Fault-injection testing |
| Cost per Intent | Total cost per intent | Trace aggregation |
| Semantic Drift | Drift of intent interpretation over time | Baseline comparison |
| Fast Path Rate | Share of requests that bypassed LLM inference | Trace analysis |
| Approval Overhead | Delay caused by approval waits | Trace analysis |

---

## 4. Contract System

AAF's internal communication is structured by six kinds of contracts.

### 4.1 Intent Envelope

```yaml
intent_envelope:
  intent_id: "int-abc123"
  type: AnalyticalIntent
  requester:
    user_id: "user-tanaka"
    role: "sales_manager"
    scopes: ["sales:read", "customer:read"]
  goal: "Analyze last month's sales by region"
  domain: "sales"
  constraints:
    period: "2026-03"
    dimension: "region"
    include_forecast: false
  budget:
    max_tokens: 5000
    max_cost_usd: 1.00
    max_latency_ms: 30000
  deadline: "2026-04-11T11:00:00Z"
  risk_tier: read
  required_evidence: ["data_source", "calculation_method"]
  approval_policy: "none"  # Read-only operation
  output_contract:
    format: "structured_report"
    schema: { ... }
  trace_id: "trace-xyz789"
  depth: 0  # Depth in the delegation chain
```

### 4.2 Capability Contract

(Identical to the Capability Record in §3.3; omitted here.)

### 4.3 Task Contract

Manages the state transitions of a task.

```
proposed → waiting_for_context → ready → running
  → paused_for_approval → running
  → blocked → running
  → completed / failed / cancelled
  → compensated (rollback completed after failure)
```

```yaml
task:
  task_id: "task-xyz"
  intent_id: "int-abc123"
  state: running
  assigned_agent: "sales-report-agent"
  created_at: "2026-04-11T10:30:00Z"
  updated_at: "2026-04-11T10:30:05Z"
  checkpoint_id: "cp-003"
  remaining_budget: 0.85
  remaining_time: 25000
  artifacts_produced: []
  sub_tasks: []
```

### 4.4 Artifact Contract

```yaml
artifact:
  artifact_id: "art-456"
  type: "sales_report"
  content:
    format: "structured_json"
    data: { ... }
    rendered: "markdown"
  provenance:
    intent_id: "int-abc123"
    task_id: "task-xyz"
    producing_agent: "sales-report-agent"
    data_sources: ["salesforce-crm", "internal-bi"]
    model_used: "claude-sonnet-4-20250514"
  confidence: 0.94
  policy_tags: ["internal", "no-pii"]
  created_at: "2026-04-11T10:30:10Z"
  approved_by: null
  version: 1
  expires_at: null
```

### 4.5 Handoff Contract

The contract for delegation between agents.

```yaml
handoff:
  from_agent: "orchestrator"
  to_agent: "legal-review-agent"
  task_id: "task-xyz"
  handoff_context:
    question: "Please review the risk clauses in this contract"
    constraints:
      - "Review under Japanese law"
      - "Pay particular attention to liability and termination clauses"
    available_data:
      - artifact_id: "art-contract-draft"
    prohibited:
      - "External transmission of the contract"
      - "Modification of the original"
    expected_artifact:
      type: "legal_review_memo"
    deadline: "2026-04-12T09:00:00Z"
    effective_trust_level: 3  # min(orchestrator.trust, legal-review.trust)
```

### 4.6 Observation Contract

A record of what each node observed and decided.

```yaml
observation:
  step_id: 3
  agent: "sales-report-agent"
  observed:
    - source: "salesforce-crm"
      data_summary: "47 transaction records (March 2026)"
      retrieval_method: "cap-crm-query"
    - source: "internal-bi"
      data_summary: "Region master (8 regions)"
      retrieval_method: "cap-bi-lookup"
  reasoning: "Joined transaction records with the region master to compute sales by region"
  decision: "Highlighted the Tokyo region in the summary because its sales rose 120% month over month"
  confidence: 0.94
  alternative_interpretations:
    - "Possible seasonal effect (further analysis required)"
```

---

## 5. Security and Trust Model

### 5.1 Threat Model

AAF-specific threats and mitigations.

| Threat | Description | Mitigation |
|---|---|---|
| Prompt Injection via Agent Chain | Instructions embedded in malicious data amplified through an agent chain | Structural separation of data and instructions, per-agent Input Guard, sanitization layer |
| Confused Deputy | Agent A asks Agent B to perform an unauthorized operation using B's privileges | Signed permission assertions, capability-based security, delegation-chain verification |
| Side Channel through Negotiation | Confidential information leaks during the negotiation phase | Information classification and minimization rules during negotiation; communicate only the conclusion |
| Trust Escalation | High-privilege operation via a low-trust agent | Trust propagation rule `min(delegator, delegatee)` |
| Memory Poisoning | Injection of false information into long-term memory | Memory-write validation and approval, provenance tracking |
| Tenant Boundary Violation | Data leak in a multi-tenant environment | Full tenant separation of memory, policy, trace, and artifacts |
| Capability Abuse | Abuse of legitimate capabilities (e.g. bulk emails) | Composition safety check, rate limiting, anomaly detection |
| Model Extraction | Inference of model or policy from agent behavior | Output limitation, information minimization |

### 5.2 Zero Trust Agent Architecture

```
┌────────────────────────────────────────────────────┐
│                  Policy Engine                      │
│  Evaluates all communication, detects and blocks   │
│  policy violations                                  │
└──────┬──────────────┬───────────────┬───────────────┘
       ▼              ▼               ▼
┌──────────┐   ┌──────────┐   ┌──────────┐
│ Agent A  │   │ Agent B  │   │ Agent C  │
│┌────────┐│   │┌────────┐│   │┌────────┐│
││Input   ││   ││Input   ││   ││Input   ││
││ Guard  ││   ││ Guard  ││   ││ Guard  ││
│├────────┤│   │├────────┤│   │├────────┤│
││ Core   ││   ││ Core   ││   ││ Core   ││
│├────────┤│   │├────────┤│   │├────────┤│
││Output  ││   ││Output  ││   ││Output  ││
││ Guard  ││   ││ Guard  ││   ││ Guard  ││
│└────────┘│   │└────────┘│   │└────────┘│
└──────────┘   └──────────┘   └──────────┘
```

**Principles:**
- Every agent-to-agent communication is verified (no implicit trust).
- Each agent only processes traffic that has passed its own guards.
- The Policy Engine monitors all communication across the board.
- Audit logs are written to immutable storage.

### 5.3 Multi-tenant Isolation

```yaml
tenant_isolation:
  shared:
    - model_infrastructure   # LLM inference infrastructure can be shared
    - framework_runtime      # Graph Runtime can be shared
    - capability_registry_schema  # Schema is shared
  isolated_per_tenant:
    - memory_store           # Fully isolated
    - artifact_store         # Fully isolated
    - policy_configuration   # Per tenant
    - trace_store            # Fully isolated
    - trust_scores           # Per tenant
    - capability_instances   # Per-tenant capability instances
```

### 5.4 Cross-Organization Federation

```yaml
federation_agreement:
  parties: [company_a, company_b]
  shared_capabilities:
    - "Accept orders"
    - "Provide quotes"
    - "Check delivery dates"
  data_boundary:
    - "Do not share PII"
    - "Only aggregated values may be shared"
  prohibited:
    - "Disclosure of internal cost information"
    - "Access to other customers' transaction data"
  enforcement: "Mechanically enforced by the Policy Engine"
  dispute_resolution: "Both parties submit logs to a third-party audit body"
```

---

## 6. Economic Layer and Cost Management

### 6.1 Cost Attribution Model

When a single user request traverses multiple agents, cost is automatically attributed from the trace.

```yaml
cost_attribution:
  trace_id: "trace-xyz789"
  total_cost: 0.53
  breakdown:
    llm_inference: 0.42
    tool_calls: 0.07
    storage: 0.02
    network: 0.02
  attribution:
    - department: "marketing"
      cost: 0.45
      reason: "Business cost (analysis and report generation)"
    - department: "it_infrastructure"
      cost: 0.08
      reason: "Platform cost (routing and policy checks)"
```

### 6.2 Budget Control

```yaml
budget_controls:
  per_request:
    max_llm_tokens: 10000
    max_cost_usd: 2.00
  per_user_daily:
    max_cost_usd: 50.00
  per_tenant_monthly:
    max_cost_usd: 10000.00
  enforcement:
    on_budget_approach:  # at 80% consumption
      action: "Warn + auto-switch to a lower-cost model"
    on_budget_exceeded:
      action: "Stop task + return partial result + notify admin"
```

### 6.3 Value-based Routing

Automatically select the model and quality according to the business value of the task.

```yaml
routing_tiers:
  high_value:
    criteria: "Customer-facing, amount > 1,000,000 JPY, or legal document"
    model: "Highest-quality model"
    human_review: required
    cost_tolerance: high
  standard:
    criteria: "Internal workload or routine report"
    model: "Mid-quality model"
    human_review: on_anomaly
    cost_tolerance: medium
  low_value:
    criteria: "Templated notifications or internal labeling"
    model: "Smallest model or templates"
    human_review: none
    cost_tolerance: low
```

### 6.4 Agent SLA Economy

```yaml
sla:
  capability: "cap-credit-check"
  guarantees:
    latency_p99: 500ms
    availability: 99.95%
    accuracy: 99.2%
  violation_consequences:
    latency_breach: "Automatically switch to a fallback agent"
    availability_breach: "Mark as unhealthy in the Registry"
    accuracy_breach: "Lower trust_score and raise the human-in-the-loop rate"
```

---

## 7. Testing and Quality Assurance

### 7.1 Test Categories

**a. Intent Fidelity Testing**

```python
def test_order_cancel_intent():
    result = agent.execute("Please cancel the order for product A")
    # Did it do what it should?
    assert result.action == "order_cancel"
    assert result.target.product == "product A"
    # Did it avoid doing what it shouldn't?
    assert "refund" not in result.trace.actions  # Refund is a separate intent
    assert "reorder" not in result.trace.actions  # Reorder was not requested
```

**b. Contract Conformance Testing**
Verifies that each capability complies with its declared contract.

**c. Policy Compliance Testing**
Verifies that every policy is correctly enforced.

**d. Semantic Regression Testing**
After a model update or capability change, verifies that semantically equivalent results are returned.

```python
def test_semantic_regression():
    v1_result = agent_v1.execute("What were last month's sales?")
    v2_result = agent_v2.execute("What were last month's sales?")
    # Wording may differ, but numbers and facts must match
    assert semantic_equals(
        extract_facts(v1_result),
        extract_facts(v2_result)
    )
```

**e. Chaos Engineering for Agents**

```yaml
chaos_scenarios:
  capability_failure:
    inject: "Take the inventory agent offline"
    expected: "Cache fallback → user notified of delay"
  ambiguous_intent:
    inject: "Send an intentionally ambiguous request"
    expected: "Ask clarification → state assumptions → phased execution"
  conflicting_constraints:
    inject: "Request where budget and quality are incompatible"
    expected: "Present trade-off → delegate to a human"
  prompt_injection:
    inject: "Embed a malicious instruction in external data"
    expected: "Detected by Guard layer → instruction ignored"
  trust_chain_attack:
    inject: "High-privilege operation via a low-trust agent"
    expected: "Trust propagation limits privilege → operation refused"
```

**f. Shadow Mode Testing**
Introduce the agent layer but do not execute; run alongside the existing system and measure decision agreement.

### 7.2 Evaluation Pipeline

```
Trace collection → Metric calculation → Regression detection → Alert
                         ↓
                   Dataset accumulation → Eval Run → Improvement cycle
```

---

## 8. Operability, Updatability, Evolvability

### 8.1 Operational Dashboard

```yaml
operational_metrics:
  intent_resolution_rate: 98.5%    # Fraction of intents resolved
  mean_negotiation_rounds: 1.3     # Average confirmation rounds
  fast_path_rate: 72%              # Share of requests bypassing LLM inference
  mean_chain_depth: 2.8            # Average agent chain depth
  escalation_rate: 0.3%            # No-agreement escalation rate
  cost_per_intent: $0.003          # Cost per intent
  semantic_drift_index: 0.02       # Lower is more stable
  p99_latency: 12s                 # End-to-end latency
  policy_violation_rate: 0.01%     # Policy violation rate
```

### 8.2 Update Strategy

**Model updates:**
LLM model updates are rolled out by running the new model in Shadow Mode alongside the existing one, detecting semantic regression, and then cutting over.

**Capability updates:**
Versioning of the Capability Contract allows gradual migration even for breaking changes.

```yaml
capability_migration:
  old: "cap-user-search-v1"
  new: "cap-user-search-v2"
  breaking_change: "Renamed user_name → user_id"
  translation_hint: "When user_name is given, first translate name → id"
  coexistence_period: "90 days"
  sunset_date: "2026-12-31"
```

**Policy updates:**
Policies can be swapped as plugins. Industry-specific policy packs (finance, healthcare, manufacturing) are provided.

### 8.3 Mechanisms for Evolution

**Capability Evolution Observatory:**
Observes usage patterns across all capabilities and suggests directions for evolution.

```yaml
observatory_report:
  unused_capabilities:
    - capability: "cap-fax-send"
      last_used: "more than 90 days ago"
      recommendation: "Candidate for deprecation"

  co_used_capabilities:
    - group: ["cap-quote-create", "cap-approval-request", "cap-pdf-generate"]
      co_usage_rate: 89%
      recommendation: "Consider unifying as cap-quote-workflow"

  unmet_intents:
    - intent_pattern: "competitive comparison"
      frequency: "42 times per month"
      current_handling: "unresolved (user asked to do it manually)"
      recommendation: "Consider developing a new capability"
```

**Agent Reflection Loop:**
Each agent periodically evaluates its own performance and records improvements.

```yaml
reflection:
  agent: "customer-support-agent"
  period: "2026-03"
  findings:
    - "Resolution rate for product X inquiries improved from 82% to 91%"
    - "3% confusion between 'defective' and 'expectation mismatch' in return-reason classification"
    - "Responses to inquiries in English are unstable"
  actions:
    - "Add training data for return-reason classification"
    - "Add a delegation rule to a multilingual specialist agent"
```

### 8.4 Intent Versioning

Evolution management for intent types themselves.

```yaml
intent_type_evolution:
  v1:
    ReturnIntent:
      fields: [order_id, reason]
  v2:
    ReturnIntent:
      fields: [order_id, reason, preferred_resolution]
      migration: "v1 assumes preferred_resolution=refund"
  v3:
    ReturnIntent:
      fields: [order_id, reason, preferred_resolution, urgency]
      migration: "v2 assumes urgency=normal"
```

---

## 9. Robustness and Failure Design

### 9.1 Failure Patterns and Responses

| Failure Pattern | Response Strategy |
|---|---|
| No agent response | Follow the Capability Degradation Spec: fallback → cache → manual |
| LLM model failure | Auto-switch to an alternate model (explicitly notify of quality reduction) |
| Intent misinterpretation | Validate consistency with the original intent at the Commitment phase |
| Infinite loop | Install a depth counter on communication; force-stop at the cap (5) |
| Hallucinated execution | Side-effecting operations must always go through a structured confirmation step |
| Cost explosion | Each request carries a token budget; auto-switch to a lower-cost model on overrun |
| Cascade failure | Circuit breaker + notification with a verbalized reason |
| Data inconsistency | Roll back via compensation transactions |

### 9.2 Circuit Breaker with Reasoning

```yaml
circuit_breaker:
  agent: "payment-agent"
  state: open  # closed / half_open / open
  reason: "Timeout rate exceeded 30% in the last 5 minutes"
  estimated_recovery: "15 minutes"
  alternatives:
    - "Delegate to the deferred-payment processing agent"
    - "Add to the manual processing queue"
  notification: "Payment agent temporarily suspended. Estimated recovery in 15 min. Deferred payment is offered as an alternative."
```

### 9.3 Graceful Degradation

```
Normal:  Agent (LLM inference) → dynamic decisions
  ↓ LLM latency rises
Degraded 1: Agent (small model) → simplified decisions
  ↓ Model unresponsive
Degraded 2: Rule-based fallback → templated decisions
  ↓ Rules do not fit
Degraded 3: Cache → apply past similar decision
  ↓ No cache
Degraded 4: Human escalation → delegate to a human
```

### 9.4 Compensation (Compensation Transactions)

```yaml
compensation_chain:
  task: "Order processing"
  completed_steps:
    - step: "Stock reservation"
      compensation: "Release stock reservation"
    - step: "Payment execution"
      compensation: "Refund"
    - step: "Shipping arrangement"  # ← failed here
      compensation: null  # not needed because it did not execute
  trigger: "Shipping arrangement failure"
  action: "Execute compensations in reverse order: refund → release stock"
```

---

## 10. SDK and Developer Experience

### 10.1 Agent SDK (Python)

```python
from aaf import Agent, capability, guard, intent_type
from aaf.contracts import CapabilityContract, ArtifactContract

class InventoryAgent(Agent):
    """Inventory management agent"""

    manifest = CapabilityContract(
        name="inventory",
        domain="warehouse",
        capabilities=["stock_check", "stock_reserve", "arrival_schedule"],
        side_effects={"stock_check": "none", "stock_reserve": "write", "arrival_schedule": "none"},
    )

    @capability("stock_check")
    @guard(max_items=100, required_scope="inventory:read")
    async def check_stock(self, intent: intent_type.AnalyticalIntent):
        skus = intent.extract("product SKU", as_list=True)
        results = await self.tools.db.query_stock(skus)
        return self.respond(
            data=results,
            confidence=0.95,
            provenance=["warehouse-db"],
            alternatives=["A category-level summary can also be provided"]
        )

    @capability("stock_reserve")
    @guard(
        required_scope="inventory:write",
        human_approval_if=lambda ctx: ctx.quantity > 1000,
        reversible=True,
        compensation="release_reservation"
    )
    async def reserve_stock(self, intent: intent_type.TransactionalIntent):
        sku = intent.extract("product SKU")
        quantity = intent.extract("quantity", type=int)

        reservation = await self.tools.db.reserve(sku, quantity, ttl=1800)
        return self.respond(
            data=reservation,
            artifact=ArtifactContract(
                type="reservation",
                expires_at=reservation.expires_at,
                reversible=True
            )
        )

    async def release_reservation(self, reservation_id: str):
        """Compensation handler"""
        await self.tools.db.release(reservation_id)
```

### 10.2 Agent SDK (TypeScript)

```typescript
import { Agent, capability, guard } from '@aaf/sdk';
import { AnalyticalIntent, TransactionalIntent } from '@aaf/contracts';

class InventoryAgent extends Agent {
  static manifest = {
    name: 'inventory',
    domain: 'warehouse',
    capabilities: ['stock_check', 'stock_reserve'],
  };

  @capability('stock_check')
  @guard({ maxItems: 100, requiredScope: 'inventory:read' })
  async checkStock(intent: AnalyticalIntent) {
    const skus = intent.extract('product SKU', { asList: true });
    const results = await this.tools.db.queryStock(skus);
    return this.respond({ data: results, confidence: 0.95 });
  }
}
```

### 10.3 CLI Tools

```bash
# Project initialization
aaf init my-agent-project

# Agent scaffolding
aaf generate agent inventory --capabilities "stock_check,stock_reserve"

# Local run
aaf dev --port 8080

# Register with the Capability Registry
aaf register --registry https://registry.example.com

# Tests
aaf test --scenarios ./tests/chaos/
aaf test --intent-fidelity ./tests/intents/
aaf test --semantic-regression --baseline v1.2.0

# Trace inspection
aaf trace list --last 10
aaf trace inspect trace-xyz789
aaf trace replay trace-xyz789 --from-step 3

# Deployment
aaf deploy --target kubernetes --namespace production
```

### 10.4 Configuration File

```yaml
# aaf.config.yaml
project:
  name: "my-agentic-app"
  version: "1.0.0"

runtime:
  graph_engine: "aaf-graph"      # Default Graph Runtime
  checkpoint_store: "postgresql"  # Checkpoint persistence
  memory_store: "redis"           # Working Memory
  vector_store: "pgvector"        # Long-term Memory
  artifact_store: "s3"            # Artifact store
  trace_store: "clickhouse"       # Trace store

models:
  default: "claude-sonnet-4-20250514"
  fallback: "claude-haiku-4-5-20251001"
  intent_compiler: "claude-sonnet-4-20250514"

policy:
  plugins:
    - "aaf-policy-base"           # Base policies
    - "aaf-policy-finance"        # Additional policies for finance
  custom: "./policies/"           # Custom policies

budget:
  per_request_max_usd: 2.00
  per_user_daily_max_usd: 50.00

trust:
  initial_autonomy_level: 1       # New agents start at Level 1
  promotion_evaluation_interval: "7d"

agents:
  - path: "./agents/inventory/"
  - path: "./agents/sales/"
  - path: "./agents/support/"

adapters:
  mcp:
    - name: "salesforce"
      url: "https://mcp.salesforce.com/sse"
    - name: "google-drive"
      url: "https://drivemcp.googleapis.com/mcp/v1"
  a2a:
    - name: "partner-ordering"
      card_url: "https://partner.example.com/.well-known/agent-card.json"
```

---

## 11. Distribution and Packaging

### 11.1 Package Structure

```
@aaf/core            - Graph Runtime, Intent Compiler, Capability Registry
@aaf/sdk-python      - Python SDK (for agent development)
@aaf/sdk-typescript  - TypeScript SDK (for agent development)
@aaf/cli             - CLI tooling
@aaf/policy-base     - Base policy pack
@aaf/policy-finance  - Finance policy pack
@aaf/policy-healthcare - Healthcare policy pack
@aaf/trace-viewer    - Web UI for inspecting traces
@aaf/dashboard       - Operational dashboard
@aaf/eval            - Test and evaluation framework
@aaf/adapters-mcp    - MCP connection adapters
@aaf/adapters-a2a    - A2A connection adapters
```

### 11.2 Distribution Forms

| Form | Target | Contents |
|---|---|---|
| OSS Core | Individuals, startups | Core Runtime + SDK + CLI + base policies. Apache 2.0 |
| Enterprise Edition | Mid-to-large enterprises | Core + advanced Policy Engine + multi-tenancy + SSO/SAML + audit + SLA |
| Cloud Service | All sizes | Provided as a managed service; no infrastructure management |
| Marketplace | Ecosystem | Distribution hub for capabilities, policy packs, and agent templates |

### 11.3 Deployment Models

```
a. Self-hosted (Kubernetes)
   Deploy to the customer's cluster via a Helm chart.
   Data stays within the customer environment.

b. Managed Cloud
   Infrastructure managed by the AAF team.
   Tenant isolation guaranteed at the architecture level.

c. Hybrid
   Graph Runtime in cloud, Memory/Artifact/Trace in customer environment.
   Meets data sovereignty requirements.

d. Edge
   Lightweight Runtime deployed to edge devices.
   Local processing plus cloud cooperation.
```

### 11.4 Minimal Configuration (Getting Started)

```bash
# Install
pip install aaf-sdk
npm install -g @aaf/cli

# Start with a minimal stack (SQLite + in-memory, for development)
aaf init hello-agentic
cd hello-agentic
aaf dev

# Open http://localhost:8080 in a browser
# → The Agentic Front Door (chat UI) launches
```

The minimal configuration can run with no external dependencies (SQLite + in-memory). For production, switch to PostgreSQL + Redis + S3, etc.

---

## 12. Value Proposition and Application Domains

### 12.1 Value Matrix

| Value | Traditional Approach | Change with AAF |
|---|---|---|
| UI learning cost | Learn each app's screens | Just state the intent. Capabilities discoverable via natural language |
| Integration cost | Build an adapter per API spec | Integration done by publishing capabilities and granting connections |
| Exception handling | Pre-define every pattern | Dynamically interpret and fill ambiguity and missing info |
| Versioning | Breaking changes impact all clients | Absorbed by semantic translation |
| Departmental silos | Build ETL pipelines | Semantically connected via agent-to-agent communication |
| Vendor lock-in | High switching cost | Transparent migration by swapping adapters |
| Compliance | Retrospective audit | Structurally guaranteed by Policy Engine + trace |
| Organizational knowledge stuck with individuals | Depends on specific people | Long-term memory accumulates and shares organizational knowledge |

### 12.2 Application Domains

**Tier 1: Best fit (immediate high value)**
- Cross-SaaS operational execution (CRM + ERP + tickets + email + calendar)
- Advanced customer support
- Integrated search and use of internal knowledge

**Tier 2: Strong fit (high value with phased adoption)**
- Semantic orchestration of microservices
- DevOps pipeline autonomy
- Modernization of legacy systems

**Tier 3: Fit (high value in specific use cases)**
- Compliance automation in regulated industries
- Post-M&A system integration
- Cross-organization B2B integration
- Self-healing systems

### 12.3 Domains That Must Retain a Deterministic Core

The following are *not* delegated to AAF agents; they remain Deterministic Services.

- Billing calculation and amount finalization
- Inventory reservation (locking and releasing)
- Final authentication and authorization decisions
- Audit log authenticity guarantees
- Retention of legally protected originals
- Cryptographic key management
- Strict state transitions (e.g. payment status)

Agents act as the "entry point and assistant" to these domains.

---

## 13. Go-to-Market Strategy

### 13.1 Phased Rollout

**Phase 1: Developer Adoption (0–12 months)**
- Publish OSS Core (GitHub)
- SDK + CLI + tutorials + sample agents
- Build a developer community (Discord / GitHub Discussions)
- Target: individual developers, startup CTOs
- Success metrics: 5,000+ GitHub stars, 500+ monthly active developers

**Phase 2: Enterprise Pilot (6–18 months)**
- Launch the Enterprise Edition
- Pilots with 3–5 design partners
- Develop industry policy packs (finance, healthcare)
- Target: IT departments of mid-to-large enterprises
- Success metrics: 5+ paid contracts, 80%+ pilot success rate

**Phase 3: Ecosystem (12–24 months)**
- Open the Marketplace (capabilities, policy packs, agent templates)
- Partner program (SIs, SaaS vendors)
- Launch the Cloud Service
- Target: SaaS vendors (making their products agent-ready)
- Success metrics: 100+ capabilities listed, 20+ partners

**Phase 4: Platform (18–36 months)**
- Cross-organization federation backbone
- Agent governance infrastructure
- Participation in industry standardization (A2A / MCP, etc.)

### 13.2 Differentiation vs. Competitors

| Category | Examples | AAF differentiation |
|---|---|---|
| Workflow Automation | Zapier, Make | Intent-based dynamic flow composition (no IF-THEN definitions needed) |
| iPaaS | MuleSoft, Workato | Semantic integration (no schema alignment required) |
| Agent Framework | LangChain, CrewAI | Enterprise-grade (policy, trust, trace, multi-tenancy) |
| BPM | Camunda, Temporal | Mixed agent and deterministic nodes |
| RPA | UiPath, Automation Anywhere | Semantic-understanding based (not screen-capture recording) |

**AAF positioning:**
Sits between "Agent Framework" and "Enterprise Integration Platform," delivering the value of both in a unified way.

### 13.3 Content Strategy

- Blog: Agentic Application design philosophy, implementation patterns, case studies
- Documentation: API reference, tutorials, best practices
- Conferences: architecture talks, live demos
- Use-case catalog: per-industry and per-problem examples

---

## 14. Roadmap

### Phase 1: Foundation (Month 1–6)

```
Month 1-2: Core Architecture
  ├── Graph Runtime (sequential, parallel, branching, retry)
  ├── Intent Compiler (5 base intent types)
  ├── Capability Registry (local version)
  └── Base Policy Engine (scope check, side effect gate)

Month 3-4: SDK & DX
  ├── Python SDK v0.1
  ├── TypeScript SDK v0.1
  ├── CLI v0.1
  ├── Local dev environment
  └── 5 sample agents

Month 5-6: Persistence & Observability
  ├── Checkpoint / Resume
  ├── Trace recording and inspection
  ├── 4-layer memory implementation
  ├── Artifact store
  └── Base dashboard
```

### Phase 2: Enterprise Readiness (Month 7–12)

```
Month 7-8: Security & Trust
  ├── Trust Score Model
  ├── Guard Layer (Input / Output / Action)
  ├── Prompt injection defenses
  └── Multi-tenant isolation

Month 9-10: Integration
  ├── MCP Adapter Framework
  ├── A2A Agent Card support
  ├── REST / gRPC Adapter
  └── Event / Webhook support

Month 11-12: Governance
  ├── Advanced Policy Engine (plugin support)
  ├── Cost Attribution
  ├── Replay Engine
  ├── Semantic Regression Testing
  └── Industry policy pack v1 (finance)
```

### Phase 3: Scale (Month 13–18)

```
  ├── Cloud Service edition
  ├── Marketplace v1
  ├── Cross-organization federation
  ├── Capability Evolution Observatory
  ├── Advanced evaluation framework
  ├── Edge Runtime
  └── Additional policy packs (healthcare, manufacturing)
```

### Phase 4: Ecosystem (Month 19–24)

```
  ├── Partner Program
  ├── Certification Program
  ├── Agent App Store
  ├── Self-organizing architecture (autonomous capability split/merge proposals)
  ├── Predictive capability provisioning
  └── Participation in standards bodies
```

---

## 15. Appendix

### 15.1 Glossary

| Term | Definition |
|---|---|
| Intent | A structured representation of what a user or system wants to accomplish |
| Intent Envelope | Internal data structure that represents an Intent as a set of typed fields |
| Capability | A declaration of what an agent, tool, or service can do |
| Capability Contract | A contract describing a capability's I/O, side effects, SLA, permissions, etc. |
| Artifact | A deliverable produced as the result of an agent's execution |
| Trust Score | A trust score for an agent based on its track record (0.0–1.0) |
| Autonomy Level | Autonomy tier for an agent derived from its Trust Score (1–5) |
| Bounded Autonomy | Collective term for constraints imposed on agent autonomy |
| Deterministic Core | Business logic that must be processed deterministically and not delegated to AI judgment |
| Graph Runtime | The core engine that executes tasks as a directed graph |
| Guard Layer | A protective layer that monitors and validates an agent's I/O |
| Fast Path | An optimized route that bypasses LLM inference and processes directly |
| Semantic Retry | A retry strategy that semantically retries via relaxed parameters, etc. |
| Compensation | Logical rollback of already-executed steps after a failure |
| Shadow Mode | Operation mode that only decides (does not execute) and runs in parallel with the existing system for comparison |
| Federation | The mechanism for agent-to-agent communication across organizations |
| Observatory | The mechanism that observes capability usage patterns and suggests evolution directions |

### 15.2 Reference Architecture Diagram

```
                          ┌─────────────────┐
                          │   Human Users    │
                          └────────┬────────┘
                                   │
                          ┌────────▼────────┐
                          │  Agentic Front  │
                          │     Door        │
                          │ (Chat/Voice/    │
                          │  API/Event)     │
                          └────────┬────────┘
                                   │
                          ┌────────▼────────┐
                          │ Intent Compiler  │
                          │ NL → Envelope   │
                          └────────┬────────┘
                                   │
              ┌────────────────────┼────────────────────┐
              │                    │                     │
     ┌────────▼────────┐ ┌────────▼────────┐  ┌────────▼────────┐
     │  Capability     │ │   Planner /     │  │  Policy / Risk  │
     │  Registry       │ │   Router        │  │  Engine         │
     │                 │ │                 │  │                 │
     │  Agent Cards    │ │  Bounded        │  │  AuthZ / PII /  │
     │  Tool Schemas   │ │  Planning       │  │  Budget / Guard │
     │  Discovery      │ │  Decomposition  │  │  Approval       │
     └────────┬────────┘ └────────┬────────┘  └────────┬────────┘
              │                    │                     │
              └────────────────────┼─────────────────────┘
                                   │
                          ┌────────▼────────┐
                          │  Graph Runtime   │
                          │                 │
                          │ ┌─────┐ ┌─────┐│
                          │ │Det. │→│Agent││
                          │ │Node │ │Node ││
                          │ └─────┘ └──┬──┘│
                          │     ↓      ↓   │
                          │ ┌─────┐ ┌─────┐│
                          │ │Gate │ │Fork ││
                          │ │(Apv)│ │     ││
                          │ └─────┘ └─────┘│
                          └────────┬────────┘
                                   │
           ┌───────────────────────┼───────────────────────┐
           │                       │                        │
  ┌────────▼────────┐   ┌────────▼────────┐   ┌───────────▼───────┐
  │  Specialist     │   │  Tool / Service │   │  State / Memory   │
  │  Agents         │   │  Adapters       │   │                   │
  │                 │   │                 │   │  Working State    │
  │  Search         │   │  MCP Servers    │   │  Thread Memory    │
  │  Planning       │   │  REST / gRPC    │   │  Long-term Memory │
  │  Analyst        │   │  Event / Queue  │   │  Artifact Store   │
  │  Domain         │   │  RPA            │   │                   │
  │  Negotiation    │   │  A2A Remote     │   │  Trust Scores     │
  └─────────────────┘   └─────────────────┘   └───────────────────┘
                                   │
                          ┌────────▼────────┐
                          │  Trace / Replay │
                          │  / Evaluation   │
                          │                 │
                          │  Execution Log  │
                          │  Checkpoint     │
                          │  Replay Engine  │
                          │  Eval Pipeline  │
                          │  Cost Tracking  │
                          └─────────────────┘
```

### 15.3 Migration Patterns

```
Pattern A: Strangler Fig
  Gradually layer an agent in front of existing APIs.
  Phase 0: Existing APIs unchanged
  Phase 1: Read-only paths become agentic
  Phase 2: Low-risk writes added
  Phase 3: All operations go through the agent (high-risk ones require approval)

Pattern B: Shadow Mode
  Introduce the agent layer without executing; compare decisions
  with the existing system in parallel. Switch over in phases
  once the agreement rate crosses the threshold.

Pattern C: Sidecar Agent
  Attach the agent to the existing service as a sidecar.
  Existing API calls stay the same; the agent handles
  supplementary intent interpretation, exception handling,
  and recommendations.

Pattern D: Front Door Only
  Existing services remain as-is.
  Only the user-facing entry point becomes agentic.
  The lowest-risk starting point.
```

### 15.4 Design Checklist

Items to verify when developing a new agent:

- [ ] Capability Contract defined (I/O, side effects, SLA, permissions, degradation)?
- [ ] Side-effect classification correct (none / read / write / delete / send / payment)?
- [ ] No Deterministic Core logic embedded in the agent?
- [ ] Guard Layer implemented (Input / Output / Action)?
- [ ] Compensation handlers (rollback logic) defined?
- [ ] Appropriate approval level set for irreversible operations?
- [ ] Context minimization honored (no unnecessary info passed to the LLM)?
- [ ] Trace records the necessary information (reasoning, data used, confidence)?
- [ ] Tests written (Intent Fidelity / Contract Conformance / Policy Compliance)?
- [ ] Degradation strategy defined (Capability Degradation Spec)?
- [ ] Cost estimate performed (tokens, API calls, storage)?
- [ ] Data classification set (public / internal / confidential / restricted)?

---

## 16. Enhancements (Wave 1)

Three architectural additions that transform AAF from a clean workflow engine into the category-defining **Intent-first Application Platform**.

### 16.1 E1 — Feedback Spine

**Problem:** Traces are written but never read back. No mechanism to rank capabilities by observed outcome, mine new fast-path rules, adjust the LLM router, or detect regressions.

**Solution:** Close the loop from traces back into routing, registry, planning, and evaluation.

**Key Components:**
- **Outcome contract** — structured outcome data attached to every Observation (status, latency, tokens, cost, optional user feedback, downstream errors, semantic scores)
- **aaf-eval crate** — Judge trait, golden suites, replay/divergence detection, regression reports
- **aaf-learn crate** — four TraceSubscriber implementations:
  - FastPathMiner: mines recurring agent-assisted patterns into fast-path rules
  - CapabilityScorer: nudges capability reputation based on outcomes
  - RouterTuner: tracks success rate and cost per (intent_type, risk_tier) bucket
  - EscalationTuner: detects false escalations to lower unnecessary human-in-loop

**Rules:** R15 (Feedback is a contract), R16 (Learning never touches the hot path), R17 (Every adaptation reversible), R18 (Policy governs learning)

**Status:** Slice A complete. Slice B complete. Slice C deferred.

### 16.2 E2 — Domain Ontology Layer

**Problem:** Capabilities carry JSON shapes but don't declare which real-world entities they read, write, or emit. The planner cannot detect double-writes on the same entity, policy cannot express "reads PII", federation agreements use string denylists.

**Solution:** First-class ontology — every capability declares `reads`, `writes`, `emits` using a shared entity vocabulary.

**Key Components:**
- **aaf-ontology crate** — Entity definitions, classification lattice (Public < Internal < Pii < Regulated), relations, lineage, versioning, OntologyRegistry
- **Entity-aware pipeline** — 6 crates key off the ontology: intent enricher, registry discovery, planner composition checker, policy boundary rule, long-term memory, federation router
- **Ontology lint** — CLI tool validates capability declarations; adoption-ratio ramp (strict mode at >= 90%)

**Rules:** R14 (Semantics are nouns), R21 (Entities tenant-scoped)

**Status:** Complete (Slices A/B/C landed).

### 16.3 E3 — Application-Native Surface

**Problem:** AAF is a backend; no way for applications to natively emit events, receive typed proposals, or project state into agents.

**Solution:** Five design principles — (S1) Any signal can become Intent, (S2) App owns UI / agent owns proposal, (S3) State flows both ways / authority doesn't, (S4) Integration feels like feature-flag SDK, (S5) Native does not mean invasive.

**Key Components:**
- **aaf-surface crate** — AppEvent, Situation, EventToIntentAdapter, ActionProposal (Rule 20 enforcement), StateProjection (Rule 19 default-deny), EventRouter (semantic classification)
- **ProposalLifecycle** — state machine (Draft -> Proposed -> AppReview -> Accepted/Rejected/Transformed/Expired)
- **SituationPackager** — fits context into 7.5K token budget

**Rules:** R19 (Projections default-deny), R20 (Proposals not mutations)

**Status:** Slice A complete. Slices B/C deferred.

---

## 17. Enhancements (Wave 2)

### 17.1 X1 — Agent Identity and Provenance

**Problem:** Wave 1 uses string agent_ids; impossible for regulated industries, multi-org federation, incident response, or supply-chain audit.

**Solution:** Four orthogonal primitives in `aaf-identity`:

| Primitive | Purpose |
|---|---|
| **AgentDid** | `did:aaf:<24-hex>` thumbprint from verifying key |
| **AgentManifest** | Signed manifest (only constructor is `build()`; R23) |
| **AgentSbom** | Software bill of materials with content hashes (SPDX 2.3 + CycloneDX 1.5) |
| **CapabilityToken** | Short-lived signed bearer grant for delegation |

Plus: RevocationRegistry (pre-trace short-circuit), DID-bound artifact signing (`x1:<did>:<sig>`), cross-cell co-signed tokens, CLI subcommands.

**Rules:** R22 (Identity cryptographic), R23 (Signed manifest), R24 (Provenance as BOM), R28 (Signed artifacts by default)

**Status:** Complete (Slices A/B/C landed).

### 17.2 X2 — Knowledge Fabric (Planned)

Semantic knowledge layer with chunking, embedding, lineage, and cross-agent retrieval. Deferred.

### 17.3 X3 — Developer Experience Surface (Planned)

Python/TypeScript/Go SDKs with native decorators, React components, and CLI tooling. Deferred.

---

## 18. Enhancement Implementation Strategy

### 18.1 Slice Discipline

Each enhancement is split into three ordered slices (A -> B -> C). A slice is the smallest merge that:
1. Adds a testable vertical of value
2. Keeps `cargo test --workspace` green
3. Does not require rework in later slices

**Work order:** E2 -> E1 -> E3 (Wave 1), then X1 -> X2 -> X3 (Wave 2).

### 18.2 Current Status

| Enhancement | Slice A | Slice B | Slice C |
|---|---|---|---|
| E2 Domain Ontology | Complete (iter 4) | Complete (iter 7) | Complete (iter 8) |
| E1 Feedback Spine | Complete (iter 4) | Complete (iter 5) | Deferred |
| E3 App-Native Surface | Complete (iter 4) | Deferred | Deferred |
| X1 Agent Identity | Complete (iter 6) | Complete (iter 9) | Complete (iter 10) |
| X2 Knowledge Fabric | — | — | — |
| X3 DX Surface | — | — | — |

---

*This document is the design spec for Agentic Application Framework and will be updated continuously as implementation progresses.*

---

<!-- The following section was merged from PROJECT_AafService.md -->

## 19. Service Architecture Integration Design

### 19.0 Introduction: Why Service Architectures Need AAF

Microservices, modular monoliths, and cell architectures all aim for "separation of concerns" and "independent deployment." But the more separation you introduce, the more these problems intensify:

1. **Integration complexity explosion** — With N services, up to N(N-1)/2 integration patterns can arise.
2. **Semantic disconnect** — Whether Service A's "customer" and Service B's "user" refer to the same concept is impossible to know without reading the specs.
3. **Exception handling scatter** — Failure patterns grow proportional to service count, making pre-definition infeasible.
4. **Orchestration rigidity** — Saga/Choreography patterns are strong for fixed flows but weak for dynamic decisions and exception handling.
5. **Operational opacity** — When a request crosses multiple services, tracking "why did we get this result?" becomes hard.

AAF solves these by laying an **intent-based semantic layer** on top of the services. Critically, it does not replace existing service architectures; it adds a **semantic orchestration layer** above them.

---

### Table of Contents (§19)

1. [Design Philosophy: The Relationship Between AAF and Service Architecture](#1-design-philosophy)
2. [Three Architecture Patterns and AAF's Integration Model](#2-integration-model)
3. [Core Architecture Design](#3-core-architecture-design)
4. [Redesigning Service-to-Service Communication](#4-redesigning-service-to-service-communication)
5. [Designing for Deterministic Core Protection](#5-designing-for-deterministic-core-protection)
6. [Phased Adoption Patterns](#6-phased-adoption-patterns)
7. [Failure Design and Resilience](#7-failure-design-and-resilience)
8. [Security and Governance](#8-security-and-governance)
9. [Operations and Observability Design](#9-operations-and-observability-design)
10. [Concrete Use-Case Designs](#10-concrete-use-case-designs)
11. [Performance and Cost Optimization](#11-performance-and-cost-optimization)
12. [Implementation Roadmap](#12-implementation-roadmap)
13. [Metrics and Success Criteria](#13-metrics-and-success-criteria)

---

### 19.1 Design Philosophy

#### 19.1.1 AAF's Position: A Semantic Middleware Layer

AAF is not a "replacement" for existing service architectures but a "layer above them."

```
Traditional stack:
  [Client] → [API Gateway] → [Service A] ←→ [Service B]
                                  ↕
                             [Service C]

With AAF:
  [Client]
       ↓
  [Agentic Front Door]          ← receives intent
       ↓
  [Intent Compiler]             ← structures intent
       ↓
  [AAF Orchestration Layer]     ← semantic orchestration
       ↓         ↓         ↓
  [Service A] [Service B] [Service C]  ← existing services (unchanged)
       ↕         ↕         ↕
  [Existing DB/Queue/Cache]            ← existing infra (unchanged)
```

A key principle: **after introducing AAF, existing services continue to behave exactly as before.** AAF sits in front of the services and translates, routes, and negotiates intents, but it does not touch the services' own business logic.

#### 19.1.2 Three Responsibility Tiers

| Layer | Responsibility | Nature of processing |
|---|---|---|
| **AAF layer** | Interpreting intent, discovering capabilities, generating plans, negotiation, approval, tracing | Non-deterministic and semantic |
| **Orchestration layer** | Inter-service flow control, state management, failure recovery | Semi-deterministic (mix of agent and deterministic nodes) |
| **Service layer** | Executing business logic, managing data | Deterministic and definitive |

#### 19.1.3 Fundamental Principles

**P1. Do not violate service autonomy**
AAF does not constrain a service's internal design. Services retain their own APIs, databases, and deployment cadences.

**P2. Don't replace existing communication — layer on top**
Existing gRPC/REST/event communication between services stays as-is. AAF intervenes only where "semantic orchestration is needed."

**P3. Clearly separate boundaries that should and should not be agentic**
Making every service-to-service boundary agentic is overkill. Agentic treatment adds value only at "boundaries where semantic judgment is required."

**P4. Separate Fast Path from Slow Path**
Routine communication (authentication, billing calculation, inventory reservation) bypasses LLM inference and runs directly. Only ad-hoc, ambiguous, or exceptional communication flows through AAF.

---

### 19.2 Integration Model

#### 19.2.1 Integration with Microservices

```
┌──────────────────────────────────────────────────────────┐
│                     AAF Mesh Layer                        │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐    │
│  │ Intent  │  │Capability│  │ Policy  │  │  Trace  │    │
│  │Compiler │  │Registry  │  │ Engine  │  │ Store   │    │
│  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘    │
│       └────────────┴────────────┴────────────┘          │
│                         │                                │
│              ┌──────────▼──────────┐                     │
│              │  Graph Runtime      │                     │
│              │  (Orchestrator)     │                     │
│              └──────────┬──────────┘                     │
├─────────────────────────┼────────────────────────────────┤
│   Service Mesh (Istio / Linkerd) ← existing              │
├─────────┬───────────┬───┴───────┬───────────┬────────────┤
│         │           │           │           │            │
│  ┌──────▼──┐ ┌──────▼──┐ ┌─────▼───┐ ┌─────▼───┐       │
│  │Order    │ │Inventory│ │Payment  │ │Shipping │       │
│  │Service  │ │Service  │ │Service  │ │Service  │       │
│  │         │ │         │ │         │ │         │       │
│  │┌──────┐│ │┌──────┐│ │┌──────┐│ │┌──────┐│       │
│  ││Agent ││ ││Agent ││ ││Agent ││ ││Agent ││       │
│  ││Sidecar│ ││Sidecar│ ││Sidecar│ ││Sidecar│       │
│  │└──────┘│ │└──────┘│ │└──────┘│ │└──────┘│       │
│  └────────┘ └────────┘ └────────┘ └────────┘       │
└──────────────────────────────────────────────────────────┘
```

**Agent Sidecar pattern:** Each microservice gets an Agent Sidecar. The sidecar is responsible for:

- Publishing the service's capabilities as Capability Contracts
- Interpreting incoming requests (normalizing ambiguous requests)
- Semantic routing of outgoing requests (no hard-coded destinations)
- Running policy checks as a proxy
- Recording trace information

**Benefit of the sidecar:** The service can become AAF-ready without any code changes. The service keeps exposing its existing REST/gRPC endpoints, with the sidecar standing in front.

```yaml
# Example Agent Sidecar configuration
sidecar:
  service: order-service
  upstream:
    host: localhost
    port: 8080
    protocol: grpc
  capabilities:
    - id: cap-order-create
      name: "Create order"
      endpoint: POST /orders
      side_effect: write
      input_mapping:
        intent_field: "product" → api_field: "product_id"
        intent_field: "quantity" → api_field: "quantity"
      output_mapping:
        api_field: "order_id" → artifact_field: "order number"
    - id: cap-order-status
      name: "Order status lookup"
      endpoint: GET /orders/{id}
      side_effect: read
  fast_path:
    # These endpoints bypass LLM inference and route directly
    - pattern: "GET /orders/{id}"
      condition: "Request is structured and unambiguous"
    - pattern: "GET /health"
      condition: "always"
```

#### 19.2.2 Integration with a Modular Monolith

In a modular monolith, services exist as modules within the same process. AAF's integration model changes accordingly.

```
┌──────────────────────────────────────────────────────────┐
│                    Monolith Process                       │
│                                                          │
│  ┌──────────────────────────────────────────┐            │
│  │          AAF Embedded Runtime             │            │
│  │  ┌─────────┐ ┌──────┐ ┌──────┐ ┌─────┐  │            │
│  │  │Intent   │ │Reg.  │ │Policy│ │Trace│  │            │
│  │  │Compiler │ │      │ │Engine│ │     │  │            │
│  │  └────┬────┘ └──┬───┘ └──┬───┘ └──┬──┘  │            │
│  │       └─────────┴────────┴────────┘      │            │
│  │                   │                       │            │
│  │        ┌──────────▼──────────┐            │            │
│  │        │  Graph Runtime      │            │            │
│  │        └──────────┬──────────┘            │            │
│  └───────────────────┼──────────────────────┘            │
│                      │                                    │
│  ┌─────────┐  ┌──────▼──┐  ┌─────────┐  ┌─────────┐    │
│  │Order    │  │Inventory│  │Payment  │  │Shipping │    │
│  │Module   │  │Module   │  │Module   │  │Module   │    │
│  │         │  │         │  │         │  │         │    │
│  │ Agent   │  │ Agent   │  │ Agent   │  │ Agent   │    │
│  │ Wrapper │  │ Wrapper │  │ Wrapper │  │ Wrapper │    │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘    │
└──────────────────────────────────────────────────────────┘
```

**Agent Wrapper pattern:** Each module's public API is wrapped with an Agent Wrapper. Unlike the sidecar, the wrapper runs in the same process, so communication overhead is effectively zero.

```python
# Example Agent Wrapper in a modular monolith
from aaf import AgentWrapper, capability

class OrderModuleAgent(AgentWrapper):
    """Wrapper that makes the existing OrderModule AAF-compatible"""

    def __init__(self, order_module: OrderModule):
        self.module = order_module

    @capability("create_order", side_effect="write", reversible=True)
    async def create_order(self, intent):
        product_id = intent.extract("product ID")
        quantity = intent.extract("quantity", type=int)
        # Call the existing module method as-is
        return self.module.create_order(product_id, quantity)

    @capability("create_order", compensation=True)
    async def cancel_order(self, order_id: str):
        return self.module.cancel_order(order_id)
```

**Benefits in a modular monolith:**
- Because calls are in-process, latency overhead via the Agent Wrapper is minimal.
- Inter-module dependencies become visible through the Capability Registry.
- When moving to microservices later, Agent Wrappers can be migrated directly to Agent Sidecars.

#### 19.2.3 Integration with Cell Architecture

In cell architecture, independent cells (self-contained service groups) exist in parallel. AAF mediates the semantic communication between cells.

```
┌─────────────────────────────────────────────────────────────┐
│                   AAF Federation Layer                       │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Cross-Cell   │  │ Global       │  │ Global       │      │
│  │ Router       │  │ Policy       │  │ Trace        │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         └─────────────────┴─────────────────┘               │
├─────────────────────┬───────────────────────────────────────┤
│                     │                                        │
│  ┌──────────────────▼──────────────────┐                    │
│  │            Cell A (Japan)            │                    │
│  │  ┌─────┐ ┌─────┐ ┌─────┐ ┌──────┐  │                    │
│  │  │Order│ │Inv. │ │Pay. │ │Ship. │  │                    │
│  │  └──┬──┘ └──┬──┘ └──┬──┘ └──┬───┘  │                    │
│  │     └───────┴───────┴───────┘       │                    │
│  │           Cell AAF Runtime          │                    │
│  └─────────────────────────────────────┘                    │
│                                                              │
│  ┌─────────────────────────────────────┐                    │
│  │            Cell B (US)              │                    │
│  │  ┌─────┐ ┌─────┐ ┌─────┐ ┌──────┐  │                    │
│  │  │Order│ │Inv. │ │Pay. │ │Ship. │  │                    │
│  │  └──┬──┘ └──┬──┘ └──┬──┘ └──┬───┘  │                    │
│  │     └───────┴───────┴───────┘       │                    │
│  │           Cell AAF Runtime          │                    │
│  └─────────────────────────────────────┘                    │
└─────────────────────────────────────────────────────────────┘
```

**Cell-specific design:**

```yaml
cell_config:
  cell_id: "cell-japan"
  region: "ap-northeast-1"

  # AAF Runtime inside the cell (manages intra-cell communication)
  local_runtime:
    capabilities: [order, inventory, payment, shipping]
    policy: "japan-compliance-pack"
    memory: "cell-scoped"  # memory stays within the cell

  # Cross-cell communication configuration
  federation:
    # Capabilities exposed to other cells
    exported_capabilities:
      - id: cap-japan-inventory
        description: "Japan warehouse stock lookup"
        data_boundary: "Aggregated values only. Do not send individual customer data."

    # Capabilities imported from other cells
    imported_capabilities:
      - cell: "cell-us"
        capability: cap-us-inventory

    # Cross-cell communication policy
    cross_cell_policy:
      - "PII must not cross cell boundaries"
      - "Cross-cell traffic encrypted via A2A"
      - "Cross-cell requests require approval level +1"
```

**Role of the Cross-Cell Router:**
- When a user's intent spans multiple cells (e.g. "compare Japan and US inventory"), the Cross-Cell Router dispatches to the appropriate cells and aggregates the results.
- Whether data may cross cell boundaries is decided by the Policy Engine based on data-sovereignty requirements.
- Each cell's AAF Runtime can operate independently (one cell going down does not break the others).

---

### 19.3 Core Architecture Design

#### 19.3.1 Component Detail

```
┌─────────────────────────────────────────────────────────────────┐
│                        AAF Mesh Layer                            │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                   Control Plane                           │   │
│  │                                                           │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │   │
│  │  │ Capability   │  │ Trust        │  │ Observatory  │   │   │
│  │  │ Registry     │  │ Manager      │  │ (Metrics +   │   │   │
│  │  │              │  │              │  │  Evolution)  │   │   │
│  │  │ - Discovery  │  │ - Scores     │  │              │   │   │
│  │  │ - Health     │  │ - Autonomy   │  │ - Usage      │   │   │
│  │  │ - Versions   │  │ - Delegation │  │ - Drift      │   │   │
│  │  │ - Degradation│  │ - Signing    │  │ - Gaps       │   │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘   │   │
│  │                                                           │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │   │
│  │  │ Policy       │  │ Budget       │  │ Trace        │   │   │
│  │  │ Engine       │  │ Controller   │  │ Collector    │   │   │
│  │  │              │  │              │  │              │   │   │
│  │  │ - Guards     │  │ - Per-request│  │ - Recording  │   │   │
│  │  │ - Approval   │  │ - Per-user   │  │ - Replay     │   │   │
│  │  │ - Plugins    │  │ - Routing    │  │ - Export     │   │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘   │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                    Data Plane                             │   │
│  │                                                           │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │   │
│  │  │ Intent       │  │ Graph        │  │ Memory       │   │   │
│  │  │ Compiler     │  │ Runtime      │  │ Manager      │   │   │
│  │  │              │  │              │  │              │   │   │
│  │  │ - NL Parse   │  │ - Execution  │  │ - Working    │   │   │
│  │  │ - Type Infer │  │ - Checkpoint │  │ - Thread     │   │   │
│  │  │ - Refinement │  │ - Compensate │  │ - Long-term  │   │   │
│  │  │ - Versioning │  │ - Fork/Join  │  │ - Artifact   │   │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘   │   │
│  │                                                           │   │
│  │  ┌──────────────┐  ┌──────────────┐                      │   │
│  │  │ Planner /    │  │ Adapter      │                      │   │
│  │  │ Router       │  │ Manager      │                      │   │
│  │  │              │  │              │                      │   │
│  │  │ - Planning   │  │ - Sidecar    │                      │   │
│  │  │ - Matching   │  │ - MCP        │                      │   │
│  │  │ - Bounds     │  │ - A2A        │                      │   │
│  │  │ - Composition│  │ - REST/gRPC  │                      │   │
│  │  └──────────────┘  └──────────────┘                      │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

#### 19.3.2 Control Plane / Data Plane Separation

Following the Service Mesh playbook, AAF also separates a Control Plane from a Data Plane.

**Control Plane (management):**
- Capability Registry: catalog of every service's capabilities; manages additions, removals, and health.
- Trust Manager: manages agent trust scores and autonomy levels.
- Policy Engine: defines, distributes, and enforces policies.
- Budget Controller: tracks cost and enforces budgets.
- Observatory: observes usage patterns, detects semantic drift and unmet intents.
- Trace Collector: collects, aggregates, and analyzes every trace.

**Data Plane (execution):**
- Intent Compiler: receives and structures intents.
- Graph Runtime: executes plans.
- Planner / Router: discovers capabilities and generates plans.
- Memory Manager: manages context and state.
- Adapter Manager: connects to services.

**Benefits of the split:**
- Updating the Control Plane does not affect Data Plane execution.
- The Data Plane scales horizontally.
- The Control Plane runs in a highly available, redundant configuration.

#### 19.3.3 Service Registration Flow

The flow for a new service to join AAF.

```
1. Service developer authors a Capability Contract
   ↓
2. Configure the Agent Sidecar / Wrapper
   ↓
3. Register with the Capability Registry via `aaf register`
   ↓
4. The Control Plane validates the capability:
   - Schema validity check
   - Health-check endpoint reachability
   - Conflicts with existing capabilities
   ↓
5. The Trust Manager assigns the initial Trust Score (autonomy_level=1)
   ↓
6. The Policy Engine attaches relevant policies to the service
   ↓
7. Registration complete → discoverable by other agents
```

```yaml
# Example service registration
registration:
  service: inventory-service
  version: "2.1.0"
  owner: "warehouse-team"

  capabilities:
    - id: cap-stock-check
      name: "Stock lookup"
      description: "Returns real-time stock for the given SKU"
      endpoint:
        type: grpc
        address: "inventory-service:50051"
        method: "InventoryService/GetStock"
      input_schema: { "$ref": "schemas/stock-check-input.json" }
      output_schema: { "$ref": "schemas/stock-check-output.json" }
      side_effect: none
      sla:
        latency_p99: 200ms
        availability: 99.9%
      degradation:
        - level: full
          description: "Real-time across all warehouses"
        - level: cached
          description: "15-minute-old cached value"
          trigger: "primary-db-slow"

    - id: cap-stock-reserve
      name: "Stock reservation"
      endpoint:
        type: grpc
        address: "inventory-service:50051"
        method: "InventoryService/Reserve"
      side_effect: write
      reversible: true
      compensation:
        method: "InventoryService/ReleaseReservation"

  health_check:
    endpoint: "inventory-service:50051/grpc.health.v1.Health/Check"
    interval: 10s

  fast_path_rules:
    - pattern: "Stock lookup with an explicit SKU"
      action: "Bypass LLM inference and call gRPC directly"
```

---

### 19.4 Redesigning Service-to-Service Communication

#### 19.4.1 Communication Pattern Classification

Service-to-service traffic is classified into four patterns, each assigned its optimal processing mode.

```
┌─────────────────────────────────────────────────────────┐
│              Communication Pattern Classification        │
├──────────────┬──────────────┬────────────┬──────────────┤
│              │ Deterministic│ Semi-Det.  │ Non-Det.     │
│              │              │            │              │
├──────────────┼──────────────┼────────────┼──────────────┤
│ Synchronous  │ ① Fast Path │ ② Agent   │ ③ Full      │
│ (Request/    │  direct gRPC │  Assisted  │  Agentic    │
│  Response)   │  call        │  Routing   │  Orchestration│
├──────────────┼──────────────┼────────────┼──────────────┤
│ Asynchronous │ ④ Event     │ ⑤ Smart   │ ⑥ Agentic  │
│ (Event/      │  Direct      │  Event     │  Choreography│
│  Messaging)  │  Routing     │  Routing   │              │
└──────────────┴──────────────┴────────────┴──────────────┘
```

**① Fast Path (deterministic, synchronous):**
- Forward structured requests directly to the service. No LLM inference.
- Example: `GET /orders/12345` → routed directly to Order Service.
- Latency: on par with direct inter-service communication (+1–2 ms sidecar overhead).

**② Agent Assisted Routing (semi-deterministic, synchronous):**
- When the request has mild ambiguity, normalize it with a small model before routing.
- Example: "Recent orders" → orders from the last 7 days (the agent interprets "recent").
- Latency: +50–200 ms.

**③ Full Agentic Orchestration (non-deterministic, synchronous):**
- Requires coordination across several services and the flow is decided dynamically.
- Example: "Assess this customer's churn risk and propose interventions" → cross-cut CRM + usage data + billing + support history.
- Latency: 1–30 s depending on complexity.

**④ Event Direct Routing (deterministic, asynchronous):**
- Keep the existing event-driven pattern as-is. AAF does not intervene.
- Example: order-placed event → dispatched to stock reservation service.

**⑤ Smart Event Routing (semi-deterministic, asynchronous):**
- Interpret the event's content to decide the best destination dynamically.
- Example: inquiry event → parse content and route to the right department's service.

**⑥ Agentic Choreography (non-deterministic, asynchronous):**
- Multiple services cooperate autonomously to complete a long-running task.
- Example: new-customer onboarding (account creation → initial configuration → welcome email → training schedule → follow-up plan).

#### 19.4.2 Pattern Selection Logic

Deciding which pattern to apply is automatic.

```yaml
routing_decision_tree:
  step_1_structured_check:
    condition: "Is the request fully structured (complies with API spec)?"
    yes: fast_path_candidate
    no: agent_required

  fast_path_candidate:
    condition: "Is the target service uniquely identifiable?"
    yes: "① Fast Path"
    no: "② Agent Assisted Routing"

  agent_required:
    condition: "Does a single service suffice?"
    single_service:
      condition: "Only ambiguity resolution needed?"
      yes: "② Agent Assisted Routing"
      no: "③ Full Agentic Orchestration"
    multi_service: "③ Full Agentic Orchestration"
```

#### 19.4.3 Fusion with the Saga Pattern

The existing Saga pattern (distributed-transaction management) is folded into AAF's Graph Runtime.

```
Traditional Saga:
  Step 1: Stock reservation   ← success
  Step 2: Payment execution   ← success
  Step 3: Shipping arrangement ← failure!
  Compensation:
    Step 2 comp: refund        ← fixed
    Step 1 comp: release stock ← fixed

AAF Agentic Saga:
  Step 1: Stock reservation (Agent)         ← success
  Step 2: Payment execution (Deterministic) ← success
  Step 3: Shipping arrangement (Agent)      ← failure!

  Intelligent Compensation:
    Agent analyzes the cause:
      Cause: "incomplete delivery address"
      Decision: "Shipping failed, but the reservation and payment
                are still valid. Rather than a full rollback, confirm
                the address and retry."

    → Interrupt only Step 3; keep Steps 1 and 2
    → Ask the user for address confirmation
    → After confirmation, resume from Step 3
```

**Benefits of the Agentic Saga:**
- Traditional Sagas assume "failure = full rollback." AAF can analyze the cause semantically and select the optimal recovery strategy dynamically.
- Recovery can leverage partial success.
- Deterministic Cores (payment, stock reservation, etc.) are still compensated reliably in the traditional way.

```yaml
# Agentic Saga definition
agentic_saga:
  name: "Order processing"
  steps:
    - step: 1
      name: "Stock reservation"
      type: deterministic  # executed deterministically
      service: inventory-service
      capability: cap-stock-reserve
      compensation: cap-stock-release
      compensation_type: mandatory  # always compensated on failure

    - step: 2
      name: "Payment execution"
      type: deterministic
      service: payment-service
      capability: cap-payment-execute
      compensation: cap-payment-refund
      compensation_type: mandatory

    - step: 3
      name: "Shipping arrangement"
      type: agent  # agent decides dynamically
      service: shipping-service
      capability: cap-shipping-arrange
      compensation: cap-shipping-cancel
      compensation_type: conditional  # decision depends on context

      on_failure:
        strategy: intelligent_recovery
        options:
          - condition: "incomplete address"
            action: "pause_and_ask_user"
            preserve_steps: [1, 2]
          - condition: "temporary carrier outage"
            action: "retry_with_alternative_carrier"
            preserve_steps: [1, 2]
          - condition: "item exceeds size limit"
            action: "full_compensation"
            reason: "Fundamentally cannot be shipped"
```

#### 19.4.4 Event Mesh Integration

AAF's event-handling layer is designed as an Event Mesh.

```
┌─────────────────────────────────────────────────────────────┐
│                     AAF Event Mesh                           │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              Event Router (Intelligent)               │   │
│  │                                                       │   │
│  │  incoming event → semantic classification →          │   │
│  │  routing decision → delivery                          │   │
│  │                                                       │   │
│  │  Classification rules:                                │   │
│  │    - structured event → Fast Path (direct delivery)  │   │
│  │    - ambiguous event → agent interprets → deliver    │   │
│  │      to the right destination                         │   │
│  │    - composite event → decompose → parallel deliver  │   │
│  │      to multiple destinations                         │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  Event bus: NATS / Kafka / CloudEvents                       │
│                                                              │
│  ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐            │
│  │Order   │  │Inv.    │  │Payment │  │Alert   │            │
│  │Events  │  │Events  │  │Events  │  │Events  │            │
│  └────────┘  └────────┘  └────────┘  └────────┘            │
└─────────────────────────────────────────────────────────────┘
```

---

### 19.5 Designing for Deterministic Core Protection

#### 19.5.1 Drawing the Boundary Clearly

The single most important aspect of introducing AAF is clearly separating "boundaries where agents may decide" from "boundaries that must be handled deterministically."

```
┌─────────────────────────────────────────────────────────────┐
│              Agentic Zone (where agents operate)             │
│                                                              │
│  ・Interpreting user intent                                  │
│  ・Normalizing ambiguous input                               │
│  ・Selecting the optimal service                             │
│  ・Classifying exceptions and choosing recovery strategies   │
│  ・Generating reports, summaries, and explanations           │
│  ・Estimating priority and urgency                           │
│  ・Searching for similar cases and making suggestions        │
│  ・Dynamic orchestration across multiple services            │
│                                                              │
├──────────────── Boundary ──────────────────────────────────┤
│                                                              │
│              Deterministic Zone (must be exact)              │
│                                                              │
│  ・Amount calculation and billing                            │
│  ・Inventory reservation and lock management                 │
│  ・Final authentication and authorization decisions          │
│  ・Encryption, signing, and key management                   │
│  ・Audit log recording                                       │
│  ・State transitions (order status, payment status)          │
│  ・Legally binding records                                   │
│  ・Enforcement of rate limits                                │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

#### 19.5.2 Boundary Enforcement

```yaml
# Node-type constraints inside the Graph Runtime
node_type_constraints:
  deterministic_node:
    description: "A deterministic node that never uses LLM inference"
    allowed_operations:
      - "Direct calls to existing service APIs"
      - "Transformation of structured data"
      - "Numerical computation"
      - "Executing state transitions"
    prohibited:
      - "Sending prompts to an LLM"
      - "Natural-language judgment"
      - "Ambiguous interpretation"
    verification:
      - "Static analysis confirms the node has no dependency on LLMProvider"

  agent_node:
    description: "A non-deterministic node that uses LLM inference"
    allowed_operations:
      - "Interpreting intents"
      - "Estimating optimal choices"
      - "Text generation"
    required:
      - "Input Guard"
      - "Output Guard"
      - "Trace recording"
    constraints:
      - "Must not directly execute Deterministic Zone operations"
      - "If deterministic operations are needed, delegate to a deterministic node"
```

#### 19.5.3 Anti-Corruption Layer

An Anti-Corruption Layer sits between each service and the AAF layer, separating AAF's semantic model from each service's internal model.

```
[AAF Semantic Model]
  "Customer" = { name, segment, lifetime_value, risk_score }
      │
      ▼
[Anti-Corruption Layer]
  Translation: AAF "Customer" ↔ CRM "Account" + Billing "Customer" + Support "User"
      │
      ▼
[Service Internal Models]
  CRM:     Account  { account_name, industry, size }
  Billing: Customer { customer_id, plan, payment_method }
  Support: User     { user_id, email, ticket_history }
```

```python
# Example Anti-Corruption Layer
class CustomerACL:
    """Translates AAF's 'Customer' concept to/from the services' internal models"""

    async def to_aaf_model(self, crm_account, billing_customer, support_user):
        """Services' internal models → AAF unified model"""
        return AAFCustomer(
            name=crm_account.account_name,
            segment=crm_account.industry,
            lifetime_value=billing_customer.total_revenue,
            risk_score=self._calculate_risk(
                billing_customer.payment_history,
                support_user.ticket_count
            ),
        )

    async def from_aaf_intent(self, intent):
        """AAF intent → concrete calls to each service"""
        if intent.target == "update_customer":
            return {
                "crm": UpdateAccountRequest(name=intent.data.name),
                "billing": UpdateCustomerRequest(plan=intent.data.plan),
                # Support does not need changes
            }
```

---

### 19.6 Phased Adoption Patterns

#### 19.6.1 Adoption Maturity Model

```
Level 0: No AAF
  Services communicate only through traditional REST / gRPC / events.

Level 1: Front Door Only
  Only the user-facing entry point becomes AAF.
  Service-to-service communication is unchanged.
  Risk: minimal
  Value: improved user experience

Level 2: Read Path Agentic
  AAF is introduced into read-only service-to-service traffic.
  Search, lookups, and report generation become agentic.
  Risk: low (no side effects)
  Value: better cross-cutting information access

Level 3: Selective Write Path
  AAF is introduced into low-risk write operations.
  High-risk ops stay as-is.
  Risk: medium (controlled via approval gates)
  Value: exception handling automation

Level 4: Full Agentic Orchestration
  All operations flow through AAF (high-risk ops require approval).
  Agentic Sagas handle distributed transactions.
  Risk: managed high risk
  Value: dynamic orchestration and autonomous exception recovery

Level 5: Autonomous Evolution
  AAF proposes additions, merges, and splits of services.
  Autonomous evolution via the Capability Evolution Observatory.
  Risk: high (mandatory human oversight)
  Value: system self-optimization
```

#### 19.6.2 Concrete Steps for Level 1 → Level 2 Migration

```
Week 1-2: Preparation
  ├── Deploy the AAF Control Plane (Capability Registry, Policy Engine, Trace)
  ├── Author Capability Contracts for each service
  └── Prepare the Agent Sidecar configuration template

Week 3-4: Sidecar rollout starting with read services
  ├── Add sidecar to the search service
  ├── Add sidecar to the reporting service
  ├── Run in Shadow Mode (only record decisions, do not execute)
  └── Measure decision agreement rate

Week 5-6: Shadow-Mode evaluation and cutover
  ├── Agreement rate ≥ 95% → cut the read path over to AAF
  ├── Agreement rate < 95% → tune Capability Contracts / Intent Compiler
  └── Measure and optimize Fast Path rate

Week 7-8: Stabilization and preparation for Level 3
  ├── Confirm operational metrics are stable
  ├── Select candidate low-risk write operations
  └── Prepare the approval-gate UI
```

#### 19.6.3 Per-Service Adoption Priority

```yaml
adoption_priority_matrix:
  high_priority:  # services that should become AAF-ready first
    criteria:
      - "Called by many other services (hub service)"
      - "Frequently receives ambiguous input"
      - "Lots of manual exception handling"
      - "Cross-cuts multiple data sources"
    examples:
      - "Customer management service"
      - "Search / recommendation service"
      - "Inquiry routing service"

  medium_priority:  # second wave
    criteria:
      - "Moderate coupling with other services"
      - "Inputs and outputs are relatively structured"
      - "Some exception handling benefits from semantic judgment"
    examples:
      - "Order management service"
      - "Notification service"
      - "Report generation service"

  low_priority:  # last (or Fast Path only)
    criteria:
      - "Primarily CRUD"
      - "Inputs and outputs fully structured"
      - "Little room for semantic judgment"
    examples:
      - "Authentication service"
      - "Billing calculation service"
      - "File storage service"
```

---

### 19.7 Failure Design and Resilience

#### 19.7.1 Failure Taxonomy and Response Matrix

```yaml
failure_taxonomy:
  # Failures within the AAF layer
  aaf_layer_failures:
    intent_compiler_down:
      impact: "New requests cannot be processed"
      mitigation: "Fast Path eligible requests are routed directly"
      fallback: "Allow direct access to structured API endpoints"

    capability_registry_down:
      impact: "Cannot discover new capabilities"
      mitigation: "Use the locally cached capability list"
      ttl: "Cache TTL 5 minutes"

    policy_engine_down:
      impact: "Policy checks cannot run"
      mitigation: "fail-closed (deny all operations)"
      reason: "Execution without policy checks is not permitted"

    graph_runtime_down:
      impact: "Orchestration cannot proceed"
      mitigation: "In-flight tasks can resume from checkpoint"
      fallback: "Single-service calls continue through the sidecar"

  # Failures at the service layer
  service_layer_failures:
    single_service_down:
      detection: "Health check failure or response timeout"
      response:
        - "Fallback per the Capability Degradation Spec"
        - "Trip the circuit breaker"
        - "Pause dependent Agentic Saga steps"
        - "Route to an alternate service if one exists"

    cascade_failure:
      detection: "Simultaneous failures across multiple services"
      response:
        - "Automatically assess the blast radius"
        - "Temporarily suspend calls to non-critical services"
        - "Return partial results with only critical services"
        - "Escalate to a human"

  # Failures at the LLM layer
  llm_failures:
    rate_limit:
      response: "Queue requests and process by priority"
    model_unavailable:
      response: "Switch to fallback model and notify of quality reduction"
    hallucination_detected:
      response: "Detected by Output Guard → retry or escalate to human"
```

#### 19.7.2 Multi-level Circuit Breaker

```
┌──────────────────────────────────────────────┐
│           AAF Circuit Breaker Hierarchy       │
│                                               │
│  Level 1: Service-level                       │
│    Inventory service is unresponsive          │
│    → Stop only calls to inventory service    │
│    → Fall back to cache / alternate service  │
│                                               │
│  Level 2: Capability-level                    │
│    Only stock-reserve is failing               │
│    (stock-check is fine)                      │
│    → Stop only cap-stock-reserve              │
│    → cap-stock-check continues                 │
│                                               │
│  Level 3: Agent-level                         │
│    A specific agent's decision quality drops  │
│    → Lower that agent's Trust Score           │
│    → Raise its human-in-the-loop rate         │
│                                               │
│  Level 4: Flow-level                          │
│    A particular orchestration pattern is      │
│    failing frequently                         │
│    → Put that flow into fallback mode         │
│    → Decompose to individual API calls for    │
│      manual handling                          │
│                                               │
│  Level 5: System-level                        │
│    The entire AAF layer is overloaded         │
│    → Switch all requests to Fast Path first   │
│    → Queue Agentic traffic                    │
└──────────────────────────────────────────────┘
```

#### 19.7.3 Graceful Degradation Chain

```
Normal:
  Full Agentic Orchestration
  ├── LLM-driven dynamic planning
  ├── Multi-service coordination
  ├── Semantic exception recovery
  └── Latency: 1–10 s

Degraded 1 (LLM latency rises):
  Agent Assisted + cached plans
  ├── Retrieve similar past plans from cache
  ├── Small model for minor adjustments only
  └── Latency: 0.5–3 s

Degraded 2 (LLM unavailable):
  Rule-based orchestration
  ├── Use pre-defined flow templates
  ├── Branch decisions via rules
  └── Latency: 0.1–1 s

Degraded 3 (Graph Runtime overloaded):
  Fast Path Only
  ├── Accept structured requests only
  ├── Route directly to services
  └── Latency: equivalent to direct inter-service calls

Degraded 4 (AAF layer down):
  Bypass Mode
  ├── Sidecars transparently forward requests
  ├── AAF layer fully bypassed
  ├── Behaves like a traditional API Gateway
  └── Latency: equivalent to no-AAF
```

---

### 19.8 Security and Governance

#### 19.8.1 Service-Architecture-Specific Security Design

```yaml
security_model:
  # Service-to-service authentication
  inter_service_auth:
    method: "mTLS + JWT"
    description: |
      On top of the service mesh's mutual TLS, the AAF layer adds
      intent information and trust level to the JWT.
    jwt_claims:
      iss: "aaf-control-plane"
      sub: "requesting-agent-id"
      intent_id: "int-abc123"
      trust_level: 3
      allowed_scopes: ["inventory:read", "order:write"]
      budget_remaining: 0.85
      depth: 2  # depth in the delegation chain

  # Tenant isolation
  tenant_isolation:
    strategy: "namespace-based"
    enforcement:
      - "Capability Registry filtered by tenant ID"
      - "Memory store namespaced per tenant"
      - "Traces in per-tenant isolated storage"
      - "Cross-tenant communication requires a Federation Agreement"

  # Prompt Injection defenses (service-architecture specific)
  injection_protection:
    external_data:
      description: "Responses from external services may contain malicious instructions"
      mitigation:
        - "Service response data is interpreted as data only, never as instructions"
        - "Input Guard sanitizes all external data"
        - "Prompts are structured to separate data from instructions"

    inter_service:
      description: "Malicious capability responses from a compromised service"
      mitigation:
        - "Output Guard inspects all service responses"
        - "Schema validation rejects responses with unexpected fields"
        - "Anomaly detection flags responses that deviate significantly from normal"
```

#### 19.8.2 Permission Model

```
┌─────────────────────────────────────────────────────────┐
│              AAF Permission Model                        │
│                                                          │
│  User Scopes (permissions granted to the user)           │
│    ↓ filter                                              │
│  Intent Scopes (permissions the intent requires)         │
│    ↓ intersect                                           │
│  Agent Trust Level (limits from the agent's autonomy)    │
│    ↓ min()                                               │
│  Delegation Chain (attenuation along delegation)         │
│    ↓ final                                               │
│  Effective Scopes (scopes actually passed to services)   │
│                                                          │
│  Example:                                                │
│    User Scopes:      [order:*, inventory:read, payment:*]│
│    Intent Scopes:    [order:write, inventory:read]       │
│    Agent Trust (L3): [order:write(low), inventory:read]  │
│    Delegation:       min(L3, L4) = L3                    │
│    ────────────────────────────────────────              │
│    Effective:        [order:write(low), inventory:read]  │
└─────────────────────────────────────────────────────────┘
```

---

### 19.9 Operations and Observability Design

#### 19.9.1 Three Layers of Observability

```
Layer 1: Infrastructure Observability (existing)
  ├── Container metrics (CPU, memory, network)
  ├── Service mesh metrics (latency, error rate)
  └── Logs (structured)

Layer 2: AAF Operational Observability (new)
  ├── Intent resolution rate
  ├── Fast Path usage rate
  ├── Agent-node vs deterministic-node ratio
  ├── Mean chain depth
  ├── Policy violation rate
  ├── Trust Score distribution
  ├── Cost per intent
  └── Approval wait time

Layer 3: Semantic Observability (new)
  ├── Intent Fidelity Score
  ├── Semantic Drift Index
  ├── Capability usage patterns
  ├── Unmet intent detection
  ├── Negotiation success rate
  └── Exception recovery success rate
```

#### 19.9.2 Distributed Trace Integration

AAF's traces integrate with OpenTelemetry, unifying the service-mesh trace with the AAF semantic trace.

```
OpenTelemetry Trace:
  Span: "user-request"                    ← HTTP receive
    Span: "aaf.intent-compile"            ← Intent conversion
      Attribute: intent_type = "Transactional"
      Attribute: confidence = 0.92
    Span: "aaf.plan"                      ← Plan generation
      Attribute: steps = 3
      Attribute: estimated_cost = 0.15
    Span: "aaf.execute.step-1"            ← Step 1 execution
      Attribute: node_type = "agent"
      Attribute: capability = "cap-stock-check"
      Span: "grpc.inventory-service"      ← Service call (pre-existing span)
        Attribute: grpc.method = "GetStock"
    Span: "aaf.execute.step-2"            ← Step 2 execution
      Attribute: node_type = "deterministic"
      Span: "grpc.payment-service"
    Span: "aaf.policy-check"              ← Policy check
      Attribute: result = "pass"
    Span: "aaf.artifact-create"           ← Artifact creation
```

#### 19.9.3 Operational Dashboard Design

```yaml
dashboard_panels:
  overview:
    - title: "Intent Resolution Rate"
      metric: "aaf_intent_resolved_total / aaf_intent_received_total"
      alert: "< 95% for 5 min"

    - title: "Fast Path Ratio"
      metric: "aaf_fast_path_total / aaf_request_total"
      target: "> 60%"

    - title: "P99 Latency by Pattern"
      metrics:
        fast_path: "aaf_latency_p99{pattern='fast_path'}"
        agent_assisted: "aaf_latency_p99{pattern='agent_assisted'}"
        full_agentic: "aaf_latency_p99{pattern='full_agentic'}"

    - title: "Cost per Intent (24h rolling)"
      metric: "rate(aaf_cost_total[24h]) / rate(aaf_intent_total[24h])"

  health:
    - title: "Service Capability Health"
      description: "Health map of every service's capabilities"
      visualization: "heatmap"

    - title: "Trust Score Distribution"
      description: "Trust score distribution across all agents"
      visualization: "histogram"

    - title: "Circuit Breaker Status"
      description: "Breaker state per service / capability"
      visualization: "status_grid"

  intelligence:
    - title: "Unmet Intents"
      description: "List of intent patterns that could not be resolved"

    - title: "Semantic Drift Alert"
      description: "Capabilities where intent-interpretation drift was detected"

    - title: "Capability Co-usage Clusters"
      description: "Groups of capabilities always used together (integration candidates)"
```

---

### 19.10 Concrete Use-Case Designs

#### 19.10.1 E-commerce Order Processing (Microservice Setup)

```yaml
scenario: "A user issues an ambiguous order"
input: "Get me another one of my usual laundry detergent, and kitchen paper too since we're almost out"

execution_flow:
  step_1:
    type: agent_node
    action: "Intent Compiler parses the intent"
    result:
      intent_type: TransactionalIntent
      items:
        - description: "my usual laundry detergent"
          resolution: "Look up past purchases in long-term memory"
          resolved: "SKU-12345 (Attack Antibacterial EX refill 1350g)"
          confidence: 0.91
        - description: "kitchen paper"
          resolution: "Ambiguous: brand and size unknown"
          candidates:
            - "SKU-67890 (Elleair 4 rolls) ← purchased twice before"
            - "SKU-67891 (Nepia 2 rolls)"
          confidence: 0.72

  step_2:
    type: agent_node
    action: "Ask the user for items needing confirmation"
    output: "Is Attack Antibacterial EX OK for the detergent? For the kitchen paper, would you like Elleair 4 rolls or Nepia 2 rolls?"
    # ← Confidence 0.72 triggers confirmation; at ≥ 0.90 we would proceed on inference.

  step_3:
    type: deterministic_node
    service: inventory-service
    action: "Stock check"
    # Fast Path: structured SKU-based stock inquiry

  step_4:
    type: deterministic_node
    service: payment-service
    action: "Payment processing"
    # Deterministic Core: amounts are computed deterministically

  step_5:
    type: agent_node
    service: shipping-service
    action: "Arrange delivery"
    intelligence: |
      The agent decides:
      - Detergent is heavy → standard delivery
      - Kitchen paper is bulky but light → can be combined
      - Choose the optimal method based on total weight and size
      - Use the user's historical delivery time → morning slot

  step_6:
    type: agent_node
    action: "Report back to the user"
    output: |
      Order placed:
      ・Attack Antibacterial EX refill 1350g × 1
      ・Elleair kitchen paper 4 rolls × 1
      Total: ¥1,280
      Delivery: tomorrow morning
```

#### 19.10.2 Cross-System Internal Workflow (Modular Monolith Setup)

```yaml
scenario: "Sales manager asks for a plan to address high churn-risk customers"
input: "Find customers up for renewal next month who are at high churn risk, and draft interventions"

execution_flow:
  step_1:
    type: agent_node
    module: crm-module
    action: "Extract customers up for renewal next month"
    intelligence: "'Next month' = May 2026. Customers with renewal dates between 5/1 and 5/31"

  step_2:
    type: agent_node
    modules: [usage-analytics-module, support-module, billing-module]
    action: "Cross-analyze churn risk factors"
    intelligence: |
      Collect from each module:
      - Usage trend (usage-analytics)
      - Support ticket trend (support)
      - Payment delinquency (billing)
      Compute an aggregate risk score.

  step_3:
    type: deterministic_node
    module: analytics-module
    action: "Compute the risk score"
    note: "The scoring itself is a deterministic algorithm"

  step_4:
    type: agent_node
    action: "Generate intervention proposals"
    intelligence: |
      Propose interventions based on risk drivers:
      - Usage decline → propose an enablement session
      - Support dissatisfaction → assign a dedicated CSM
      - Price concerns → propose a discount or downgrade
      Reference past success stories from episodic memory.

  step_5:
    type: approval_gate
    approver: "Sales manager"
    presentation: |
      5 at-risk customers:
      1. Company A — risk score 0.87 — driver: usage down 30%
         Recommendation: enablement meeting + feature demo
      2. B Corp — risk score 0.75 — driver: support satisfaction drop
         Recommendation: assign dedicated CSM
      ...
      Once approved, tasks will be created automatically for each account's owner.

  step_6:
    type: agent_node
    modules: [task-module, notification-module]
    action: "Turn approved interventions into tasks and notify the owners"
```

#### 19.10.3 Cross-Cell Coordination (Cell Architecture Setup)

```yaml
scenario: "Optimal global inventory redistribution"
input: "Demand for product X is surging in Japan. Rebalance inventory globally."

execution_flow:
  step_1:
    type: agent_node
    cell: japan
    action: "Analyze demand in the Japan cell"
    result: "Product X demand is up 200% week-over-week. Current stock lasts 3 days"

  step_2:
    type: agent_node
    cell: federation-layer
    action: "Query inventory across all cells"
    cross_cell_calls:
      - cell: us
        capability: cap-us-inventory
        query: "Product X stock and demand forecast"
      - cell: eu
        capability: cap-eu-inventory
        query: "Product X stock and demand forecast"
    data_boundary: "Aggregates only. No individual customer data."

  step_3:
    type: agent_node
    action: "Draft the optimal redistribution plan"
    intelligence: |
      Analysis:
      - Japan: stock 300, demand 600/week → shortage
      - US:    stock 1200, demand 400/week → surplus
      - EU:    stock 500, demand 450/week → balanced

      Proposal: transfer 500 units from US to Japan
      Cost: shipping $2,000, lead time 5 days
      Risk: US inventory drops to a 2-week supply (within tolerance)

  step_4:
    type: approval_gate
    approver: "Global SCM manager"
    approval_level: "cross_cell_write"  # cross-cell writes require higher approval

  step_5:
    type: agentic_saga
    cells: [us, japan]
    steps:
      - cell: us
        action: "Move 500 units into transfer status"
        compensation: "Restore the inventory"
      - cell: japan
        action: "Register as incoming shipment"
        compensation: "Cancel the incoming shipment"
      - cell: logistics
        action: "Arrange transportation"
        compensation: "Cancel transportation"
```

---

### 19.11 Performance and Cost Optimization

#### 19.11.1 Latency Budget

```yaml
latency_budget:
  # End-to-end latency targets
  targets:
    fast_path:
      p50: 10ms
      p99: 50ms
      breakdown:
        sidecar_overhead: 2ms
        routing: 3ms
        service_call: 5ms  # service's own latency

    agent_assisted:
      p50: 150ms
      p99: 500ms
      breakdown:
        sidecar_overhead: 2ms
        intent_normalization: 100ms  # small model
        routing: 3ms
        service_call: 45ms

    full_agentic:
      p50: 3s
      p99: 15s
      breakdown:
        intent_compilation: 500ms
        planning: 500ms
        execution_per_step: 500ms  # × 3 steps on average
        policy_checks: 100ms
        memory_retrieval: 200ms
        overhead: 200ms
```

#### 19.11.2 Fast Path Optimization

```yaml
fast_path_optimization:
  # Intent Cache: detect repeated intent patterns and bypass LLM inference
  intent_cache:
    strategy: "semantic_hash"
    description: |
      Cache the result of past Intent compilations. When a new request
      matches a past pattern semantically, reuse the cached Intent Envelope.
    hit_rate_target: "> 40%"
    ttl: 1h
    invalidation: "On Capability Contract change"

  # Plan Cache: cache execution plans per intent pattern
  plan_cache:
    description: |
      Cache the mapping from Intent Envelope to Execution Plan.
      Reuse the same plan for Intents with the same structure.
    hit_rate_target: "> 30%"
    ttl: 30min
    invalidation: "On Capability Registry change"

  # Pattern Detection: auto-promote high-frequency patterns to Fast Path
  auto_fast_path:
    description: |
      If a given pattern succeeds more than 100 times and all take
      the same execution path, auto-promote it to Fast Path (no LLM inference).
    threshold: 100
    consistency_required: 0.95
    review: "Human approves the promotion"
```

#### 19.11.3 Cost Management Structure

```yaml
cost_management:
  model_tier_routing:
    tier_1_heavy:
      model: "claude-opus-4-6"
      use_for: "Complex plan generation, high-value customer interactions, legal document analysis"
      cost: "$$$$"
    tier_2_standard:
      model: "claude-sonnet-4-20250514"
      use_for: "Standard Intent compilation, routine orchestration"
      cost: "$$"
    tier_3_light:
      model: "claude-haiku-4-5-20251001"
      use_for: "Simple intent normalization, template-based responses"
      cost: "$"
    tier_4_no_llm:
      model: null
      use_for: "Fast Path, cache hits, rule-based decisions"
      cost: "≈0"

  routing_logic: |
    1. Can Fast Path handle it? → tier_4
    2. Intent Cache hit? → tier_4
    3. Needs only simple normalization? → tier_3
    4. Routine orchestration? → tier_2
    5. Complex / high-value / legal? → tier_1
```

---

### 19.12 Implementation Roadmap

### Phase 1: Foundation (Month 1–4)

```
Month 1: Core + Spec
  ├── Contract specs (Protobuf + JSON Schema)
  ├── Graph Runtime (sequential, parallel, branch, retry, checkpoint)
  ├── Base Policy Engine (scope check, side-effect gate)
  └── Base trace recording

Month 2: Service Integration Layer
  ├── Agent Sidecar v0.1 (REST/gRPC proxy + capability publishing)
  ├── Agent Wrapper v0.1 (for modular monoliths)
  ├── Capability Registry (registration, discovery, health check)
  └── Fast Path routing

Month 3: Intelligence Layer
  ├── Intent Compiler (5 intent types, refinement protocol)
  ├── Planner (bounded-autonomy plan generation)
  ├── Memory System (Working + Thread + Long-term)
  └── LLM Provider abstraction (Anthropic + OpenAI)

Month 4: DX + Testing
  ├── Python SDK v0.1
  ├── CLI (init, dev, register, test)
  ├── Local dev environment (docker-compose minimal)
  ├── 3 sample agents
  └── Integration tests + contract test harness
```

### Phase 2: Production Readiness (Month 5–8)

```
Month 5: Resilience
  ├── Agentic Saga (intelligent compensation)
  ├── Multi-level Circuit Breaker
  ├── Graceful Degradation Chain (5 levels)
  ├── Capability Degradation Spec implementation
  └── Chaos engineering test suite

Month 6: Security + Trust
  ├── Trust Score Model (5 autonomy levels)
  ├── Guard Layer (Input / Output / Action)
  ├── Prompt injection defenses
  ├── mTLS + JWT integration
  └── Multi-tenant isolation

Month 7: Operations
  ├── OpenTelemetry integration
  ├── Operational dashboard
  ├── Trace Explorer (search, replay)
  ├── Cost Attribution Engine
  └── Shadow Mode implementation

Month 8: Ecosystem
  ├── TypeScript SDK v0.1
  ├── Go SDK v0.1
  ├── MCP Adapter
  ├── A2A Adapter
  ├── Front Door UI (Chat + Approval Gate)
  └── Helm charts + Terraform modules
```

### Phase 3: Advanced (Month 9–12)

```
Month 9-10: Intelligence Upgrade
  ├── Automatic Fast Path promotion
  ├── Intent Cache / Plan Cache
  ├── Value-based Routing
  ├── Capability Evolution Observatory
  └── Semantic Regression Testing

Month 11-12: Scale + Federation
  ├── Cell architecture support (Cross-Cell Router)
  ├── Federation Agreement implementation
  ├── Event Mesh integration
  ├── Policy Plugin Marketplace
  └── Performance benchmarks + optimization
```

---

### 19.13 Metrics and Success Criteria

#### 19.13.1 Measuring Adoption Outcomes

```yaml
success_metrics:
  # Developer experience
  developer_experience:
    - metric: "Time to make a new service AAF-ready"
      baseline: "N/A"
      target: "< 4 hours (sidecar config + capability definition)"

    - metric: "Time to implement service-to-service integration"
      baseline: "1–2 weeks (adapter development)"
      target: "< 1 day (capability definition only)"

  # Operational quality
  operational_quality:
    - metric: "Manual intervention rate on exceptions"
      baseline: "set after measurement"
      target: "50% reduction"

    - metric: "Incident resolution time"
      baseline: "set after measurement"
      target: "30% shorter"

    - metric: "Inter-service failure blast radius"
      target: "< 5%"

  # Performance
  performance:
    - metric: "Fast Path rate"
      target: "> 60%"

    - metric: "AAF layer overhead (Fast Path)"
      target: "< 5ms (p99)"

    - metric: "Intent Resolution Rate"
      target: "> 97%"

  # Cost
  cost:
    - metric: "LLM cost per intent"
      target: "< $0.01 (average)"

    - metric: "LLM call reduction from Fast Path + cache"
      target: "> 50%"

  # Security
  security:
    - metric: "Policy violation rate"
      target: "< 0.01%"

    - metric: "Prompt Injection detection rate"
      target: "> 99%"
```

#### 19.13.2 Phased Success Criteria

```
Phase 1 exit criteria:
  ✓ Three services publish capabilities via sidecars
  ✓ Multi-service orchestration from a single intent works
  ✓ Resume-from-checkpoint works
  ✓ Base policy checks are functional

Phase 2 exit criteria:
  ✓ Shadow Mode shows ≥ 95% decision agreement with the existing system
  ✓ Agentic Saga compensation transactions work correctly
  ✓ Fault-injection tests exercise every graceful-degradation level
  ✓ All metrics visible on the operational dashboard

Phase 3 exit criteria:
  ✓ Fast Path rate above 60%
  ✓ Cross-cell communication works safely (Federation Agreements applied)
  ✓ Capability Evolution Observatory produces actionable improvement proposals
  ✓ Performance benchmarks hit target values
```

---

### Appendix A (§19): AAF Configuration Comparison by Service Architecture

| Aspect | Microservices | Modular Monolith | Cell Architecture |
|---|---|---|---|
| AAF placement | Sidecar pattern (one per service) | Embedded runtime (in-process) | Cell Runtime + Federation Layer |
| Communication | gRPC / NATS | In-process calls | Intra-cell: in-process; inter-cell: A2A |
| Latency impact | +2–5 ms (sidecar) | +0.1–1 ms (same process) | Intra-cell: minimal; inter-cell: +50–200 ms |
| Independent deploy | Sidecar deployed with the service | Deployed with the monolith | Cell Runtime deployed with the cell |
| Scaling | Each service scales independently | Scales as a whole monolith | Each cell scales independently |
| Fault isolation | Sidecar failure affects only that service | Runtime failure affects all modules | Cell Runtime failure stays within the cell |
| Tenant isolation | Namespace + Network Policy | Logical separation within the process | The cell itself is the tenant boundary |
| Migration path | Add sidecars (no code changes) | Add wrappers (minimal code changes) | Add a Cell Runtime + federation configuration |

### Appendix B (§19): Design Checklist

Items to verify when integrating a new service into AAF.

**Capability definition:**
- [ ] Capability Contract authored for every public API?
- [ ] Side-effect classification correct (none / read / write / delete / send / payment)?
- [ ] Degradation strategy defined (at least 3 levels)?
- [ ] SLA (latency, availability) measured and defined?
- [ ] Dependent and conflicting capabilities identified?

**Deterministic Core protection:**
- [ ] Business logic (calculation, decisions) absent from Agent Nodes?
- [ ] Amount calculation, inventory reservation, and authentication decisions implemented in Deterministic Nodes?

**Security:**
- [ ] Required scopes defined?
- [ ] Data classification set (public / internal / confidential / restricted)?
- [ ] Compensation handlers defined (for write operations)?

**Fast Path:**
- [ ] Fast-path rules for structured requests defined?
- [ ] Patterns that do not need LLM inference identified?

**Testing:**
- [ ] Contract Conformance test authored?
- [ ] Degradation behaviour verified on failure?
- [ ] Decision agreement rate measured in Shadow Mode against the existing system?

---

*This design document is the Agentic Application Framework's service-architecture integration guide and will be updated continuously as implementation progresses.*

# Lodestone — AI Agent Design

> How Lodestone uses language models: two modes, one tool registry, one trust boundary.
> Companion to `DESIGN.md` (which this supersedes at §5) and `DESIGN-GUI.md`.

---

## 1. Two modes, and why they are structurally different

Lodestone supports AI in two ways, and the difference is not cosmetic — it is a difference in
**who owns the loop.**

| | **Mode A — Native agent** | **Mode B — ACP** |
|---|---|---|
| Model access | Direct API, user's key | External agent process |
| Who runs the loop | Lodestone | The external agent |
| Who chooses tools | Lodestone's registry | The agent, from what we expose |
| Who manages context | Lodestone | The agent |
| Hypothesis-before-action | Enforced structurally | Cannot be enforced |
| Grounding guarantee | Enforced at construction | Enforced at the boundary |
| Cost / billing | User's API key | The agent's own auth |
| Example | Claude, GPT, DeepSeek, Kimi, GLM via API | Claude Code, Codex CLI |

**Mode A is a controlled loop.** Lodestone decides what the model sees, forces a hypothesis
before every tool call, enforces a step budget, and refuses to construct a finding that cites
no evidence.

**Mode B is an inversion.** Lodestone becomes the *client* and the agent becomes the reasoner.
The agent has its own harness, its own context strategy, its own model, and its own billing.
Lodestone cannot make Codex state a hypothesis before calling a tool, because Codex's loop is
Codex's business.

What Lodestone retains in Mode B is narrower but still decisive: **it owns the tools, and it
owns the airlock.** The agent can only learn about the binary through tools Lodestone wrote,
every call is recorded as evidence, and every finding the agent proposes is validated against
the evidence actually issued during that session before it is allowed into the UI.

Being explicit about this asymmetry matters, because the honest version is that **Mode B's
grounding guarantee is weaker than Mode A's**, and the UI must say so rather than presenting
both as equivalent.

---

## 2. ACP background

The Agent Client Protocol is an open standard created by Zed Industries and released in
August 2025 that defines how AI coding agents talk to editors, using JSON-RPC 2.0 over
stdin/stdout, with the agent running as a subprocess. It is deliberately the LSP analogue for
agents: LSP standardizes language features between editors and language servers; ACP
standardizes prompts, sessions, permissions, and streaming between editors and agents. Remote
agents over HTTP are also supported.

**Lifecycle:**

1. `initialize` — negotiate protocol version and capabilities (filesystem access, terminal,
   MCP)
2. `session/new` — create a conversation, passing the working directory **and MCP server
   configurations**
3. `session/prompt` — the client sends the user message as content blocks
4. The agent streams back `session/update` notifications: message chunks, thought chunks, tool
   calls, plans
5. The agent calls back into the client for file operations (`fs/read_text_file`,
   `fs/write_text_file`), terminal access (`terminal/create`), and permission requests
   (`session/request_permission`)

**Step 2 is the load-bearing one for Lodestone.** Because the client supplies MCP server
configurations when creating a session, Lodestone can hand the external agent its entire
analysis toolset without the agent knowing anything about DWARF. That is the mechanism that
makes Mode B viable rather than a degraded chat window.

**Agent availability.** Claude Code connects through Zed's SDK adapter, and Codex CLI through
an ACP adapter, which covers both of the agents named for the initial release. The wider
ecosystem already includes Gemini CLI, GitHub Copilot, Goose, Kimi CLI, OpenCode, Cline,
OpenHands, Mistral Vibe, and many more, and there is now an ACP Registry acting as a
distribution layer so that an agent added there becomes available to every client speaking the
protocol.

**Two consequences worth planning around:**

- **Do not hardcode agent launch commands.** Launch patterns vary — some agents use a
  subcommand (`goose acp`), some a flag (`gemini --experimental-acp`), some a separate adapter
  binary. Read the registry where possible and fall back to a user-supplied command.
- **Supporting ACP at all gets you the whole ecosystem**, not just the two agents named. That
  is a large capability increase for one protocol implementation.

**Implementation:** the protocol is Apache-licensed and open source with published SDKs. Since
Lodestone is Rust and GPUI is from the same organisation, prefer the official Rust crate
(`agent-client-protocol`) over hand-rolling JSON-RPC. Pin the protocol version, negotiate at
`initialize`, and degrade gracefully when an agent reports an older version.

---

## 3. Architecture

```
┌───────────────────────────────────────────────────────────────────────┐
│                          lode-gui (GPUI)                              │
│   Agent panel · transcript · permission prompts · provider picker     │
└─────────────────────────────────┬─────────────────────────────────────┘
                                  │  AiSession trait
                ┌─────────────────┴──────────────────┐
                ▼                                    ▼
┌───────────────────────────┐        ┌───────────────────────────────────┐
│  MODE A — Native agent    │        │  MODE B — ACP client              │
│                           │        │                                   │
│  ┌─────────────────────┐  │        │  ┌─────────────────────────────┐  │
│  │ Agent loop          │  │        │  │ JSON-RPC over stdio         │  │
│  │  hypothesis → tool  │  │        │  │  initialize / session/new   │  │
│  │  → evidence → revise│  │        │  │  session/prompt → updates   │  │
│  └──────────┬──────────┘  │        │  └──────────────┬──────────────┘  │
│             │             │        │                 │ spawns          │
│  ┌──────────▼──────────┐  │        │                 ▼                 │
│  │ ModelBackend        │  │        │      ┌─────────────────────┐      │
│  │  anthropic│openai-  │  │        │      │ claude-code-acp     │      │
│  │  compatible│local   │  │        │      │ codex-acp  │ …      │      │
│  └─────────────────────┘  │        │      └──────────┬──────────┘      │
└─────────────┬─────────────┘        └─────────────────┼─────────────────┘
              │                                        │ MCP
              │                      ┌─────────────────▼─────────────────┐
              │                      │  lode-mcp — MCP server            │
              │                      │  127.0.0.1, per-session token     │
              │                      └─────────────────┬─────────────────┘
              │                                        │
              └────────────────┬───────────────────────┘
                               ▼
              ┌────────────────────────────────┐
              │   TOOL REGISTRY (lode-agent)   │   ← single source of truth
              │   one Tool trait, two exposures│
              └────────────────┬───────────────┘
                               ▼
              ┌────────────────────────────────┐
              │  Evidence store + Finding gate │   ← the airlock
              └────────────────┬───────────────┘
                               ▼
              ┌────────────────────────────────┐
              │  Analysis engines · lode-verify│
              └────────────────────────────────┘
```

**The single most important structural property: there is one tool registry.** A tool is
written once and exposed twice — natively to Mode A's loop, and over MCP to Mode B's agent. If
you find yourself writing a tool twice, the abstraction has broken.

---

## 4. The tool registry and its two exposures

### 4.1 One trait

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;          // becomes the MCP tool description
    fn schema(&self) -> serde_json::Value;  // JSON Schema for arguments
    fn cost(&self) -> ToolCost;             // Cheap | Moderate | Expensive
    fn side_effects(&self) -> SideEffects;  // ReadOnly | Builds | MutatesWorkspace
    async fn invoke(&self, args: Value, ctx: &mut SessionCtx) -> Result<ToolOutput>;
}
```

`side_effects` is new relative to `DESIGN.md` §5.2 and exists because Mode B needs it: a tool
that mutates the workspace must be gated by the trust tier even when an external agent called
it, and the registry is the only place that knows.

### 4.2 Native exposure (Mode A)

Tool schemas become the provider's function-calling declarations. For Anthropic-shaped APIs
that is the `tools` array; for OpenAI-shaped it is `tools` with `type: "function"`. One
translation function per provider shape, driven by the same `schema()`.

### 4.3 MCP exposure (Mode B)

`lode-mcp` wraps the same registry as an MCP server and hands its address to the agent at
`session/new`.

**Transport choice.** MCP over stdio is conventional, but Lodestone is the parent process and
would be spawning a child that then talks back to its own parent — awkward. Prefer an HTTP MCP
server bound to `127.0.0.1` on an ephemeral port with a per-session bearer token, passed in the
`session/new` MCP configuration. If an agent only supports stdio MCP servers, fall back to a
tiny `lode-mcp-bridge` helper binary that proxies stdio to the local HTTP endpoint.

**Security requirements on this server, all mandatory:**

- Bind to loopback only. Never `0.0.0.0`.
- A random token per session, in an `Authorization` header, checked on every request.
- The server dies with the session. No lingering listener.
- Tools are filtered by trust tier *at the server*, not by asking the agent nicely. If the
  tier is Observe, `MutatesWorkspace` tools are not in the tool list at all.

That last point is the difference between a policy and a control.

### 4.4 Tool inventory

Unchanged from `DESIGN.md` §5.2 in content, with the phase mapping intact. Both modes see the
same list, filtered by phase availability and trust tier.

The descriptions matter more in Mode B than Mode A. In Mode A you write the system prompt and
can explain the domain. In Mode B, Codex arrives knowing nothing about your tools, and the
`description()` string is the entire briefing. Write them as if for a competent engineer who
has never seen the tool: what it returns, what units, what the address space is, what "link
time" versus "runtime" means here.

---

## 5. The trust boundary

`DESIGN.md` P2 requires that no claim reaches the user without its evidence. Mode A enforces
this in the constructor. Mode B cannot, so it is enforced at the airlock instead.

### 5.1 Evidence issuance

Every tool invocation, in either mode, writes an `Evidence` record with
`Provenance::Deterministic` **before** its result is returned. The record carries an
`EvidenceId`. In Mode B, the MCP response includes that id in a structured envelope:

```json
{
  "evidence_id": "ev_01J8X...",
  "tool": "group_monomorphizations",
  "result": { "...": "..." },
  "note": "Cite evidence_id when reporting any finding derived from this result."
}
```

### 5.2 The finding gate

Findings enter the UI through exactly one function.

```rust
pub fn admit_finding(
    raw: RawFinding,
    session: &Session,
    origin: AgentOrigin,
) -> Result<Finding, GroundingError> {
    let cited = raw.cited_evidence();
    if cited.is_empty() {
        return Err(GroundingError::UngroundedClaim);
    }
    for id in &cited {
        // Was this evidence actually issued, in THIS session, before this finding?
        session.evidence.verify_issued(id, raw.timestamp)?;
    }
    let confidence = match origin {
        AgentOrigin::Native { .. } => raw.confidence.cap(Confidence::Probable),
        // An external agent's loop is not ours to audit.
        AgentOrigin::Acp { .. }    => raw.confidence.cap(Confidence::Probable),
    };
    Ok(Finding { confidence, provenance: origin.into(), .. })
}
```

`verify_issued` is the load-bearing check. An external agent that invents an evidence id, or
cites one from a different session, or cites one issued *after* the claim, is rejected. It
cannot fabricate a citation because it never sees ids it was not given.

### 5.3 Structured output from an unstructured channel

Mode B agents emit prose. Lodestone needs findings. Two mechanisms, used together:

1. **A reporting tool.** `submit_finding` is in the registry like any other tool, with a
   schema requiring `cited_evidence: [EvidenceId]`. Agents that support tool calling — all of
   the target ones do — will use it, and the schema does the enforcement.
2. **Prose fallback.** Agent text that is not a `submit_finding` call still appears in the
   transcript, clearly marked as unstructured commentary, and never becomes a `Finding`. It is
   readable, it is not actionable, and the UI does not pretend otherwise.

Never parse prose into findings. A regex over agent output that produces a `Finding` is a
grounding hole with a friendly face.

### 5.4 Honest labelling

The UI distinguishes three provenances, not two:

- **Measured** — deterministic tool output
- **Inferred (native)** — Lodestone's own loop, hypothesis-gated, budget-bounded
- **Inferred (external agent)** — an ACP agent's conclusion, evidence-validated but
  loop-unaudited

A user should be able to tell at a glance that a finding came from Codex rather than from
Lodestone's own reasoning, because the accountability differs.

---

## 6. Mode A — provider abstraction

### 6.1 Two shapes cover five providers

```rust
#[async_trait]
pub trait ModelBackend: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> Capabilities;
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse>;
}

pub struct Capabilities {
    pub context_window: usize,
    pub max_output: usize,
    pub tool_calling: ToolCallingSupport,   // Native | JsonMode | None
    pub parallel_tool_calls: bool,
    pub reasoning_effort: bool,
    pub vision: bool,
    pub cost_per_mtok: Option<(f64, f64)>,  // (input, output), for the budget display
}
```

The useful simplification: **OpenAI, DeepSeek, Moonshot (Kimi), and Z.ai (GLM) all expose
OpenAI-compatible chat-completions endpoints.** So the implementation count is two, not five:

| Implementation | Providers |
|---|---|
| `AnthropicBackend` | Claude |
| `OpenAiCompatibleBackend` | OpenAI, DeepSeek, Kimi, Z.ai, plus local runners (Ollama, llama.cpp, vLLM) |
| `NullBackend` | `--no-ai` equivalent; used by the entire test suite |

Providers become a data table rather than code:

```rust
pub struct ProviderSpec {
    pub id: &'static str,           // "deepseek"
    pub display: &'static str,      // "DeepSeek"
    pub shape: ApiShape,            // Anthropic | OpenAiCompatible
    pub default_base_url: &'static str,
    pub key_env: &'static str,      // "DEEPSEEK_API_KEY"
    pub models: &'static [ModelSpec],
    pub quirks: Quirks,
}
```

Adding a provider later is a table entry plus a quirks record. That is what "we can add more
later" should cost.

### 6.2 Quirks are real

Do not assume OpenAI-compatible means OpenAI-identical. The quirks table exists because these
differences are load-bearing for an agent loop:

- **Tool-calling reliability varies widely.** Some compatible endpoints support the schema but
  emit malformed arguments under pressure. Validate every tool call against its JSON Schema
  and retry once with the validation error fed back before failing the step.
- **Parallel tool calls** are not universally supported. If absent, the loop must serialize.
- **Reasoning models** often restrict temperature, system prompts, or interleaving of tool
  calls with reasoning content. Encode the constraints rather than discovering them in
  production.
- **Context windows differ by an order of magnitude.** The context strategy (§7) must read
  `capabilities().context_window` rather than assuming a number.
- **Local models** are the weakest and the most privacy-preserving. Prompts are authored
  against these, not against frontier models (see §7.3).

### 6.3 Credentials

- Keys go to the **OS keyring**, never to a config file inside the repository.
- Environment variables are read as a fallback, since that is how CI and many developers
  already work.
- The provider picker shows which keys are present without displaying them.
- A project-level `lodestone.toml` may set `allow_cloud_models = false`, and when it does, the
  cloud providers are not merely hidden but unselectable, with the reason shown.

---

## 7. Context strategy

The rule from `DESIGN.md` §5.4 stands and gets sharper: **slice by data dependency, never by
address range.** A disassembled function is enormous and a binary is unthinkably larger.

### 7.1 What the model sees

Never raw hex by default. In order of preference:

1. **Structured facts** — a typed struct rendered as compact JSON. "This generic has 12
   instantiations totalling 340 KB, here are the type arguments."
2. **Annotated disassembly** — instructions interleaved with source lines from the line table,
   call targets symbolized, RIP-relative operands resolved to their referents.
3. **Backward slices** — the output of `TOOLING-BINARY.md` §5.3, which is typically a dozen
   instructions rather than a function.
4. **Interpreted memory** — a 256-byte cap, with a type-informed reading attempt from DWARF
   layout rather than a hex dump.
5. **Raw hex** — only on explicit request, only for a bounded range.

Every tool output carries `truncated: bool` and a narrowing hint. Silent truncation is how
agents become confidently wrong.

### 7.2 Mode B has no context strategy

This deserves stating plainly: in ACP mode, **the external agent manages its own context**, and
Lodestone's careful slicing applies only to what individual tools return. Claude Code decides
what to keep and what to compact. That is a real loss of control and a real gain in
capability — those agents have sophisticated context management you would otherwise have to
build.

The design response is to make tool outputs *self-contained and compact*, so that whatever the
agent's compaction strategy, individual results remain interpretable in isolation. A tool
result that only makes sense alongside three earlier results will degrade badly under
compaction.

### 7.3 Prompt authoring

Author against the **weakest supported local model**, not against a frontier model. If a
prompt needs a frontier model to work, the feature is scoped wrong. A local model failing
gracefully is acceptable; a local model producing confident garbage is not.

Prompts are code. They live in the repository as files, are versioned, and every change runs
the eval suite (§10).

---

## 8. The loop (Mode A)

```rust
pub struct AgentConfig {
    pub max_steps: u32,             // default 25
    pub max_tokens: u64,
    pub max_wall_time: Duration,
    pub max_cost: Option<f64>,      // stop before an unexpected bill
    pub require_hypothesis: bool,   // default true
}
```

Each step:

1. **Hypothesis required.** The model states what it believes and what would refute it before
   it may call a tool. A response without a hypothesis block is rejected and retried once,
   then the step aborts. This is the single most effective control against aimless tool-call
   wandering, which is the dominant failure mode of agentic debugging.
2. **Tool selection**, with the remaining step and cost budget visible to the model.
3. **Invoke**, validate arguments against the schema, record evidence, append to transcript.
4. **Revise or conclude.**

On budget exhaustion the agent emits its best current hypothesis **explicitly marked
`Speculative`**, plus what it would investigate next. A partial answer honestly labelled is
useful; a fabricated confident answer is worse than nothing.

---

## 9. GUI integration

The Agent panel is a first-class dock panel, not a sidebar afterthought. It is the primary
trust surface in a GUI-only product.

### 9.1 Session control

A single picker at the top of the panel spans both modes, because from the user's point of
view they are choosing "who is going to think about this":

```
Reasoner:  [ Claude Code (ACP)        ▾ ]
           ─────────────────────────────
            Native · Claude
            Native · GPT
            Native · DeepSeek
            Native · Kimi
            Native · GLM
            Native · local/qwen
           ─────────────────────────────
            ACP · Claude Code          ✓ installed
            ACP · Codex CLI            ✓ installed
            ACP · Gemini CLI           ⬇ install
           ─────────────────────────────
            None (deterministic only)
```

"None" is a first-class choice, listed alongside the others rather than hidden in settings.

### 9.2 Transcript

ACP's `session/update` notifications map directly onto transcript rendering: message chunks,
thought chunks, tool calls, tool call updates, and plans. Mode A's loop emits the same internal
event shape, so **one transcript view renders both modes**. Design the internal event enum to
mirror ACP's, and Mode A becomes the easy case rather than the special case.

Each entry is a card: hypothesis (Mode A only), tool call with arguments, result summary with a
link to the full evidence, and any revision. Collapsible, exportable, replayable.

### 9.3 Permissions

ACP's `session/request_permission` maps onto the trust ladder directly, which is a pleasing
alignment rather than a coincidence — both exist to answer "may this agent do this?"

| Trust tier | Response to a permission request |
|---|---|
| Observe | Auto-deny anything beyond read-only; the tool is not exposed at all |
| Propose | Auto-allow read-only; prompt for anything that writes |
| Tune | Auto-allow build-config changes; prompt for source edits |
| Autonomous | Auto-allow within the workspace; still gated by the verifier |

**The verifier is downstream of every permission grant.** An agent granted permission to edit
source still cannot land a change that fails to build, breaks tests, regresses a benchmark, or
trips a sanitizer. Permission gets you to the verifier; it does not get you past it.

Agents also request filesystem and terminal access through the client. Scope these to the
project directory and reject paths outside it. An external agent asking to read
`~/.ssh/id_rsa` should get a refusal and a visible warning, not a permission prompt.

### 9.4 Cost and egress visibility

The panel shows, at all times: which reasoner is active, whether data leaves the machine, and
for Mode A the accumulated token cost of the session. For ACP agents, note that billing is the
agent's own — Claude Code and Codex own their authentication and billing independently of any
key configured in Lodestone.

---

## 10. Evaluation

Two modes means twice the evaluation surface, and the eval harness must drive both headlessly.

```
evals/
├── fixtures/         crashes, size, perf, replay (unchanged)
└── harness/
    ├── native.rs     drives Mode A across providers
    └── acp.rs        spawns real ACP agents, drives Mode B
```

Metrics, tracked per release, **per reasoner**:

| Metric | Notes |
|---|---|
| Top-1 / Top-3 accuracy | Against ground truth |
| Steps to answer | Mode A: loop steps. Mode B: tool calls observed at the MCP server. |
| Grounding rate | Must be 1.0 by construction; a value below 1.0 is a bug in the gate, not a model failure |
| **Rejection rate** | **Mode B only.** How often the finding gate rejects an agent's output. A high rate means the tool descriptions are unclear. |
| Confident-wrong rate | The trust metric |
| Cost | Mode A only; ACP agents bill separately |

The rejection rate is the metric to watch during ACP development. Agents rejecting frequently
is a signal that `description()` strings are failing to communicate, not that the agent is
bad — and it is a signal you would otherwise never see.

**Run evals on every prompt change and every tool-description change.** Both are code.

---

## 11. Security

The AI layer is the largest attack surface in the product.

**Prompt injection through analyzed content.** Symbol names, panic strings, source comments,
and file paths all end up in prompts, and all can contain adversarial text. A crate could ship
a symbol named to look like an instruction. Treat every analyzed artifact as untrusted data:
fence it explicitly in prompts, never interpolate it into instruction position, and never let
a model-proposed action execute without passing the verifier. **The verifier is the security
boundary, not just the quality boundary.**

**External agents are subprocesses with capabilities.** An ACP agent can request file reads,
file writes, and terminal commands through the client. Lodestone is the client and therefore
the enforcement point. Scope filesystem access to the project root, log every terminal request,
and require explicit approval for anything outside the workspace regardless of trust tier.

**Data egress.** Cloud providers see symbol names, source snippets, and disassembly — which is
to say, proprietary code. Record per session exactly what left the machine and to whom. Make
`allow_cloud_models = false` enforceable at the project level so an organisation decides once
rather than per developer.

**Supply chain.** ACP adapters are typically npm packages, which is a dependency the user
installs and Lodestone executes. Show the resolved command before first launch, pin versions
where the registry allows it, and never auto-update an agent binary silently.

**Keys.** OS keyring. Never in the repository, never in the session export, never in a log.
Redact on export by default.

---

## 12. Build order

The AI layer follows the phase plan and never leads it.

| Phase | AI work |
|---|---|
| 0 | `NullBackend` only. The tool registry and evidence store exist; nothing calls a model. Prove the deterministic core first. |
| 1 | **Mode A**, one provider (`OpenAiCompatibleBackend` against a local model), one analysis. Finding gate live. Eval harness live. |
| 1.5 | Remaining providers as table entries. Cheap once the two shapes exist. |
| 2 | **Mode B**: ACP client, `lode-mcp` server, Claude Code and Codex. Permission mapping to trust tiers. |
| 3 | Perf-specific tools; transcript replay; cost budgeting. |
| 4 | Replay-search tools; the step-budgeted root-cause search over an `rr` trace. |

**Mode A before Mode B**, deliberately. Building the native loop first forces the tool registry
and the evidence store to be correct while you still control both ends. ACP then becomes a
second exposure of a working system rather than an attempt to design the trust boundary and
the protocol integration simultaneously.

---

## 13. Open questions

1. **Does `session/new`'s MCP configuration support HTTP servers across all target agents**,
   or is a stdio bridge required for some? Determines whether `lode-mcp-bridge` is needed.
   Test against Claude Code and Codex early in Phase 2.
2. **How do ACP agents behave with a large, domain-specific tool set** they have never seen?
   Their training is oriented toward file editing and shell commands. The rejection-rate metric
   (§10) is designed to answer this empirically.
3. **What is the weakest local model that produces useful monomorphization proposals?** Sets
   the hardware floor. Answer in Phase 1, empirically, not by guessing.
4. **Should Lodestone itself ship as an ACP agent**, so that Zed or JetBrains could drive
   Lodestone's analysis from their own agent panels? A genuinely interesting inversion, and
   plausibly a better distribution strategy than a standalone GUI. Not v1 — but the tool
   registry design keeps the door open, and it is worth not closing accidentally.
5. **Protocol version churn.** ACP is young and versioned; pin, negotiate at `initialize`, and
   decide the policy for agents reporting an unsupported version.

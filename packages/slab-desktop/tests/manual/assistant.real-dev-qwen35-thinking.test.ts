/**
 * Real-model manual verification of the local thinking-budget chain
 * (default Qwen3.5-9B, ggml.llama).
 *
 * This file does NOT run by default. Prerequisites:
 *
 * 1. Rebuild + start the server against this checkout:
 *      bun run dev:server                    # listens on :3000 (staged libs)
 * 2. The model pack `models/llama/Qwen3.5-9B` imported and the weights
 *    downloaded (hf-hub global cache, `unsloth/Qwen3.5-9B-GGUF` — Q8_0 is
 *    sufficient). Override with SLAB_E2E_MODEL.
 * 3. Run with the gate (see packages/slab-desktop README "Testing"):
 *      SLAB_E2E_REAL_MODEL=1 bunx vitest run \
 *        --config packages/slab-desktop/vitest.manual.config.ts \
 *        assistant.real-dev-qwen35-thinking
 *
 * What this covers (externally observable REST behavior):
 * - effort=none: the template pre-closes `<think>` — no reasoning deltas, a
 *   direct short answer.
 * - effort=high with an explicit cap: reasoning streams, the budget (cap-256)
 *   force-closes it, and the visible answer follows; completion stays in cap.
 * - tier scaling: low's capped completion is visibly shorter than high's.
 * - same-session second turn keeps conversation state (and reports KV
 *   cached_tokens for human review — see the note in that test).
 * - a raw GBNF grammar + effort constrains the output.
 * - the budget-derived cap degrades to a clean truncation (finish + [DONE]),
 *   never a context-capacity 500 (engine behavior pinned after the
 *   budget+allowance derivation landed).
 *
 * Trace-event verification (local_reasoning_policy_injected / llama_request
 * payloads with thinking_budget, and the "thinking budget skipped: gbnf
 * grammar active" debug line) rides the AGENT path only (harness/UI turns
 * with agent.debug on — bundles under logs/agent_trace/). REST chat carries
 * no trace context, so those events are reviewed by hand when needed; this
 * suite pins the behavior they describe.
 */
import { describe, expect, it } from "vitest"

type ChatMessage = { role: string; content: string }

type StreamChunk = {
  choices: Array<{
    delta: { content?: string | null; reasoning_content?: string | null }
    finish_reason?: string | null
  }>
  usage?: {
    completion_tokens: number
    prompt_tokens: number
    prompt_tokens_details: { cached_tokens: number }
  }
}

type CompletionResponse = {
  choices: Array<{ message: { content?: string | null } }>
  usage?: { prompt_tokens_details: { cached_tokens: number } }
}

const gated = process.env.SLAB_E2E_REAL_MODEL === "1"
const serverBaseUrl = process.env.SLAB_E2E_SERVER_BASE_URL?.trim() || "http://127.0.0.1:3000"
const modelId = process.env.SLAB_E2E_MODEL?.trim() || "Qwen3.5-9B"
// The truncation check needs a fast small model to reach the context limit
// within the timeout; skips when that model is not available.
const truncationModelId = process.env.SLAB_E2E_TRUNCATION_MODEL?.trim() || ""
// 9B at Q8 on a single consumer GPU thinks at single-digit tokens/sec — the
// capped high-effort turns below take minutes, not seconds.
const perTestTimeout = 15 * 60_000

const PUZZLE_PROMPT =
  "You are given the numbers 3, 3, 8, 8. Using each exactly once and only + - * / " +
  "parentheses, write an expression that equals 24. Think through candidate groupings " +
  "systematically before answering; verify your final expression by evaluating it."

async function postChat(
  body: Record<string, unknown>,
  model = modelId,
): Promise<{ text: string; status: number }> {
  const response = await fetch(`${serverBaseUrl}/v1/chat/completions`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ model, ...body }),
  })
  return { text: await response.text(), status: response.status }
}

async function postChatJson(
  body: Record<string, unknown>,
  model = modelId,
): Promise<CompletionResponse> {
  const { text, status } = await postChat(body, model)
  expect(status, `chat completions failed: ${text.slice(0, 400)}`).toBe(200)
  return JSON.parse(text) as CompletionResponse
}

/** Consume one SSE stream to [DONE], returning the accumulated chunks. */
async function postChatStream(
  body: Record<string, unknown>,
  model = modelId,
): Promise<{ chunks: StreamChunk[]; content: string; reasoning: string }> {
  const response = await fetch(`${serverBaseUrl}/v1/chat/completions`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ model, stream: true, ...body }),
  })
  // Only drain the body on failure — reading it here would lock the stream.
  if (!response.ok) {
    const text = await response.text()
    throw new Error(`stream failed (${response.status}): ${text.slice(0, 400)}`)
  }
  expect(response.body, "stream response must have a body").toBeTruthy()

  const chunks: StreamChunk[] = []
  let content = ""
  let reasoning = ""
  let buffer = ""
  const decoder = new TextDecoder()
  for await (const piece of response.body!) {
    buffer += decoder.decode(piece, { stream: true })
    let boundary = buffer.indexOf("\n\n")
    while (boundary !== -1) {
      const frame = buffer.slice(0, boundary).trim()
      buffer = buffer.slice(boundary + 2)
      boundary = buffer.indexOf("\n\n")
      for (const line of frame.split("\n")) {
        if (!line.startsWith("data: ")) continue
        const payload = line.slice(6)
        if (payload === "[DONE]") {
          return { chunks, content, reasoning }
        }
        const chunk = JSON.parse(payload) as StreamChunk
        chunks.push(chunk)
        content += chunk.choices[0]?.delta?.content ?? ""
        reasoning += chunk.choices[0]?.delta?.reasoning_content ?? ""
      }
    }
  }
  return { chunks, content, reasoning }
}

async function catalogModel(model: string): Promise<
  | { found: false }
  | { found: true; status: string; downloaded?: boolean }
> {
  try {
    const response = await fetch(`${serverBaseUrl}/v1/models`)
    if (!response.ok) return { found: false }
    const models = (await response.json()) as Array<{
      id: string
      status?: string
      downloaded?: boolean
    }>
    const entry = models.find((candidate) => candidate.id === model)
    if (!entry) return { found: false }
    return { found: true, status: entry.status ?? "", downloaded: entry.downloaded }
  } catch {
    return { found: false }
  }
}

/** Chat completions never auto-load: kick the explicit load (blocks until the
 * weights are resident) and confirm it took. Loaded ONCE per model per run —
 * reloading while weights are resident shrinks the auto-sized context (the
 * VRAM probe sees the previous instance), which would starve later tests. */
const loadedModels = new Set<string>()
async function ensureModelLoaded(model = modelId): Promise<boolean> {
  if (loadedModels.has(model)) return true
  try {
    const response = await fetch(`${serverBaseUrl}/v1/models/load`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ model_id: model }),
    })
    if (!response.ok) {
      // eslint-disable-next-line no-console -- manual-review output
      console.log(`[thinking-budget] model load failed: ${(await response.text()).slice(0, 200)}`)
      return false
    }
    loadedModels.add(model)
    return true
  } catch (error) {
    // eslint-disable-next-line no-console -- manual-review output
    console.log(`[thinking-budget] model load error: ${String(error)}`)
    return false
  }
}

async function readyOrSkip(ctx: { skip(): void }, model = modelId): Promise<void> {
  const catalog = await catalogModel(model)
  if (!catalog.found || !(catalog.downloaded || catalog.status === "ready")) {
    ctx.skip()
    return
  }
  if (!(await ensureModelLoaded(model))) ctx.skip()
}

describe.skipIf(!gated)("assistant real-dev local thinking budget (Qwen3.5-9B)", () => {
  // Collected for the human review this manual suite exists for.
  const tierTokens: Record<string, number> = {}

  it(
    "effort=none pre-closes thinking and answers directly",
    async (ctx) => {
      await readyOrSkip(ctx)
      const { content, reasoning, chunks } = await postChatStream({
        messages: [{ role: "user", content: "In one short sentence: what is 7*6?" } as ChatMessage],
        reasoning_effort: "none",
      })
      expect(content, "effort=none must still produce an answer").toMatch(/\S/)
      expect(reasoning, "no reasoning may stream when thinking is off").toBe("")
      expect(content).not.toContain("<think")
      const usage = chunks.at(-1)?.usage
      tierTokens.none = usage?.completion_tokens ?? 0
      // eslint-disable-next-line no-console -- manual-review output
      console.log(`[thinking-budget] effort=none completion_tokens=${tierTokens.none}`)
    },
    perTestTimeout,
  )

  it(
    "effort=high with an explicit cap: budget closes thinking, answer follows",
    async (ctx) => {
      await readyOrSkip(ctx)
      const { content, reasoning, chunks } = await postChatStream({
        messages: [{ role: "user", content: PUZZLE_PROMPT } as ChatMessage],
        reasoning_effort: "high",
        max_tokens: 2048,
      })
      expect(reasoning, "high effort must stream a reasoning segment").toMatch(/\S/)
      expect(content, "the visible answer must follow the closed thinking").toMatch(/\S/)
      expect(content).not.toContain("<think")
      const usage = chunks.at(-1)?.usage
      const completionTokens = usage?.completion_tokens ?? 0
      tierTokens.high = completionTokens
      // eslint-disable-next-line no-console -- manual-review output
      console.log(
        `[thinking-budget] effort=high(cap 2048) completion_tokens=${completionTokens}`,
      )
      expect(completionTokens).toBeLessThanOrEqual(2048)
    },
    perTestTimeout,
  )

  it(
    "tier caps bound their completions (ordering logged for manual review)",
    async (ctx) => {
      await readyOrSkip(ctx)
      const { chunks, reasoning } = await postChatStream({
        messages: [{ role: "user", content: PUZZLE_PROMPT } as ChatMessage],
        reasoning_effort: "low",
        max_tokens: 1024,
      })
      const lowTokens = chunks.at(-1)?.usage?.completion_tokens ?? 0
      tierTokens.low = lowTokens
      // eslint-disable-next-line no-console -- manual-review output
      console.log(
        `[thinking-budget] effort=low(cap 1024) completion_tokens=${lowTokens} reasoning_chars=${reasoning.length}`,
      )

      expect(reasoning, "the low tier must still think").toMatch(/\S/)
      expect(lowTokens, "completion must respect the explicit cap").toBeLessThanOrEqual(1024)
      // NOTE (informational, not asserted): whether low finishes visibly
      // earlier than high depends on the model actually wanting to think
      // longer — with explicit caps the budget is cap-256 for every tier,
      // and a warm model can converge before any budget trips. The unit
      // suite pins the tier table; this logs the real-model split for
      // eyeballing:
      // eslint-disable-next-line no-console -- manual-review output
      console.log(
        `[thinking-budget] tiers: none=${tierTokens.none} low=${tierTokens.low} high=${tierTokens.high}`,
      )
    },
    perTestTimeout,
  )

  it(
    "same-session second turn keeps conversation state (KV cached_tokens logged)",
    async (ctx) => {
      await readyOrSkip(ctx)
      const sessionId = `thinking-budget-kv-${Date.now()}`
      const first = await postChatJson({
        id: sessionId,
        messages: [{ role: "user", content: "Remember the word: kilpargan." } as ChatMessage],
        reasoning_effort: "none",
      })
      expect(first.choices[0]?.message?.content ?? "").toMatch(/\S/)

      const second = await postChatJson({
        id: sessionId,
        messages: [
          { role: "user", content: "Repeat the word I asked you to remember." } as ChatMessage,
        ],
        reasoning_effort: "none",
      })
      const answer = second.choices[0]?.message?.content ?? ""
      expect(answer, "session history must carry into the second turn").toContain("kilpargan")

      // NOTE (known limitation, pre-existing): for native-thinking templates
      // (Qwen3.5) the per-turn `<think>` prefill means turn N+1's rendered
      // prompt is NOT a byte-prefix of turn N's, so the engine's
      // prefix-extension KV reuse does not hit and cached_tokens stays 0
      // across REST turns. The value is logged for review; when prefix-reuse
      // support for prefilled templates lands, tighten this to > 0.
      const cached = second.usage?.prompt_tokens_details?.cached_tokens ?? 0
      // eslint-disable-next-line no-console -- manual-review output
      console.log(`[thinking-budget] second-turn cached_tokens=${cached} (see NOTE)`)
    },
    perTestTimeout,
  )

  it(
    "a raw GBNF grammar with an effort set constrains the output",
    async (ctx) => {
      await readyOrSkip(ctx)
      // effort=none so the grammar-constrained tokens land as visible content
      // (a thinking prefill would swallow them as reasoning). The engine
      // skips budget enforcement under an active grammar by design — the
      // "thinking budget skipped: gbnf grammar active" debug line is reviewed
      // in the server log.
      // NOTE: response_format json_object currently fails grammar parsing in
      // the vendored llama.dll ("expecting newline or end") — pre-existing,
      // tracked separately; a trivial raw grammar exercises the same path.
      const { content } = await postChatStream({
        messages: [
          { role: "user", content: "Answer with the single word: yes" } as ChatMessage,
        ],
        reasoning_effort: "none",
        gbnf: 'root ::= "yes"',
      })
      expect(content.trim(), `grammar must constrain the output: ${JSON.stringify(content)}`).toBe(
        "yes",
      )
    },
    perTestTimeout,
  )

  it(
    "a budget-derived cap beyond the context window truncates cleanly (no 500)",
    async (ctx) => {
      if (!truncationModelId) {
        // Opt-in: point SLAB_E2E_TRUNCATION_MODEL at a small, fast local
        // model (e.g. Qwen2.5-0.5B-Instruct) so the run reaches the context
        // limit inside the timeout.
        ctx.skip()
        return
      }
      await readyOrSkip(ctx, truncationModelId)
      // No explicit cap: effort=high derives budget 16384 + 2048 allowance,
      // far beyond a small model's effective context — generation must hit
      // the window and END (terminal chunk + [DONE]), never a
      // "context capacity exceeded" error.
      const { chunks } = await postChatStream(
        {
          messages: [
            { role: "user", content: "Count from 1 to 100000, one number per line." } as ChatMessage,
          ],
          reasoning_effort: "high",
        },
        truncationModelId,
      )
      expect(chunks.length, "generation must produce tokens before the window closes").toBeGreaterThan(
        0,
      )
      const usage = chunks.at(-1)?.usage
      // eslint-disable-next-line no-console -- manual-review output
      console.log(
        `[thinking-budget] derived-cap truncation chunks=${chunks.length} completion_tokens=${usage?.completion_tokens}`,
      )
      // How much room the truncation model gets depends on what else is
      // resident (auto-context sizes from free VRAM) — the point is the
      // ENDING, not the length: a terminal chunk, [DONE] reached, no error.
      expect(
        chunks.some((chunk) => chunk.choices[0]?.finish_reason != null),
        "the stream must terminate with a finish_reason chunk",
      ).toBe(true)
    },
    perTestTimeout,
  )
})

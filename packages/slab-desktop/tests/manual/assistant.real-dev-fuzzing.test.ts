import { mkdir, readFile, rm, writeFile } from "node:fs/promises"
import { dirname, join, normalize } from "node:path"
import { setTimeout as delay } from "node:timers/promises"
import { TextDecoder } from "node:util"

import { afterAll, beforeAll, describe, expect, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Locator, type Page } from "playwright"

import type { components } from "@slab/api/v1"

type Schema = components["schemas"]
type AgentConfigInput = Schema["AgentConfigInput"]
type AgentResponsesServerMessage = Schema["AgentResponsesServerMessage"]
type AgentAck = Extract<AgentResponsesServerMessage, { type: "agent.ack" }>
type AgentSessionRestored = Extract<
  AgentResponsesServerMessage,
  { type: "agent.session.restored" }
>
type AgentThreadMessageResponse = Schema["AgentThreadMessageResponse"]
type ChatToolCall = Schema["ChatToolCall"]
type SessionResponse = Schema["SessionResponse"]
type SetupStatusResponse = Schema["SetupStatusResponse"]
type SystemDiagnosticsResponse = Schema["SystemDiagnosticsResponse"]
type UiStateValueResponse = Schema["UiStateValueResponse"]
type UnifiedModelResponse = Schema["UnifiedModelResponse"]

type JsonRequestInit = Omit<RequestInit, "body"> & {
  json?: unknown
}

type PersistedHeaderUiState = {
  selections?: Record<string, string>
}

type PersistedAssistantUiState = {
  currentSessionId?: string
  deepThink?: boolean
  sessionLabels?: Record<string, string>
}

type UiStateBackup = {
  key: string
  value: string | null
}

type CompletedAssistantReply = {
  restore: AgentSessionRestored
  text: string
}

type ToolName =
  | "apply_patch"
  | "code_lsp_status"
  | "delegate_subagent"
  | "file_glob"
  | "fs_watch"
  | "grep"
  | "list_dir"
  | "plan_update"
  | "read_file"
  | "shell"
  | "web_search"
  | "write_file"

type ToolExecutionResult = {
  finalText: string
  restore: AgentSessionRestored
  toolCalls: ChatToolCall[]
  toolMessages: AgentThreadMessageResponse[]
}

type ToolFuzzCase = {
  name: ToolName
  args: Record<string, unknown>
  approve?: boolean
  timeoutMs?: number
  validate: (result: ToolExecutionResult) => Promise<void> | void
}

type AgentStreamEvent = {
  call_id?: string
  response?: {
    status?: string
  }
  thread_id: string
  tool_name?: string
  type: string
}

type ApprovalResponder = {
  done: Promise<void>
  stop: () => void
}

const serverBaseUrl = process.env.SLAB_E2E_SERVER_BASE_URL?.trim() || "http://127.0.0.1:3000"
const uiBaseUrl = process.env.SLAB_E2E_UI_BASE_URL?.trim() || "http://localhost:1420"
const assistantModelId = "Qwen3.5-9B"
const assistantUiStateKey = "zustand:assistant-ui"
const headerUiStateKey = "zustand:header-ui"
const composerPlaceholder = /^(Type a message or drop files\.\.\.|\u8f93\u5165\u6d88\u606f\u6216\u62d6\u5165\u6587\u4ef6\.\.\.)$/u
const createSessionButtonName = /^(Create session|\u521b\u5efa\u4f1a\u8bdd)$/u
const deleteMenuItemName = /^(Delete|\u5220\u9664)$/u
const emptySessionTitles = [
  "Start a new thread and keep the stage focused.",
  "\u5f00\u59cb\u4e00\u4e2a\u65b0\u7684\u52a9\u624b\u7ebf\u7a0b\uff0c\u8ba9\u4e0a\u4e0b\u6587\u4fdd\u6301\u805a\u7126\u3002",
]
const manageSessionsButtonName = /^(Manage sessions|\u7ba1\u7406\u4f1a\u8bdd)$/u
const sendButtonName = /^(Send message|\u53d1\u9001\u6d88\u606f)$/u

const uiStateBackups: UiStateBackup[] = []
const testSessionIds = new Set<string>()
let browser: Browser | undefined
let context: BrowserContext | undefined
let page: Page
let marker = ""
let primarySession: SessionResponse
let workspaceRootPath = ""
let workspaceMarkerRoot = ""

describe.sequential("assistant real-dev e2e fuzzing", () => {
  beforeAll(async () => {
    marker = `slab-e2e-fuzzing-${Date.now()}`
    workspaceMarkerRoot = `.slab-e2e-fuzzing/${marker}`
    workspaceRootPath = await preflightRealDevEnvironment()
    await backupUiState([headerUiStateKey, assistantUiStateKey])
    primarySession = await createMarkedSession(`${marker} primary`)
    await selectAssistantSession(primarySession.id, primarySession.name)

    browser = await chromium.launch({ headless: true })
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
    })
    await context.addInitScript(() => {
      window.localStorage.setItem("slab.ui.language", "en-US")
    })
    page = await context.newPage()
    await page.goto(uiBaseUrl, { waitUntil: "domcontentloaded", timeout: 60_000 })
    await waitForComposerReady(page)
  })

  afterAll(async () => {
    await context?.close().catch(() => {})
    await browser?.close().catch(() => {})
    await cleanupMarkedSessions().catch((error) => {
      console.warn(`Failed to clean up assistant fuzzing sessions: ${String(error)}`)
    })
    await cleanupWorkspaceMarkerRoot().catch((error) => {
      console.warn(`Failed to clean up assistant fuzzing workspace files: ${String(error)}`)
    })
    await restoreUiState().catch((error) => {
      console.warn(`Failed to restore assistant fuzzing UI state: ${String(error)}`)
    })
  })

  it("drives real assistant replies, tool loops, persistence, and session management through the UI", async () => {
    const normalPrompt = `${marker} normal reply: answer with one short sentence.`
    await sendAssistantMessage(page, normalPrompt)
    await expectPageText(page, normalPrompt)

    const normalReply = await waitForCompletedAssistantReply(primarySession.id, normalPrompt)
    expect(nonBlank(normalReply.text)).toBe(true)
    await expectPageText(page, visibleNeedle(normalReply.text))

    const loopPrompt =
      `${marker} tool loop: use the plan_update tool once with a concise two-step plan, ` +
      "then continue and provide a final non-empty answer."
    await sendAssistantMessage(page, loopPrompt)
    await expectPageText(page, loopPrompt)

    const loopRestore = await waitForPlanUpdateLoop(primarySession.id, loopPrompt)
    const finalReply = latestAssistantTextAfter(loopRestore.messages, loopPrompt)
    expect(nonBlank(finalReply)).toBe(true)
    await expectPageText(page, visibleNeedle(finalReply))
    await expectPageText(page, "tool_call")

    await page.reload({ waitUntil: "domcontentloaded", timeout: 60_000 })
    await waitForComposerReady(page)
    await expectPageText(page, normalPrompt)
    await expectPageText(page, loopPrompt)
    await expectPageText(page, visibleNeedle(finalReply))

    await page.getByRole("button", { name: createSessionButtonName }).click()
    const createdSessionId = await waitForCurrentAssistantSession((sessionId) => {
      return sessionId !== primarySession.id
    })
    const uiCreatedSession = await renameSession(createdSessionId, `${marker} ui-created`)
    testSessionIds.add(uiCreatedSession.id)
    await expectPageTextAny(page, emptySessionTitles)

    const currentUiState = await getPersistedUiState<PersistedAssistantUiState>(assistantUiStateKey)
    expect(currentUiState?.currentSessionId).toBe(uiCreatedSession.id)

    await deleteSessionFromSheet(page, primarySession)
    await eventually("deleted primary fuzzing session disappears", async () => {
      const sessions = await listSessions()
      return sessions.every((session) => session.id !== primarySession.id)
    })
    expect((await restoreSession(primarySession.id)).thread).toBeNull()
  })

  it("preserves multi-turn context and restores the same responses thread", async () => {
    const session = await createMarkedSession(`${marker} direct-context`)
    const codeword = `CTX${Date.now().toString(36).toUpperCase()}`
    const seedPrompt =
      `${marker} context seed. Remember this exact codeword: ${codeword}. ` +
      `Reply with exactly "stored ${codeword}".`
    const ack = await createAgentResponse(session.id, seedPrompt, {
      max_tokens: 160,
      max_turns: 2,
      reasoning_effort: "none",
      tool_choice: { type: "none" },
    })
    const firstReply = await waitForCompletedAssistantReply(session.id, seedPrompt)
    expect(firstReply.restore.thread?.id).toBe(ack.thread_id)
    expect(firstReply.text).toContain(codeword)

    const recallPrompt =
      `${marker} context recall. What exact codeword did I ask you to remember? ` +
      "Reply with only that codeword."
    await sendAgentInput(ack.thread_id, recallPrompt)
    const recallReply = await waitForCompletedAssistantReply(session.id, recallPrompt, 900_000)
    expect(recallReply.restore.thread?.id).toBe(ack.thread_id)
    expect(recallReply.text).toContain(codeword)

    const restored = await restoreSession(session.id)
    expect(restored.thread?.id).toBe(ack.thread_id)
    expect(restored.messages.some((message) => message.role === "user" && message.content === seedPrompt)).toBe(true)
    expect(restored.messages.some((message) => message.role === "user" && message.content === recallPrompt)).toBe(true)
  }, 1_200_000)

  it("honors thinking controls on and off through /v1/agents/responses", async () => {
    const offToken = `THINKOFF${Date.now().toString(36).toUpperCase()}`
    const offSession = await createMarkedSession(`${marker} thinking-off`)
    const offPrompt = `${marker} thinking off. Answer with exactly this token: ${offToken}.`
    await createAgentResponse(offSession.id, offPrompt, {
      max_tokens: 160,
      max_turns: 2,
      reasoning_effort: "none",
      tool_choice: { type: "none" },
    })
    const offReply = await waitForCompletedAssistantReply(offSession.id, offPrompt)
    expect(offReply.text).toContain(offToken)
    expect(offReply.text.toLowerCase()).not.toContain("<think")

    const onToken = `THINKON${Date.now().toString(36).toUpperCase()}`
    const onSession = await createMarkedSession(`${marker} thinking-on`)
    const onPrompt =
      `${marker} thinking on. Think in one short sentence, then answer with exactly this token: ${onToken}.`
    await createAgentResponse(onSession.id, onPrompt, {
      max_tokens: 1_200,
      max_turns: 2,
      reasoning_effort: "medium",
      tool_choice: { type: "none" },
    })
    const onReply = await waitForCompletedAssistantReply(onSession.id, onPrompt, 900_000)
    const thinking = extractDoneThinking(onReply.text)
    expect(nonBlank(thinking)).toBe(true)
    expect(answerWithoutThinking(onReply.text)).toContain(onToken)
  })

  it("executes each production default assistant tool and persists structured tool calls", async () => {
    await seedToolWorkspaceFiles()

    for (const toolCase of toolFuzzCases()) {
      // eslint-disable-next-line no-await-in-loop
      const session = await createMarkedSession(`${marker} tool-${toolCase.name}`)
      const finalMarker = `${marker} ${toolCase.name} final`
      const prompt = toolPrompt(toolCase.name, toolCase.args, finalMarker)
      // eslint-disable-next-line no-await-in-loop
      const ack = await createAgentResponse(session.id, prompt, toolAgentConfig(toolCase.name))
      const approvalResponder = toolCase.approve
        ? startApprovalResponder(ack.thread_id, toolCase.name)
        : null

      try {
        // eslint-disable-next-line no-await-in-loop
        const result = await waitForToolExecution(
          session.id,
          prompt,
          toolCase.name,
          toolCase.timeoutMs ?? 900_000
        )
        expect(result.finalText).toContain(finalMarker)
        // eslint-disable-next-line no-await-in-loop
        try {
          // eslint-disable-next-line no-await-in-loop
          await toolCase.validate(result)
        } catch (error) {
          throw new Error(
            `${toolCase.name} validation failed. ` +
              `tool_calls=${JSON.stringify(result.toolCalls)} ` +
              `tool_messages=${JSON.stringify(result.toolMessages.map((message) => message.content))}. ` +
              String(error), { cause: error }
          )
        }
      } finally {
        approvalResponder?.stop()
        // eslint-disable-next-line no-await-in-loop
        await approvalResponder?.done.catch(() => {})
      }
    }
  }, 3_600_000)
})

async function preflightRealDevEnvironment(): Promise<string> {
  const health = await fetch(`${serverBaseUrl}/health`).catch((error) => {
    throw new Error(
      `Cannot reach slab-server at ${serverBaseUrl}/health. Start dev with 'bun run dev:app'. ${String(error)}`
    )
  })
  if (!health.ok) {
    throw new Error(`slab-server health check failed with ${health.status}: ${await health.text()}`)
  }

  const setup = await requestJson<SetupStatusResponse>("/v1/setup/status")
  if (!setup.initialized) {
    throw new Error("slab-server setup is not initialized. Complete setup before running the manual real-dev fuzzing suite.")
  }

  const ui = await fetch(uiBaseUrl).catch((error) => {
    throw new Error(
      `Cannot reach the desktop dev UI at ${uiBaseUrl}. Start dev with 'bun run dev:app'. ${String(error)}`
    )
  })
  if (!ui.ok) {
    throw new Error(`Desktop dev UI check failed with ${ui.status}: ${await ui.text()}`)
  }

  const diagnostics = await requestJson<SystemDiagnosticsResponse>("/v1/system/diagnostics")
  const rootPath = workspaceRootFromDiagnostics(diagnostics)
  if (!rootPath) {
    throw new Error(
      "Cannot resolve the real-dev workspace root from /v1/system/diagnostics. " +
        "Assistant fuzzing needs a workspace root for file, shell, patch, watch, and LSP tools."
    )
  }

  const models = await requestJson<UnifiedModelResponse[]>("/v1/models?capability=chat_generation")
  const model = models.find((item) => item.id === assistantModelId)
  if (!model) {
    throw new Error(
      `Required assistant fuzzing model '${assistantModelId}' was not returned by /v1/models?capability=chat_generation.`
    )
  }

  if (!modelUsableWithoutDownload(model)) {
    throw new Error(
      `Required assistant fuzzing model '${assistantModelId}' is not ready. ` +
        `status=${model.status}, kind=${model.kind}. Import/download it before running the manual real-dev fuzzing suite.`
    )
  }
  if (!model.chat_capabilities?.reasoning_controls) {
    throw new Error(
      `Required assistant fuzzing model '${assistantModelId}' does not advertise reasoning_controls; ` +
        "the suite validates thinking on/off behavior through /v1/agents/responses."
    )
  }

  return rootPath
}

async function backupUiState(keys: string[]): Promise<void> {
  for (const key of keys) {
    // eslint-disable-next-line no-await-in-loop
    const value = await getRawUiStateValue(key)
    uiStateBackups.push({ key, value })
  }
}

async function selectAssistantSession(sessionId: string, sessionName: string): Promise<void> {
  const headerState = (await getPersistedUiState<PersistedHeaderUiState>(headerUiStateKey)) ?? {}
  await putPersistedUiState<PersistedHeaderUiState>(headerUiStateKey, {
    ...headerState,
    selections: {
      ...headerState.selections,
      "assistant:model": assistantModelId,
    },
  })

  const assistantState =
    (await getPersistedUiState<PersistedAssistantUiState>(assistantUiStateKey)) ?? {}
  await putPersistedUiState<PersistedAssistantUiState>(assistantUiStateKey, {
    ...assistantState,
    currentSessionId: sessionId,
    deepThink: false,
    sessionLabels: {
      ...assistantState.sessionLabels,
      [sessionId]: sessionName,
    },
  })
}

async function createMarkedSession(name: string): Promise<SessionResponse> {
  const session = await requestJson<SessionResponse>("/v1/sessions", {
    json: {},
    method: "POST",
  })
  testSessionIds.add(session.id)
  return renameSession(session.id, name)
}

async function renameSession(sessionId: string, name: string): Promise<SessionResponse> {
  return requestJson<SessionResponse>(`/v1/sessions/${encodeURIComponent(sessionId)}`, {
    json: { name } satisfies Schema["UpdateSessionRequest"],
    method: "PUT",
  })
}

async function sendAssistantMessage(targetPage: Page, message: string): Promise<void> {
  await waitForComposerReady(targetPage)
  await targetPage.getByPlaceholder(composerPlaceholder).fill(message)
  await targetPage.getByRole("button", { name: sendButtonName }).click()
}

async function waitForComposerReady(targetPage: Page): Promise<Locator> {
  const composer = targetPage.getByPlaceholder(composerPlaceholder)
  await composer.waitFor({ state: "visible", timeout: 90_000 })
  await eventually("assistant composer is editable", async () => composer.isEditable(), 90_000)
  return composer
}

async function expectPageText(targetPage: Page, text: string): Promise<void> {
  const expected = normalizeVisibleText(text)
  await targetPage.waitForFunction(
    (needle) => {
      const visible = document.body.innerText
        .replace(/[`*_#>[\]]/g, "")
        .replace(/\s+/g, " ")
        .trim()
      return visible.includes(needle)
    },
    expected,
    { timeout: 180_000 }
  )
}

async function expectPageTextAny(targetPage: Page, texts: string[]): Promise<void> {
  const expected = texts.map(normalizeVisibleText)
  await targetPage.waitForFunction(
    (needles) => {
      const visible = document.body.innerText
        .replace(/[`*_#>[\]]/g, "")
        .replace(/\s+/g, " ")
        .trim()
      return needles.some((needle) => visible.includes(needle))
    },
    expected,
    { timeout: 180_000 }
  )
}

async function createAgentResponse(
  sessionId: string,
  prompt: string,
  configOverrides: Partial<AgentConfigInput> = {}
): Promise<AgentAck & { thread_id: string }> {
  const response = await requestJson<AgentResponsesServerMessage>("/v1/agents/responses", {
    json: {
      config: {
        model: assistantModelId,
        temperature: 0,
        ...configOverrides,
      },
      messages: [{ content: prompt, role: "user" }],
      request_id: `create-${Date.now()}`,
      session_id: sessionId,
      type: "agent.response.create",
    } satisfies Schema["AgentResponsesClientMessage"],
    method: "POST",
  })

  if (response.type !== "agent.ack") {
    throw new Error(`Expected agent.ack, received ${response.type}`)
  }
  if (!response.accepted || !response.thread_id) {
    throw new Error(`Agent response create was not accepted: ${JSON.stringify(response)}`)
  }

  return response as AgentAck & { thread_id: string }
}

async function sendAgentInput(threadId: string, content: string): Promise<AgentAck> {
  const response = await requestJson<AgentResponsesServerMessage>("/v1/agents/responses", {
    json: {
      content,
      request_id: `input-${Date.now()}`,
      thread_id: threadId,
      type: "agent.input",
    } satisfies Schema["AgentResponsesClientMessage"],
    method: "POST",
  })

  if (response.type !== "agent.ack") {
    throw new Error(`Expected agent.ack for agent.input, received ${response.type}`)
  }
  if (!response.accepted) {
    throw new Error(`Agent input was not accepted: ${JSON.stringify(response)}`)
  }

  return response
}

async function waitForCompletedAssistantReply(
  sessionId: string,
  prompt: string,
  timeoutMs = 600_000
): Promise<CompletedAssistantReply> {
  return eventually(
    `completed assistant reply for '${prompt}'`,
    async () => {
      const restore = await restoreSession(sessionId)
      if (restore.thread?.status === "errored") {
        throw new Error(`Agent thread errored: ${restore.thread.completion_text ?? "unknown error"}`)
      }
      if (restore.thread?.status !== "completed") {
        return null
      }
      const text = latestAssistantTextAfter(restore.messages, prompt)
      return nonBlank(text) ? { restore, text } : null
    },
    timeoutMs,
    1_000
  )
}

async function waitForPlanUpdateLoop(sessionId: string, prompt: string): Promise<AgentSessionRestored> {
  return eventually(
    `plan_update tool loop for '${prompt}'`,
    async () => {
      const restore = await restoreSession(sessionId)
      if (restore.thread?.status === "errored") {
        throw new Error(`Agent thread errored: ${restore.thread.completion_text ?? "unknown error"}`)
      }
      if (restore.thread?.status !== "completed") {
        return null
      }

      const promptIndex = restore.messages.findIndex(
        (message) => message.role === "user" && message.content === prompt
      )
      if (promptIndex < 0) {
        return null
      }

      const afterPrompt = restore.messages.slice(promptIndex + 1)
      const planCallIds = afterPrompt.flatMap((message) =>
        message.role === "assistant"
          ? (message.tool_calls ?? [])
              .filter((toolCall) => toolCall.function.name === "plan_update")
              .map((toolCall) => toolCall.id)
              .filter((id): id is string => typeof id === "string" && id.length > 0)
          : []
      )
      const hasToolResult = afterPrompt.some(
        (message) =>
          message.role === "tool" &&
          typeof message.tool_call_id === "string" &&
          planCallIds.includes(message.tool_call_id) &&
          nonBlank(message.content)
      )
      const hasFinalAssistantReply = nonBlank(latestFinalAssistantTextAfterTool(afterPrompt, planCallIds))

      return planCallIds.length > 0 && hasToolResult && hasFinalAssistantReply ? restore : null
    },
    900_000,
    1_000
  )
}

async function waitForToolExecution(
  sessionId: string,
  prompt: string,
  toolName: ToolName,
  timeoutMs: number
): Promise<ToolExecutionResult> {
  return eventually(
    `${toolName} tool execution for '${prompt}'`,
    async () => {
      const restore = await restoreSession(sessionId)
      if (restore.thread?.status === "errored") {
        throw new Error(`Agent thread errored: ${restore.thread.completion_text ?? "unknown error"}`)
      }

      const promptIndex = restore.messages.findIndex(
        (message) => message.role === "user" && message.content === prompt
      )
      if (promptIndex < 0) {
        return null
      }

      const afterPrompt = restore.messages.slice(promptIndex + 1)
      const toolCalls = afterPrompt.flatMap((message) =>
        message.role === "assistant"
          ? (message.tool_calls ?? []).filter((toolCall) => toolCall.function.name === toolName)
          : []
      )
      const callIds = toolCalls
        .map((toolCall) => toolCall.id)
        .filter((id): id is string => typeof id === "string" && id.length > 0)
      const toolMessages = afterPrompt.filter(
        (message) =>
          message.role === "tool" &&
          typeof message.tool_call_id === "string" &&
          callIds.includes(message.tool_call_id) &&
          nonBlank(message.content)
      )
      const finalText = latestFinalAssistantTextAfterTool(afterPrompt, callIds)

      if (restore.thread?.status !== "completed") {
        return null
      }
      if (toolCalls.length === 0 || toolMessages.length === 0 || !nonBlank(finalText)) {
        return null
      }

      return { finalText, restore, toolCalls, toolMessages }
    },
    timeoutMs,
    1_000
  )
}

async function waitForCurrentAssistantSession(predicate: (sessionId: string) => boolean): Promise<string> {
  return eventually("assistant current session persisted", async () => {
    const state = await getPersistedUiState<PersistedAssistantUiState>(assistantUiStateKey)
    const currentSessionId = state?.currentSessionId
    if (!currentSessionId || !predicate(currentSessionId)) {
      return null
    }
    const sessions = await listSessions()
    return sessions.some((session) => session.id === currentSessionId) ? currentSessionId : null
  })
}

async function deleteSessionFromSheet(targetPage: Page, session: SessionResponse): Promise<void> {
  if (!session.name.includes(marker)) {
    throw new Error(`Refusing to delete unmarked assistant session '${session.name}' from real-dev fuzzing.`)
  }

  await targetPage.getByRole("button", { name: manageSessionsButtonName }).click()
  const dialog = targetPage.getByRole("dialog", { name: manageSessionsButtonName })
  await dialog.waitFor({ state: "visible", timeout: 30_000 })

  const row = dialog.locator(".workspace-soft-panel", { hasText: session.name }).first()
  await row.waitFor({ state: "visible", timeout: 30_000 })
  await row.locator("button").last().click()
  await targetPage.getByRole("menuitem", { name: deleteMenuItemName }).click()
}

function startApprovalResponder(threadId: string, toolName: ToolName): ApprovalResponder {
  const controller = new AbortController()
  const done = approveMatchingToolRequest(threadId, toolName, controller.signal)

  return {
    done,
    stop: () => controller.abort(),
  }
}

async function approveMatchingToolRequest(
  threadId: string,
  toolName: ToolName,
  signal: AbortSignal
): Promise<void> {
  const response = await fetch(
    `${serverBaseUrl}/v1/agents/responses?transport=sse&thread_id=${encodeURIComponent(threadId)}`,
    { signal }
  )
  if (!response.ok || !response.body) {
    throw new Error(`SSE subscribe failed with ${response.status}: ${await response.text()}`)
  }

  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ""

  while (!signal.aborted) {
    // eslint-disable-next-line no-await-in-loop
    const { done, value } = await reader.read()
    if (done) {
      break
    }

    buffer += decoder.decode(value, { stream: true })
    const chunks = buffer.split(/\r?\n\r?\n/)
    buffer = chunks.pop() ?? ""
    for (const chunk of chunks) {
      const event = parseSseEvent(chunk)
      if (!event) {
        continue
      }
      if (event.type === "response.tool_call.approval_required" && event.tool_name === toolName) {
        if (!event.call_id) {
          throw new Error(`Approval event for ${toolName} did not include call_id`)
        }
        // eslint-disable-next-line no-await-in-loop
        await resolveApproval(threadId, event.call_id, true)
        return
      }
      if (event.type === "response.completed" || event.type === "response.failed") {
        return
      }
    }
  }
}

async function resolveApproval(threadId: string, callId: string, approved: boolean): Promise<void> {
  const response = await requestJson<AgentResponsesServerMessage>("/v1/agents/responses", {
    json: {
      approved,
      call_id: callId,
      request_id: `approval-${Date.now()}`,
      thread_id: threadId,
      type: "agent.approval.resolve",
    } satisfies Schema["AgentResponsesClientMessage"],
    method: "POST",
  })

  if (response.type !== "agent.ack" || !response.accepted) {
    throw new Error(`Approval resolve failed: ${JSON.stringify(response)}`)
  }
}

async function cleanupMarkedSessions(): Promise<void> {
  const sessions = await listSessions().catch(() => [])
  const markedSessions = sessions.filter(
    (session) => session.name.includes(marker) || testSessionIds.has(session.id)
  )

  for (const session of markedSessions) {
    if (!session.name.includes(marker)) {
      continue
    }
    // eslint-disable-next-line no-await-in-loop
    await fetch(`${serverBaseUrl}/v1/sessions/${encodeURIComponent(session.id)}`, {
      method: "DELETE",
    }).catch(() => {})
  }
}

async function cleanupWorkspaceMarkerRoot(): Promise<void> {
  if (!workspaceRootPath || !workspaceMarkerRoot) {
    return
  }

  const target = workspaceAbsolutePath(workspaceMarkerRoot)
  const allowedRoot = normalize(join(workspaceRootPath, ".slab-e2e-fuzzing")).toLowerCase()
  const normalizedTarget = normalize(target).toLowerCase()
  if (!normalizedTarget.startsWith(allowedRoot)) {
    throw new Error(`Refusing to clean unexpected assistant fuzzing path: ${target}`)
  }

  await rm(target, { force: true, recursive: true })
}

async function restoreUiState(): Promise<void> {
  for (const backup of uiStateBackups.toReversed()) {
    if (backup.value === null) {
      // eslint-disable-next-line no-await-in-loop
      await fetch(`${serverBaseUrl}/v1/ui-state/${encodeURIComponent(backup.key)}`, {
        method: "DELETE",
      })
      continue
    }

    // eslint-disable-next-line no-await-in-loop
    await putRawUiStateValue(backup.key, backup.value)
  }
}

async function restoreSession(sessionId: string): Promise<AgentSessionRestored> {
  const response = await requestJson<AgentResponsesServerMessage>("/v1/agents/responses", {
    json: {
      request_id: `restore-${Date.now()}`,
      session_id: sessionId,
      type: "agent.session.restore",
    } satisfies Schema["AgentResponsesClientMessage"],
    method: "POST",
  })

  if (response.type !== "agent.session.restored") {
    throw new Error(`Expected agent.session.restored, received ${response.type}`)
  }

  return response
}

async function listSessions(): Promise<SessionResponse[]> {
  return requestJson<SessionResponse[]>("/v1/sessions")
}

async function getPersistedUiState<T>(key: string): Promise<T | null> {
  const value = await getRawUiStateValue(key)
  if (value === null) {
    return null
  }
  const persisted = JSON.parse(value) as { state?: T }
  return persisted.state ?? null
}

async function putPersistedUiState<T>(key: string, state: T): Promise<void> {
  await putRawUiStateValue(
    key,
    JSON.stringify({
      state,
      version: 0,
    })
  )
}

async function getRawUiStateValue(key: string): Promise<string | null> {
  const response = await fetch(`${serverBaseUrl}/v1/ui-state/${encodeURIComponent(key)}`)
  if (response.status === 404) {
    return null
  }
  if (!response.ok) {
    throw new Error(`GET /v1/ui-state/${key} failed with ${response.status}: ${await response.text()}`)
  }
  return ((await response.json()) as UiStateValueResponse).value
}

async function putRawUiStateValue(key: string, value: string): Promise<void> {
  await requestJson<UiStateValueResponse>(`/v1/ui-state/${encodeURIComponent(key)}`, {
    json: { value } satisfies Schema["UpdateUiStateRequest"],
    method: "PUT",
  })
}

async function requestJson<T>(path: string, init: JsonRequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers)
  if (init.json !== undefined && !headers.has("content-type")) {
    headers.set("content-type", "application/json")
  }

  const response = await fetch(`${serverBaseUrl}${path}`, {
    ...init,
    body: init.json === undefined ? undefined : JSON.stringify(init.json),
    headers,
  })
  const text = await response.text()
  const body = text ? (JSON.parse(text) as T) : (undefined as T)

  if (!response.ok) {
    throw new Error(`${init.method ?? "GET"} ${path} failed with ${response.status}: ${text}`)
  }

  return body
}

async function eventually<T>(
  label: string,
  assertion: () => Promise<T | false | null | undefined> | T | false | null | undefined,
  timeoutMs = 30_000,
  intervalMs = 250
): Promise<T> {
  const deadline = Date.now() + timeoutMs
  let lastError: unknown

  while (Date.now() < deadline) {
    try {
      // eslint-disable-next-line no-await-in-loop
      const result = await assertion()
      if (result) {
        return result
      }
    } catch (error) {
      lastError = error
    }

    // eslint-disable-next-line no-await-in-loop
    await delay(intervalMs)
  }

  const suffix = lastError instanceof Error ? ` Last error: ${lastError.message}` : ""
  throw new Error(`${label} timed out after ${timeoutMs}ms.${suffix}`)
}

async function seedToolWorkspaceFiles(): Promise<void> {
  await writeWorkspaceFile(markerRelativePath("read", "source.txt"), `${marker} read file\nline two\n`)
  await writeWorkspaceFile(markerRelativePath("listed", "list-child.txt"), `${marker} list dir\n`)
  await writeWorkspaceFile(markerRelativePath("glob", "glob-target.txt"), `${marker} glob\n`)
  await writeWorkspaceFile(markerRelativePath("glob", "skip.md"), `${marker} skip\n`)
  await writeWorkspaceFile(markerRelativePath("grep", "grep-target.txt"), `${marker} grep needle\n`)
  await writeWorkspaceFile(markerRelativePath("patch", "apply-patch.txt"), "one\ntwo\n")
  await mkdir(workspaceAbsolutePath(markerRelativePath("watch")), { recursive: true })
}

function toolFuzzCases(): ToolFuzzCase[] {
  const writeRelativePath = markerRelativePath("write", "write-file.txt")
  const writePath = workspaceToolPath(writeRelativePath)
  const writeContent = `${marker} write_file content`
  const readPath = workspaceToolPath(markerRelativePath("read", "source.txt"))
  const listPath = workspaceToolPath(markerRelativePath("listed"))
  const globPath = workspaceToolPath(markerRelativePath("glob"))
  const grepPath = workspaceToolPath(markerRelativePath("grep"))
  const grepNeedle = `${marker} grep needle`
  const patchPath = markerRelativePath("patch", "apply-patch.txt")
  const patchReplacement = `${marker} patched`
  const patch = [
    `--- a/${patchPath}`,
    `+++ b/${patchPath}`,
    "@@ -1,2 +1,2 @@",
    " one",
    "-two",
    `+${patchReplacement}`,
    "",
  ].join("\n")
  const shellNeedle = `${marker}-shell-ok`
  const childNeedle = `${marker}-child-ok`

  return [
    {
      args: {
        items: [
          { status: "completed", step: `${marker} inspect` },
          { status: "in_progress", step: `${marker} execute` },
        ],
        summary: `${marker} plan`,
      },
      name: "plan_update",
      validate: ({ toolMessages }) => {
        const output = parseToolJson(toolMessages[0].content)
        expect(output.summary).toBe(`${marker} plan`)
        expect(output.items).toEqual([
          { status: "completed", step: `${marker} inspect` },
          { status: "in_progress", step: `${marker} execute` },
        ])
        expect(output.counts).toMatchObject({ completed: 1, in_progress: 1 })
      },
    },
    {
      args: { content: writeContent, path: writePath },
      name: "write_file",
      validate: async ({ toolMessages }) => {
        const output = parseToolJson(toolMessages[0].content)
        expect(output.written).toBe(writePath)
        expect(output.bytes).toBe(writeContent.length)
        await expectWorkspaceFile(writeRelativePath, writeContent)
      },
    },
    {
      args: { end_line: 2, path: readPath, start_line: 1 },
      name: "read_file",
      validate: ({ toolMessages }) => {
        const output = parseToolJson(toolMessages[0].content)
        expect(output.content).toContain(`${marker} read file`)
        expect(output.returned_lines).toBe(2)
      },
    },
    {
      args: { path: listPath },
      name: "list_dir",
      validate: ({ toolMessages }) => {
        const output = parseToolJson(toolMessages[0].content)
        expect(Array.isArray(output.entries)).toBe(true)
        expect(JSON.stringify(output.entries)).toContain("list-child.txt")
      },
    },
    {
      args: { max_results: 10, path: globPath, pattern: "*.txt" },
      name: "file_glob",
      validate: ({ toolMessages }) => {
        const output = parseToolJson(toolMessages[0].content)
        expect(output.total).toBeGreaterThanOrEqual(1)
        expect(JSON.stringify(output.matches)).toContain("glob-target.txt")
      },
    },
    {
      args: {
        glob: "*.txt",
        max_results: 5,
        path: grepPath,
        pattern: grepNeedle,
      },
      name: "grep",
      validate: ({ toolMessages }) => {
        const output = parseToolJson(toolMessages[0].content)
        expect(output.total).toBeGreaterThanOrEqual(1)
        expect(JSON.stringify(output.matches)).toContain(grepNeedle)
      },
    },
    {
      args: { patch },
      name: "apply_patch",
      validate: async ({ toolMessages }) => {
        const output = parseToolJson(toolMessages[0].content)
        expect(output.result).toBe("ok")
        expect(JSON.stringify(output.applied_files)).toContain(patchPath)
        await expectWorkspaceFile(patchPath, `one\n${patchReplacement}\n`)
      },
    },
    {
      args: { language_id: "typescript" },
      name: "code_lsp_status",
      validate: ({ toolMessages }) => {
        const output = parseToolJson(toolMessages[0].content)
        expect(output.language_id).toBe("typescript")
        expect(typeof output.available).toBe("boolean")
        expect(typeof output.workspace_root).toBe("string")
        expect(output.workspace_root).not.toBe("")
      },
    },
    {
      args: {
        path: workspaceToolPath(markerRelativePath("watch")),
        recursive: false,
        timeout_ms: 25,
      },
      name: "fs_watch",
      validate: ({ toolMessages }) => {
        const output = parseToolJson(toolMessages[0].content)
        expect(Array.isArray(output.changed_paths)).toBe(true)
        expect(typeof output.timed_out).toBe("boolean")
      },
    },
    {
      args: {
        max_results: 1,
        provider: "duckduckgo",
        query: `${marker} Slab assistant fuzzing`,
        timeout_ms: 5_000,
      },
      name: "web_search",
      timeoutMs: 1_200_000,
      validate: ({ toolMessages }) => {
        const output = parseToolJson(toolMessages[0].content)
        expect(output.provider).toBe("duckduckgo")
        expect(output.query).toBe(`${marker} Slab assistant fuzzing`)
        expect(typeof output.total).toBe("number")
      },
    },
    {
      approve: true,
      args: { command: `Write-Output ${shellNeedle}`, timeout_secs: 10 },
      name: "shell",
      validate: ({ toolMessages }) => {
        const output = parseToolJson(toolMessages[0].content)
        expect(String(output.stdout ?? "")).toContain(shellNeedle)
        expect(output.exit_code).toBe(0)
        expect(output.timed_out).toBe(false)
      },
    },
    {
      args: {
        allowed_tools: ["plan_update"],
        max_turns: 1,
        model: assistantModelId,
        system_prompt: "Reply with the exact requested marker only. Do not call tools.",
        task: `Return exactly ${childNeedle}. Do not use tools.`,
      },
      name: "delegate_subagent",
      timeoutMs: 1_200_000,
      validate: ({ toolMessages }) => {
        const output = parseToolJson(toolMessages[0].content)
        expect(typeof output.child_thread_id).toBe("string")
        expect(output.status).toBe("completed")
        expect(String(output.completion_text ?? "")).toContain(childNeedle)
      },
    },
  ]
}

function toolAgentConfig(toolName: ToolName): AgentConfigInput {
  return {
    allowed_tools: [toolName],
    invalid_tool_call_retries: 1,
    max_tokens: 900,
    max_turns: 4,
    reasoning_effort: "none",
    system_prompt:
      "You are running a Slab agent real-dev tool test. " +
      "For the first assistant turn, call the single available tool exactly once using the JSON arguments from the user. " +
      "After a tool result is returned, do not call any more tools. Reply with the requested final marker.",
    temperature: 0,
    tool_concurrency: 1,
  }
}

function toolPrompt(toolName: ToolName, args: Record<string, unknown>, finalMarker: string): string {
  return [
    `${marker} tool fuzz case for ${toolName}.`,
    `Call ${toolName} exactly once with this JSON argument object:`,
    "```json",
    JSON.stringify(args),
    "```",
    "Your first assistant output must be the tool call only. Do not add text after </tool_call>.",
    `After the tool result, answer with a concise sentence containing this exact final marker: ${finalMarker}`,
  ].join("\n")
}

async function writeWorkspaceFile(relativePath: string, content: string): Promise<void> {
  const path = workspaceAbsolutePath(relativePath)
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, content, "utf8")
}

async function expectWorkspaceFile(relativePath: string, expectedContent: string): Promise<void> {
  const content = await readFile(workspaceAbsolutePath(relativePath), "utf8")
  expect(content).toBe(expectedContent)
}

function workspaceAbsolutePath(relativePath: string): string {
  if (!workspaceRootPath) {
    throw new Error("workspaceRootPath is not initialized")
  }
  const normalizedRelative = relativePath.split("/").filter(Boolean)
  return join(workspaceRootPath, ...normalizedRelative)
}

function workspaceToolPath(relativePath: string): string {
  return workspaceAbsolutePath(relativePath).replace(/\\/g, "/")
}

function markerRelativePath(...segments: string[]): string {
  return [workspaceMarkerRoot, ...segments].join("/")
}

function workspaceRootFromDiagnostics(diagnostics: SystemDiagnosticsResponse): string | null {
  const explicit = diagnostics.paths.find((path) => path.label === "workspace_root" && path.exists)
  if (explicit?.path.trim()) {
    return explicit.path
  }

  const settingsFile = diagnostics.paths.find((path) => path.label === "settings_file" && path.exists)
  if (!settingsFile?.path.trim()) {
    return null
  }

  const normalizedPath = settingsFile.path.replace(/\\/g, "/")
  const slabDirIndex = normalizedPath.toLowerCase().lastIndexOf("/.slab/")
  if (slabDirIndex <= 0) {
    return null
  }

  return settingsFile.path.slice(0, slabDirIndex)
}

function parseSseEvent(chunk: string): AgentStreamEvent | null {
  const data = chunk
    .split(/\r?\n/)
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.slice("data:".length).trimStart())
    .join("\n")
    .trim()
  if (!data) {
    return null
  }
  return JSON.parse(data) as AgentStreamEvent
}

function parseToolJson(content: string): Record<string, unknown> {
  const jsonText = leadingJsonObject(content.trim())
  const value = JSON.parse(jsonText) as unknown
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`Expected tool output object, received: ${content}`)
  }
  return value as Record<string, unknown>
}

function leadingJsonObject(content: string): string {
  if (!content.startsWith("{")) {
    throw new Error(`Tool output is not JSON: ${content}`)
  }

  let depth = 0
  let escaped = false
  let inString = false
  for (let index = 0; index < content.length; index += 1) {
    const char = content[index]
    if (escaped) {
      escaped = false
      continue
    }
    if (char === "\\") {
      escaped = true
      continue
    }
    if (char === '"') {
      inString = !inString
      continue
    }
    if (inString) {
      continue
    }
    if (char === "{") {
      depth += 1
    } else if (char === "}") {
      depth -= 1
      if (depth === 0) {
        return content.slice(0, index + 1)
      }
    }
  }

  throw new Error(`Tool output JSON object is incomplete: ${content}`)
}

function modelUsableWithoutDownload(model: UnifiedModelResponse): boolean {
  if (model.kind === "cloud") {
    return true
  }

  return model.status === "ready" && Boolean(model.spec.local_path)
}

function latestAssistantTextAfter(messages: AgentThreadMessageResponse[], prompt: string): string {
  const promptIndex = messages.findIndex(
    (message) => message.role === "user" && message.content === prompt
  )
  if (promptIndex < 0) {
    return ""
  }

  const assistantMessages = messages
    .slice(promptIndex + 1)
    .filter((message) => message.role === "assistant" && nonBlank(message.content))
  return assistantMessages.at(-1)?.content.trim() ?? ""
}

function latestFinalAssistantTextAfterTool(
  messagesAfterPrompt: AgentThreadMessageResponse[],
  callIds: string[]
): string {
  const lastToolMessageIndex = messagesAfterPrompt.findLastIndex(
    (message) =>
      message.role === "tool" &&
      typeof message.tool_call_id === "string" &&
      callIds.includes(message.tool_call_id)
  )
  if (lastToolMessageIndex < 0) {
    return ""
  }

  return (
    messagesAfterPrompt
      .slice(lastToolMessageIndex + 1)
      .findLast(
        (message) =>
          message.role === "assistant" &&
          (message.tool_calls ?? []).length === 0 &&
          nonBlank(message.content)
      )
      ?.content.trim() ?? ""
  )
}

function extractDoneThinking(text: string): string | null {
  const match = text.match(/<think\b[^>]*>\s*([\s\S]*?)\s*<\/think>/i)
  return match?.[1]?.trim() || null
}

function answerWithoutThinking(text: string): string {
  return text.replace(/<think\b[^>]*>[\s\S]*?<\/think>/gi, "").trim()
}

function visibleNeedle(text: string): string {
  const normalized = normalizeVisibleText(text)
  if (normalized.length <= 80) {
    return normalized
  }
  return normalized.slice(0, 80).trim()
}

function normalizeVisibleText(text: string): string {
  return text
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
    .replace(/[`*_#>[\]]/g, "")
    .replace(/\s+/g, " ")
    .trim()
}

function nonBlank(value: string | null | undefined): value is string {
  return typeof value === "string" && value.trim().length > 0
}

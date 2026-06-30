import type { Locator, Page } from "playwright"
import {
  eventually,
  getPersistedUiState,
  restoreSession,
  type AgentSessionRestored,
  type AgentThreadMessageResponse,
  type ChatToolCall,
} from "./fullstack-dev"

type AssistantUiState = {
  currentSessionId?: string
}

export type CompletedAssistantReply = {
  restore: AgentSessionRestored
  text: string
}

export type ToolExecutionResult = {
  finalText: string
  restore: AgentSessionRestored
  toolCalls: ChatToolCall[]
  toolMessages: AgentThreadMessageResponse[]
}

const approveButtonName = /^(Approve|\u6279\u51c6)$/u

export async function openAssistant(page: Page, uiBaseUrl: string): Promise<void> {
  // WorkspaceModeSync (App.tsx) redirects a *fresh full load* at `/` to
  // `/workspace` once when a workspace is active (intended workspace-mode
  // behavior). The e2e stack always configures a workspace, and `page.goto`
  // is a full reload (which remounts the app and re-triggers that redirect), so
  // a full load at `/` always lands on `/workspace` and the assistant composer
  // never renders. Reach the Assistant page with a client-side SPA navigation
  // instead: full-load at `/workspace` (no `/`-redirect), then click the sidebar
  // Assistant link (a react-router <Link/>) which navigates to `/` without
  // remounting, so the one-time redirect guard does not re-fire.
  await page.goto(`${uiBaseUrl}/workspace`, { waitUntil: "domcontentloaded", timeout: 60_000 })
  await page.getByTestId("sidebar-link-assistant").click()
  await waitForComposerReady(page)
}

export async function sendAssistantMessage(page: Page, message: string): Promise<void> {
  const composer = await waitForComposerReady(page)
  await composer.fill(message)
  await page.getByTestId("assistant-send-button").locator("button").click()
}

export async function waitForComposerReady(page: Page): Promise<Locator> {
  const composer = page.getByTestId("assistant-composer-input").locator("textarea")
  await composer.waitFor({ state: "visible", timeout: 90_000 })
  await eventually("assistant composer is editable", async () => composer.isEditable(), 90_000)
  return composer
}

export async function waitForCurrentAssistantSession(
  baseUrl: string,
  predicate: (sessionId: string) => boolean = () => true
): Promise<string> {
  return eventually("assistant current session persisted", async () => {
    const state = await getPersistedUiState<AssistantUiState>(baseUrl, "zustand:assistant-ui")
    const currentSessionId = state?.currentSessionId
    return currentSessionId && predicate(currentSessionId) ? currentSessionId : null
  }, 90_000)
}

export async function waitForCompletedAssistantReply(
  baseUrl: string,
  sessionId: string,
  prompt: string,
  timeoutMs = 900_000
): Promise<CompletedAssistantReply> {
  return eventually(
    `completed assistant reply for '${prompt}'`,
    async () => {
      const restore = await restoreSession(baseUrl, sessionId)
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

export async function waitForToolExecution(
  baseUrl: string,
  sessionId: string,
  prompt: string,
  toolName: string,
  timeoutMs = 900_000
): Promise<ToolExecutionResult> {
  return eventually(
    `${toolName} tool execution for '${prompt}'`,
    async () => {
      const restore = await restoreSession(baseUrl, sessionId)
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

export async function approvePendingToolCall(page: Page): Promise<void> {
  await page.getByRole("button", { name: approveButtonName }).click({ timeout: 240_000 })
}

export async function expectAssistantPageText(page: Page, text: string): Promise<void> {
  const needle = visibleNeedle(text)
  // Message bubbles are markdown-rendered (AssistantMarkdown), so the DOM text
  // can differ from the raw prompt by markdown formatting chars — e.g. a marker
  // like `SLAB_AGENT_E2E_…` renders with its underscores intact, but
  // `visibleNeedle` strips `_`. Matching the stripped needle against raw DOM
  // text therefore misses prompts that contain `_`/`*`/`#`/`>`/`[`/`]`. Normalize
  // the DOM text the same way before comparing.
  await eventually(
    `assistant page text '${needle}'`,
    async () => {
      const messages = page.locator('[data-testid^="assistant-message-"]')
      const count = await messages.count()
      for (let index = 0; index < count; index += 1) {
        // eslint-disable-next-line no-await-in-loop
        const raw = await messages.nth(index).textContent()
        if (raw && normalizeVisibleText(raw).includes(needle)) {
          return true
        }
      }
      return null
    },
    180_000
  )
}

export function latestAssistantTextAfter(
  messages: AgentSessionRestored["messages"],
  prompt: string
): string {
  const promptIndex = messages.findIndex(
    (message) => message.role === "user" && message.content === prompt
  )
  if (promptIndex < 0) {
    return ""
  }

  return messages
    .slice(promptIndex + 1)
    .findLast((message) => message.role === "assistant" && nonBlank(message.content))?.content ?? ""
}

export function nonBlank(value: string | null | undefined): boolean {
  return typeof value === "string" && value.trim().length > 0
}

export function parseToolJson(content: string): Record<string, unknown> {
  const value = JSON.parse(leadingJsonObject(content.trim())) as unknown
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`Expected tool output object, received: ${content}`)
  }
  return value as Record<string, unknown>
}

export function visibleNeedle(text: string): string {
  return normalizeVisibleText(text).slice(0, 120)
}

function latestFinalAssistantTextAfterTool(
  messages: AgentSessionRestored["messages"],
  callIds: string[]
): string {
  if (callIds.length === 0) {
    return ""
  }

  const lastToolIndex = messages.findLastIndex(
    (message) =>
      message.role === "tool" &&
      typeof message.tool_call_id === "string" &&
      callIds.includes(message.tool_call_id)
  )
  if (lastToolIndex < 0) {
    return ""
  }

  return messages
    .slice(lastToolIndex + 1)
    .findLast((message) => message.role === "assistant" && nonBlank(message.content))?.content ?? ""
}

function normalizeVisibleText(text: string): string {
  return text
    .replace(/[`*_#>[\]]/g, "")
    .replace(/\s+/g, " ")
    .trim()
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

  throw new Error(`Could not parse leading JSON object from: ${content}`)
}

/**
 * Shared `TurnItem` → UI mapping for the harness protocol.
 *
 * This is the single source of truth for how a finalized harness {@link TurnItem}
 * becomes UI parts, used by BOTH the history path (`thread/resume` →
 * {@link turnItemsToMessages}) and the live streaming path (`stream.ts`
 * `toolChunksFromItem`, via {@link toolItemFields}). History calls
 * {@link turnItemToUiParts} directly; live builds the same final parts
 * incrementally via chunks but shares the tool-field extraction here, so the two
 * paths cannot drift on how a command/mcp/file/websearch item maps to fields.
 *
 * (The React rendering itself — `messagePartComponents` — is already shared
 * between live and history; this module closes the remaining gap at the data
 * layer, which is where the old `projectThread` diverged and dropped content.)
 */

import type { UIMessage } from "ai"

import { SERVER_BASE_URL } from "@slab/api/config"
import type { ReasoningText, TurnItem, UserMessageContent } from "@slab/api/harness"

import { getImageSrcPort } from "../platform/image-src"

/** A single UI message part (the finalized shape `useChat` assembles). */
type UiPart = UIMessage["parts"][number]

/**
 * Resolve an image reference carried on the wire into a URL the browser can
 * fetch for inline rendering. Handles:
 * - `data:` URIs and absolute `http(s)://` URLs (already fetchable).
 * - slab-server artifact paths (`/v1/images/...`) — resolved against the
 *   configured API base so they load in both web and Tauri.
 * - Native filesystem paths (Tauri `localImage` / persisted user image) —
 *   rendered via the injected `ImageSrcPort` (Tauri asset protocol on desktop,
 *   `null` on web where local paths cannot be fetched).
 */
function resolveImageUrl(pathOrUrl: string): string | null {
  if (!pathOrUrl) return null
  if (pathOrUrl.startsWith("data:") || /^https?:\/\//i.test(pathOrUrl)) return pathOrUrl
  if (pathOrUrl.startsWith("/v1/")) return `${SERVER_BASE_URL}${pathOrUrl}`
  const imageSrc = getImageSrcPort()
  return imageSrc.canResolveLocalPaths() ? imageSrc.resolve(pathOrUrl) : null
}

/** Build an inline image UI part from a fetchable URL, or `null`. */
function inlineImagePart(pathOrUrl: string, mimeType = "image/png"): UiPart | null {
  const url = resolveImageUrl(pathOrUrl)
  if (!url) return null
  return { type: "file", mediaType: mimeType, url } as UiPart
}

/** Tool-shaped fields extracted from a finalized tool-like {@link TurnItem}. */
export type ToolItemFields = {
  toolName: string
  input: unknown
  output?: unknown
  errorText?: string
  failed: boolean
}

/**
 * Extract the tool fields from a finalized tool-like item (`commandExecution`,
 * `mcpToolCall`, `fileChange`, `toolCall`, `webSearch`). Returns `null` for
 * non-tool items.
 *
 * Shared by the history part-builder and the live chunk-emitter so both agree
 * on input/output/error derivation (e.g. `exitCode !== 0` ⇒ failed).
 */
export function toolItemFields(item: TurnItem): ToolItemFields | null {
  switch (item.type) {
    case "commandExecution": {
      const failed = item.exitCode !== undefined && item.exitCode !== 0
      return {
        toolName: "commandExecution",
        input: { command: item.command, cwd: item.cwd },
        output: !failed && item.aggregatedOutput ? item.aggregatedOutput : undefined,
        errorText: failed ? item.aggregatedOutput ?? `exit code ${item.exitCode}` : undefined,
        failed,
      }
    }
    case "mcpToolCall": {
      const failed = item.error !== undefined && item.error !== null
      return {
        toolName: item.tool,
        input: item.arguments,
        output:
          !failed && item.result !== undefined && item.result !== null ? item.result : undefined,
        errorText: failed ? stringifyToolValue(item.error) : undefined,
        failed,
      }
    }
    case "fileChange":
      return {
        toolName: "fileChange",
        input: { changes: item.changes },
        output: { status: item.status },
        failed: false,
      }
    case "toolCall": {
      // Generic built-in tool call (read_file / grep / git_* / …) — the
      // server-default render. Mirrors mcpToolCall: failed calls carry `error`,
      // successful ones carry `result`.
      const failed = item.error !== undefined && item.error !== null
      return {
        toolName: item.tool,
        input: item.arguments,
        output:
          !failed && item.result !== undefined && item.result !== null ? item.result : undefined,
        errorText: failed ? stringifyToolValue(item.error) : undefined,
        failed,
      }
    }
    case "webSearch":
      return { toolName: "webSearch", input: { query: item.query }, failed: false }
    case "plan":
      // The full Plan object is the card payload. Surfacing it as both input
      // and output marks the static (non-streaming) card as `output-available`.
      return { toolName: "plan", input: item.plan, output: item.plan, failed: false }
    default:
      return null
  }
}

/** Stringify a tool error/result value of unknown shape for display. */
export function stringifyToolValue(value: unknown): string {
  if (typeof value === "string") return value
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function reasoningToString(value: ReasoningText): string {
  return Array.isArray(value) ? value.join("\n") : value
}

/**
 * Complete `<think …>…</think>` blocks the server used to embed into the
 * persisted agentMessage text (LLM-context form). History renders item text
 * verbatim, so legacy rollout files still carrying the block must be cleaned
 * here or the raw thinking shows up in the message body. Mirrors the
 * server-side `strip_think_blocks` emission guard.
 */
const THINK_BLOCK_PATTERN = /<think\b[^>]*>[\s\S]*?<\/think>/gi

function stripThinkBlocks(text: string): string {
  return text.replace(THINK_BLOCK_PATTERN, "").trim()
}

/**
 * Build the finalized UI parts for one assistant-side {@link TurnItem}
 * (agentMessage / reasoning / imageView / tool items). `userMessage` is handled
 * by {@link turnItemsToMessages} (it starts a new user message).
 */
export function turnItemToUiParts(item: TurnItem): UiPart[] {
  switch (item.type) {
    case "agentMessage": {
      const text = stripThinkBlocks(item.text ?? "")
      return text ? ([{ text, type: "text" }] as UiPart[]) : []
    }
    case "reasoning": {
      // Use `content` (the full trace) — the live reasoning-delta stream
      // accumulates content, so this keeps history aligned with live rather
      // than collapsing to the summary recap.
      const text = reasoningToString(item.content)
      return text
        ? ([{ state: "done", text, type: "reasoning" }] as UiPart[])
        : []
    }
    case "imageView": {
      // Generated-image artifacts are served by slab-server at a `/v1/...`
      // path; resolve it against the API base and render inline. Falls back to
      // no part when the path cannot be fetched (e.g. bare path on web).
      const part = inlineImagePart(item.path)
      return part ? [part] : []
    }
    default: {
      const fields = toolItemFields(item)
      if (!fields) return []
      const part: UiPart = {
        type: `tool-${fields.toolName}`,
        toolCallId: item.id,
        toolName: fields.toolName,
        input: fields.input,
        state: fields.failed ? "output-error" : "output-available",
        ...(fields.output !== undefined ? { output: fields.output } : {}),
        ...(fields.errorText !== undefined ? { errorText: fields.errorText } : {}),
      } as UiPart
      return [part]
    }
  }
}

function userContentToParts(content: UserMessageContent): UiPart[] {
  if (content.type === "text") {
    return content.text ? ([{ text: content.text, type: "text" }] as UiPart[]) : []
  }
  // Image content: prefer a ready URL, else rebuild a data URL from base64.
  // Note: the wire fields are snake_case (`image_url`/`mime_type`) — the Rust
  // enum variant carries no serde renames for its fields.
  const mimeType = content.mime_type ?? "image/png"
  const source = content.image_url
    ?? (content.base64 ? `data:${mimeType};base64,${content.base64}` : undefined)
  if (!source) return []
  const part = inlineImagePart(source, mimeType)
  return part ? [part] : []
}

/**
 * Project a flat, ordered list of finalized {@link TurnItem}s into `UIMessage`s.
 *
 * Grouping mirrors the live stream: a `userMessage` item starts a user message
 * and flushes any in-flight assistant group; consecutive non-user items are
 * folded into one assistant message whose id is the first item's id. Empty
 * groups produce no message. Replaces the old lossy `projectThread`.
 */
export function turnItemsToMessages(items: TurnItem[]): UIMessage[] {
  const messages: UIMessage[] = []
  let pendingAssistantId: string | null = null
  let pendingParts: UiPart[] = []

  const flushAssistant = () => {
    if (pendingAssistantId !== null && pendingParts.length > 0) {
      messages.push({ id: pendingAssistantId, parts: pendingParts, role: "assistant" })
    }
    pendingAssistantId = null
    pendingParts = []
  }

  for (const item of items) {
    if (item.type === "userMessage") {
      flushAssistant()
      const parts = item.content.flatMap(userContentToParts)
      if (parts.length > 0) {
        messages.push({ id: item.id, parts, role: "user" })
      }
      continue
    }
    if (pendingAssistantId === null) pendingAssistantId = item.id
    pendingParts.push(...turnItemToUiParts(item))
  }
  flushAssistant()

  return messages
}

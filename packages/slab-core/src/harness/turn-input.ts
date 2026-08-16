/**
 * Shared turn-input construction for the harness JSON-RPC protocol: mapping an
 * AI-SDK {@link UIMessage} conversation onto the harness `UserInput` wire
 * variants. Used by both {@link HarnessChatTransport} (the `useChat` send
 * path) and {@link ConversationController.send} (the programmatic path) so
 * there is a single source of truth for the mapping.
 */

import type { UIMessage } from "ai"

import { getImageSrcPort } from "../platform/image-src"

import type { UserInput } from "./types"

/**
 * Build the new turn's `input` from the latest user message: its text plus any
 * image attachments (AI-SDK `file` parts with an image media type), mapped to
 * harness `UserInput` variants. Non-image files are ignored here (no harness
 * upload path yet).
 */
export function buildTurnInput(messages: UIMessage[]): UserInput[] {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i]
    if (message.role !== "user") continue
    const input: UserInput[] = []
    const text = message.parts
      .filter((part): part is { type: "text"; text: string } => part.type === "text")
      .map((part) => part.text)
      .join("")
      .trim()
    if (text) input.push({ type: "text", text, textElements: [] })
    for (const part of message.parts) {
      if (part.type !== "file") continue
      const file = part as { type: "file"; mediaType?: string; url: string }
      if (!file.mediaType?.startsWith("image")) continue
      // On Tauri the picker yields a native path; send `localImage` so the
      // server reads the file directly (no base64 round-trip). Web / paste /
      // drop yield a `data:` URL → send `image`. Both are handled server-side.
      if (getImageSrcPort().canResolveLocalPaths() && !file.url.startsWith("data:")) {
        input.push({ type: "localImage", path: file.url, detail: "auto" })
      } else {
        input.push({ type: "image", imageUrl: file.url, detail: "auto" })
      }
    }
    return input
  }
  return []
}

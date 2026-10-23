/**
 * Regenerate support: locate the user message that produced an assistant
 * message. Pure — the pane owns the `messages` array from useChat, so the
 * lookup walks protocol order backwards from the assistant row.
 */

export interface PrecedingUserMessage {
  id: string
  /** Concatenated text parts of the user message (attachments are not restored). */
  text: string
}

type MessageLike = {
  id: string
  role: string
  parts?: ReadonlyArray<{ type: string; text?: string }>
}

export function findPrecedingUserMessage<T extends MessageLike>(
  messages: readonly T[],
  assistantMessageId: string,
): PrecedingUserMessage | null {
  const assistantIndex = messages.findIndex((message) => message.id === assistantMessageId)
  if (assistantIndex <= 0) return null

  for (let index = assistantIndex - 1; index >= 0; index -= 1) {
    const message = messages[index]
    if (message.role !== "user") continue
    const text = (message.parts ?? [])
      .filter((part) => part.type === "text")
      .map((part) => part.text ?? "")
      .join("")
      .trim()
    return { id: message.id, text }
  }

  return null
}

import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"

function MessageFallbackPart({
  part,
}: MessagePartRenderProps<TMessagePart, TMessage>) {
  if (part.type === "text") {
    return <span className="whitespace-pre-wrap">{part.text}</span>
  }

  // Unknown / unhandled part kind (file, source, data, custom, …): surface a
  // read-only JSON dump instead of vanishing, so nothing renders silently.
  let dump: string
  try {
    dump = JSON.stringify(part, null, 2)
  } catch {
    dump = String(part)
  }

  return (
    <pre className="overflow-x-auto rounded-md bg-muted/50 p-3 font-mono text-xs text-muted-foreground">
      {dump}
    </pre>
  )
}

export default MessageFallbackPart

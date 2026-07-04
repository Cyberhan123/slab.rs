
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"

function MessageFallbackPart({
  part,
}: MessagePartRenderProps<TMessagePart, TMessage>) {
  if (part.type !== "text") {
    return null
  }

  return (
    <span className="whitespace-pre-wrap">
     {part.text}
    </span>
  )
}

export default MessageFallbackPart
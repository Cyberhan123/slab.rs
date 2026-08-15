import { Markdown } from "./markdown"
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"

function MessageTextPart({
  part,
  kind,
}: MessagePartRenderProps<TMessagePart, TMessage>) {
  if (kind !== "text") {
    return null
  }

  // While the assistant is still streaming this chunk, keep Streamdown in its
  // streaming mode so partial markdown renders gracefully.
  const isStreaming = part.state === "streaming"

  return (
    <div className="min-w-0">
      <Markdown hasNextChunk={isStreaming}>{part.text}</Markdown>
    </div>
  )
}

export default MessageTextPart

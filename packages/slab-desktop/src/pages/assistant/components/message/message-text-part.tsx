import {Markdown} from "./markdown"
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"

function MessageTextPart({
  part,
  kind,
}: MessagePartRenderProps<TMessagePart, TMessage>) {
  if (kind !== "text") {
    return null
  }

  return (
    <span className="whitespace-pre-wrap">
      <Markdown>{part.text}</Markdown>
    </span>
  )
}

export default MessageTextPart
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"
import {
    Marker,
    MarkerContent,
    MarkerIcon,
} from "@slab/components/marker"
import { Spinner } from "@slab/components/spinner"

function MessageToolPart({
    part,
    kind,
}: MessagePartRenderProps<TMessagePart, TMessage>) {
    if (kind !== "tool") {
        return null
    }

    const isStreaming = part.state === "streaming"
    return (
        <span className="whitespace-pre-wrap">
            <Marker role="status">
                {
                    isStreaming ? (
                        <MarkerIcon>
                            <Spinner />
                        </MarkerIcon>
                    ) : null
                }
                <MarkerContent className={isStreaming ? "shimmer" : ""}>{isStreaming ? "Thinking..." : "Thinking"}</MarkerContent>
            </Marker>
            {part.text}
        </span>
    )
}

export default MessageToolPart

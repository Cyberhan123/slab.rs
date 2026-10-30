"use client"

import { BrainIcon } from "lucide-react"
import { useEffect, useRef, useState } from "react"
import {
  TraceCollapsible,
  TraceCollapsibleContent,
  TraceCollapsibleTrigger,
  TraceIcon,
  TraceLine,
  TraceTitle,
} from "@slab/components/trace"

import { Markdown } from "./markdown"
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"
import { useTranslation } from "@slab/i18n"

const MS_IN_S = 1000

/**
 * Reasoning ("thinking") part on the shadcn Trace primitives: a collapsible
 * whose line shimmers while streaming ("Thinking"), then reports the duration
 * ("Thought for Ns"). Follows the ai-elements Reasoning behavior — open while
 * streaming, closed once done, an explicit user toggle wins over the automatic
 * open/close — with the duration timer kept from the previous implementation.
 * The line stays bare until text exists (a collapsible always has content).
 */
function MessageReasoningPart({
  part,
  kind,
}: MessagePartRenderProps<TMessagePart, TMessage>) {
  const { t } = useTranslation()
  const text = (part.text as string | undefined) ?? ""
  const isStreaming = part.state === "streaming"

  // Track when streaming starts and compute the duration once it ends.
  const [duration, setDuration] = useState<number | undefined>(undefined)
  const startTimeRef = useRef<number | null>(null)
  useEffect(() => {
    if (isStreaming) {
      if (startTimeRef.current === null) startTimeRef.current = Date.now()
    } else if (startTimeRef.current !== null) {
      setDuration(Math.ceil((Date.now() - startTimeRef.current) / MS_IN_S))
      startTimeRef.current = null
    }
  }, [isStreaming])

  // `undefined` means "no user choice yet" — follow the streaming state.
  const [userOpen, setUserOpen] = useState<boolean | undefined>(undefined)
  const open = userOpen ?? isStreaming

  const line = (
    <TraceLine status={isStreaming ? "running" : undefined}>
      <TraceIcon>
        <BrainIcon />
      </TraceIcon>
      <TraceTitle>
        {isStreaming || duration === 0
          ? t("pages.assistant.thinking.loading")
          : duration === undefined
            ? t("pages.assistant.thinking.thoughtForAFewSeconds")
            : t("pages.assistant.thinking.thoughtForSeconds", { seconds: duration })}
      </TraceTitle>
    </TraceLine>
  )

  if (kind !== "reasoning") return null
  if (!text) {
    // Nothing to reveal yet — a collapsible always has content, so stay a line.
    return line
  }

  return (
    <TraceCollapsible open={open} onOpenChange={setUserOpen}>
      <TraceCollapsibleTrigger>{line}</TraceCollapsibleTrigger>
      <TraceCollapsibleContent>
        <Markdown hasNextChunk={isStreaming}>{text}</Markdown>
      </TraceCollapsibleContent>
    </TraceCollapsible>
  )
}

export default MessageReasoningPart

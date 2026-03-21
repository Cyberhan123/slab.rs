import { Copy, RefreshCcw } from "lucide-react"
import XMarkdown from "@ant-design/x-markdown"

import { Button } from "@/components/ui/button"
import { ChatThinkingPanel } from "@/pages/chat/components/chat-thinking-panel"
import { cn } from "@/lib/utils"

type ChatMessageRecord = {
  id: string | number
  status?: string
  message: {
    role: "assistant" | "user"
    content?: string | null
    extraInfo?: unknown
  }
}

type ParsedThinkingContent = {
  thinking: string | null
  answer: string
  thinkingLoading: boolean
}

function parseThinkingContent(rawContent: string): ParsedThinkingContent {
  const openTagIndex = rawContent.indexOf("<think")
  if (openTagIndex < 0) {
    return { thinking: null, answer: rawContent, thinkingLoading: false }
  }

  const openTagEnd = rawContent.indexOf(">", openTagIndex)
  if (openTagEnd < 0) {
    return { thinking: null, answer: rawContent, thinkingLoading: false }
  }

  const openTag = rawContent.slice(openTagIndex, openTagEnd + 1)
  const thinkingMarkedDone = /\bstatus\s*=\s*["']?done["']?/i.test(openTag)
  const closeTag = "</think>"
  const closeTagIndex = rawContent.indexOf(closeTag, openTagEnd + 1)

  if (closeTagIndex < 0) {
    const thinking = rawContent.slice(openTagEnd + 1).trimStart()

    return {
      thinking: thinking || null,
      answer: rawContent.slice(0, openTagIndex).trimEnd(),
      thinkingLoading: !thinkingMarkedDone,
    }
  }

  const thinking = rawContent.slice(openTagEnd + 1, closeTagIndex).trim()
  const before = rawContent.slice(0, openTagIndex)
  const after = rawContent.slice(closeTagIndex + closeTag.length)

  return {
    thinking: thinking || null,
    answer: `${before}${after}`.trimStart(),
    thinkingLoading: false,
  }
}

type ChatMessageBubbleProps = {
  item: ChatMessageRecord
  markdownClassName?: string
  onRetry?: (id: string | number) => void
}

export function ChatMessageBubble({
  item,
  markdownClassName,
  onRetry,
}: ChatMessageBubbleProps) {
  const role = item.message.role
  const isAssistant = role === "assistant"
  const rawContent = String(item.message.content ?? "")
  const { thinking, answer, thinkingLoading } = parseThinkingContent(rawContent)
  const hasNextChunk = item.status === "updating"
  const isBusy = item.status === "loading" || item.status === "updating"

  const copyMessage = async () => {
    await navigator.clipboard.writeText(rawContent)
  }

  return (
    <div className={cn("flex w-full", isAssistant ? "justify-start" : "justify-end")}>
      <div className={cn("flex max-w-[min(100%,46rem)] flex-col gap-2", !isAssistant && "items-end")}>
        <div
          className={cn(
            "rounded-[28px] px-5 py-4 shadow-[0_18px_40px_-32px_color-mix(in_oklab,var(--foreground)_35%,transparent)]",
            isAssistant
              ? "border border-border/60 bg-[var(--surface-1)] text-foreground"
              : "bg-[var(--brand-teal)] text-[var(--brand-teal-foreground)]"
          )}
        >
          {thinking ? (
            <div className="mb-3">
              <ChatThinkingPanel thinking={thinking} loading={thinkingLoading && isBusy}>
                <XMarkdown
                  paragraphTag="div"
                  className={cn("chat-markdown", markdownClassName)}
                  streaming={{
                    hasNextChunk,
                    enableAnimation: true,
                  }}
                >
                  {thinking}
                </XMarkdown>
              </ChatThinkingPanel>
            </div>
          ) : null}

          {answer ? (
            <XMarkdown
              paragraphTag="div"
              className={cn("chat-markdown", markdownClassName)}
              streaming={{
                hasNextChunk,
                enableAnimation: true,
              }}
            >
              {answer}
            </XMarkdown>
          ) : isBusy ? (
            <p className="text-sm opacity-80">Waiting for response...</p>
          ) : null}
        </div>

        <div className="flex items-center gap-2 px-1">
          <Button
            type="button"
            variant="quiet"
            size="sm"
            className="h-8 rounded-full px-3 text-xs"
            onClick={() => void copyMessage()}
          >
            <Copy className="size-3.5" />
            Copy
          </Button>
          {isAssistant && !isBusy && onRetry ? (
            <Button
              type="button"
              variant="quiet"
              size="sm"
              className="h-8 rounded-full px-3 text-xs"
              onClick={() => onRetry(item.id)}
            >
              <RefreshCcw className="size-3.5" />
              Retry
            </Button>
          ) : null}
        </div>
      </div>
    </div>
  )
}

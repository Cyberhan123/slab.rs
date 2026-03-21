import { BotMessageSquare, Copy, RefreshCcw, UserRound } from "lucide-react"
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
      <div className={cn("flex max-w-[min(100%,40.8rem)] flex-col gap-2", !isAssistant && "items-end")}>
        <div className={cn("flex items-center gap-2 px-1", !isAssistant && "flex-row-reverse")}>
          <span
            className={cn(
              "flex size-6 shrink-0 items-center justify-center",
              isAssistant
                ? "rounded-[8px] bg-[#008378] text-white"
                : "rounded-full border border-[#bcc9c64d] bg-white text-[#3d4947]"
            )}
          >
            {isAssistant ? (
              <BotMessageSquare className="size-3.5" />
            ) : (
              <UserRound className="size-3.5" />
            )}
          </span>
          <span className="text-[11px] font-bold uppercase tracking-[0.14em] text-[#6d7a77]">
            {isAssistant ? "Ethereal Assistant" : "User"}
          </span>
        </div>

        <div
          className={cn(
            "px-6 py-4 shadow-[0_1px_2px_rgba(0,0,0,0.05)]",
            isAssistant
              ? "rounded-tl-[24px] rounded-tr-[24px] rounded-br-[24px] rounded-bl-[8px] bg-[#e6e8ea] text-[#191c1e]"
              : "rounded-tl-[24px] rounded-tr-[24px] rounded-br-[8px] rounded-bl-[24px] bg-[#d5e3fd] text-[#0d1c2f]"
          )}
        >
          {thinking ? (
            <div className="mb-4">
              <ChatThinkingPanel thinking={thinking} loading={thinkingLoading && isBusy}>
                <XMarkdown
                  paragraphTag="div"
                  className={cn(
                    "chat-markdown chat-markdown--assistant text-base leading-[1.625]",
                    markdownClassName
                  )}
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
              className={cn(
                "chat-markdown text-base leading-[1.625]",
                isAssistant ? "chat-markdown--assistant" : "chat-markdown--user",
                markdownClassName
              )}
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

        <div className={cn("flex items-center gap-2 px-1", !isAssistant && "justify-end")}>
          <Button
            type="button"
            variant="quiet"
            size="sm"
            className="h-7 rounded-full px-3 text-[11px] text-[#6d7a77] hover:text-[#191c1e]"
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
              className="h-7 rounded-full px-3 text-[11px] text-[#6d7a77] hover:text-[#191c1e]"
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

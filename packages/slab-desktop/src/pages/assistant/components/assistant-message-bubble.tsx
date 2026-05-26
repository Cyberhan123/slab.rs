import { CodeHighlighter, Think, ThoughtChain, type ThoughtChainItemType } from "@ant-design/x"
import { BotMessageSquare, CheckCircle2, Copy, UserRound, XCircle } from "lucide-react"

import { Button } from "@slab/components/button"
import { useTranslation } from "@slab/i18n"
import {
  getAssistantMessageTextContent,
  stripTrailingAssistantTurnArtifacts,
  type AssistantMessageRecord,
  type AssistantThought,
} from "@/pages/assistant/assistant-context"
import { cn } from "@/lib/utils"

import { AssistantMarkdown } from "./assistant-markdown"

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
      answer: rawContent.slice(0, openTagIndex).trimEnd(),
      thinking: thinking || null,
      thinkingLoading: !thinkingMarkedDone,
    }
  }

  const thinking = rawContent.slice(openTagEnd + 1, closeTagIndex).trim()
  const before = rawContent.slice(0, openTagIndex)
  const after = rawContent.slice(closeTagIndex + closeTag.length)

  return {
    answer: `${before}${after}`.trimStart(),
    thinking: thinking || null,
    thinkingLoading: false,
  }
}

function guessCodeLanguage(value: string) {
  const trimmed = value.trim()
  if (trimmed.startsWith("diff --git") || trimmed.startsWith("--- ") || trimmed.startsWith("*** ")) {
    return "diff"
  }

  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    return "json"
  }

  return "text"
}

function renderThoughtContent(
  thought: AssistantThought,
  approving: boolean,
  onApprove: ((approved: boolean) => void) | undefined,
  labels: {
    approve: string
    reject: string
  }
) {
  if (thought.pendingApproval) {
    return (
      <div className="space-y-3">
        <CodeHighlighter lang="shell" className="rounded-[14px] border border-border/60 text-xs">
          {thought.pendingApproval.command}
        </CodeHighlighter>
        <div className="flex justify-end gap-2">
          <Button
            variant="quiet"
            size="sm"
            onClick={() => onApprove?.(false)}
            disabled={approving}
          >
            <XCircle className="size-4" />
            {labels.reject}
          </Button>
          <Button
            variant="pill"
            size="sm"
            onClick={() => onApprove?.(true)}
            disabled={approving}
          >
            <CheckCircle2 className="size-4" />
            {labels.approve}
          </Button>
        </div>
      </div>
    )
  }

  return thought.detail ? (
    <CodeHighlighter
      lang={guessCodeLanguage(thought.detail)}
      className="rounded-[14px] border border-border/60 text-xs"
    >
      {thought.detail}
    </CodeHighlighter>
  ) : null
}

function toThoughtChainItems(
  thoughts: AssistantThought[] | undefined,
  approving: boolean,
  onApprove: ((approved: boolean) => void) | undefined,
  labels: {
    approve: string
    reject: string
  }
): ThoughtChainItemType[] {
  return (thoughts ?? []).map((thought) => ({
    blink: thought.status === "loading",
    collapsible: true,
    content: renderThoughtContent(thought, approving, onApprove, labels),
    description: thought.toolName ?? thought.callId,
    key: thought.id,
    status: thought.status,
    title: thought.title,
  }))
}

type AssistantMessageBubbleProps = {
  approving?: boolean
  item: AssistantMessageRecord
  markdownClassName?: string
  onApprove?: (approved: boolean) => void
}

export function AssistantMessageBubble({
  approving = false,
  item,
  markdownClassName,
  onApprove,
}: AssistantMessageBubbleProps) {
  const { t } = useTranslation()
  const role = item.message.role
  const isAssistant = role === "assistant"
  const isBusy = item.status === "loading" || item.status === "updating"
  const hasNextChunk = item.status === "updating"
  const rawContent = stripTrailingAssistantTurnArtifacts(getAssistantMessageTextContent(item.message))
  const parsed = parseThinkingContent(rawContent)
  const liveThinking = typeof item.message.reasoningContent === "string"
    ? item.message.reasoningContent.trim()
    : ""
  const thinking = liveThinking || parsed.thinking
  const answer = liveThinking
    ? (rawContent.includes("<think") ? parsed.answer : rawContent)
    : parsed.answer
  const thinkingLoading = liveThinking ? isBusy : parsed.thinkingLoading
  const thoughtItems = toThoughtChainItems(item.message.thoughts, approving, onApprove, {
    approve: t("pages.assistant.actions.approve"),
    reject: t("pages.assistant.actions.reject"),
  })

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
                ? "rounded-[8px] bg-[var(--brand-teal)] text-[var(--brand-teal-foreground)]"
                : "rounded-full border border-border/30 bg-[var(--shell-card)] text-foreground/70"
            )}
          >
            {isAssistant ? (
              <BotMessageSquare className="size-3.5" />
            ) : (
              <UserRound className="size-3.5" />
            )}
          </span>
          <span className="text-[11px] font-bold uppercase tracking-[0.14em] text-muted-foreground">
            {isAssistant ? t("pages.assistant.message.assistant") : t("pages.assistant.message.user")}
          </span>
        </div>

        <div
          className={cn(
            "px-6 py-4 shadow-[var(--shell-elevation)]",
            isAssistant
              ? "rounded-tl-[24px] rounded-tr-[24px] rounded-br-[24px] rounded-bl-[8px] bg-[var(--ai-bubble)] text-[var(--ai-bubble-foreground)]"
              : "rounded-tl-[24px] rounded-tr-[24px] rounded-br-[8px] rounded-bl-[24px] bg-[var(--user-bubble)] text-[var(--user-bubble-foreground)]",
            item.status === "error" && "border border-destructive/35"
          )}
        >
          {thinking ? (
            <Think
              title={thinkingLoading && isBusy ? t("pages.assistant.thinking.loading") : t("pages.assistant.thinking.ready")}
              loading={thinkingLoading && isBusy}
              blink={thinkingLoading && isBusy}
              defaultExpanded
              className="mb-4 rounded-[18px] border border-border/50 bg-background/35 px-4 py-3"
            >
              <AssistantMarkdown
                className={cn("assistant-markdown--assistant", markdownClassName)}
                hasNextChunk={hasNextChunk}
              >
                {thinking}
              </AssistantMarkdown>
            </Think>
          ) : null}

          {isAssistant && thoughtItems.length > 0 ? (
            <ThoughtChain
              items={thoughtItems}
              defaultExpandedKeys={thoughtItems.map((thought) => thought.key).filter((key): key is string => Boolean(key))}
              className="mb-4 rounded-[18px] border border-border/50 bg-background/30 px-4 py-3"
            />
          ) : null}

          {answer ? (
            <AssistantMarkdown
              className={cn(
                isAssistant ? "assistant-markdown--assistant" : "assistant-markdown--user",
                markdownClassName
              )}
              hasNextChunk={hasNextChunk}
            >
              {answer}
            </AssistantMarkdown>
          ) : isBusy ? (
            <p className="text-sm opacity-80">{t("pages.assistant.message.waitingForResponse")}</p>
          ) : null}
        </div>

        <div className={cn("flex items-center gap-2 px-1", !isAssistant && "justify-end")}>
          <Button
            type="button"
            variant="quiet"
            size="sm"
            className="h-7 rounded-full px-3 text-[11px] text-muted-foreground hover:text-foreground"
            onClick={() => void copyMessage()}
          >
            <Copy className="size-3.5" />
            {t("pages.assistant.message.copy")}
          </Button>
        </div>
      </div>
    </div>
  )
}

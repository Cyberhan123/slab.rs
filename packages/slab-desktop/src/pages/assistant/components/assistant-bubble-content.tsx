import {
  CodeHighlighter,
  ThoughtChain,
  type BubbleListProps,
  type ThoughtChainItemType,
} from "@ant-design/x"
import { useClipboard } from "@mantine/hooks"
import { BotMessageSquare, CheckCircle2, Copy, UserRound, XCircle } from "lucide-react"
import { memo, useMemo } from "react"

import { Button } from "@slab/components/button"
import { cn } from "@/lib/utils"

import {
  getAssistantMessageTextContent,
  stripTrailingAssistantTurnArtifacts,
  type AssistantMessageRecord,
  type AssistantThought,
} from "../assistant-context"
import { AssistantMarkdown } from "./assistant-markdown"

export type AssistantBubbleContent = {
  approving: boolean
  item: AssistantMessageRecord
  labels: {
    approve: string
    assistant: string
    copy: string
    reject: string
    thinkingLoading: string
    thinkingReady: string
    user: string
    waitingForResponse: string
  }
  markdownClassName?: string
  onApprove?: (approved: boolean) => void
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
    return {
      thinking: null,
      answer: rawContent.slice(0, openTagIndex).trimEnd(),
      thinkingLoading: true,
    }
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
      <div className="min-w-0 max-w-full space-y-3 overflow-hidden">
        <div className="min-w-0 max-w-full overflow-x-auto">
          <CodeHighlighter
            lang="shell"
            prismLightMode={false}
            className="max-w-full rounded-[14px] border border-border/60 text-xs"
          >
            {thought.pendingApproval.command}
          </CodeHighlighter>
        </div>
        <div className="flex flex-wrap justify-end gap-2">
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
    <div className="min-w-0 max-w-full overflow-x-auto">
      <CodeHighlighter
        lang={guessCodeLanguage(thought.detail)}
        className="max-w-full rounded-[14px] border border-border/60 text-xs"
      >
        {thought.detail}
      </CodeHighlighter>
    </div>
  ) : null
}

function toThoughtChainItems(
  thoughts: AssistantThought[] | undefined,
  approving: boolean,
  onApprove: ((approved: boolean) => void) | undefined,
  labels: {
    approve: string
    reject: string
  },
  thinking?: {
    content: string
    key: string
    loading: boolean
    title: string
  }
): ThoughtChainItemType[] {
  const items = (thoughts ?? []).map((thought) => ({
    blink: thought.status === "loading",
    collapsible: true,
    content: renderThoughtContent(thought, approving, onApprove, labels),
    description: thought.summary ?? thought.toolName ?? thought.callId,
    icon: false,
    key: thought.id,
    status: thought.status,
    title: thought.title,
  }))

  if (!thinking?.content) {
    return items
  }

  return [
    {
      blink: thinking.loading,
      collapsible: true,
      content: (
        <AssistantMarkdown
          className="assistant-markdown--assistant"
          hasNextChunk={thinking.loading}
        >
          {thinking.content}
        </AssistantMarkdown>
      ),
      icon: false,
      key: thinking.key,
      status: thinking.loading ? "loading" : "success",
      title: thinking.title,
    },
    ...items,
  ]
}

const AssistantBubbleContentView = memo(function AssistantBubbleContentView({
  content,
}: {
  content: AssistantBubbleContent
}) {
  const role = content.item.message.role
  const isAssistant = role === "assistant"
  const isBusy = content.item.status === "loading" || content.item.status === "updating"
  const hasNextChunk = content.item.status === "updating"
  const rawContent = stripTrailingAssistantTurnArtifacts(
    getAssistantMessageTextContent(content.item.message)
  )
  const parsed = useMemo(() => parseThinkingContent(rawContent), [rawContent])
  const liveThinking =
    typeof content.item.message.reasoningContent === "string"
      ? content.item.message.reasoningContent.trim()
      : ""
  const thinking = liveThinking || parsed.thinking
  const answer = liveThinking
    ? rawContent.includes("<think")
      ? parsed.answer
      : rawContent
    : parsed.answer
  const thinkingLoading = liveThinking ? isBusy : parsed.thinkingLoading
  const thoughtItems = useMemo(
    () =>
      toThoughtChainItems(
        content.item.message.thoughts,
        content.approving,
        content.onApprove,
        {
          approve: content.labels.approve,
          reject: content.labels.reject,
        },
        thinking
          ? {
              content: thinking,
              key: `${content.item.id}-thinking`,
              loading: thinkingLoading && isBusy,
              title:
                thinkingLoading && isBusy
                  ? content.labels.thinkingLoading
                  : content.labels.thinkingReady,
            }
          : undefined
      ),
    [
      content.approving,
      content.item.id,
      content.item.message.thoughts,
      content.labels.approve,
      content.labels.reject,
      content.labels.thinkingLoading,
      content.labels.thinkingReady,
      content.onApprove,
      isBusy,
      thinking,
      thinkingLoading,
    ]
  )
  const expandedThoughtKeys = useMemo(
    () => thoughtItems.map((thought) => thought.key).filter((key): key is string => Boolean(key)),
    [thoughtItems]
  )

  return (
    <div className="min-w-0 max-w-full space-y-4 overflow-hidden">
      {isAssistant && thoughtItems.length > 0 ? (
        <ThoughtChain
          items={thoughtItems}
          defaultExpandedKeys={expandedThoughtKeys}
          className="min-w-0 max-w-full overflow-hidden rounded-[18px] border border-border/50 bg-background/30 px-4 py-3"
        />
      ) : null}

      {answer ? (
        <AssistantMarkdown
          className={cn(
            isAssistant ? "assistant-markdown--assistant" : "assistant-markdown--user",
            content.markdownClassName
          )}
          hasNextChunk={hasNextChunk}
        >
          {answer}
        </AssistantMarkdown>
      ) : isBusy ? (
        <p className="text-sm opacity-80">{content.labels.waitingForResponse}</p>
      ) : null}
    </div>
  )
})

function renderAssistantBubbleContent(content: AssistantBubbleContent) {
  return <AssistantBubbleContentView content={content} />
}

function AssistantCopyButton({ content }: { content: AssistantBubbleContent }) {
  const clipboard = useClipboard()

  return (
    <Button
      type="button"
      variant="quiet"
      size="sm"
      className="h-7 rounded-full px-3 text-[11px] text-muted-foreground hover:text-foreground"
      onClick={() => clipboard.copy(getAssistantMessageTextContent(content.item.message))}
    >
      <Copy className="size-3.5" />
      {content.labels.copy}
    </Button>
  )
}

export const ASSISTANT_BUBBLE_ROLES = {
  assistant: {
    avatar: (
      <span className="flex size-6 shrink-0 items-center justify-center rounded-[8px] bg-[var(--brand-teal)] text-[var(--brand-teal-foreground)]">
        <BotMessageSquare className="size-3.5" />
      </span>
    ),
    contentRender: renderAssistantBubbleContent,
    footer: (content: AssistantBubbleContent) => <AssistantCopyButton content={content} />,
    header: (content: AssistantBubbleContent) => content.labels.assistant,
    placement: "start",
    shape: "corner",
    styles: {
      content: {
        background: "var(--ai-bubble)",
        color: "var(--ai-bubble-foreground)",
        maxWidth: "min(100%, 42rem)",
        minWidth: 0,
        overflow: "hidden",
      },
    },
    variant: "filled",
  },
  user: {
    avatar: (
      <span className="flex size-6 shrink-0 items-center justify-center rounded-full border border-border/30 bg-[var(--shell-card)] text-foreground/70">
        <UserRound className="size-3.5" />
      </span>
    ),
    contentRender: renderAssistantBubbleContent,
    footer: (content: AssistantBubbleContent) => <AssistantCopyButton content={content} />,
    header: (content: AssistantBubbleContent) => content.labels.user,
    placement: "end",
    shape: "corner",
    styles: {
      content: {
        background: "var(--user-bubble)",
        color: "var(--user-bubble-foreground)",
        maxWidth: "min(100%, 42rem)",
        minWidth: 0,
        overflow: "hidden",
      },
    },
    variant: "filled",
  },
  system: {
    shape: "round",
    styles: {
      content: {
        maxWidth: "100%",
        minWidth: 0,
        overflow: "hidden",
      },
    },
    variant: "outlined",
  },
} satisfies BubbleListProps["role"]

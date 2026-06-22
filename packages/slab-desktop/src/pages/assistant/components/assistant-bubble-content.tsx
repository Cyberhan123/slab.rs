import {
  CodeHighlighter,
  ThoughtChain,
  type BubbleListProps,
  type ThoughtChainItemType,
} from "@ant-design/x"
import { useClipboard } from "@mantine/hooks"
import {
  AlertCircle,
  BotMessageSquare,
  Check,
  CheckCircle2,
  Copy,
  Pencil,
  RotateCcw,
  UserRound,
  XCircle,
} from "lucide-react"
import { memo, useMemo, useState } from "react"

import { Button } from "@slab/components/button"
import { cn } from "@/lib/utils"

import {
  getAssistantMessageTextContent,
  stripThinkTags,
  stripTrailingAssistantTurnArtifacts,
  type AssistantMessageRecord,
  type AssistantThought,
} from "../assistant-context"
import { AssistantMarkdown } from "./assistant-markdown"

export type AssistantBubbleContent = {
  approvingCallIds: string[]
  item: AssistantMessageRecord
  labels: {
    approve: string
    assistant: string
    cancelEdit: string
    copy: string
    edit: string
    regenerate: string
    reject: string
    retry: string
    saveEdit: string
    terminalCancelled: string
    thinkingLoading: string
    thinkingReady: string
    user: string
    waitingForResponse: string
  }
  markdownClassName?: string
  onApprove?: (callId: string, approved: boolean) => void
  onEdit?: (messageId: string, nextContent: string) => void | Promise<void>
  onRegenerate?: (messageId: string) => void | Promise<void>
  onRetry?: () => void
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

function formatJsonCode(value: string) {
  const trimmed = value.trim()
  if (!trimmed || (trimmed[0] !== "{" && trimmed[0] !== "[")) {
    return value
  }

  try {
    return JSON.stringify(JSON.parse(trimmed), null, 2)
  } catch {
    return value
  }
}

function renderThoughtContent(
  thought: AssistantThought,
  approving: boolean,
  onApprove: ((callId: string, approved: boolean) => void) | undefined,
  labels: {
    approve: string
    reject: string
  }
) {
  if (thought.pendingApproval) {
    const callId = thought.pendingApproval.callId
    return (
      <div
        className="min-w-0 max-w-full space-y-3 overflow-hidden"
        data-testid={`assistant-thought-${thought.id}`}
      >
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
            data-testid={`thought-reject-${callId}`}
            onClick={() => onApprove?.(callId, false)}
            disabled={approving}
          >
            <XCircle className="size-4" />
            {labels.reject}
          </Button>
          <Button
            variant="pill"
            size="sm"
            data-testid={`thought-approve-${callId}`}
            onClick={() => onApprove?.(callId, true)}
            disabled={approving}
          >
            <CheckCircle2 className="size-4" />
            {labels.approve}
          </Button>
        </div>
      </div>
    )
  }

  if (!thought.detail) {
    return null
  }

  const language = guessCodeLanguage(thought.detail)
  const detail = language === "json" ? formatJsonCode(thought.detail) : thought.detail

  return (
    <div className="min-w-0 max-w-full overflow-x-auto" data-testid={`assistant-thought-${thought.id}`}>
      <CodeHighlighter
        lang={language}
        className="max-w-full rounded-[14px] border border-border/60 text-xs"
      >
        {detail}
      </CodeHighlighter>
    </div>
  )
}

function toThoughtChainItems(
  thoughts: AssistantThought[] | undefined,
  approvingCallIds: string[],
  onApprove: ((callId: string, approved: boolean) => void) | undefined,
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
    content: renderThoughtContent(
      thought,
      Boolean(thought.callId && approvingCallIds.includes(thought.callId)),
      onApprove,
      labels
    ),
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
        <div data-testid={`assistant-thinking-${thinking.key.replace(/-thinking$/, "")}`}>
          <AssistantMarkdown
            className="assistant-markdown--assistant"
            hasNextChunk={thinking.loading}
          >
            {thinking.content}
          </AssistantMarkdown>
        </div>
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
        content.approvingCallIds,
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
      content.approvingCallIds,
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
    <div
      className="min-w-0 max-w-full space-y-4 overflow-hidden"
      data-testid={`assistant-message-${content.item.id}`}
    >
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

function AssistantBubbleFooter({ content }: { content: AssistantBubbleContent }) {
  const clipboard = useClipboard()
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(() => getAssistantMessageTextContent(content.item.message))
  const isAssistant = content.item.message.role === "assistant"
  const isBusy = content.item.status === "loading" || content.item.status === "updating"
  const textContent = stripThinkTags(getAssistantMessageTextContent(content.item.message))
  const terminalNotice = content.item.message.role === "assistant"
    ? content.item.message.terminalNotice
    : undefined

  return (
    <div className="flex max-w-[min(100%,42rem)] flex-col gap-2">
      {terminalNotice ? (
        <div
          className={cn(
            "flex items-start gap-2 rounded-xl border px-3 py-2 text-xs leading-5",
            terminalNotice.type === "error"
              ? "border-destructive/30 bg-destructive/10 text-destructive"
              : "border-border/60 bg-[var(--surface-soft)] text-muted-foreground"
          )}
          data-testid={`assistant-terminal-notice-${content.item.id}`}
        >
          <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
          <span className="min-w-0 break-words">
            {terminalNotice.type === "cancelled"
              ? content.labels.terminalCancelled
              : terminalNotice.message}
          </span>
        </div>
      ) : null}
      {editing ? (
        <form
          className="flex flex-col gap-2 rounded-xl border border-border/60 bg-background/70 p-2"
          onSubmit={(event) => {
            event.preventDefault()
            const nextContent = draft.trim()
            if (!nextContent) {
              return
            }
            setEditing(false)
            void content.onEdit?.(String(content.item.id), nextContent)
          }}
        >
          <textarea
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            className="min-h-24 resize-y rounded-lg border border-border/60 bg-background px-3 py-2 text-sm outline-none focus:border-[var(--brand-teal)]"
            data-testid={`assistant-edit-${content.item.id}`}
            aria-label={content.labels.edit}
          />
          <div className="flex justify-end gap-2">
            <Button
              type="button"
              variant="quiet"
              size="sm"
              className="h-7 rounded-full px-3 text-[11px]"
              onClick={() => {
                setDraft(getAssistantMessageTextContent(content.item.message))
                setEditing(false)
              }}
            >
              <XCircle className="size-3.5" />
              {content.labels.cancelEdit}
            </Button>
            <Button
              type="submit"
              variant="pill"
              size="sm"
              className="h-7 rounded-full px-3 text-[11px]"
              disabled={!draft.trim()}
              data-testid={`assistant-save-edit-${content.item.id}`}
            >
              <Check className="size-3.5" />
              {content.labels.saveEdit}
            </Button>
          </div>
        </form>
      ) : null}
      <div className="flex flex-wrap items-center gap-2">
        <Button
          type="button"
          variant="quiet"
          size="sm"
          className="h-7 rounded-full px-3 text-[11px] text-muted-foreground hover:text-foreground"
          onClick={() => clipboard.copy(textContent)}
        >
          <Copy className="size-3.5" />
          {content.labels.copy}
        </Button>
        {!isAssistant && content.onEdit ? (
          <Button
            type="button"
            variant="quiet"
            size="sm"
            className="h-7 rounded-full px-3 text-[11px] text-muted-foreground hover:text-foreground"
            disabled={isBusy}
            onClick={() => {
              setDraft(getAssistantMessageTextContent(content.item.message))
              setEditing(true)
            }}
            data-testid={`assistant-edit-button-${content.item.id}`}
          >
            <Pencil className="size-3.5" />
            {content.labels.edit}
          </Button>
        ) : null}
        {isAssistant && !isBusy && content.onRegenerate ? (
          <Button
            type="button"
            variant="quiet"
            size="sm"
            className="h-7 rounded-full px-3 text-[11px] text-muted-foreground hover:text-foreground"
            onClick={() => {
              void content.onRegenerate?.(String(content.item.id))
            }}
            data-testid={`assistant-regenerate-${content.item.id}`}
          >
            <RotateCcw className="size-3.5" />
            {content.labels.regenerate}
          </Button>
        ) : null}
        {terminalNotice?.type === "error" ? (
          <Button
            type="button"
            variant="quiet"
            size="sm"
            className="h-7 rounded-full px-3 text-[11px] text-muted-foreground hover:text-foreground"
            onClick={content.onRetry}
          >
            <RotateCcw className="size-3.5" />
            {content.labels.retry}
          </Button>
        ) : null}
      </div>
    </div>
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
    footer: (content: AssistantBubbleContent) => <AssistantBubbleFooter content={content} />,
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
    footer: (content: AssistantBubbleContent) => <AssistantBubbleFooter content={content} />,
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

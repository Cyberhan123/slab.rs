import { useClipboard } from '@mantine/hooks'
import {
  AlertCircle,
  BotMessageSquare,
  Check,
  CheckCircle2,
  ChevronDown,
  Copy,
  Pencil,
  RotateCcw,
  UserRound,
  XCircle,
} from 'lucide-react'
import { memo, useMemo, useState, type ReactNode } from 'react'

import { Badge } from '@slab/components/badge'
import { Button } from '@slab/components/button'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@slab/components/collapsible'
import { Textarea } from '@slab/components/textarea'
import { cn } from '@/lib/utils'

import {
  getAssistantMessageTextContent,
  stripThinkTags,
  stripTrailingAssistantTurnArtifacts,
  type AssistantMessageRecord,
  type AssistantThought,
  type AssistantThoughtStatus,
} from '../assistant-context'
import { AgentActionCard } from './agent-action-card'
import { AssistantMarkdown } from './message/markdown'

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
    taskActionBlockedPath: string
    taskActionFeedback: string
    taskActionOpen: string
    taskActionReview: string
    taskActionTitle: string
    terminalCancelled: string
    thinkingLoading: string
    thinkingReady: string
    user: string
    waitingForResponse: string
  }
  onApprove?: (callId: string, approved: boolean) => void
  onEdit?: (messageId: string, nextContent: string) => void | Promise<void>
  onFeedback?: (prompt: string) => void
  onRegenerate?: (messageId: string) => void | Promise<void>
  onRetry?: () => void
}

type ParsedThinkingContent = {
  thinking: string | null
  answer: string
  thinkingLoading: boolean
}

type ReasoningTraceItem = {
  content: ReactNode
  description?: string
  key: string
  loading: boolean
  status: AssistantThoughtStatus
  title: string
}

export function AssistantMessageAvatar({ role }: { role: 'assistant' | 'user' }) {
  if (role === 'assistant') {
    return (
      <span className="flex size-6 shrink-0 items-center justify-center rounded-[8px] bg-[var(--brand-teal)] text-[color:var(--brand-teal-foreground)]">
        <BotMessageSquare />
      </span>
    )
  }

  return (
    <span className="flex size-6 shrink-0 items-center justify-center rounded-full border border-border/30 bg-[var(--shell-card)] text-foreground/70">
      <UserRound />
    </span>
  )
}

export function getAssistantMessageLabel(content: AssistantBubbleContent) {
  return content.item.message.role === 'assistant' ? content.labels.assistant : content.labels.user
}

function parseThinkingContent(rawContent: string): ParsedThinkingContent {
  const openTagIndex = rawContent.indexOf('<think')
  if (openTagIndex < 0) {
    return { thinking: null, answer: rawContent, thinkingLoading: false }
  }

  const openTagEnd = rawContent.indexOf('>', openTagIndex)
  if (openTagEnd < 0) {
    return {
      thinking: null,
      answer: rawContent.slice(0, openTagIndex).trimEnd(),
      thinkingLoading: true,
    }
  }

  const openTag = rawContent.slice(openTagIndex, openTagEnd + 1)
  const thinkingMarkedDone = /\bstatus\s*=\s*["']?done["']?/i.test(openTag)
  const closeTag = '</think>'
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
  if (trimmed.startsWith('diff --git') || trimmed.startsWith('--- ') || trimmed.startsWith('*** ')) {
    return 'diff'
  }

  if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
    return 'json'
  }

  return 'text'
}

function formatJsonCode(value: string) {
  const trimmed = value.trim()
  if (!trimmed || (trimmed[0] !== '{' && trimmed[0] !== '[')) {
    return value
  }

  try {
    return JSON.stringify(JSON.parse(trimmed), null, 2)
  } catch {
    return value
  }
}

function AssistantCodeBlock({ value }: { value: string }) {
  const language = guessCodeLanguage(value)
  const detail = language === 'json' ? formatJsonCode(value) : value

  return (
    <div className="min-w-0 max-w-full overflow-x-auto rounded-[14px] border border-border/60 bg-background/80 text-xs">
      <div className="border-b border-border/60 px-3 py-2 font-medium text-muted-foreground">
        {language}
      </div>
      <pre className="m-0 max-w-full overflow-x-auto p-3">
        <code>{detail}</code>
      </pre>
    </div>
  )
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
        className="flex min-w-0 max-w-full flex-col gap-3 overflow-hidden"
        data-testid={`assistant-thought-${thought.id}`}
      >
        <AssistantCodeBlock value={thought.pendingApproval.command} />
        <div className="flex flex-wrap justify-end gap-2">
          <Button
            variant="ghost"
            size="sm"
            data-testid={`thought-reject-${callId}`}
            onClick={() => onApprove?.(callId, false)}
            disabled={approving}
          >
            <XCircle />
            {labels.reject}
          </Button>
          <Button
            size="sm"
            data-testid={`thought-approve-${callId}`}
            onClick={() => onApprove?.(callId, true)}
            disabled={approving}
          >
            <CheckCircle2 />
            {labels.approve}
          </Button>
        </div>
      </div>
    )
  }

  if (!thought.detail) {
    return null
  }

  return (
    <div className="min-w-0 max-w-full" data-testid={`assistant-thought-${thought.id}`}>
      <AssistantCodeBlock value={thought.detail} />
    </div>
  )
}

function toReasoningTraceItems(
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
): ReasoningTraceItem[] {
  const items = (thoughts ?? []).map<ReasoningTraceItem>((thought) => ({
    content: renderThoughtContent(
      thought,
      Boolean(thought.callId && approvingCallIds.includes(thought.callId)),
      onApprove,
      labels
    ),
    description: thought.summary ?? thought.toolName ?? thought.callId,
    key: thought.id,
    loading: thought.status === 'loading',
    status: thought.status,
    title: thought.title,
  }))

  if (!thinking?.content) {
    return items
  }

  return [
    {
      content: (
        <div data-testid={`assistant-thinking-${thinking.key.replace(/-thinking$/, '')}`}>
          <AssistantMarkdown
            className="assistant-markdown--assistant"
            hasNextChunk={thinking.loading}
          >
            {thinking.content}
          </AssistantMarkdown>
        </div>
      ),
      key: thinking.key,
      loading: thinking.loading,
      status: thinking.loading ? 'loading' : 'success',
      title: thinking.title,
    },
    ...items,
  ]
}

function AssistantReasoningTrace({ items }: { items: ReasoningTraceItem[] }) {
  return (
    <div className="flex min-w-0 max-w-full flex-col gap-2 overflow-hidden rounded-[18px] border border-border/50 bg-background/30 px-4 py-3">
      {items.map((item) => (
        <Collapsible key={item.key} defaultOpen>
          <CollapsibleTrigger asChild>
            <button
              type="button"
              className="flex w-full min-w-0 items-center justify-between gap-3 rounded-md px-1 py-2 text-left text-sm transition hover:bg-muted/50"
            >
              <span className="flex min-w-0 flex-col gap-1">
                <span className={cn('font-medium', item.loading && 'shimmer')}>
                  {item.title}
                </span>
                {item.description ? (
                  <span className="truncate text-xs text-muted-foreground">
                    {item.description}
                  </span>
                ) : null}
              </span>
              <span className="flex shrink-0 items-center gap-2">
                <Badge
                  variant={item.status === 'error' ? 'destructive' : 'secondary'}
                  className="capitalize"
                >
                  {item.status}
                </Badge>
                <ChevronDown />
              </span>
            </button>
          </CollapsibleTrigger>
          <CollapsibleContent className="px-1 pb-3">
            {item.content}
          </CollapsibleContent>
        </Collapsible>
      ))}
    </div>
  )
}

const AssistantBubbleContentView = memo(function AssistantBubbleContentView({
  content,
}: {
  content: AssistantBubbleContent
}) {
  const role = content.item.message.role
  const isAssistant = role === 'assistant'
  const isBusy = content.item.status === 'loading' || content.item.status === 'updating'
  const hasNextChunk = content.item.status === 'updating'
  const rawContent = stripTrailingAssistantTurnArtifacts(
    getAssistantMessageTextContent(content.item.message)
  )
  const parsed = useMemo(() => parseThinkingContent(rawContent), [rawContent])
  const liveThinking =
    typeof content.item.message.reasoningContent === 'string'
      ? content.item.message.reasoningContent.trim()
      : ''
  const thinking = liveThinking || parsed.thinking
  const answer = liveThinking
    ? rawContent.includes('<think')
      ? parsed.answer
      : rawContent
    : parsed.answer
  const thinkingLoading = liveThinking ? isBusy : parsed.thinkingLoading
  const thoughtItems = useMemo(
    () =>
      toReasoningTraceItems(
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

  return (
    <div
      className="flex min-w-0 max-w-full flex-col gap-4 overflow-hidden"
      data-testid={`assistant-message-${content.item.id}`}
    >
      {isAssistant && thoughtItems.length > 0 ? (
        <AssistantReasoningTrace items={thoughtItems} />
      ) : null}

      {answer ? (
        <AssistantMarkdown
          className={cn(
            isAssistant ? 'assistant-markdown--assistant' : 'assistant-markdown--user'
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

export function AssistantBubbleContentViewByContent({
  content,
}: {
  content: AssistantBubbleContent
}) {
  return <AssistantBubbleContentView content={content} />
}

export function AssistantBubbleFooter({ content }: { content: AssistantBubbleContent }) {
  const clipboard = useClipboard()
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(() => getAssistantMessageTextContent(content.item.message))
  const isAssistant = content.item.message.role === 'assistant'
  const isBusy = content.item.status === 'loading' || content.item.status === 'updating'
  const textContent = stripThinkTags(getAssistantMessageTextContent(content.item.message))
  const terminalNotice = content.item.message.role === 'assistant'
    ? content.item.message.terminalNotice
    : undefined
  const artifactRefs = isAssistant ? content.item.message.artifactRefs ?? [] : []

  return (
    <div className="flex max-w-[min(100%,42rem)] flex-col gap-2">
      {artifactRefs.length > 0 ? (
        <AgentActionCard
          artifactRefs={artifactRefs}
          labels={{
            blockedPath: content.labels.taskActionBlockedPath,
            feedback: content.labels.taskActionFeedback,
            open: content.labels.taskActionOpen,
            review: content.labels.taskActionReview,
            title: content.labels.taskActionTitle,
          }}
          onFeedback={(prompt) => content.onFeedback?.(prompt)}
        />
      ) : null}
      {terminalNotice ? (
        <div
          className={cn(
            'flex items-start gap-2 rounded-xl border px-3 py-2 text-xs leading-5',
            terminalNotice.type === 'error'
              ? 'border-destructive/30 bg-destructive/10 text-destructive'
              : 'border-border/60 bg-[var(--surface-soft)] text-muted-foreground'
          )}
          data-testid={`assistant-terminal-notice-${content.item.id}`}
        >
          <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
          <span className="min-w-0 break-words">
            {terminalNotice.type === 'cancelled'
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
          <Textarea
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            className="min-h-24 resize-y text-sm"
            data-testid={`assistant-edit-${content.item.id}`}
            aria-label={content.labels.edit}
          />
          <div className="flex justify-end gap-2">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => {
                setDraft(getAssistantMessageTextContent(content.item.message))
                setEditing(false)
              }}
            >
              <XCircle />
              {content.labels.cancelEdit}
            </Button>
            <Button
              type="submit"
              size="sm"
              disabled={!draft.trim()}
              data-testid={`assistant-save-edit-${content.item.id}`}
            >
              <Check />
              {content.labels.saveEdit}
            </Button>
          </div>
        </form>
      ) : null}
      <div className="flex flex-wrap items-center gap-2">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => clipboard.copy(textContent)}
        >
          <Copy />
          {content.labels.copy}
        </Button>
        {!isAssistant && content.onEdit ? (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={isBusy}
            onClick={() => {
              setDraft(getAssistantMessageTextContent(content.item.message))
              setEditing(true)
            }}
            data-testid={`assistant-edit-button-${content.item.id}`}
          >
            <Pencil />
            {content.labels.edit}
          </Button>
        ) : null}
        {isAssistant && !isBusy && content.onRegenerate ? (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => {
              void content.onRegenerate?.(String(content.item.id))
            }}
            data-testid={`assistant-regenerate-${content.item.id}`}
          >
            <RotateCcw />
            {content.labels.regenerate}
          </Button>
        ) : null}
        {terminalNotice?.type === 'error' ? (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={content.onRetry}
          >
            <RotateCcw />
            {content.labels.retry}
          </Button>
        ) : null}
      </div>
    </div>
  )
}

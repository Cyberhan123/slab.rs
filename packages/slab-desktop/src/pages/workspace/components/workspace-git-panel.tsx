import { GitBranch, GitCommitHorizontal, Loader2, Minus, Plus, RefreshCcw, RotateCcw } from "lucide-react"
import { useMemo, useState } from "react"

import { Button } from "@slab/components/button"
import { useTranslation } from "@slab/i18n"
import {
  type WorkspaceGitFileStatus,
  type WorkspaceGitStatus,
  type WorkspaceGitStatusEntry,
} from "@/lib/workspace-bridge"
import { cn } from "@/lib/utils"

const statusClassName: Record<WorkspaceGitFileStatus, string> = {
  added: "bg-emerald-500/12 text-emerald-600 dark:text-emerald-300",
  modified: "bg-amber-500/12 text-amber-700 dark:text-amber-300",
  deleted: "bg-rose-500/12 text-rose-600 dark:text-rose-300",
  renamed: "bg-sky-500/12 text-sky-600 dark:text-sky-300",
  copied: "bg-indigo-500/12 text-indigo-600 dark:text-indigo-300",
  untracked: "bg-muted text-muted-foreground",
  conflicted: "bg-destructive/12 text-destructive",
}

type WorkspaceGitPanelProps = {
  gitStatus: WorkspaceGitStatus | undefined
  gitStatusFetching: boolean
  operationPending: boolean
  onCommit: (message: string) => Promise<void>
  onDiscard: (path: string) => Promise<void>
  onRefresh: () => Promise<void>
  onSelectDiff: (entry: WorkspaceGitStatusEntry) => void
  onStage: (path: string) => Promise<void>
  selectedEntry: WorkspaceGitStatusEntry | null
  onUnstage: (path: string) => Promise<void>
}

export function WorkspaceGitPanel({
  gitStatus,
  gitStatusFetching,
  operationPending,
  onCommit,
  onDiscard,
  onRefresh,
  onSelectDiff,
  onStage,
  selectedEntry,
  onUnstage,
}: WorkspaceGitPanelProps) {
  const { t } = useTranslation()
  const [commitMessage, setCommitMessage] = useState("")
  const stagedEntries = useMemo(() => gitStatus?.entries.filter((entry) => entry.staged) ?? [], [gitStatus])
  const unstagedEntries = useMemo(() => gitStatus?.entries.filter((entry) => !entry.staged) ?? [], [gitStatus])
  const hasChanges = (gitStatus?.entries.length ?? 0) > 0

  if (!gitStatus) {
    return (
      <div className="flex min-h-[240px] items-center justify-center">
        <Loader2 className="size-4 animate-spin text-muted-foreground" />
      </div>
    )
  }

  if (!gitStatus.available || !gitStatus.isRepository) {
    return (
      <div className="flex min-h-[240px] flex-col items-center justify-center gap-3 px-4 text-center text-sm text-muted-foreground">
        <GitBranch className="size-8 text-muted-foreground/70" />
        <p>{gitStatus.message ?? t("pages.workspace.git.notRepository")}</p>
      </div>
    )
  }

  const summaryItems = [
    ["added", gitStatus.summary.added],
    ["modified", gitStatus.summary.modified],
    ["deleted", gitStatus.summary.deleted],
    ["renamed", gitStatus.summary.renamed],
    ["copied", gitStatus.summary.copied],
    ["untracked", gitStatus.summary.untracked],
    ["conflicted", gitStatus.summary.conflicted],
  ] as const

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <form
        className="rounded-[12px] border border-border/50 bg-[var(--surface-1)] px-3 py-3"
        onSubmit={(event) => {
          event.preventDefault()
          const message = commitMessage.trim()
          if (!message) {
            return
          }
          void onCommit(message).then(() => setCommitMessage(""))
        }}
      >
        <div className="flex items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-2">
            <GitBranch className="size-4 shrink-0 text-[var(--brand-teal)]" />
            <span className="truncate text-sm font-semibold">
              {gitStatus.branch ?? t("pages.workspace.git.detached")}
            </span>
          </div>
          <Button
            type="button"
            variant="quiet"
            size="icon"
            className="size-7"
            onClick={() => {
              void onRefresh()
            }}
            aria-label={t("pages.workspace.git.refresh")}
          >
            {gitStatusFetching ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <RefreshCcw className="size-3.5" />
            )}
          </Button>
        </div>
        <div className="mt-3 flex flex-wrap gap-1.5">
          {summaryItems.map(([status, count]) =>
            count > 0 ? (
              <span
                key={status}
                className={cn(
                  "rounded-full px-2 py-0.5 text-[11px] font-medium",
                  statusClassName[status],
                )}
              >
                {t(`pages.workspace.git.status.${status}`)} {count}
              </span>
            ) : null,
          )}
          {gitStatus.entries.length === 0 ? (
            <span className="rounded-full bg-muted px-2 py-0.5 text-[11px] text-muted-foreground">
              {t("pages.workspace.git.clean")}
            </span>
          ) : null}
        </div>

        <input
          value={commitMessage}
          onChange={(event) => setCommitMessage(event.target.value)}
          className="mt-3 h-8 w-full rounded-[8px] border border-border/60 bg-background px-2 text-xs outline-none transition focus:border-[var(--brand-teal)]"
          placeholder={t("pages.workspace.git.commitPlaceholder")}
          disabled={operationPending || !hasChanges}
        />
        <div className="mt-2">
          <Button
            type="submit"
            variant="cta"
            size="sm"
            className="w-full min-w-0"
            disabled={operationPending || !hasChanges || !commitMessage.trim()}
          >
            {operationPending ? <Loader2 className="size-3.5 animate-spin" /> : <GitCommitHorizontal className="size-3.5" />}
            {t("pages.workspace.git.commit")}
          </Button>
        </div>
      </form>

      <div className="min-h-0 flex-1 overflow-y-auto rounded-[12px] bg-[var(--surface-1)] py-1">
        {gitStatus.entries.length > 0 ? (
          <div className="space-y-3 px-1">
            <GitEntryGroup
              title={t("pages.workspace.git.staged")}
              entries={stagedEntries}
              operationPending={operationPending}
              selectedEntry={selectedEntry}
              onDiscard={onDiscard}
              onSelectDiff={onSelectDiff}
              onStage={onStage}
              onUnstage={onUnstage}
            />
            <GitEntryGroup
              title={t("pages.workspace.git.changes")}
              entries={unstagedEntries}
              operationPending={operationPending}
              selectedEntry={selectedEntry}
              onDiscard={onDiscard}
              onSelectDiff={onSelectDiff}
              onStage={onStage}
              onUnstage={onUnstage}
            />
          </div>
        ) : (
          <div className="flex min-h-[180px] items-center justify-center px-4 text-center text-sm text-muted-foreground">
            {t("pages.workspace.git.cleanDescription")}
          </div>
        )}
      </div>
    </div>
  )
}

function GitEntryGroup({
  title,
  entries,
  operationPending,
  selectedEntry,
  onDiscard,
  onSelectDiff,
  onStage,
  onUnstage,
}: {
  title: string
  entries: WorkspaceGitStatusEntry[]
  operationPending: boolean
  selectedEntry: WorkspaceGitStatusEntry | null
  onDiscard: (path: string) => Promise<void>
  onSelectDiff: (entry: WorkspaceGitStatusEntry) => void
  onStage: (path: string) => Promise<void>
  onUnstage: (path: string) => Promise<void>
}) {
  const { t } = useTranslation()

  if (entries.length === 0) {
    return null
  }

  return (
    <section className="space-y-1">
      <div className="px-2 pt-2 text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
        {title}
      </div>
      {entries.map((entry) => {
        const canDiscard = entry.status !== "conflicted"
        const selected = selectedEntry?.path === entry.path && selectedEntry.staged === entry.staged

        return (
          <div
            key={`${entry.status}:${entry.staged}:${entry.path}`}
            className={cn(
              "group flex min-w-0 items-center gap-1 rounded-[8px] px-2 py-1.5 text-sm transition hover:bg-[var(--surface-selected)]",
              selected && "bg-[var(--surface-selected)]",
            )}
          >
            <button
              type="button"
              className="flex min-w-0 flex-1 items-center gap-2 text-left"
              onClick={() => {
                onSelectDiff(entry)
              }}
            >
              <GitCommitHorizontal className="size-4 shrink-0 text-muted-foreground" />
              <span className="min-w-0 flex-1 truncate">
                {entry.originalPath ? `${entry.originalPath} -> ${entry.path}` : entry.path}
              </span>
              <span
                className={cn(
                  "shrink-0 rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase",
                  statusClassName[entry.status],
                )}
              >
                {t(`pages.workspace.git.statusShort.${entry.status}`)}
              </span>
            </button>
            <Button
              type="button"
              variant="quiet"
              size="icon-xs"
              disabled={operationPending}
              title={entry.staged ? t("pages.workspace.git.unstage") : t("pages.workspace.git.stage")}
              onClick={() => {
                void (entry.staged ? onUnstage(entry.path) : onStage(entry.path))
              }}
            >
              {entry.staged ? <Minus className="size-3.5" /> : <Plus className="size-3.5" />}
            </Button>
            <Button
              type="button"
              variant="quiet"
              size="icon-xs"
              disabled={operationPending || !canDiscard}
              title={t("pages.workspace.git.discard")}
              onClick={() => {
                if (window.confirm(t("pages.workspace.confirm.discardGitChange", { path: entry.path }))) {
                  void onDiscard(entry.path)
                }
              }}
            >
              <RotateCcw className="size-3.5" />
            </Button>
          </div>
        )
      })}
    </section>
  )
}

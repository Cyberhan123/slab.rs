import { GitBranch, GitCommitHorizontal, Loader2, RefreshCcw } from "lucide-react"

import { Button } from "@slab/components/button"
import { useTranslation } from "@slab/i18n"
import type { WorkspaceGitFileStatus, WorkspaceGitStatus } from "@/lib/workspace-bridge"
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
  onOpenFile: (relativePath: string) => Promise<void>
  onRefresh: () => Promise<void>
}

export function WorkspaceGitPanel({
  gitStatus,
  gitStatusFetching,
  onOpenFile,
  onRefresh,
}: WorkspaceGitPanelProps) {
  const { t } = useTranslation()

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
      <div className="rounded-[12px] border border-border/50 bg-[var(--surface-1)] px-3 py-3">
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
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto rounded-[12px] bg-[var(--surface-1)] py-1">
        {gitStatus.entries.length > 0 ? (
          gitStatus.entries.map((entry) => {
            const canOpen = entry.status !== "deleted"

            return (
              <button
                key={`${entry.status}:${entry.path}`}
                type="button"
                disabled={!canOpen}
                className="flex w-full min-w-0 items-center gap-2 px-3 py-2 text-left text-sm transition enabled:hover:bg-[var(--surface-selected)] disabled:cursor-default disabled:opacity-70"
                onClick={() => {
                  if (canOpen) {
                    void onOpenFile(entry.path)
                  }
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
            )
          })
        ) : (
          <div className="flex min-h-[180px] items-center justify-center px-4 text-center text-sm text-muted-foreground">
            {t("pages.workspace.git.cleanDescription")}
          </div>
        )}
      </div>
    </div>
  )
}

import { useTranslation } from "@slab/i18n"
import { Popover, PopoverContent, PopoverTrigger } from "@slab/components/popover"
import { AlertCircle, AlertTriangle, CircleCheck, Info } from "lucide-react"
import { countBy } from "lodash-es"
import type * as Monaco from "monaco-editor"

import { cn } from "@/lib/utils"

export type WorkspaceEditorCursor = {
  column: number
  lineNumber: number
}

export type WorkspaceEditorProblem = Monaco.editor.IMarker

type WorkspaceEditorStatusBarProps = {
  cursor: WorkspaceEditorCursor | null
  language: string
  problems: WorkspaceEditorProblem[]
  tabSize: number
  onRevealProblem: (problem: WorkspaceEditorProblem) => void
}

export function WorkspaceEditorStatusBar({
  cursor,
  language,
  problems,
  tabSize,
  onRevealProblem,
}: WorkspaceEditorStatusBarProps) {
  const { t } = useTranslation()
  const problemCounts = countBy(problems, (problem) => {
    if (problem.severity >= 8) return "error"
    if (problem.severity === 4) return "warning"
    return "info"
  })
  const errorCount = problemCounts.error ?? 0
  const warningCount = problemCounts.warning ?? 0
  const infoCount = problems.length - errorCount - warningCount

  return (
    <div className="flex h-7 shrink-0 items-center justify-between gap-3 border-t border-border/60 bg-[var(--surface-1)] px-3 text-caption text-muted-foreground">
      <div className="flex min-w-0 items-center gap-2">
        <Popover>
          <PopoverTrigger asChild>
            <button
              type="button"
              className={cn(
                "flex h-5 items-center gap-1 rounded-[4px] px-1.5 transition hover:bg-background hover:text-foreground",
                problems.length > 0 && "text-amber-600 dark:text-amber-300",
                errorCount > 0 && "text-destructive",
              )}
            >
              {problems.length > 0 ? <AlertCircle className="size-3.5" /> : <CircleCheck className="size-3.5" />}
              <span>
                {problems.length > 0
                  ? t("pages.workspace.editor.problemSummary", { count: problems.length })
                  : t("pages.workspace.editor.noProblems")}
              </span>
            </button>
          </PopoverTrigger>
          <PopoverContent className="w-[min(28rem,calc(100vw-2rem))] overflow-hidden p-0" align="start">
            <div className="border-b border-border/60 px-3 py-2 text-xs font-medium">
              {t("pages.workspace.editor.problems")}
            </div>
            {problems.length > 0 ? (
              <div className="max-h-80 overflow-y-auto py-1">
                {problems.map((problem) => (
                  <button
                    key={`${problem.resource.toString()}:${problem.startLineNumber}:${problem.startColumn}:${problem.message}`}
                    type="button"
                    className="grid w-full grid-cols-[1rem_minmax(0,1fr)] gap-2 px-3 py-2 text-left text-xs transition hover:bg-[var(--surface-selected)]"
                    onClick={() => onRevealProblem(problem)}
                  >
                    <ProblemIcon problem={problem} />
                    <span className="min-w-0">
                      <span className="block break-words text-foreground">{problem.message}</span>
                      <span className="mt-1 block text-caption text-muted-foreground">
                        {t("pages.workspace.editor.problemLineColumn", {
                          column: problem.startColumn,
                          line: problem.startLineNumber,
                        })}
                      </span>
                    </span>
                  </button>
                ))}
              </div>
            ) : (
              <div className="px-3 py-4 text-xs text-muted-foreground">
                {t("pages.workspace.editor.noProblems")}
              </div>
            )}
          </PopoverContent>
        </Popover>
        {problems.length > 0 ? (
          <span className="hidden items-center gap-1 sm:flex">
            {errorCount > 0 ? <span>{t("pages.workspace.editor.problemErrors", { count: errorCount })}</span> : null}
            {warningCount > 0 ? <span>{t("pages.workspace.editor.problemWarnings", { count: warningCount })}</span> : null}
            {infoCount > 0 ? <span>{t("pages.workspace.editor.problemInfos", { count: infoCount })}</span> : null}
          </span>
        ) : null}
      </div>
      <div className="flex shrink-0 items-center gap-3">
        <span className="hidden uppercase sm:inline">{language}</span>
        <span className="hidden sm:inline">{t("pages.workspace.editor.statusTabSize", { size: tabSize })}</span>
        {cursor ? (
          <span>{t("pages.workspace.editor.statusLineColumn", { column: cursor.column, line: cursor.lineNumber })}</span>
        ) : null}
      </div>
    </div>
  )
}

function ProblemIcon({ problem }: { problem: WorkspaceEditorProblem }) {
  if (problem.severity >= 8) {
    return <AlertCircle className="mt-0.5 size-3.5 text-destructive" />
  }

  if (problem.severity === 4) {
    return <AlertTriangle className="mt-0.5 size-3.5 text-amber-600 dark:text-amber-300" />
  }

  return <Info className="mt-0.5 size-3.5 text-muted-foreground" />
}

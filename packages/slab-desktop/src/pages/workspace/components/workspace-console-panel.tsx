import { Loader2, Play, Terminal, Trash2 } from "lucide-react"

import { Button } from "@slab/components/button"
import { useTranslation } from "@slab/i18n"
import type { WorkspaceConsoleEntry } from "../hooks/use-workspace-page"
import { cn } from "@/lib/utils"

type WorkspaceConsolePanelProps = {
  command: string
  entries: WorkspaceConsoleEntry[]
  isRunning: boolean
  onChangeCommand: (command: string) => void
  onClear: () => void
  onRun: () => Promise<void>
}

export function WorkspaceConsolePanel({
  command,
  entries,
  isRunning,
  onChangeCommand,
  onClear,
  onRun,
}: WorkspaceConsolePanelProps) {
  const { t } = useTranslation()

  return (
    <section className="workspace-soft-panel flex h-[240px] shrink-0 flex-col overflow-hidden rounded-[18px]">
      <div className="flex h-10 shrink-0 items-center justify-between gap-3 border-b border-border/60 px-3">
        <div className="flex items-center gap-2 text-sm font-semibold">
          <Terminal className="size-4 text-[var(--brand-teal)]" />
          {t("pages.workspace.console.title")}
        </div>
        <Button
          type="button"
          variant="quiet"
          size="icon-xs"
          onClick={onClear}
          aria-label={t("pages.workspace.console.clear")}
          disabled={entries.length === 0}
        >
          <Trash2 className="size-3.5" />
        </Button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto bg-[var(--surface-1)] px-3 py-2 font-mono text-xs leading-5">
        {entries.length > 0 ? (
          entries.map((entry) => (
            <div key={entry.id} className="border-b border-border/40 py-2 last:border-0">
              <div className="flex min-w-0 items-center gap-2 text-muted-foreground">
                <span className="text-[var(--brand-teal)]">$</span>
                <span className="min-w-0 flex-1 truncate">{entry.command}</span>
                <span
                  className={cn(
                    "shrink-0 rounded-full px-2 py-0.5 text-[10px]",
                    entry.exitCode === 0
                      ? "bg-emerald-500/12 text-emerald-600 dark:text-emerald-300"
                      : "bg-muted text-muted-foreground",
                    entry.timedOut && "bg-destructive/12 text-destructive",
                  )}
                >
                  {entry.timedOut
                    ? t("pages.workspace.console.timedOut")
                    : t("pages.workspace.console.exitCode", {
                        code: entry.exitCode ?? "-",
                      })}
                </span>
              </div>
              {entry.stdout ? (
                <pre className="mt-2 whitespace-pre-wrap break-words text-foreground">{entry.stdout}</pre>
              ) : null}
              {entry.stderr ? (
                <pre className="mt-2 whitespace-pre-wrap break-words text-destructive">{entry.stderr}</pre>
              ) : null}
            </div>
          ))
        ) : (
          <div className="flex h-full items-center justify-center text-center text-muted-foreground">
            {t("pages.workspace.console.empty")}
          </div>
        )}
      </div>

      <form
        className="flex h-12 shrink-0 items-center gap-2 border-t border-border/60 bg-[var(--surface-soft)] px-3"
        onSubmit={(event) => {
          event.preventDefault()
          void onRun()
        }}
      >
        <span className="font-mono text-sm text-[var(--brand-teal)]">$</span>
        <input
          value={command}
          onChange={(event) => onChangeCommand(event.target.value)}
          className="h-8 min-w-0 flex-1 rounded-[8px] border border-border/60 bg-[var(--surface-1)] px-3 font-mono text-xs outline-none transition focus:border-[var(--brand-teal)]"
          placeholder={t("pages.workspace.console.placeholder")}
          disabled={isRunning}
        />
        <Button type="submit" variant="cta" size="sm" disabled={!command.trim() || isRunning}>
          {isRunning ? <Loader2 className="size-3.5 animate-spin" /> : <Play className="size-3.5" />}
          {t("pages.workspace.console.run")}
        </Button>
      </form>
    </section>
  )
}

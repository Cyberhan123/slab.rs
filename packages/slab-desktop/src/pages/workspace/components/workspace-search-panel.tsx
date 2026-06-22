import { FileCode2, Loader2, Search, X } from "lucide-react"

import { useTranslation } from "@slab/i18n"
import type {
  WorkspaceFileEntry,
  WorkspaceTextSearchFileMatch,
  WorkspaceTextSearchLineMatch,
} from "@/lib/workspace-bridge"
import { cn } from "@/lib/utils"

type WorkspaceSearchPanelProps = {
  activeFilePath: string | null
  fileFetching: boolean
  fileResults: WorkspaceFileEntry[]
  fileTruncated: boolean
  query: string
  textFetching: boolean
  textResults: WorkspaceTextSearchFileMatch[]
  textTruncated: boolean
  onOpenFile: (relativePath: string) => Promise<unknown>
  onOpenMatch: (relativePath: string, match: WorkspaceTextSearchLineMatch) => Promise<void>
  onQueryChange: (query: string) => void
}

export function WorkspaceSearchPanel({
  activeFilePath,
  fileFetching,
  fileResults,
  fileTruncated,
  query,
  textFetching,
  textResults,
  textTruncated,
  onOpenFile,
  onOpenMatch,
  onQueryChange,
}: WorkspaceSearchPanelProps) {
  const { t } = useTranslation()
  const hasQuery = query.trim().length > 0
  const fetching = fileFetching || textFetching
  const hasResults = fileResults.length > 0 || textResults.length > 0

  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      <div className="relative px-1">
        <Search className="pointer-events-none absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <input
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
          className="h-8 w-full rounded-[8px] border border-border/50 bg-[var(--surface-1)] pl-8 pr-8 text-xs outline-none transition focus:border-[var(--brand-teal)]"
          placeholder={t("pages.workspace.search.placeholder")}
          aria-label={t("pages.workspace.search.placeholder")}
        />
        {fetching ? (
          <Loader2 className="absolute right-3 top-1/2 size-3.5 -translate-y-1/2 animate-spin text-muted-foreground" />
        ) : hasQuery ? (
          <button
            type="button"
            className="absolute right-2 top-1/2 flex size-5 -translate-y-1/2 items-center justify-center rounded-[4px] text-muted-foreground transition hover:bg-muted hover:text-foreground"
            aria-label={t("pages.workspace.search.clear")}
            onClick={() => onQueryChange("")}
          >
            <X className="size-3" />
          </button>
        ) : null}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto rounded-[12px] bg-[var(--surface-1)] py-1">
        {hasQuery ? (
          hasResults ? (
            <>
              {fileResults.length > 0 ? (
                <section className="border-b border-border/40 py-1">
                  <div className="px-3 py-1 text-caption font-semibold uppercase tracking-eyebrow text-muted-foreground">
                    {t("pages.workspace.commandPalette.files")}
                  </div>
                  {fileResults.map((entry) => (
                    <button
                      key={entry.relativePath}
                      type="button"
                      className={cn(
                        "flex w-full min-w-0 items-center gap-2 px-3 py-1.5 text-left text-sm transition hover:bg-[var(--surface-selected)]",
                        activeFilePath === entry.relativePath && "bg-[var(--surface-selected)] text-[var(--brand-teal)]",
                      )}
                      title={entry.relativePath}
                      onClick={() => {
                        void onOpenFile(entry.relativePath)
                      }}
                    >
                      <FileCode2 className="size-4 shrink-0 text-muted-foreground" />
                      <span className="min-w-0 flex-1 truncate">{entry.name}</span>
                      <span className="min-w-0 max-w-[54%] truncate font-mono text-caption text-muted-foreground">
                        {entry.relativePath}
                      </span>
                    </button>
                  ))}
                  {fileTruncated ? (
                    <div className="px-3 py-2 text-xs text-muted-foreground">
                      {t("pages.workspace.search.truncated")}
                    </div>
                  ) : null}
                </section>
              ) : null}
              {textResults.length > 0 ? (
                <section className="py-1">
                  <div className="px-3 py-1 text-caption font-semibold uppercase tracking-eyebrow text-muted-foreground">
                    {t("pages.workspace.textSearch.results")}
                  </div>
                  {textResults.map((result) => (
                    <div key={result.relativePath} className="border-b border-border/40 py-1 last:border-b-0">
                      <div
                        className={cn(
                          "flex min-w-0 items-center gap-2 px-3 py-1 text-xs font-medium",
                          activeFilePath === result.relativePath && "text-[var(--brand-teal)]",
                        )}
                        title={result.relativePath}
                      >
                        <FileCode2 className="size-3.5 shrink-0 text-muted-foreground" />
                        <span className="min-w-0 flex-1 truncate">{result.name}</span>
                        <span className="min-w-0 max-w-[58%] truncate font-mono text-caption text-muted-foreground">
                          {result.relativePath}
                        </span>
                      </div>
                      <div className="space-y-0.5">
                        {result.lineMatches.map((match) => (
                          <button
                            key={`${result.relativePath}:${match.lineNumber}:${match.matchStart}`}
                            type="button"
                            className="grid w-full grid-cols-[3rem_minmax(0,1fr)] gap-2 px-3 py-1.5 text-left text-xs transition hover:bg-[var(--surface-selected)]"
                            onClick={() => {
                              void onOpenMatch(result.relativePath, match)
                            }}
                          >
                            <span className="text-right font-mono text-caption text-muted-foreground">
                              {match.lineNumber}
                            </span>
                            <span className="min-w-0 truncate font-mono text-foreground">
                              <HighlightedLine match={match} />
                            </span>
                          </button>
                        ))}
                      </div>
                    </div>
                  ))}
                  {textTruncated ? (
                    <div className="px-3 py-2 text-xs text-muted-foreground">
                      {t("pages.workspace.textSearch.truncated")}
                    </div>
                  ) : null}
                </section>
              ) : null}
            </>
          ) : (
            <div className="flex h-full min-h-[180px] items-center justify-center px-4 text-center text-sm text-muted-foreground">
              {fetching ? t("pages.workspace.tree.loading") : t("pages.workspace.search.empty")}
            </div>
          )
        ) : null}
      </div>
    </div>
  )
}

function HighlightedLine({ match }: { match: WorkspaceTextSearchLineMatch }) {
  const characters = Array.from(match.lineText)
  const before = characters.slice(0, match.matchStart).join("")
  const selected = characters.slice(match.matchStart, match.matchEnd).join("")
  const after = characters.slice(match.matchEnd).join("")

  return (
    <>
      {before}
      <mark className="rounded-[3px] bg-[color:color-mix(in_oklab,var(--brand-teal)_20%,transparent)] px-0.5 text-[var(--brand-teal)]">
        {selected}
      </mark>
      {after}
    </>
  )
}

import "@xterm/xterm/css/xterm.css"

import { FitAddon } from "@xterm/addon-fit"
import { Unicode11Addon } from "@xterm/addon-unicode11"
import { WebLinksAddon } from "@xterm/addon-web-links"
import { Terminal as XtermTerminal, type ITheme } from "@xterm/xterm"
import { Loader2, Play, Terminal, Trash2 } from "lucide-react"
import { useEffect, useMemo, useRef } from "react"

import { Button } from "@slab/components/button"
import { useTranslation } from "@slab/i18n"
import type { WorkspaceConsoleEntry } from "../hooks/use-workspace-page"

type WorkspaceConsolePanelProps = {
  command: string
  entries: WorkspaceConsoleEntry[]
  isRunning: boolean
  onChangeCommand: (command: string) => void
  onClear: () => void
  onHistory: (direction: "previous" | "next") => void
  onRun: () => Promise<void>
  themeMode: "light" | "dark"
}

const lightTheme: ITheme = {
  background: "#f8fafc",
  foreground: "#111827",
  cursor: "#0f766e",
  selectionBackground: "#99f6e466",
  black: "#111827",
  red: "#dc2626",
  green: "#059669",
  yellow: "#ca8a04",
  blue: "#2563eb",
  magenta: "#7c3aed",
  cyan: "#0891b2",
  white: "#f8fafc",
  brightBlack: "#6b7280",
  brightRed: "#ef4444",
  brightGreen: "#10b981",
  brightYellow: "#eab308",
  brightBlue: "#3b82f6",
  brightMagenta: "#8b5cf6",
  brightCyan: "#06b6d4",
  brightWhite: "#ffffff",
}

const darkTheme: ITheme = {
  background: "#0b1120",
  foreground: "#e5e7eb",
  cursor: "#5eead4",
  selectionBackground: "#2dd4bf55",
  black: "#020617",
  red: "#f87171",
  green: "#34d399",
  yellow: "#facc15",
  blue: "#60a5fa",
  magenta: "#c084fc",
  cyan: "#22d3ee",
  white: "#e5e7eb",
  brightBlack: "#64748b",
  brightRed: "#fca5a5",
  brightGreen: "#6ee7b7",
  brightYellow: "#fde047",
  brightBlue: "#93c5fd",
  brightMagenta: "#d8b4fe",
  brightCyan: "#67e8f9",
  brightWhite: "#ffffff",
}

export function WorkspaceConsolePanel({
  command,
  entries,
  isRunning,
  onChangeCommand,
  onClear,
  onHistory,
  onRun,
  themeMode,
}: WorkspaceConsolePanelProps) {
  const { t } = useTranslation()
  const hostRef = useRef<HTMLDivElement | null>(null)
  const terminalRef = useRef<XtermTerminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const renderedEntryCountRef = useRef(0)
  const initialThemeModeRef = useRef(themeMode)
  const theme = useMemo(() => (themeMode === "dark" ? darkTheme : lightTheme), [themeMode])

  useEffect(() => {
    const host = hostRef.current
    if (!host) {
      return
    }

    const terminal = new XtermTerminal({
      allowProposedApi: true,
      convertEol: true,
      cursorBlink: false,
      disableStdin: true,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
      fontSize: 12,
      scrollback: 1_000,
      theme: initialThemeModeRef.current === "dark" ? darkTheme : lightTheme,
    })
    const fitAddon = new FitAddon()
    terminal.loadAddon(fitAddon)
    terminal.loadAddon(new WebLinksAddon())
    terminal.loadAddon(new Unicode11Addon())
    terminal.unicode.activeVersion = "11"
    terminal.open(host)
    fitAddon.fit()
    terminalRef.current = terminal
    fitAddonRef.current = fitAddon

    const observer = new ResizeObserver(() => fitAddon.fit())
    observer.observe(host)

    return () => {
      observer.disconnect()
      terminal.dispose()
      terminalRef.current = null
      fitAddonRef.current = null
      renderedEntryCountRef.current = 0
    }
  }, [])

  useEffect(() => {
    if (terminalRef.current) {
      terminalRef.current.options.theme = theme
    }
  }, [theme])

  useEffect(() => {
    const terminal = terminalRef.current
    if (!terminal) {
      return
    }

    if (entries.length === 0) {
      terminal.clear()
      renderedEntryCountRef.current = 0
      terminal.writeln(t("pages.workspace.console.empty"))
      return
    }

    if (renderedEntryCountRef.current > entries.length) {
      terminal.clear()
      renderedEntryCountRef.current = 0
    }

    entries.slice(renderedEntryCountRef.current).forEach((entry) => {
      terminal.writeln(`\x1b[36m$\x1b[0m ${entry.command}`)
      if (entry.stdout) {
        terminal.write(entry.stdout.replaceAll("\n", "\r\n"))
        if (!entry.stdout.endsWith("\n")) {
          terminal.writeln("")
        }
      }
      if (entry.stderr) {
        terminal.write(`\x1b[31m${entry.stderr.replaceAll("\n", "\r\n")}\x1b[0m`)
        if (!entry.stderr.endsWith("\n")) {
          terminal.writeln("")
        }
      }
      const status = entry.timedOut
        ? t("pages.workspace.console.timedOut")
        : t("pages.workspace.console.exitCode", { code: entry.exitCode ?? "-" })
      terminal.writeln(`\x1b[90m${status}\x1b[0m`)
      terminal.writeln("")
    })
    renderedEntryCountRef.current = entries.length
  }, [entries, t])

  return (
    <section className="workspace-soft-panel flex h-[260px] shrink-0 flex-col overflow-hidden rounded-[18px]">
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

      <div ref={hostRef} className="min-h-0 flex-1 bg-[var(--surface-1)] px-2 py-2" />

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
          onKeyDown={(event) => {
            if (event.key === "ArrowUp") {
              event.preventDefault()
              onHistory("previous")
            }
            if (event.key === "ArrowDown") {
              event.preventDefault()
              onHistory("next")
            }
          }}
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

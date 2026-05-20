import "@xterm/xterm/css/xterm.css"

import { FitAddon } from "@xterm/addon-fit"
import { Unicode11Addon } from "@xterm/addon-unicode11"
import { WebLinksAddon } from "@xterm/addon-web-links"
import { Terminal as XtermTerminal, type IDisposable, type ITheme } from "@xterm/xterm"
import { Plus, Terminal, Trash2, X } from "lucide-react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { toast } from "sonner"

import { Button } from "@slab/components/button"
import { useTranslation } from "@slab/i18n"
import { getErrorMessage } from "@slab/api"
import { workspaceTerminalSession } from "@/lib/workspace-bridge"
import { cn } from "@/lib/utils"

type WorkspaceConsolePanelProps = {
  themeMode: "light" | "dark"
  workspaceRoot: string
}

type TerminalSession = {
  id: string
}

type TerminalControlMessage =
  | {
      type: "input"
      data: string
    }
  | {
      type: "resize"
      cols: number
      rows: number
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

function createTerminalSession(): TerminalSession {
  return {
    id: `terminal-${Date.now()}-${crypto.randomUUID()}`,
  }
}

export function WorkspaceConsolePanel({ themeMode, workspaceRoot }: WorkspaceConsolePanelProps) {
  const { t } = useTranslation()
  const terminalRefs = useRef(new Map<string, XtermTerminal>())
  const workspaceRootRef = useRef(workspaceRoot)
  const [sessions, setSessions] = useState<TerminalSession[]>(() => [createTerminalSession()])
  const [activeSessionId, setActiveSessionId] = useState<string | null>(() => sessions[0].id)
  const theme = useMemo(() => (themeMode === "dark" ? darkTheme : lightTheme), [themeMode])

  useEffect(() => {
    if (workspaceRootRef.current === workspaceRoot) {
      return
    }
    workspaceRootRef.current = workspaceRoot
    const session = createTerminalSession()
    terminalRefs.current.clear()
    setSessions([session])
    setActiveSessionId(session.id)
  }, [workspaceRoot])

  const handleTerminalReady = useCallback((sessionId: string, terminal: XtermTerminal | null) => {
    if (terminal) {
      terminalRefs.current.set(sessionId, terminal)
      return
    }
    terminalRefs.current.delete(sessionId)
  }, [])

  const handleNewTerminal = useCallback(() => {
    const session = createTerminalSession()
    setSessions((current) => [...current, session])
    setActiveSessionId(session.id)
  }, [])

  const handleCloseTerminal = useCallback(
    (sessionId: string) => {
      const closingIndex = sessions.findIndex((session) => session.id === sessionId)
      if (closingIndex < 0) {
        return
      }

      const nextSessions = sessions.filter((session) => session.id !== sessionId)
      setSessions(nextSessions)
      setActiveSessionId((currentActiveId) => {
        if (currentActiveId !== sessionId) {
          return currentActiveId
        }
        return nextSessions[Math.min(closingIndex, nextSessions.length - 1)]?.id ?? null
      })
    },
    [sessions],
  )

  return (
    <section className="workspace-soft-panel flex h-[260px] shrink-0 flex-col overflow-hidden rounded-[18px]">
      <div className="flex h-10 shrink-0 items-center justify-between gap-3 border-b border-border/60 px-3">
        <div className="flex min-w-0 items-center gap-2">
          <div className="flex shrink-0 items-center gap-2 text-sm font-semibold">
            <Terminal className="size-4 text-[var(--brand-teal)]" />
            {t("pages.workspace.console.title")}
          </div>
          <div className="flex min-w-0 items-center gap-1 overflow-x-auto" role="tablist">
            {sessions.map((session, index) => {
              const active = session.id === activeSessionId
              const name = `${t("pages.workspace.console.terminal")} ${index + 1}`

              return (
                <div
                  key={session.id}
                  className={cn(
                    "flex h-6 shrink-0 items-center gap-1 rounded-full pl-2 pr-1 text-[11px] font-medium text-muted-foreground transition hover:bg-[var(--surface-selected)] hover:text-foreground",
                    active && "bg-[var(--surface-selected)] text-foreground",
                  )}
                >
                  <button
                    type="button"
                    role="tab"
                    aria-selected={active}
                    className="h-full min-w-0 outline-none"
                    onClick={() => setActiveSessionId(session.id)}
                  >
                    {name}
                  </button>
                  <button
                    type="button"
                    className="flex size-4 shrink-0 items-center justify-center rounded-full text-muted-foreground transition hover:bg-muted hover:text-foreground"
                    aria-label={t("pages.workspace.tabs.close", { name })}
                    title={t("pages.workspace.tabs.close", { name })}
                    onClick={() => handleCloseTerminal(session.id)}
                  >
                    <X className="size-3" />
                  </button>
                </div>
              )
            })}
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <Button
            type="button"
            variant="quiet"
            size="icon-xs"
            onClick={handleNewTerminal}
            aria-label={t("pages.workspace.console.newTerminal")}
          >
            <Plus className="size-3.5" />
          </Button>
          <Button
            type="button"
            variant="quiet"
            size="icon-xs"
            disabled={!activeSessionId}
            onClick={() => {
              if (activeSessionId) {
                terminalRefs.current.get(activeSessionId)?.clear()
              }
            }}
            aria-label={t("pages.workspace.console.clear")}
          >
            <Trash2 className="size-3.5" />
          </Button>
        </div>
      </div>

      <div className="relative min-h-0 flex-1 bg-[var(--surface-1)]">
        {sessions.length > 0 ? (
          sessions.map((session) => (
            <TerminalSessionPane
              key={session.id}
              active={session.id === activeSessionId}
              sessionId={session.id}
              theme={theme}
              themeMode={themeMode}
              workspaceRoot={workspaceRoot}
              onTerminalReady={handleTerminalReady}
            />
          ))
        ) : (
          <div className="flex h-full items-center justify-center">
            <Button type="button" variant="quiet" size="sm" onClick={handleNewTerminal}>
              <Plus className="size-3.5" />
              {t("pages.workspace.console.newTerminal")}
            </Button>
          </div>
        )}
      </div>
    </section>
  )
}

function TerminalSessionPane({
  active,
  onTerminalReady,
  sessionId,
  theme,
  themeMode,
  workspaceRoot,
}: {
  active: boolean
  onTerminalReady: (sessionId: string, terminal: XtermTerminal | null) => void
  sessionId: string
  theme: ITheme
  themeMode: "light" | "dark"
  workspaceRoot: string
}) {
  const { t } = useTranslation()
  const hostRef = useRef<HTMLDivElement | null>(null)
  const terminalRef = useRef<XtermTerminal | null>(null)
  const socketRef = useRef<WebSocket | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const initialThemeModeRef = useRef(themeMode)

  useEffect(() => {
    const host = hostRef.current
    if (!host) {
      return
    }

    let disposed = false
    const disposables: IDisposable[] = []
    const terminal = new XtermTerminal({
      allowProposedApi: true,
      cursorBlink: true,
      disableStdin: false,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
      fontSize: 12,
      scrollback: 2_000,
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
    onTerminalReady(sessionId, terminal)

    const sendControl = (message: TerminalControlMessage) => {
      const socket = socketRef.current
      if (socket?.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify(message))
      }
    }

    const sendResize = () => {
      sendControl({ type: "resize", cols: terminal.cols, rows: terminal.rows })
    }

    disposables.push(
      terminal.onData((data) => {
        sendControl({ type: "input", data })
      }),
    )
    disposables.push(terminal.onResize(({ cols, rows }) => sendControl({ type: "resize", cols, rows })))

    const observer = new ResizeObserver(() => {
      fitAddon.fit()
      sendResize()
    })
    observer.observe(host)

    void workspaceTerminalSession()
      .then(({ url }) => {
        if (disposed) {
          return
        }
        const socket = new WebSocket(url)
        socket.binaryType = "arraybuffer"
        socketRef.current = socket
        socket.addEventListener("open", () => {
          sendResize()
        })
        socket.addEventListener("message", (event) => {
          if (event.data instanceof ArrayBuffer) {
            terminal.write(new Uint8Array(event.data))
            return
          }
          if (event.data instanceof Blob) {
            void event.data.arrayBuffer().then((buffer) => terminal.write(new Uint8Array(buffer)))
            return
          }
          terminal.write(String(event.data))
        })
        socket.addEventListener("error", () => {
          toast.error(t("pages.workspace.toast.consoleFailed"))
        })
        socket.addEventListener("close", () => {
          if (!disposed) {
            terminal.write("\r\n\x1b[90m[terminal disconnected]\x1b[0m\r\n")
          }
        })
      })
      .catch((error) => {
        toast.error(t("pages.workspace.toast.consoleFailed"), {
          description: getErrorMessage(error),
        })
      })

    return () => {
      disposed = true
      observer.disconnect()
      disposables.forEach((disposable) => disposable.dispose())
      socketRef.current?.close()
      socketRef.current = null
      terminal.dispose()
      terminalRef.current = null
      fitAddonRef.current = null
      onTerminalReady(sessionId, null)
    }
  }, [onTerminalReady, sessionId, t, workspaceRoot])

  useEffect(() => {
    if (terminalRef.current) {
      terminalRef.current.options.theme = theme
    }
  }, [theme])

  useEffect(() => {
    if (!active) {
      return
    }
    fitAddonRef.current?.fit()
    terminalRef.current?.focus()
  }, [active])

  return (
    <div
      role="tabpanel"
      ref={hostRef}
      className={cn("absolute inset-0 min-h-0 px-2 py-2", !active && "pointer-events-none opacity-0")}
      aria-hidden={!active}
    />
  )
}

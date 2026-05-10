import "@xterm/xterm/css/xterm.css"

import { LocalEchoAddon } from "@gytx/xterm-local-echo"
import { FitAddon } from "@xterm/addon-fit"
import { Unicode11Addon } from "@xterm/addon-unicode11"
import { WebLinksAddon } from "@xterm/addon-web-links"
import { Terminal as XtermTerminal, type IDisposable, type ITerminalAddon, type ITheme } from "@xterm/xterm"
import { Terminal, Trash2 } from "lucide-react"
import { useEffect, useMemo, useRef } from "react"
import { toast } from "sonner"

import { Button } from "@slab/components/button"
import { useTranslation } from "@slab/i18n"
import { getErrorMessage } from "@slab/api"
import { workspaceTerminalSession } from "@/lib/workspace-bridge"

type WorkspaceConsolePanelProps = {
  themeMode: "light" | "dark"
  workspaceRoot: string
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

const localAutocompleteCommands = [
  "bun",
  "cargo",
  "cd",
  "clear",
  "dir",
  "git",
  "ls",
  "npm",
  "pnpm",
  "pwd",
  "yarn",
]

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

export function WorkspaceConsolePanel({ themeMode, workspaceRoot }: WorkspaceConsolePanelProps) {
  const { t } = useTranslation()
  const hostRef = useRef<HTMLDivElement | null>(null)
  const terminalRef = useRef<XtermTerminal | null>(null)
  const socketRef = useRef<WebSocket | null>(null)
  const initialThemeModeRef = useRef(themeMode)
  const theme = useMemo(() => (themeMode === "dark" ? darkTheme : lightTheme), [themeMode])

  useEffect(() => {
    const host = hostRef.current
    if (!host) {
      return
    }

    let disposed = false
    const disposables: IDisposable[] = []
    const terminal = new XtermTerminal({
      allowProposedApi: true,
      convertEol: true,
      cursorBlink: true,
      disableStdin: false,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
      fontSize: 12,
      scrollback: 2_000,
      theme: initialThemeModeRef.current === "dark" ? darkTheme : lightTheme,
    })
    const fitAddon = new FitAddon()
    const localEcho = new LocalEchoAddon({
      enableAutocomplete: true,
      enableIncompleteInput: true,
      historySize: 50,
      maxAutocompleteEntries: 80,
    })

    terminal.loadAddon(fitAddon)
    terminal.loadAddon(new WebLinksAddon())
    terminal.loadAddon(new Unicode11Addon())
    terminal.loadAddon(localEcho as unknown as ITerminalAddon)
    localEcho.addAutocompleteHandler((index: number) => (index === 0 ? localAutocompleteCommands : []))
    terminal.unicode.activeVersion = "11"
    terminal.open(host)
    fitAddon.fit()
    terminal.focus()
    terminalRef.current = terminal

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
          terminal.focus()
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
    }
  }, [t, workspaceRoot])

  useEffect(() => {
    if (terminalRef.current) {
      terminalRef.current.options.theme = theme
    }
  }, [theme])

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
          onClick={() => terminalRef.current?.clear()}
          aria-label={t("pages.workspace.console.clear")}
        >
          <Trash2 className="size-3.5" />
        </Button>
      </div>

      <div ref={hostRef} className="min-h-0 flex-1 bg-[var(--surface-1)] px-2 py-2" />
    </section>
  )
}

import { getCurrentWindow } from "@tauri-apps/api/window"
import { Minus, Square, X } from "lucide-react"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import useIsTauri from "@/hooks/use-tauri"

type WindowControlAction = "minimize" | "toggleMaximize" | "close"

const WINDOW_CONTROL_LABELS: Record<WindowControlAction, string> = {
  minimize: "Minimize window",
  toggleMaximize: "Maximize window",
  close: "Close window",
}

function getWindowControlErrorMessage(error: unknown) {
  const message = error instanceof Error ? error.message : String(error)

  if (message.includes("not allowed")) {
    return "Window controls need a Tauri restart after capability changes."
  }

  return message
}

export function WindowControls() {
  const isTauri = useIsTauri()

  if (!isTauri) {
    return null
  }

  const runWindowAction = async (action: WindowControlAction) => {
    try {
      const appWindow = getCurrentWindow()

      switch (action) {
        case "minimize":
          await appWindow.minimize()
          break
        case "toggleMaximize":
          await appWindow.toggleMaximize()
          break
        case "close":
          await appWindow.close()
          break
      }
    } catch (error) {
      toast.error(`Failed to ${WINDOW_CONTROL_LABELS[action].toLowerCase()}.`, {
        description: getWindowControlErrorMessage(error),
      })
    }
  }

  return (
    <div className="mr-2 flex items-center gap-1" data-tauri-drag-region="false">
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label={WINDOW_CONTROL_LABELS.minimize}
        title={WINDOW_CONTROL_LABELS.minimize}
        className="size-7 rounded-[10px] text-[var(--shell-rail-label)] hover:bg-white/80 hover:text-[var(--shell-title)]"
        onClick={() => {
          void runWindowAction("minimize")
        }}
      >
        <Minus className="size-4" />
      </Button>

      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label={WINDOW_CONTROL_LABELS.toggleMaximize}
        title={WINDOW_CONTROL_LABELS.toggleMaximize}
        className="size-7 rounded-[10px] text-[var(--shell-rail-label)] hover:bg-white/80 hover:text-[var(--shell-title)]"
        onClick={() => {
          void runWindowAction("toggleMaximize")
        }}
      >
        <Square className="size-[13px]" />
      </Button>

      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label={WINDOW_CONTROL_LABELS.close}
        title={WINDOW_CONTROL_LABELS.close}
        className="size-7 rounded-[10px] text-[var(--shell-rail-label)] hover:bg-[#ef4444]/12 hover:text-[#dc2626]"
        onClick={() => {
          void runWindowAction("close")
        }}
      >
        <X className="size-4" />
      </Button>
    </div>
  )
}

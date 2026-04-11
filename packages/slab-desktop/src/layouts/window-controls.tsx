import { getCurrentWindow } from "@tauri-apps/api/window"
import { Minus, Plus, Square, X } from "lucide-react"
import { toast } from "sonner"

import { Button } from "@slab/components/button"
import useDesktopPlatform from "@/hooks/use-desktop-platform"
import useIsTauri from "@/hooks/use-tauri"
import { cn } from "@/lib/utils"

type WindowControlAction = "minimize" | "toggleMaximize" | "close"
type WindowControlsPlacement = "sidebar" | "trailing"

const WINDOW_CONTROL_LABELS: Record<WindowControlAction, string> = {
  minimize: "Minimize window",
  toggleMaximize: "Maximize window",
  close: "Close window",
}

type MacControl = {
  action: WindowControlAction
  label: string
  toneClassName: string
  icon: typeof X
}

const MAC_CONTROLS: MacControl[] = [
  {
    action: "close",
    label: WINDOW_CONTROL_LABELS.close,
    toneClassName:
      "border-[#ec6a5f] bg-[#ff5f57] text-[#5a1f1b] shadow-[inset_0_1px_0_rgb(255_255_255_/_0.18)]",
    icon: X,
  },
  {
    action: "minimize",
    label: WINDOW_CONTROL_LABELS.minimize,
    toneClassName:
      "border-[#d8a23a] bg-[#ffbd2e] text-[#6a4a00] shadow-[inset_0_1px_0_rgb(255_255_255_/_0.18)]",
    icon: Minus,
  },
  {
    action: "toggleMaximize",
    label: WINDOW_CONTROL_LABELS.toggleMaximize,
    toneClassName:
      "border-[#3ca44a] bg-[#28c840] text-[#0b4f19] shadow-[inset_0_1px_0_rgb(255_255_255_/_0.18)]",
    icon: Plus,
  },
]

function getWindowControlErrorMessage(error: unknown) {
  const message = error instanceof Error ? error.message : String(error)

  if (message.includes("not allowed")) {
    return "Window controls need a Tauri restart after capability changes."
  }

  return message
}

async function runWindowAction(action: WindowControlAction) {
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

function MacWindowControls({ inSidebar = false }: { inSidebar?: boolean }) {
  return (
    <div
      className={cn(
        "shell-window-controls flex items-center gap-2",
        inSidebar ? "w-full justify-center px-3" : "pr-2"
      )}
      data-tauri-drag-region="false"
      role="toolbar"
      aria-label="Window controls"
    >
      {MAC_CONTROLS.map(({ action, label, toneClassName, icon: Icon }) => (
        <button
          key={action}
          type="button"
          aria-label={label}
          title={label}
          className={`group flex size-3 items-center justify-center rounded-full border transition-transform hover:scale-105 ${toneClassName}`}
          onClick={() => {
            void runWindowAction(action)
          }}
        >
          <Icon className="size-2.5 opacity-0 transition-opacity group-hover:opacity-85" strokeWidth={2.6} />
        </button>
      ))}
    </div>
  )
}

function DesktopWindowControls() {
  return (
    <div
      className="shell-window-controls mr-2 flex items-center gap-1"
      data-tauri-drag-region="false"
      role="toolbar"
      aria-label="Window controls"
    >
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label={WINDOW_CONTROL_LABELS.minimize}
        title={WINDOW_CONTROL_LABELS.minimize}
        className="size-7 rounded-[10px] text-[var(--shell-rail-label)] hover:bg-[var(--shell-card)]/80 hover:text-[var(--shell-title)]"
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
        className="size-7 rounded-[10px] text-[var(--shell-rail-label)] hover:bg-[var(--shell-card)]/80 hover:text-[var(--shell-title)]"
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
        className="size-7 rounded-[10px] text-[var(--shell-rail-label)] hover:bg-destructive/12 hover:text-destructive"
        onClick={() => {
          void runWindowAction("close")
        }}
      >
        <X className="size-4" />
      </Button>
    </div>
  )
}

type WindowControlsProps = {
  placement?: WindowControlsPlacement
}

export function WindowControls({ placement = "trailing" }: WindowControlsProps) {
  const isTauri = useIsTauri()
  const platform = useDesktopPlatform()

  if (!isTauri) {
    return null
  }

  if (platform === "macos") {
    return placement === "sidebar" ? <MacWindowControls inSidebar /> : null
  }

  if (platform === "windows") {
    return  <DesktopWindowControls /> 
  }

  return placement === "trailing" ? <DesktopWindowControls /> : null
}

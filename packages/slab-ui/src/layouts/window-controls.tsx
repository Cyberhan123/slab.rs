import { Minus, Plus, Square, X } from "lucide-react"
import { toast } from "sonner"
import { getErrorMessage } from "@slab/api"
import { useTranslation } from "@slab/i18n"

import { Button } from "@slab/components/button"
import useDesktopPlatform, { type DesktopPlatform } from "@slab/ui/hooks/use-desktop-platform"
import { useSlab } from "@slab/ui/provider/slab-provider"
import { cn } from "@slab/ui/lib/utils"

type WindowControlAction = "minimize" | "toggleMaximize" | "close"
type WindowControlsPlacement = "sidebar" | "header"
type WindowControlsVariant = "mac" | "desktop"
type WindowControlsConfig = {
  placement: WindowControlsPlacement
  variant: WindowControlsVariant
}
type Translate = (key: string, options?: Record<string, unknown>) => string

const WINDOW_CONTROL_LABEL_KEYS: Record<WindowControlAction, string> = {
  minimize: "layouts.header.windowControls.minimize",
  toggleMaximize: "layouts.header.windowControls.toggleMaximize",
  close: "layouts.header.windowControls.close",
}

const WINDOW_CONTROL_ERROR_KEYS: Record<WindowControlAction, string> = {
  minimize: "layouts.header.windowControls.errors.minimize",
  toggleMaximize: "layouts.header.windowControls.errors.toggleMaximize",
  close: "layouts.header.windowControls.errors.close",
}

type MacControl = {
  action: WindowControlAction
  toneClassName: string
  icon: typeof X
}

const MAC_CONTROLS: MacControl[] = [
  {
    action: "close",
    toneClassName:
      "border-[#ec6a5f] bg-[#ff5f57] text-[#5a1f1b] shadow-[inset_0_1px_0_rgb(255_255_255_/_0.18)]",
    icon: X,
  },
  {
    action: "minimize",
    toneClassName:
      "border-[#d8a23a] bg-[#ffbd2e] text-[#6a4a00] shadow-[inset_0_1px_0_rgb(255_255_255_/_0.18)]",
    icon: Minus,
  },
  {
    action: "toggleMaximize",
    toneClassName:
      "border-[#3ca44a] bg-[#28c840] text-[#0b4f19] shadow-[inset_0_1px_0_rgb(255_255_255_/_0.18)]",
    icon: Plus,
  },
]

const WINDOW_CONTROLS_CONFIG_BY_PLATFORM: Record<DesktopPlatform, WindowControlsConfig> = {
  macos: {
    placement: "sidebar",
    variant: "mac",
  },
  windows: {
    placement: "header",
    variant: "desktop",
  },
  linux: {
    placement: "header",
    variant: "desktop",
  },
  unknown: {
    placement: "header",
    variant: "desktop",
  },
}

function getWindowControlLabel(action: WindowControlAction, t: Translate) {
  return t(WINDOW_CONTROL_LABEL_KEYS[action])
}

function getWindowControlErrorMessage(error: unknown, t: Translate) {
  const message = getErrorMessage(error)

  if (message.includes("not allowed")) {
    return t("layouts.header.windowControls.errors.capabilityRestart")
  }

  return message
}

async function runWindowAction(
  action: WindowControlAction,
  t: Translate,
  windowChrome: { minimize(): Promise<void>; toggleMaximize(): Promise<void>; close(): Promise<void> },
) {
  try {
    switch (action) {
      case "minimize":
        await windowChrome.minimize()
        break
      case "toggleMaximize":
        await windowChrome.toggleMaximize()
        break
      case "close":
        await windowChrome.close()
        break
    }
  } catch (error) {
    toast.error(t(WINDOW_CONTROL_ERROR_KEYS[action]), {
      description: getWindowControlErrorMessage(error, t),
    })
  }
}

function MacWindowControls({ placement }: { placement: WindowControlsPlacement }) {
  const { t } = useTranslation()
  const { ports } = useSlab()

  return (
    <div
      className={cn(
        "shell-window-controls flex items-center gap-2",
        placement === "sidebar" ? "w-full justify-center px-3" : "pr-2"
      )}
      data-tauri-drag-region="false"
      role="toolbar"
      aria-label={t("layouts.header.windowControls.toolbar")}
    >
      {MAC_CONTROLS.map(({ action, toneClassName, icon: Icon }) => {
        const label = getWindowControlLabel(action, t)

        return (
          <button
            key={action}
            type="button"
            aria-label={label}
            title={label}
            className={`group flex size-3 items-center justify-center rounded-full border transition-transform hover:scale-105 ${toneClassName}`}
            onClick={() => {
              void runWindowAction(action, t, ports.windowChrome)
            }}
          >
            <Icon className="size-2.5 opacity-0 transition-opacity group-hover:opacity-85" strokeWidth={2.6} />
          </button>
        )
      })}
    </div>
  )
}

function DesktopWindowControls() {
  const { t } = useTranslation()
  const { ports } = useSlab()
  const minimizeLabel = getWindowControlLabel("minimize", t)
  const toggleMaximizeLabel = getWindowControlLabel("toggleMaximize", t)
  const closeLabel = getWindowControlLabel("close", t)

  return (
    <div
      className="shell-window-controls mr-2 flex items-center gap-1"
      data-tauri-drag-region="false"
      role="toolbar"
      aria-label={t("layouts.header.windowControls.toolbar")}
    >
      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label={minimizeLabel}
        title={minimizeLabel}
        className="size-7 rounded-[10px] text-[color:var(--shell-rail-label)] hover:bg-glass-bg-strong hover:text-[color:var(--shell-title)]"
        onClick={() => {
          void runWindowAction("minimize", t, ports.windowChrome)
        }}
      >
        <Minus className="size-4" />
      </Button>

      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label={toggleMaximizeLabel}
        title={toggleMaximizeLabel}
        className="size-7 rounded-[10px] text-[color:var(--shell-rail-label)] hover:bg-glass-bg-strong hover:text-[color:var(--shell-title)]"
        onClick={() => {
          void runWindowAction("toggleMaximize", t, ports.windowChrome)
        }}
      >
        <Square className="size-[13px]" />
      </Button>

      <Button
        type="button"
        variant="ghost"
        size="icon-sm"
        aria-label={closeLabel}
        title={closeLabel}
        className="size-7 rounded-[10px] text-[color:var(--shell-rail-label)] hover:bg-destructive/12 hover:text-destructive"
        onClick={() => {
          void runWindowAction("close", t, ports.windowChrome)
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

function getWindowControlsConfig(platform: DesktopPlatform) {
  return WINDOW_CONTROLS_CONFIG_BY_PLATFORM[platform]
}

export function WindowControls({
  placement = "header",
}: WindowControlsProps) {
  const isDesktop = useSlab().ports.platformInfo.desktop
  const platform = useDesktopPlatform()
  const config = getWindowControlsConfig(platform)

  if (!isDesktop || config.placement !== placement) {
    return null
  }

  if (config.variant === "mac") {
    return <MacWindowControls placement={placement} />
  }

  return <DesktopWindowControls />
}

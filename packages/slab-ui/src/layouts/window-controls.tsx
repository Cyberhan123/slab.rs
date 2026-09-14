import { Minus, Plus, X } from "lucide-react"
import { toast } from "sonner"
import { getErrorMessage } from "@slab/api"
import { useTranslation } from "@slab/i18n"

import useDesktopPlatform from "@slab/ui/hooks/use-desktop-platform"
import { useSlab } from "@slab/ui/provider/slab-provider"

type WindowControlAction = "minimize" | "toggleMaximize" | "close"
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

function MacWindowControls() {
  const { t } = useTranslation()
  const { ports } = useSlab()

  return (
    <div
      className="[app-region:no-drag] flex w-full items-center justify-center px-3"
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

/**
 * Every desktop platform except macOS runs with native window decorations
 * (`decorations: true` in the tauri platform configs), so the OS owns the
 * minimize/maximize/close chrome. macOS alone runs borderless and draws its
 * traffic lights inside the sidebar rail.
 */
export function WindowControls() {
  const isDesktop = useSlab().ports.platformInfo.desktop
  const platform = useDesktopPlatform()

  if (!isDesktop || platform !== "macos") {
    return null
  }

  return <MacWindowControls />
}

"use client"

import { useState } from "react"
import { useTranslation } from "@slab/i18n"
import { Button } from "@slab/components/button"
import { Popover, PopoverContent, PopoverTrigger } from "@slab/components/popover"
import { FolderOpen } from "lucide-react"

import { useSlab } from "@slab/ui/provider/slab-provider"

type OpenWorkspaceButtonProps = {
  /** Native folder dialog path (Tauri shell). */
  onOpenFolder: () => void | Promise<void>
  /** Direct path-open path (browser fallback + recent list share this). */
  onOpenWorkspacePath: (rootPath: string) => void | Promise<void>
  variant?: React.ComponentProps<typeof Button>["variant"]
  size?: React.ComponentProps<typeof Button>["size"]
  className?: string
}

/**
 * The single workspace-open entry point.
 *
 * Inside the Tauri shell it opens the host's native folder dialog. In the
 * browser there is no native dialog (`pickFolder()` returns null), so the same
 * button reveals a small path-entry popover — the workspace can still be opened
 * by typing a path. This replaces the former always-visible "open path" form
 * that sat beside the folder button and duplicated it.
 */
export function OpenWorkspaceButton({
  onOpenFolder,
  onOpenWorkspacePath,
  variant = "cta",
  size = "pill",
  className,
}: OpenWorkspaceButtonProps) {
  const { t } = useTranslation()
  const [fallbackOpen, setFallbackOpen] = useState(false)
  const [pathInput, setPathInput] = useState("")
  const trimmedPath = pathInput.trim()
  const tauri = useSlab().ports.platformInfo.desktop

  const submitPath = async () => {
    if (!trimmedPath) return
    const next = trimmedPath
    setPathInput("")
    setFallbackOpen(false)
    await onOpenWorkspacePath(next)
  }

  // In Tauri the button drives the native dialog directly. In the browser the
  // click is left to the Radix PopoverTrigger (controlled via `fallbackOpen`),
  // so onClick stays undefined to avoid a double toggle.
  const trigger = (
    <Button
      variant={variant}
      size={size}
      className={className}
      data-testid="workspace-open-folder-button"
      onClick={tauri ? () => void onOpenFolder() : undefined}
    >
      <FolderOpen className="size-4" />
      {t("pages.workspace.actions.openFolder")}
    </Button>
  )

  if (tauri) {
    return trigger
  }

  return (
    <Popover open={fallbackOpen} onOpenChange={setFallbackOpen}>
      <PopoverTrigger asChild>{trigger}</PopoverTrigger>
      <PopoverContent align="center" side="top" className="w-80">
        <form
          className="flex w-full flex-col gap-2"
          onSubmit={(event) => {
            event.preventDefault()
            void submitPath()
          }}
        >
          <input
            value={pathInput}
            onChange={(event) => setPathInput(event.target.value)}
            className="focus-ring h-10 min-w-0 flex-1 rounded-lg border border-border/60 bg-background px-3 text-sm transition duration-[var(--dur-180)] ease-out-expo focus:border-[var(--brand-teal)]"
            placeholder={t("pages.workspace.actions.pathPlaceholder")}
            aria-label={t("pages.workspace.actions.pathPlaceholder")}
            data-testid="workspace-path-input"
          />
          <Button
            type="button"
            variant="cta"
            size="pill"
            disabled={!trimmedPath}
            onClick={() => void submitPath()}
            data-testid="workspace-open-path-button"
          >
            <FolderOpen className="size-4" />
            {t("pages.workspace.actions.openPath")}
          </Button>
        </form>
      </PopoverContent>
    </Popover>
  )
}

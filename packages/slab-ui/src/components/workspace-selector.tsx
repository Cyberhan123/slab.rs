import { useEffect, useRef, useState } from "react"
import { ChevronDown, FolderOpenIcon, GlobeIcon, FolderIcon } from "lucide-react"

import { useTranslation } from "@slab/i18n"
import { useSlab } from "@slab/ui/provider/slab-provider"
import { useWorkspaceUiStore } from "@slab/ui/store/useWorkspaceUiStore"

import {
  isSameRoot,
  type WorkspaceSelection,
} from "@slab/ui/pages/assistant/hooks/use-workspace-switch"

type CurrentWorkspace = {
  rootPath: string
  name: string
}

/** Best-effort display name for a root path (last path segment). */
function leafName(rootPath: string): string {
  return rootPath.split(/[\\/]/).filter(Boolean).findLast(Boolean) ?? rootPath
}

type WorkspaceSelectorProps = {
  /** The selection this control represents (controlled). */
  value: WorkspaceSelection
  onValueChange: (selection: WorkspaceSelection) => void
  /** The server-side active workspace, when one is open. */
  currentWorkspace?: CurrentWorkspace | null
  /** Disable interaction (e.g. while a switch or a turn is in flight). */
  busy?: boolean
  /** Hide the folder picker (always hidden on web — no native dialog). */
  showChooseFolder?: boolean
}

/**
 * Dropdown that answers "where does this chat run?": the current workspace,
 * a recent workspace, a freshly picked folder, or global (no workspace).
 *
 * Controlled and presentational about the *choice* — call sites decide what a
 * change means (the new-chat dialog defers the switch to submit; the Sender
 * toolbar applies it immediately via `useWorkspaceSwitch`). This keeps
 * workspace and non-workspace chats first-class side by side: a workspace being
 * open never removes the global option.
 */
export function WorkspaceSelector({
  value,
  onValueChange,
  currentWorkspace,
  busy = false,
  showChooseFolder = true,
}: WorkspaceSelectorProps) {
  const { t } = useTranslation()
  const { ports } = useSlab()
  const recentWorkspaces = useWorkspaceUiStore((state) => state.recentWorkspaces)
  const [open, setOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement | null>(null)
  const isDesktop = ports.platformInfo.desktop

  useEffect(() => {
    if (!open) {
      return
    }
    const handler = (event: MouseEvent) => {
      if (!containerRef.current?.contains(event.target as Node)) {
        setOpen(false)
      }
    }
    document.addEventListener("mousedown", handler)
    return () => document.removeEventListener("mousedown", handler)
  }, [open])

  const recents = recentWorkspaces.filter(
    (workspace) => !isSameRoot(workspace.rootPath, currentWorkspace?.rootPath),
  )

  const handleChooseFolder = async () => {
    setOpen(false)
    const picked = await ports.fileDialog.pickFolder()
    if (typeof picked === "string" && picked.trim()) {
      onValueChange({ kind: "root", rootPath: picked.trim() })
    }
  }

  const activeLabel = value.kind === "global"
    ? t("pages.assistant.newChat.global")
    : value.name || leafName(value.rootPath)

  const isSelected = (selection: WorkspaceSelection): boolean => {
    if (selection.kind !== value.kind) return false
    if (selection.kind === "global") return true
    return value.kind === "root" && isSameRoot(selection.rootPath, value.rootPath)
  }

  return (
    <div ref={containerRef} className="relative inline-block" data-testid="workspace-selector">
      <button
        type="button"
        className="inline-flex items-center gap-1 rounded px-2 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-50"
        disabled={busy}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={t("pages.assistant.newChat.workspaceSection")}
        onClick={() => setOpen((next) => !next)}
      >
        {value.kind === "global" ? (
          <GlobeIcon className="size-3.5" />
        ) : (
          <FolderIcon className="size-3.5" />
        )}
        <span className="max-w-[10rem] truncate">{activeLabel}</span>
        <ChevronDown className="size-3" />
      </button>
      {open && (
        <ul
          className="absolute z-50 mt-1 max-h-80 w-72 overflow-auto rounded border bg-background shadow-lg"
          data-testid="workspace-selector-list"
        >
          {currentWorkspace ? (
            <li>
              <p className="px-3 pt-2 pb-1 text-muted-foreground text-xs">
                {t("pages.assistant.newChat.currentLabel")}
              </p>
              <button
                type="button"
                disabled={busy}
                data-testid="workspace-selector-item-current"
                aria-current={isSelected({ kind: "root", rootPath: currentWorkspace.rootPath }) ? "true" : undefined}
                className="flex w-full flex-col items-start gap-0.5 px-3 py-2 text-left hover:bg-muted disabled:opacity-50"
                onClick={() => {
                  onValueChange({
                    kind: "root",
                    rootPath: currentWorkspace.rootPath,
                    name: currentWorkspace.name,
                  })
                  setOpen(false)
                }}
              >
                <span className="truncate text-sm font-medium">{currentWorkspace.name}</span>
                <span className="truncate text-xs opacity-60">{currentWorkspace.rootPath}</span>
              </button>
            </li>
          ) : null}
          {recents.length > 0 ? (
            <li>
              <p className="px-3 pt-2 pb-1 text-muted-foreground text-xs">
                {t("pages.assistant.newChat.recentLabel")}
              </p>
              {recents.map((workspace) => (
                <button
                  key={workspace.rootPath}
                  type="button"
                  disabled={busy}
                  data-testid={`workspace-selector-item-recent-${workspace.rootPath}`}
                  aria-current={isSelected({ kind: "root", rootPath: workspace.rootPath }) ? "true" : undefined}
                  className="flex w-full flex-col items-start gap-0.5 px-3 py-2 text-left hover:bg-muted disabled:opacity-50"
                  onClick={() => {
                    onValueChange({ kind: "root", rootPath: workspace.rootPath, name: workspace.name })
                    setOpen(false)
                  }}
                >
                  <span className="truncate text-sm font-medium">{workspace.name}</span>
                  <span className="truncate text-xs opacity-60">{workspace.rootPath}</span>
                </button>
              ))}
            </li>
          ) : null}
          {showChooseFolder && isDesktop ? (
            <li>
              <button
                type="button"
                disabled={busy}
                data-testid="workspace-selector-choose-folder"
                className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-muted disabled:opacity-50"
                onClick={() => void handleChooseFolder()}
              >
                <FolderOpenIcon className="size-4" />
                <span>{t("pages.assistant.newChat.chooseFolder")}</span>
              </button>
            </li>
          ) : null}
          <li className="border-t">
            <button
              type="button"
              disabled={busy}
              data-testid="workspace-selector-item-global"
              aria-current={isSelected({ kind: "global" }) ? "true" : undefined}
              className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-muted disabled:opacity-50"
              onClick={() => {
                onValueChange({ kind: "global" })
                setOpen(false)
              }}
            >
              <GlobeIcon className="size-4" />
              <span>{t("pages.assistant.newChat.global")}</span>
            </button>
          </li>
        </ul>
      )}
    </div>
  )
}

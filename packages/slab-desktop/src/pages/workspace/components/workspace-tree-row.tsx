import type { NodeRendererProps } from "react-arborist"
import { ChevronDown, ChevronRight, FileCode2, Folder, Loader2 } from "lucide-react"

import { cn } from "@/lib/utils"
import type { WorkspaceTreeNode } from "../lib/workspace-page-utils"

export function WorkspaceTreeRow({
  node,
  style,
  selectedPath,
  loadingPaths,
  onOpenDirectory,
  onOpenFile,
}: NodeRendererProps<WorkspaceTreeNode> & {
  selectedPath: string | null
  loadingPaths: Set<string>
  onOpenDirectory: (relativePath: string) => Promise<unknown>
  onOpenFile: (relativePath: string) => Promise<unknown>
}) {
  const isDirectory = node.data.kind === "directory"
  const selected = selectedPath === node.data.relativePath
  const loading = loadingPaths.has(node.data.relativePath)
  const Icon = isDirectory ? Folder : FileCode2

  return (
    <button
      type="button"
      style={style}
      data-testid={`workspace-tree-row-${node.data.relativePath.replaceAll("/", "-").replaceAll(".", "-") || "root"}`}
      className={cn(
        "flex w-full min-w-0 items-center gap-1.5 px-2 text-left text-sm outline-none transition hover:bg-[var(--surface-selected)]",
        selected && "bg-[var(--surface-selected)] text-[color:var(--brand-teal)]",
      )}
      onClick={() => {
        node.select()
        if (isDirectory) {
          if (!node.data.loaded) {
            void onOpenDirectory(node.data.relativePath)
          }
          node.toggle()
          return
        }
        void onOpenFile(node.data.relativePath)
      }}
    >
      <span className="flex size-4 items-center justify-center text-muted-foreground">
        {isDirectory ? (
          loading ? (
            <Loader2 className="size-3.5 animate-spin" />
          ) : node.isOpen ? (
            <ChevronDown className="size-3.5" />
          ) : (
            <ChevronRight className="size-3.5" />
          )
        ) : null}
      </span>
      <Icon className={cn("size-4 shrink-0", isDirectory ? "text-[color:var(--brand-teal)]" : "text-muted-foreground")} />
      <span className="truncate">{node.data.name}</span>
    </button>
  )
}

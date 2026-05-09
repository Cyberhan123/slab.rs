import type { WorkspaceFileEntry } from "@/lib/workspace-bridge"
import type { WorkspaceFileTab } from "@/store/useWorkspaceUiStore"

export type WorkspaceTreeNode = WorkspaceFileEntry & {
  children?: WorkspaceTreeNode[]
  loaded?: boolean
}

export function entryToTreeNode(entry: WorkspaceFileEntry): WorkspaceTreeNode {
  return {
    ...entry,
    loaded: entry.kind === "file",
    children: entry.kind === "directory" ? [] : undefined,
  }
}

export function insertChildren(
  nodes: WorkspaceTreeNode[],
  relativePath: string,
  children: WorkspaceTreeNode[],
): WorkspaceTreeNode[] {
  return nodes.map((node) => {
    if (node.relativePath === relativePath) {
      return { ...node, children, loaded: true }
    }
    if (!node.children) {
      return node
    }
    return { ...node, children: insertChildren(node.children, relativePath, children) }
  })
}

export function languageForFile(fileName: string) {
  const extension = fileName.split(".").pop()?.toLowerCase()
  switch (extension) {
    case "ts":
    case "tsx":
      return "typescript"
    case "js":
    case "jsx":
      return "javascript"
    case "rs":
      return "rust"
    case "json":
      return "json"
    case "md":
      return "markdown"
    case "css":
      return "css"
    case "html":
      return "html"
    case "toml":
      return "toml"
    case "yaml":
    case "yml":
      return "yaml"
    default:
      return "plaintext"
  }
}

export function upsertFileTab(tabs: WorkspaceFileTab[], tab: WorkspaceFileTab) {
  if (tabs.some((item) => item.relativePath === tab.relativePath)) {
    return tabs.map((item) => (item.relativePath === tab.relativePath ? tab : item))
  }

  return [...tabs, tab]
}

export function sortDirectoryPaths(paths: string[]) {
  return [...new Set(paths)]
    .filter((path) => path.trim().length > 0)
    .toSorted((left, right) => {
      const leftDepth = left.split("/").length
      const rightDepth = right.split("/").length

      if (leftDepth !== rightDepth) {
        return leftDepth - rightDepth
      }

      return left.localeCompare(right)
    })
}

export const SLAB_DIR_NAME = ".slab"

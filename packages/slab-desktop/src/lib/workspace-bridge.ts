import { invoke } from "@tauri-apps/api/core"

import { isTauri } from "@/hooks/use-tauri"

export const WORKSPACE_STATE_QUERY_KEY = ["workspace-state"] as const

export type WorkspaceInfo = {
  rootPath: string
  name: string
  slabDir: string
  settingsPath: string
  workspaceConfigPath: string
  databasePath: string
  modelConfigDir: string
  sessionStateDir: string
}

export type RecentWorkspace = {
  rootPath: string
  name: string
  lastOpenedAt: number
}

export type WorkspacePluginConfig = {
  enabled?: boolean | null
}

export type WorkspaceConfig = {
  schemaVersion: number
  plugins: Record<string, WorkspacePluginConfig>
}

export type WorkspaceStateResponse = {
  current: WorkspaceInfo | null
  recent: RecentWorkspace[]
  config: WorkspaceConfig | null
}

export type WorkspaceFileEntry = {
  id: string
  name: string
  relativePath: string
  kind: "directory" | "file"
  hasChildren: boolean
}

export type WorkspaceDirectoryResponse = {
  relativePath: string
  entries: WorkspaceFileEntry[]
  truncated: boolean
}

export type WorkspaceFileSearchResponse = {
  query: string
  entries: WorkspaceFileEntry[]
  truncated: boolean
}

export type WorkspaceTextSearchLineMatch = {
  lineNumber: number
  lineText: string
  matchStart: number
  matchEnd: number
}

export type WorkspaceTextSearchFileMatch = {
  relativePath: string
  name: string
  lineMatches: WorkspaceTextSearchLineMatch[]
}

export type WorkspaceTextSearchResponse = {
  query: string
  matches: WorkspaceTextSearchFileMatch[]
  truncated: boolean
}

export type WorkspaceFileContent = {
  relativePath: string
  name: string
  content: string
  sizeBytes: number
  contentHash: string
}

export type WorkspaceGitFileStatus =
  | "added"
  | "modified"
  | "deleted"
  | "renamed"
  | "copied"
  | "untracked"
  | "conflicted"

export type WorkspaceGitStatusEntry = {
  path: string
  originalPath: string | null
  status: WorkspaceGitFileStatus
  staged: boolean
}

export type WorkspaceGitStatusSummary = {
  added: number
  modified: number
  deleted: number
  renamed: number
  copied: number
  untracked: number
  conflicted: number
}

export type WorkspaceGitStatus = {
  available: boolean
  isRepository: boolean
  branch: string | null
  repositoryRoot: string | null
  message: string | null
  summary: WorkspaceGitStatusSummary
  entries: WorkspaceGitStatusEntry[]
}

export type WorkspaceConsoleOutput = {
  command: string
  exitCode: number | null
  stdout: string
  stderr: string
  timedOut: boolean
}

export type WorkspaceTerminalSession = {
  url: string
}

export type WorkspaceWriteFileCommand = {
  relativePath: string
  content: string
  expectedHash?: string | null
}

export type WorkspaceCreateFileCommand = {
  relativePath: string
}

export type WorkspaceCreateDirectoryCommand = {
  relativePath: string
}

export type WorkspaceRenamePathCommand = {
  fromRelativePath: string
  toRelativePath: string
}

export type WorkspaceDeletePathCommand = {
  relativePath: string
  recursive: boolean
}

export type WorkspaceWriteFileResult = {
  relativePath: string
  sizeBytes: number
  contentHash: string
}

export type WorkspacePathResult = {
  relativePath: string
}

export type WorkspaceGitOperationResult = {
  status: WorkspaceGitStatus
}

export type WorkspaceGitDiffCommand = {
  path: string
  staged: boolean
}

export type WorkspaceGitDiff = {
  path: string
  staged: boolean
  diff: string
}

export type WorkspacePluginPreferenceUpdate = {
  pluginId: string
  enabled?: boolean | null
}

export async function workspaceState(): Promise<WorkspaceStateResponse> {
  if (!isTauri()) {
    return { current: null, recent: [], config: null }
  }

  return invoke<WorkspaceStateResponse>("workspace_state")
}

export async function workspaceOpen(rootPath: string): Promise<WorkspaceStateResponse> {
  if (!isTauri()) {
    throw new Error("workspaces are only available in the desktop app")
  }

  return invoke<WorkspaceStateResponse>("workspace_open", { rootPath })
}

export async function workspaceClose(): Promise<WorkspaceStateResponse> {
  if (!isTauri()) {
    return { current: null, recent: [], config: null }
  }

  return invoke<WorkspaceStateResponse>("workspace_close")
}

export async function workspaceReadDirectory(
  relativePath?: string,
  options?: { includeIgnored?: boolean },
): Promise<WorkspaceDirectoryResponse> {
  if (!isTauri()) {
    return { relativePath: relativePath ?? "", entries: [], truncated: false }
  }

  return invoke<WorkspaceDirectoryResponse>("workspace_read_directory", {
    includeIgnored: options?.includeIgnored,
    relativePath,
  })
}

export async function workspaceReadFile(relativePath: string): Promise<WorkspaceFileContent> {
  if (!isTauri()) {
    throw new Error("workspace files are only available in the desktop app")
  }

  return invoke<WorkspaceFileContent>("workspace_read_file", { relativePath })
}

export async function workspaceSearchFiles(query: string): Promise<WorkspaceFileSearchResponse> {
  if (!isTauri()) {
    return { query, entries: [], truncated: false }
  }

  return invoke<WorkspaceFileSearchResponse>("workspace_search_files", { query })
}

export async function workspaceSearchText(query: string): Promise<WorkspaceTextSearchResponse> {
  if (!isTauri()) {
    return { query, matches: [], truncated: false }
  }

  return invoke<WorkspaceTextSearchResponse>("workspace_search_text", { query })
}

export async function workspaceWriteFile(command: WorkspaceWriteFileCommand): Promise<WorkspaceWriteFileResult> {
  if (!isTauri()) {
    throw new Error("workspace files are only available in the desktop app")
  }

  return invoke<WorkspaceWriteFileResult>("workspace_write_file", { command })
}

export async function workspaceCreateFile(command: WorkspaceCreateFileCommand): Promise<WorkspacePathResult> {
  if (!isTauri()) {
    throw new Error("workspace files are only available in the desktop app")
  }

  return invoke<WorkspacePathResult>("workspace_create_file", { command })
}

export async function workspaceCreateDirectory(command: WorkspaceCreateDirectoryCommand): Promise<WorkspacePathResult> {
  if (!isTauri()) {
    throw new Error("workspace files are only available in the desktop app")
  }

  return invoke<WorkspacePathResult>("workspace_create_directory", { command })
}

export async function workspaceRenamePath(command: WorkspaceRenamePathCommand): Promise<WorkspacePathResult> {
  if (!isTauri()) {
    throw new Error("workspace files are only available in the desktop app")
  }

  return invoke<WorkspacePathResult>("workspace_rename_path", { command })
}

export async function workspaceDeletePath(command: WorkspaceDeletePathCommand): Promise<WorkspacePathResult> {
  if (!isTauri()) {
    throw new Error("workspace files are only available in the desktop app")
  }

  return invoke<WorkspacePathResult>("workspace_delete_path", { command })
}

export async function workspaceGitStatus(): Promise<WorkspaceGitStatus> {
  if (!isTauri()) {
    return {
      available: false,
      isRepository: false,
      branch: null,
      repositoryRoot: null,
      message: "Git is only available in the desktop app.",
      summary: {
        added: 0,
        modified: 0,
        deleted: 0,
        renamed: 0,
        copied: 0,
        untracked: 0,
        conflicted: 0,
      },
      entries: [],
    }
  }

  return invoke<WorkspaceGitStatus>("workspace_git_status")
}

export async function workspaceGitStage(path: string): Promise<WorkspaceGitOperationResult> {
  if (!isTauri()) {
    throw new Error("workspace Git operations are only available in the desktop app")
  }

  return invoke<WorkspaceGitOperationResult>("workspace_git_stage", { command: { path } })
}

export async function workspaceGitUnstage(path: string): Promise<WorkspaceGitOperationResult> {
  if (!isTauri()) {
    throw new Error("workspace Git operations are only available in the desktop app")
  }

  return invoke<WorkspaceGitOperationResult>("workspace_git_unstage", { command: { path } })
}

export async function workspaceGitDiscard(path: string): Promise<WorkspaceGitOperationResult> {
  if (!isTauri()) {
    throw new Error("workspace Git operations are only available in the desktop app")
  }

  return invoke<WorkspaceGitOperationResult>("workspace_git_discard", { command: { path } })
}

export async function workspaceGitCommit(message: string): Promise<WorkspaceGitOperationResult> {
  if (!isTauri()) {
    throw new Error("workspace Git operations are only available in the desktop app")
  }

  return invoke<WorkspaceGitOperationResult>("workspace_git_commit", { command: { message } })
}

export async function workspaceGitDiff(command: WorkspaceGitDiffCommand): Promise<WorkspaceGitDiff> {
  if (!isTauri()) {
    return { path: command.path, staged: command.staged, diff: "" }
  }

  return invoke<WorkspaceGitDiff>("workspace_git_diff", { command })
}

export async function workspaceConsoleRun(command: string): Promise<WorkspaceConsoleOutput> {
  if (!isTauri()) {
    throw new Error("workspace console is only available in the desktop app")
  }

  return invoke<WorkspaceConsoleOutput>("workspace_console_run", { command })
}

export async function workspaceTerminalSession(): Promise<WorkspaceTerminalSession> {
  if (!isTauri()) {
    throw new Error("workspace terminal is only available in the desktop app")
  }

  return invoke<WorkspaceTerminalSession>("workspace_terminal_session")
}

export async function workspaceUpdatePluginPreference(
  update: WorkspacePluginPreferenceUpdate,
): Promise<WorkspaceStateResponse> {
  if (!isTauri()) {
    throw new Error("workspace plugin preferences are only available in the desktop app")
  }

  return invoke<WorkspaceStateResponse>("workspace_update_plugin_preference", { update })
}

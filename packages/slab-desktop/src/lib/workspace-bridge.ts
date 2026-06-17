import { invoke } from "@tauri-apps/api/core"
import { ApiError, apiClient } from "@slab/api"

import { isTauri } from "@/hooks/use-tauri"

export const WORKSPACE_STATE_QUERY_KEY = ["workspace-state"] as const

export type WorkspaceInfo = {
  rootPath: string
  name: string
  slabDir: string
  settingsPath: string
  databasePath?: string
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
  plugins?: Record<string, WorkspacePluginConfig>
}

export type WorkspaceStateResponse = {
  current?: WorkspaceInfo | null
  recent?: RecentWorkspace[]
  config?: WorkspaceConfig | null
}

export type WorkspaceFileEntry = {
  id: string
  name: string
  relativePath: string
  kind: "directory" | "file"
  hasChildren: boolean
  sizeBytes?: number | null
  modifiedAt?: number | null
  createdAt?: number | null
}

export type WorkspaceDirectoryResponse = {
  relativePath: string
  entries: WorkspaceFileEntry[]
  truncated: boolean
}

export type WorkspacePathMetadata = {
  relativePath: string
  kind: "directory" | "file"
  sizeBytes: number
  modifiedAt: number
  createdAt: number
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
  originalPath?: string | null
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
  branch?: string | null
  repositoryRoot?: string | null
  message?: string | null
  summary: WorkspaceGitStatusSummary
  entries: WorkspaceGitStatusEntry[]
}

export type WorkspaceConsoleOutput = {
  command: string
  exitCode?: number | null
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

function requireWorkspaceData<T>(
  result: { data?: T; error?: unknown; response: Response },
  emptyMessage: string,
): T {
  if (!result.response.ok || result.error) {
    throw ApiError.fromResponse(result.response, result.error)
  }

  if (result.data === undefined) {
    throw new Error(emptyMessage)
  }

  return result.data
}

export async function workspaceState(): Promise<WorkspaceStateResponse> {
  return requireWorkspaceData(
    await apiClient.GET("/v1/workspace"),
    "Workspace state returned an empty response.",
  )
}

export async function workspaceOpen(rootPath: string): Promise<WorkspaceStateResponse> {
  return requireWorkspaceData(
    await apiClient.POST("/v1/workspace/open", {
      body: { rootPath },
    }),
    "Workspace open returned an empty response.",
  )
}

export async function workspaceClose(): Promise<WorkspaceStateResponse> {
  return requireWorkspaceData(
    await apiClient.POST("/v1/workspace/close"),
    "Workspace close returned an empty response.",
  )
}

export async function workspaceReadDirectory(
  relativePath?: string,
  options?: { includeIgnored?: boolean },
): Promise<WorkspaceDirectoryResponse> {
  return requireWorkspaceData(
    await apiClient.GET("/v1/workspace/directory", {
      params: {
        query: {
          includeIgnored: options?.includeIgnored,
          relativePath,
        },
      },
    }),
    "Workspace directory returned an empty response.",
  )
}

export async function workspaceReadFile(relativePath: string): Promise<WorkspaceFileContent> {
  return requireWorkspaceData(
    await apiClient.GET("/v1/workspace/files", {
      params: { query: { relativePath } },
    }),
    `Workspace file '${relativePath}' returned an empty response.`,
  )
}

export async function workspaceStatPath(relativePath: string): Promise<WorkspacePathMetadata> {
  return requireWorkspaceData(
    await apiClient.GET("/v1/workspace/path/stat", {
      params: { query: { relativePath } },
    }),
    `Workspace path '${relativePath}' returned an empty response.`,
  )
}

export async function workspaceSearchFiles(query: string): Promise<WorkspaceFileSearchResponse> {
  return requireWorkspaceData(
    await apiClient.GET("/v1/workspace/search", {
      params: { query: { query } },
    }),
    "Workspace file search returned an empty response.",
  )
}

export async function workspaceSearchText(query: string): Promise<WorkspaceTextSearchResponse> {
  return requireWorkspaceData(
    await apiClient.GET("/v1/workspace/search/text", {
      params: { query: { query } },
    }),
    "Workspace text search returned an empty response.",
  )
}

export async function workspaceWriteFile(command: WorkspaceWriteFileCommand): Promise<WorkspaceWriteFileResult> {
  return requireWorkspaceData(
    await apiClient.PUT("/v1/workspace/files", {
      body: command,
    }),
    `Workspace file '${command.relativePath}' write returned an empty response.`,
  )
}

export async function workspaceCreateFile(command: WorkspaceCreateFileCommand): Promise<WorkspacePathResult> {
  return requireWorkspaceData(
    await apiClient.POST("/v1/workspace/files", {
      body: command,
    }),
    `Workspace file '${command.relativePath}' create returned an empty response.`,
  )
}

export async function workspaceCreateDirectory(command: WorkspaceCreateDirectoryCommand): Promise<WorkspacePathResult> {
  return requireWorkspaceData(
    await apiClient.POST("/v1/workspace/directories", {
      body: command,
    }),
    `Workspace directory '${command.relativePath}' create returned an empty response.`,
  )
}

export async function workspaceRenamePath(command: WorkspaceRenamePathCommand): Promise<WorkspacePathResult> {
  return requireWorkspaceData(
    await apiClient.PATCH("/v1/workspace/path", {
      body: command,
    }),
    `Workspace path '${command.fromRelativePath}' rename returned an empty response.`,
  )
}

export async function workspaceDeletePath(command: WorkspaceDeletePathCommand): Promise<WorkspacePathResult> {
  return requireWorkspaceData(
    await apiClient.DELETE("/v1/workspace/path", {
      body: command,
    }),
    `Workspace path '${command.relativePath}' delete returned an empty response.`,
  )
}

export async function workspaceGitStatus(): Promise<WorkspaceGitStatus> {
  return requireWorkspaceData(
    await apiClient.GET("/v1/workspace/git/status"),
    "Workspace Git status returned an empty response.",
  )
}

export async function workspaceGitStage(path: string): Promise<WorkspaceGitOperationResult> {
  return requireWorkspaceData(
    await apiClient.POST("/v1/workspace/git/stage", {
      body: { path },
    }),
    `Workspace Git stage '${path}' returned an empty response.`,
  )
}

export async function workspaceGitUnstage(path: string): Promise<WorkspaceGitOperationResult> {
  return requireWorkspaceData(
    await apiClient.POST("/v1/workspace/git/unstage", {
      body: { path },
    }),
    `Workspace Git unstage '${path}' returned an empty response.`,
  )
}

export async function workspaceGitDiscard(path: string): Promise<WorkspaceGitOperationResult> {
  return requireWorkspaceData(
    await apiClient.POST("/v1/workspace/git/discard", {
      body: { path },
    }),
    `Workspace Git discard '${path}' returned an empty response.`,
  )
}

export async function workspaceGitCommit(message: string): Promise<WorkspaceGitOperationResult> {
  return requireWorkspaceData(
    await apiClient.POST("/v1/workspace/git/commit", {
      body: { message },
    }),
    "Workspace Git commit returned an empty response.",
  )
}

export async function workspaceGitDiff(command: WorkspaceGitDiffCommand): Promise<WorkspaceGitDiff> {
  return requireWorkspaceData(
    await apiClient.POST("/v1/workspace/git/diff", {
      body: command,
    }),
    `Workspace Git diff '${command.path}' returned an empty response.`,
  )
}

export async function workspaceConsoleRun(command: string): Promise<WorkspaceConsoleOutput> {
  return requireWorkspaceData(
    await apiClient.POST("/v1/workspace/console/run", {
      body: { command },
    }),
    "Workspace console command returned an empty response.",
  )
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

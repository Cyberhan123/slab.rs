import { URI } from "@codingame/monaco-vscode-api/vscode/vs/base/common/uri"
import type { WorkspaceEditorSettings } from "@/store/useWorkspaceUiStore"
import {
  ensureWorkspaceLspServices,
  isWorkspaceServicesInitialized,
  syncWorkspaceVscodeRoot,
} from "./workspace-services"
import {
  workspaceLspModelPath,
  workspaceLspRelativePathFromUri,
  workspaceVscodeDirtyCloseTarget,
  workspaceVscodeResourceStringFromEditorInput,
} from "./workspace-uri"

/**
 * Editor-facing orchestration over the VS Code services: opening files, observing
 * active/dirty/close state, running commands, reading the selection, and applying
 * editor settings. Everything here consumes the service container initialized in
 * {@link "./workspace-services.ts"} and the URI helpers in {@link "./workspace-uri.ts"}.
 */

export type WorkspaceVscodeEditorFile = {
  name: string
  relativePath: string
}

export type WorkspaceVscodeEditorState = {
  activeRelativePath: string | null
  openFiles: WorkspaceVscodeEditorFile[]
}

export type WorkspaceEditorSelection = {
  endColumn: number
  endLineNumber: number
  relativePath: string
  startColumn: number
  startLineNumber: number
  text: string
}

type WorkspaceVscodeEditorGroup = {
  activeEditor: unknown | null
  closeEditor: (editor?: unknown, options?: unknown) => Promise<boolean>
  closeEditors?: (editors: readonly unknown[] | Record<string, unknown>, options?: unknown) => Promise<boolean>
  editors?: readonly unknown[]
  onWillDispose?: (listener: () => void) => { dispose(): void }
}

type WorkspaceVscodeEditorGroupsService = {
  groups: readonly WorkspaceVscodeEditorGroup[]
  onDidAddGroup: (listener: (group: WorkspaceVscodeEditorGroup) => void) => { dispose(): void }
}

type WorkspaceVscodeWorkingCopyService = {
  isDirty: (resource: URI) => boolean
}

export async function openWorkspaceVscodeFile({
  options,
  relativePath,
  workspaceRoot,
}: {
  options?: import("./workspace-uri").WorkspaceLspOpenFileOptions
  relativePath: string
  workspaceRoot: string
}) {
  await ensureWorkspaceLspServices(workspaceRoot)
  const { getService, IEditorService } = await import("@codingame/monaco-vscode-api")
  const editorService = await getService(IEditorService)
  await editorService.openEditor({
    resource: URI.parse(workspaceLspModelPath(workspaceRoot, relativePath)),
    options: {
      pinned: true,
      revealIfOpened: true,
      selection: options?.startLineNumber && options.startColumn
        ? {
          endColumn: options.endColumn ?? options.startColumn,
          endLineNumber: options.endLineNumber ?? options.startLineNumber,
          startColumn: options.startColumn,
          startLineNumber: options.startLineNumber,
        }
        : undefined,
    },
  })
}

export async function watchWorkspaceVscodeActiveFile(
  workspaceRoot: string,
  onChange: (relativePath: string | null) => void,
) {
  return watchWorkspaceVscodeEditorState(workspaceRoot, (state) => {
    onChange(state.activeRelativePath)
  })
}

/**
 * Reports the dirty state of the active VS Code editor part. The desktop editor
 * surface (`WorkspaceVscodePart part="editor"`) edits through the VS Code working
 * copy service, not the React `editorContent` state, so this is the only signal
 * that catches unsaved edits made directly in the embedded editor. Any failure to
 * acquire the services (e.g. browser fallback without Monaco) rejects so the caller
 * can fall back to the React-state comparison.
 */
export async function watchWorkspaceVscodeEditorDirty(
  workspaceRoot: string,
  onChange: (dirty: boolean) => void,
): Promise<{ dispose(): void }> {
  await ensureWorkspaceLspServices(workspaceRoot)
  const { getService, IEditorService, IWorkingCopyService } = await import("@codingame/monaco-vscode-api")
  const editorService = await getService(IEditorService)
  const workingCopyService = await getService(IWorkingCopyService)

  const emit = () => {
    const activeEditor = editorService.activeEditor
    const resource = activeEditor ? workspaceVscodeResourceStringFromEditorInput(activeEditor) : null
    if (!resource) {
      onChange(false)
      return
    }

    try {
      onChange(workingCopyService.isDirty(URI.parse(resource)))
    } catch (error) {
      console.debug("workspace VS Code dirty lookup failed", { error })
      onChange(false)
    }
  }

  emit()
  const dirtyDisposable = workingCopyService.onDidChangeDirty(emit)
  const activeDisposable = editorService.onDidActiveEditorChange(emit)
  return {
    dispose() {
      dirtyDisposable.dispose()
      activeDisposable.dispose()
    },
  }
}

export async function watchWorkspaceVscodeEditorCloseRequests(
  workspaceRoot: string,
  confirmCloseDirtyFile: (relativePath: string) => Promise<boolean>,
): Promise<{ dispose(): void }> {
  await ensureWorkspaceLspServices(workspaceRoot)
  const { getService, IEditorGroupsService, IWorkingCopyService } = await import("@codingame/monaco-vscode-api")
  const editorGroupsService = await getService(IEditorGroupsService) as WorkspaceVscodeEditorGroupsService
  const workingCopyService = await getService(IWorkingCopyService) as WorkspaceVscodeWorkingCopyService
  const disposables: Array<{ dispose(): void }> = []
  const patchedGroups = new Map<WorkspaceVscodeEditorGroup, () => void>()

  const confirmDirtyEditors = async (editors: readonly unknown[]) => {
    for (const editor of editors) {
      const relativePath = workspaceVscodeDirtyCloseTarget(workspaceRoot, editor, (resource) =>
        workingCopyService.isDirty(URI.parse(resource)),
      )
      // eslint-disable-next-line no-await-in-loop -- close prompts must stop at the first denied dirty file.
      if (relativePath && !(await confirmCloseDirtyFile(relativePath))) {
        return false
      }
    }
    return true
  }

  const patchGroup = (group: WorkspaceVscodeEditorGroup) => {
    if (patchedGroups.has(group)) {
      return
    }

    const originalCloseEditor = group.closeEditor.bind(group)
    const originalCloseEditors = group.closeEditors?.bind(group)
    const patchedCloseEditor: WorkspaceVscodeEditorGroup["closeEditor"] = async (editor, options) => {
      const target = editor ?? group.activeEditor
      if (target && !(await confirmDirtyEditors([target]))) {
        return false
      }
      return originalCloseEditor(editor, options)
    }

    const patchedCloseEditors: WorkspaceVscodeEditorGroup["closeEditors"] = originalCloseEditors
      ? async (editorsOrFilter, options) => {
          const editors = Array.isArray(editorsOrFilter)
            ? editorsOrFilter
            : closeEditorsFilterCandidates(group, editorsOrFilter as Record<string, unknown>)
          if (!(await confirmDirtyEditors(editors))) {
            return false
          }
          return originalCloseEditors(editorsOrFilter, options)
        }
      : undefined

    const previousCloseEditor = group.closeEditor
    const previousCloseEditors = group.closeEditors
    group.closeEditor = patchedCloseEditor
    if (patchedCloseEditors) {
      group.closeEditors = patchedCloseEditors
    }

    const restore = () => {
      if (group.closeEditor === patchedCloseEditor) {
        group.closeEditor = previousCloseEditor
      }
      if (patchedCloseEditors && group.closeEditors === patchedCloseEditors) {
        group.closeEditors = previousCloseEditors
      }
      patchedGroups.delete(group)
    }

    patchedGroups.set(group, restore)
    if (group.onWillDispose) {
      disposables.push(group.onWillDispose(restore))
    }
  }

  editorGroupsService.groups.forEach(patchGroup)
  disposables.push(editorGroupsService.onDidAddGroup(patchGroup))

  return {
    dispose() {
      disposables.forEach((disposable) => disposable.dispose())
      Array.from(patchedGroups.values()).forEach((restore) => restore())
      patchedGroups.clear()
    },
  }
}

export async function watchWorkspaceVscodeEditorState(
  workspaceRoot: string,
  onChange: (state: WorkspaceVscodeEditorState) => void,
) {
  await ensureWorkspaceLspServices(workspaceRoot)
  const { getService, IEditorService } = await import("@codingame/monaco-vscode-api")
  const editorService = await getService(IEditorService)
  const emitEditorState = () => {
    onChange({
      activeRelativePath: relativePathFromEditorInput(workspaceRoot, editorService.activeEditor),
      openFiles: openFilesFromEditorInputs(workspaceRoot, editorService.editors),
    })
  }

  emitEditorState()
  const activeDisposable = editorService.onDidActiveEditorChange(emitEditorState)
  const editorsDisposable = editorService.onDidEditorsChange(emitEditorState)
  return {
    dispose() {
      activeDisposable.dispose()
      editorsDisposable.dispose()
    },
  }
}

export async function runWorkspaceVscodeCommand(commandId: string, workspaceRoot?: string | null) {
  await ensureWorkspaceLspServices(workspaceRoot)
  const { commands } = await import("vscode")
  await commands.executeCommand(commandId)
}

export async function getWorkspaceVscodeSelection(
  workspaceRoot?: string | null,
): Promise<WorkspaceEditorSelection | null> {
  if (!workspaceRoot) {
    return null
  }

  await ensureWorkspaceLspServices(workspaceRoot)
  const { window } = await import("vscode")
  const editor = window.activeTextEditor
  if (!editor || editor.selection.isEmpty) {
    return null
  }

  const relativePath = workspaceLspRelativePathFromUri(
    workspaceRoot,
    editor.document.uri.toString(),
  )
  if (!relativePath) {
    return null
  }

  return {
    endColumn: editor.selection.end.character + 1,
    endLineNumber: editor.selection.end.line + 1,
    relativePath,
    startColumn: editor.selection.start.character + 1,
    startLineNumber: editor.selection.start.line + 1,
    text: editor.document.getText(editor.selection),
  }
}

export async function applyWorkspaceEditorSettings(
  settings: WorkspaceEditorSettings,
  workspaceRoot?: string | null,
) {
  if (!isWorkspaceServicesInitialized()) {
    return
  }
  if (workspaceRoot) {
    void syncWorkspaceVscodeRoot(workspaceRoot).catch((error) => {
      console.debug("workspace VS Code root sync failed", { error, workspaceRoot })
    })
  }
  const { ConfigurationTarget, workspace } = await import("vscode")
  const editorConfig = workspace.getConfiguration("editor")
  await Promise.all([
    editorConfig.update("fontSize", settings.fontSize, ConfigurationTarget.Global),
    editorConfig.update("tabSize", settings.tabSize, ConfigurationTarget.Global),
    editorConfig.update("wordWrap", settings.wordWrap, ConfigurationTarget.Global),
    editorConfig.update("minimap.enabled", settings.minimapEnabled, ConfigurationTarget.Global),
  ])
}

function relativePathFromEditorInput(workspaceRoot: string, input: unknown) {
  const resource = workspaceVscodeResourceStringFromEditorInput(input)
  if (!resource) {
    return null
  }

  return workspaceLspRelativePathFromUri(workspaceRoot, resource)
}

function openFilesFromEditorInputs(workspaceRoot: string, inputs: readonly unknown[]) {
  const seen = new Set<string>()
  const files: WorkspaceVscodeEditorFile[] = []

  for (const input of inputs) {
    const relativePath = relativePathFromEditorInput(workspaceRoot, input)
    if (!relativePath || seen.has(relativePath)) {
      continue
    }

    seen.add(relativePath)
    files.push({
      relativePath,
      name: relativePath.split("/").findLast(Boolean) ?? relativePath,
    })
  }

  return files
}

function closeEditorsFilterCandidates(
  group: WorkspaceVscodeEditorGroup,
  filter: Record<string, unknown>,
) {
  if (filter.savedOnly) {
    return []
  }

  const except = filter.except
  return (group.editors ?? []).filter((editor) => editor !== except)
}

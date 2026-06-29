import * as monaco from "monaco-editor"
import { URI } from "@codingame/monaco-vscode-api/vscode/vs/base/common/uri"
import type { ITextResourceEditorInput } from "@codingame/monaco-vscode-api/vscode/vs/platform/editor/common/editor"
import type {
  IResourceDiffEditorInput,
  IUntypedEditorInput,
} from "@codingame/monaco-vscode-api/vscode/vs/workbench/common/editor"
import type { WorkspaceGitDiff } from "@/lib/workspace-bridge"
import type { WorkspaceEditorSettings } from "@/store/useWorkspaceUiStore"
import { lspLanguageForFile } from "./workspace-page-utils"
import {
  startWorkspaceLspSession,
  type WorkspaceLspSession,
} from "./workspace-language-client"
import {
  ensureWorkspaceLspServices,
  getWorkspaceVscodeApi,
  isWorkspaceServicesInitialized,
  syncWorkspaceVscodeRoot,
} from "./workspace-services"
import {
  workspaceLspImportSpecifierPositionForTarget,
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
  id: number
  onWillDispose?: (listener: () => void) => { dispose(): void }
}

type WorkspaceVscodeEditorGroupsService = {
  activeGroup?: WorkspaceVscodeEditorGroup
  groups: readonly WorkspaceVscodeEditorGroup[]
  onDidAddGroup: (listener: (group: WorkspaceVscodeEditorGroup) => void) => { dispose(): void }
}

type WorkspaceVscodeWorkingCopyService = {
  isDirty: (resource: URI) => boolean
}

type WorkspaceVscodeCodeEditor = monaco.editor.ICodeEditor & {
  addAction?: (descriptor: monaco.editor.IActionDescriptor) => monaco.IDisposable
}

type WorkspaceVscodeTextModel = monaco.editor.ITextModel

type WorkspaceVscodeEditorControl = {
  getModifiedEditor?: () => WorkspaceVscodeCodeEditor
  getModel?: () => WorkspaceVscodeTextModel | null
}

type WorkspaceVscodeEditorDebugState = {
  activeRelativePath: string | null
  activeGroup?: {
    id: number
    isStandalone: boolean
  }
  groups: Array<{
    editorCount: number
    id: number
    isStandalone: boolean
  }>
  groupEditors: WorkspaceVscodeEditorFile[]
  openFiles: WorkspaceVscodeEditorFile[]
  tabCount: number
}

const workspaceDefinitionProviderTimeoutMs = 5_000

type WorkspaceLspSessionState = {
  language: string
  session: WorkspaceLspSession
  workspaceRoot: string
} | null

let workspaceLspSession: WorkspaceLspSessionState = null
let workspaceLspSessionStart:
  | {
    key: string
    promise: Promise<WorkspaceLspSessionState>
  }
  | null = null
let workspaceDefinitionNavigation: {
  disposables: Array<{ dispose(): void }>
  editor: WorkspaceVscodeCodeEditor
} | null = null

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
  const editorGroupId = await workspaceEditorGroupId()
  const resource = URI.parse(workspaceLspModelPath(workspaceRoot, relativePath))
  const editorService = await getService(IEditorService)
  const input: IUntypedEditorInput = {
    resource,
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
  }
  await editorService.openEditor(
    input,
    editorGroupId,
  )
}

export async function openWorkspaceVscodeDiff({
  diff,
  workspaceRoot,
}: {
  diff: WorkspaceGitDiff
  workspaceRoot: string
}) {
  await ensureWorkspaceLspServices(workspaceRoot)
  const { getService, IEditorService } = await import("@codingame/monaco-vscode-api")
  const editorService = await getService(IEditorService)
  const editorGroupId = await workspaceEditorGroupId()
  const languageId = languageIdForPath(diff.path)
  const encodedPath = encodeURIComponent(diff.path)
  const side = diff.staged ? "staged" : "worktree"
  const original: Omit<ITextResourceEditorInput, "options"> = {
    contents: diff.originalContent,
    languageId,
    resource: URI.parse(`untitled:slab-diff/${side}/${encodedPath}/original`),
  }
  const modified: Omit<ITextResourceEditorInput, "options"> = {
    contents: diff.modifiedContent,
    languageId,
    resource: URI.parse(`untitled:slab-diff/${side}/${encodedPath}/modified`),
  }
  const input: IResourceDiffEditorInput = {
    label: `${diff.path} (${side})`,
    original,
    modified,
    options: {
      pinned: true,
      revealIfOpened: true,
    },
  }
  await editorService.openEditor(
    input,
    editorGroupId,
  )
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
  const { getService, IEditorGroupsService, IEditorService } = await import("@codingame/monaco-vscode-api")
  const editorService = await getService(IEditorService)
  const editorGroupsService = await getService(IEditorGroupsService) as WorkspaceVscodeEditorGroupsService
  const emitEditorState = () => {
    const editorGroups = workspaceEditorGroups(editorGroupsService)
    const groupEditors = openFilesFromEditorInputs(
      workspaceRoot,
      editorGroups.flatMap((group) => group.editors ?? []),
    )
    const state = {
      activeRelativePath: relativePathFromEditorInput(workspaceRoot, editorService.activeEditor),
      openFiles: openFilesFromEditorInputs(workspaceRoot, editorService.editors),
    }
    const openFiles = groupEditors.length > state.openFiles.length ? groupEditors : state.openFiles
    setWorkspaceVscodeEditorDebugState({
      activeRelativePath: state.activeRelativePath,
      activeGroup: editorGroupsService.activeGroup
        ? {
          id: editorGroupsService.activeGroup.id,
          isStandalone: isStandaloneWorkspaceEditorGroup(editorGroupsService.activeGroup),
        }
        : undefined,
      groups: editorGroupsService.groups.map((group) => ({
        editorCount: group.editors?.length ?? 0,
        id: group.id,
        isStandalone: isStandaloneWorkspaceEditorGroup(group),
      })),
      groupEditors,
      openFiles,
      tabCount: Math.max(editorService.editors.length, groupEditors.length),
    })
    onChange({
      activeRelativePath: state.activeRelativePath,
      openFiles,
    })
    void syncActiveWorkspaceLspSession(
      workspaceRoot,
      state.activeRelativePath,
      editorService.activeTextEditorControl as WorkspaceVscodeEditorControl | undefined,
    ).catch((error) => {
      console.debug("workspace VS Code LSP session sync failed", { error, workspaceRoot })
    })
  }

  emitEditorState()
  const activeDisposable = editorService.onDidActiveEditorChange(emitEditorState)
  const editorsDisposable = editorService.onDidEditorsChange(emitEditorState)
  return {
    dispose() {
      activeDisposable.dispose()
      editorsDisposable.dispose()
      void workspaceLspSession?.session.dispose()
      workspaceLspSession = null
    },
  }
}

export async function runWorkspaceVscodeCommand(commandId: string, workspaceRoot?: string | null) {
  if (commandId === "editor.action.revealDefinition" || commandId === "slab.workspace.goToDefinition") {
    await runWorkspaceDefinitionNavigation(workspaceRoot)
    return
  }

  const { commands } = await getWorkspaceVscodeApi(workspaceRoot)
  await commands.executeCommand(commandId)
}

export async function getWorkspaceVscodeSelection(
  workspaceRoot?: string | null,
): Promise<WorkspaceEditorSelection | null> {
  if (!workspaceRoot) {
    return null
  }

  const { window } = await getWorkspaceVscodeApi(workspaceRoot)
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
  const { ConfigurationTarget, workspace } = await getWorkspaceVscodeApi(workspaceRoot)
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

async function syncActiveWorkspaceLspSession(
  workspaceRoot: string,
  relativePath: string | null,
  activeControl: WorkspaceVscodeEditorControl | undefined,
) {
  const activeModel = activeWorkspaceTextModel(activeControl)
  if (!relativePath || !activeModel) {
    setWorkspaceLspSessionDebugState(null)
    await disposeWorkspaceLspSession()
    return
  }

  const language = lspLanguageForFile(relativePath)
  const sessionKey = `${workspaceRoot}\0${language}`
  if (
    workspaceLspSession &&
    (workspaceLspSession.workspaceRoot !== workspaceRoot || workspaceLspSession.language !== language)
  ) {
    await disposeWorkspaceLspSession()
  }
  if (workspaceLspSessionStart && workspaceLspSessionStart.key !== sessionKey) {
    workspaceLspSessionStart = null
  }

  if (!workspaceLspSession) {
    let sessionStart = workspaceLspSessionStart
    if (!sessionStart) {
      sessionStart = {
        key: sessionKey,
        promise: startWorkspaceLspSession({
          language,
          model: activeModel,
          workspaceRoot,
        }).then((session) => {
          if (!session) {
            return null
          }
          return { language, session, workspaceRoot }
        }).finally(() => {
          if (workspaceLspSessionStart?.key === sessionKey) {
            workspaceLspSessionStart = null
          }
        }),
      }
      workspaceLspSessionStart = sessionStart
    }

    const session = await sessionStart.promise
    if (!session) {
      setWorkspaceLspSessionDebugState({
        activeRelativePath: relativePath,
        language,
        ready: false,
        workspaceRoot,
      })
      disposeWorkspaceDefinitionNavigation()
      return
    }
    workspaceLspSession = session
  } else {
    await workspaceLspSession.session.registerModel(activeModel)
  }
  registerWorkspaceDefinitionNavigation(
    workspaceRoot,
    activeControl,
    workspaceLspSession.session,
  )
  setWorkspaceLspSessionDebugState({
    activeRelativePath: relativePath,
    language,
    ready: true,
    workspaceRoot,
  })
}

function activeWorkspaceTextModel(control: WorkspaceVscodeEditorControl | undefined) {
  return (control?.getModifiedEditor?.() ?? control)?.getModel?.() ?? null
}

async function disposeWorkspaceLspSession() {
  const session = workspaceLspSession
  workspaceLspSession = null
  workspaceLspSessionStart = null
  disposeWorkspaceDefinitionNavigation()
  await session?.session.dispose()
}

function setWorkspaceLspSessionDebugState(state: unknown) {
  if (typeof window === "undefined") {
    return
  }

  ;(window as typeof window & { __SLAB_WORKSPACE_LSP_SESSION__?: unknown })[
    "__SLAB_WORKSPACE_LSP_SESSION__"
  ] = state
}

function registerWorkspaceDefinitionNavigation(
  workspaceRoot: string,
  activeControl: WorkspaceVscodeEditorControl | undefined,
  session: WorkspaceLspSession,
) {
  const editor = (activeControl?.getModifiedEditor?.() ?? activeControl) as WorkspaceVscodeCodeEditor | undefined
  if (!editor?.getModel) {
    disposeWorkspaceDefinitionNavigation()
    return
  }
  if (workspaceDefinitionNavigation?.editor === editor) {
    return
  }

  disposeWorkspaceDefinitionNavigation()

  const runDefinitionNavigation = async (position?: import("monaco-editor").IPosition | null) => {
    const model = editor.getModel()
    const targetPosition = position ?? editor.getPosition()
    if (!model || !targetPosition) {
      return
    }

    const target = await definitionTargetForPosition(
      workspaceRoot,
      model,
      targetPosition,
      session,
    )
    setWorkspaceDefinitionDebugState({
      modelUri: model.uri.toString(),
      position: targetPosition,
      target,
    })
    if (!target) {
      return
    }

    await openWorkspaceVscodeFile({
      options: target,
      relativePath: target.relativePath,
      workspaceRoot,
    })
  }

  const disposables: Array<{ dispose(): void }> = []
  if (editor.addAction) {
    disposables.push(editor.addAction({
      id: "slab.workspace.goToDefinition",
      keybindings: [monaco.KeyCode.F12],
      label: "Go to Definition",
      run: () => runWorkspaceDefinitionNavigation(workspaceRoot).catch((error) => {
        console.debug("workspace VS Code definition action failed", { error, workspaceRoot })
      }),
    }))
  }
  disposables.push(editor.onMouseDown((event) => {
    const position = event.target.position
    if (!position || !event.event.leftButton || (!event.event.ctrlKey && !event.event.metaKey)) {
      return
    }

    event.event.preventDefault()
    event.event.stopPropagation()
    void runDefinitionNavigation(position).catch((error) => {
      console.debug("workspace VS Code definition click failed", {
        error,
        position,
        workspaceRoot,
      })
    })
  }))

  workspaceDefinitionNavigation = { disposables, editor }
}

async function runWorkspaceDefinitionNavigation(workspaceRoot?: string | null) {
  if (!workspaceRoot || !workspaceLspSession) {
    return
  }

  const { getService, IEditorService } = await import("@codingame/monaco-vscode-api")
  const editorService = await getService(IEditorService)
  const editor = activeWorkspaceCodeEditor(editorService.activeTextEditorControl as WorkspaceVscodeEditorControl | undefined)
  const model = editor?.getModel()
  const position = editor?.getPosition()
  if (!model || !position) {
    return
  }

  const target = await definitionTargetForPosition(
    workspaceRoot,
    model,
    position,
    workspaceLspSession.session,
  )
  setWorkspaceDefinitionDebugState({
    modelUri: model.uri.toString(),
    position,
    target,
  })
  if (!target) {
    return
  }

  await openWorkspaceVscodeFile({
    options: target,
    relativePath: target.relativePath,
    workspaceRoot,
  })
}

function activeWorkspaceCodeEditor(activeControl: WorkspaceVscodeEditorControl | undefined) {
  return (activeControl?.getModifiedEditor?.() ?? activeControl) as WorkspaceVscodeCodeEditor | undefined
}

async function definitionTargetForPosition(
  workspaceRoot: string,
  model: WorkspaceVscodeTextModel,
  position: monaco.IPosition,
  session: WorkspaceLspSession,
) {
  const providerTarget = await definitionTargetFromVscodeProviderWithTimeout(
    workspaceRoot,
    model,
    position,
  )
  if (!providerTarget) {
    return session.definitionTarget(model, position)
  }

  const currentRelativePath = workspaceLspRelativePathFromUri(workspaceRoot, model.uri.toString())
  if (currentRelativePath !== providerTarget.relativePath || !providerTarget.startLineNumber) {
    return providerTarget
  }

  const importSpecifierPosition = workspaceLspImportSpecifierPositionForTarget(
    model.getLineContent(providerTarget.startLineNumber),
    providerTarget,
  )
  if (!importSpecifierPosition) {
    return providerTarget
  }

  return session.definitionTarget(model, importSpecifierPosition) ?? providerTarget
}

async function definitionTargetFromVscodeProvider(
  workspaceRoot: string,
  model: WorkspaceVscodeTextModel,
  position: monaco.IPosition,
) {
  const { Position, Uri, commands } = await getWorkspaceVscodeApi(workspaceRoot)
  const definitions = await commands.executeCommand<unknown[]>(
    "vscode.executeDefinitionProvider",
    Uri.parse(model.uri.toString()),
    new Position(position.lineNumber - 1, position.column - 1),
  )
  return workspaceDefinitionTargetFromVscodeResult(workspaceRoot, definitions)
}

async function definitionTargetFromVscodeProviderWithTimeout(
  workspaceRoot: string,
  model: WorkspaceVscodeTextModel,
  position: monaco.IPosition,
) {
  return Promise.race([
    definitionTargetFromVscodeProvider(workspaceRoot, model, position),
    new Promise<null>((resolve) => {
      window.setTimeout(() => resolve(null), workspaceDefinitionProviderTimeoutMs)
    }),
  ])
}

function workspaceDefinitionTargetFromVscodeResult(workspaceRoot: string, definitions: unknown) {
  const entries = Array.isArray(definitions) ? definitions : definitions ? [definitions] : []
  for (const entry of entries) {
    if (!entry || typeof entry !== "object") {
      continue
    }

    const record = entry as Record<string, unknown>
    const uri = record.targetUri ?? record.uri
    if (!uri || typeof uri !== "object" || !("toString" in uri)) {
      continue
    }

    const relativePath = workspaceLspRelativePathFromUri(workspaceRoot, uri.toString())
    if (relativePath === null) {
      continue
    }

    const range = vscodeRangeLike(record.targetSelectionRange ?? record.range ?? record.targetRange)
    return {
      relativePath,
      ...range,
    }
  }
  return null
}

function vscodeRangeLike(range: unknown) {
  if (!range || typeof range !== "object") {
    return {}
  }

  const record = range as Record<string, unknown>
  const start = vscodePositionLike(record.start)
  if (!start) {
    return {}
  }
  const end = vscodePositionLike(record.end)
  return {
    endColumn: end?.startColumn ?? start.startColumn,
    endLineNumber: end?.startLineNumber ?? start.startLineNumber,
    startColumn: start.startColumn,
    startLineNumber: start.startLineNumber,
  }
}

function vscodePositionLike(position: unknown) {
  if (!position || typeof position !== "object") {
    return null
  }

  const record = position as Record<string, unknown>
  if (typeof record.line !== "number" || typeof record.character !== "number") {
    return null
  }

  return {
    startColumn: record.character + 1,
    startLineNumber: record.line + 1,
  }
}

function disposeWorkspaceDefinitionNavigation() {
  workspaceDefinitionNavigation?.disposables.forEach((disposable) => disposable.dispose())
  workspaceDefinitionNavigation = null
}

function setWorkspaceDefinitionDebugState(state: unknown) {
  if (typeof window === "undefined") {
    return
  }

  ;(window as typeof window & { __SLAB_WORKSPACE_DEFINITION_TARGET__?: unknown })[
    "__SLAB_WORKSPACE_DEFINITION_TARGET__"
  ] = state
}

async function workspaceEditorGroupId() {
  const { getService, IEditorGroupsService } = await import("@codingame/monaco-vscode-api")
  const editorGroupsService = await getService(IEditorGroupsService) as WorkspaceVscodeEditorGroupsService
  const activeGroup = editorGroupsService.activeGroup
  if (activeGroup && !isStandaloneWorkspaceEditorGroup(activeGroup)) {
    return activeGroup.id
  }
  return workspaceEditorGroups(editorGroupsService)[0]?.id
}

function workspaceEditorGroups(editorGroupsService: WorkspaceVscodeEditorGroupsService) {
  return editorGroupsService.groups.filter((group) => !isStandaloneWorkspaceEditorGroup(group))
}

function isStandaloneWorkspaceEditorGroup(group: { id: number }) {
  const label = "label" in group ? String((group as { label?: unknown }).label ?? "") : ""
  return group.id < 0 || label.startsWith("standalone editor")
}

function setWorkspaceVscodeEditorDebugState(state: WorkspaceVscodeEditorDebugState) {
  if (typeof window === "undefined") {
    return
  }

  ;(window as typeof window & { __SLAB_WORKSPACE_EDITOR_STATE__?: WorkspaceVscodeEditorDebugState })[
    "__SLAB_WORKSPACE_EDITOR_STATE__"
  ] = state
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

function languageIdForPath(path: string) {
  const extension = path.split(".").pop()?.toLowerCase()
  switch (extension) {
    case "ts":
      return "typescript"
    case "tsx":
      return "typescriptreact"
    case "js":
    case "mjs":
    case "cjs":
      return "javascript"
    case "jsx":
      return "javascriptreact"
    case "json":
    case "jsonc":
      return "json"
    case "css":
      return "css"
    case "scss":
      return "scss"
    case "less":
      return "less"
    case "html":
      return "html"
    case "md":
    case "mdx":
      return "markdown"
    case "rs":
      return "rust"
    case "py":
      return "python"
    case "go":
      return "go"
    case "c":
    case "h":
      return "c"
    case "cc":
    case "cpp":
    case "cxx":
    case "hpp":
      return "cpp"
    case "yaml":
    case "yml":
      return "yaml"
    case "xml":
    case "svg":
      return "xml"
    default:
      return "plaintext"
  }
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

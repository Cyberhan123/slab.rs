import type * as Monaco from "monaco-editor"
import type { WorkspaceEditorSettings } from "@/store/useWorkspaceUiStore"
import { URI } from "@codingame/monaco-vscode-api/vscode/vs/base/common/uri"
import { IWorkspaceContextService } from "@codingame/monaco-vscode-api/vscode/vs/platform/workspace/common/workspace.service"
import {
  initialize as initializeMonacoWrapper,
  isInitialized as workspaceMonacoIsInitialized,
  registerEditorOpenHandler,
  registerServices,
  registerWorker,
  Worker as MonacoWorker,
} from "@codingame/monaco-editor-wrapper"
import "@codingame/monaco-editor-wrapper/features/search"
import extensionHostWorkerUrl from "@codingame/monaco-vscode-api/workers/extensionHost.worker?worker&url"
import getAccessibilityServiceOverride from "@codingame/monaco-vscode-accessibility-service-override"
import getConfigurationServiceOverride, { reinitializeWorkspace } from "@codingame/monaco-vscode-configuration-service-override"
import { whenReady as cppExtensionReady } from "@codingame/monaco-vscode-cpp-default-extension"
import getDialogsServiceOverride from "@codingame/monaco-vscode-dialogs-service-override"
import getExplorerServiceOverride from "@codingame/monaco-vscode-explorer-service-override"
import getExtensionServiceOverride from "@codingame/monaco-vscode-extensions-service-override"
import getLifecycleServiceOverride from "@codingame/monaco-vscode-lifecycle-service-override"
import { whenReady as goExtensionReady } from "@codingame/monaco-vscode-go-default-extension"
import getKeybindingsServiceOverride from "@codingame/monaco-vscode-keybindings-service-override"
import getLanguageDetectionWorkerServiceOverride from "@codingame/monaco-vscode-language-detection-worker-service-override"
import getLanguagesServiceOverride from "@codingame/monaco-vscode-languages-service-override"
import getMarkersServiceOverride from "@codingame/monaco-vscode-markers-service-override"
import getModelServiceOverride from "@codingame/monaco-vscode-model-service-override"
import getNotificationsServiceOverride from "@codingame/monaco-vscode-notifications-service-override"
import getOutputServiceOverride from "@codingame/monaco-vscode-output-service-override"
import getSearchServiceOverride from "@codingame/monaco-vscode-search-service-override"
import { whenReady as sqlExtensionReady } from "@codingame/monaco-vscode-sql-default-extension"
import getStorageServiceOverride from "@codingame/monaco-vscode-storage-service-override"
import getTerminalServiceOverride from "@codingame/monaco-vscode-terminal-service-override"
import getTextmateServiceOverride from "@codingame/monaco-vscode-textmate-service-override"
import getThemeServiceOverride from "@codingame/monaco-vscode-theme-service-override"
import { whenReady as setiThemeExtensionReady } from "@codingame/monaco-vscode-theme-seti-default-extension"
import getViewsServiceOverride from "@codingame/monaco-vscode-views-service-override"
import getWorkingCopyServiceOverride from "@codingame/monaco-vscode-working-copy-service-override"
import { whenReady as xmlExtensionReady } from "@codingame/monaco-vscode-xml-default-extension"
import { whenReady as emmetExtensionReady } from "@codingame/monaco-vscode-emmet-default-extension"
import { whenReady as dockerExtensionReady } from "@codingame/monaco-vscode-docker-default-extension"
import { whenReady as dotenvExtensionReady } from "@codingame/monaco-vscode-dotenv-default-extension"
import {
  BaseLanguageClient,
  CloseAction,
  ErrorAction,
  type LanguageClientOptions,
  type MessageTransports,
} from "vscode-languageclient/browser.js"
import { SERVER_BASE_URL } from "@slab/api/config"
import {
  workspaceLspDefinitionTargetFromResult,
  workspaceLspFileUri,
  workspaceLspImportSpecifierPositionForTarget,
  supportsWorkspaceLsp,
  workspaceLspModelPath,
  workspaceLspRelativePathFromUri,
  workspaceVscodeDirtyCloseTarget,
  workspaceVscodeResourceStringFromEditorInput,
  type WorkspaceLspDefinitionTarget,
  type WorkspaceLspOpenFileOptions,
} from "./workspace-lsp-utils"
import { slabTerminalBackend } from "./workspace-terminal-service"
import { getStandaloneMonacoEditorOverrides } from "./workspace-standalone-monaco"
import {
  clearSlabWorkspaceFileSystemCache,
  ensureSlabWorkspaceFileSystem,
  preloadSlabWorkspaceFileSystem,
  setSlabWorkspaceFileSystemRoot,
} from "./workspace-file-system-provider"

export {
  supportsWorkspaceLsp,
  workspaceLspDefinitionTargetFromResult,
  workspaceLspImportSpecifierPositionForTarget,
  workspaceLspModelPath,
  workspaceLspRelativePathFromUri,
  workspaceVscodeDirtyCloseTarget,
} from "./workspace-lsp-utils"
export type {
  WorkspaceLspDefinitionTarget,
  WorkspaceLspOpenFileOptions,
} from "./workspace-lsp-utils"

export type WorkspaceLspSession = {
  definitionTarget: (
    model: Monaco.editor.ITextModel,
    position: Monaco.IPosition,
  ) => Promise<WorkspaceLspDefinitionTarget | null>
  registerModel: (model: Monaco.editor.ITextModel) => Promise<void>
  dispose: () => Promise<void>
}

export type WorkspaceLspOpenFile = (
  relativePath: string,
  options?: WorkspaceLspOpenFileOptions,
) => Promise<Monaco.editor.IStandaloneCodeEditor | undefined>

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

const WORKSPACE_LSP_RECONNECT_INITIAL_DELAY_MS = 250
const WORKSPACE_LSP_RECONNECT_MAX_DELAY_MS = 5_000

type WorkspaceFileService = {
  root: string | null
}

type WorkspaceLanguageClientOptions = {
  clientOptions: LanguageClientOptions
  id: string
  messageTransports: MessageTransports
  name: string
}

class WorkspaceLanguageClient extends BaseLanguageClient {
  private readonly messageTransports: MessageTransports

  constructor({
    clientOptions,
    id,
    messageTransports,
    name,
  }: WorkspaceLanguageClientOptions) {
    super(id, name, clientOptions)
    this.messageTransports = messageTransports
  }

  protected createMessageTransports() {
    return Promise.resolve(this.messageTransports)
  }
}

let monacoVscodeApiReady: Promise<void> | null = null
let currentOpenFile: WorkspaceLspOpenFile | null = null
let currentWorkspaceFileService: WorkspaceFileService = { root: null }
let workspaceFileSystemOverlayRegistered = false
let workspaceEditorOpenHandlerRegistered = false
let workspaceExtensionHostWorkerRegistered = false
let workspaceVscodeServiceOverridesRegistered = false
let workspaceVscodeRoot: string | null = null

function setWorkspaceLspDebugStage(stage: string) {
  if (typeof window === "undefined") {
    return
  }

  ;(window as typeof window & { __SLAB_WORKSPACE_LSP_STAGE__?: string }).__SLAB_WORKSPACE_LSP_STAGE__ = stage
}

function setWorkspaceLspDebugContext(context: unknown) {
  if (typeof window === "undefined") {
    return
  }

  ;(window as typeof window & { __SLAB_WORKSPACE_LSP_CONTEXT__?: unknown }).__SLAB_WORKSPACE_LSP_CONTEXT__ = context
}

function pushWorkspaceLspDebugDirectory(entry: unknown) {
  if (typeof window === "undefined") {
    return
  }

  const target = window as typeof window & { __SLAB_WORKSPACE_LSP_DIRECTORIES__?: unknown[] }
  target.__SLAB_WORKSPACE_LSP_DIRECTORIES__ = [...(target.__SLAB_WORKSPACE_LSP_DIRECTORIES__ ?? []).slice(-20), entry]
}

function pushWorkspaceLspDebugStat(entry: unknown) {
  if (typeof window === "undefined") {
    return
  }

  const target = window as typeof window & { __SLAB_WORKSPACE_LSP_STATS__?: unknown[] }
  target.__SLAB_WORKSPACE_LSP_STATS__ = [...(target.__SLAB_WORKSPACE_LSP_STATS__ ?? []).slice(-30), entry]
}

function workspaceRootUri(workspaceRoot: string) {
  return URI.parse(workspaceLspFileUri(workspaceRoot))
}

export function workspaceLspModelUri(
  monaco: typeof Monaco,
  workspaceRoot: string,
  relativePath: string,
) {
  return monaco.Uri.parse(workspaceLspModelPath(workspaceRoot, relativePath))
}

export function setWorkspaceLspOpenFile(openFile: WorkspaceLspOpenFile | null) {
  currentOpenFile = openFile
}

export function setWorkspaceLspFileServiceRoot(workspaceRoot: string | null) {
  const previousWorkspaceRoot = currentWorkspaceFileService.root
  currentWorkspaceFileService = { root: workspaceRoot }
  if (previousWorkspaceRoot !== workspaceRoot) {
    // Tell the backend delegate its new root before clearing/preloading so its
    // preload race-guard sees the updated root.
    setSlabWorkspaceFileSystemRoot(workspaceRoot)
    clearSlabWorkspaceFileSystemCache()
    if (workspaceRoot) {
      // Pre-load the first few directory levels in one request so the explorer
      // can render without re-fetching each folder as it expands.
      void preloadSlabWorkspaceFileSystem(workspaceRoot).catch((error) => {
        console.debug("workspace preload failed", { workspaceRoot, error })
      })
    }
  }
  if (workspaceRoot && workspaceMonacoIsInitialized()) {
    void syncWorkspaceVscodeRoot(workspaceRoot).catch((error) => {
      console.debug("workspace VS Code root sync failed", { workspaceRoot, error })
    })
  }
}

export function workspaceLspServicesReady() {
  return workspaceMonacoIsInitialized()
}

export function ensureWorkspaceLspServices(workspaceRoot?: string | null) {
  if (workspaceRoot !== undefined) {
    setWorkspaceLspFileServiceRoot(workspaceRoot)
  }

  monacoVscodeApiReady ??= (async () => {
    if (!workspaceMonacoIsInitialized()) {
      setWorkspaceLspDebugStage("register-service-overrides")
      registerWorkspaceVscodeServiceOverrides()
      if (!workspaceFileSystemOverlayRegistered) {
        setWorkspaceLspDebugStage("register-file-system-overlay")
        ensureSlabWorkspaceFileSystem({
          backend: {
            debug: {
              pushDirectory: pushWorkspaceLspDebugDirectory,
              pushStat: pushWorkspaceLspDebugStat,
            },
          },
        })
        setSlabWorkspaceFileSystemRoot(currentWorkspaceFileService.root)
        workspaceFileSystemOverlayRegistered = true
        // setWorkspaceLspFileServiceRoot ran before this registration, so its
        // preload was a no-op on first init; fire it now so the explorer renders
        // without one request per folder.
        if (currentWorkspaceFileService.root) {
          void preloadSlabWorkspaceFileSystem(currentWorkspaceFileService.root).catch((error) => {
            console.debug("workspace preload failed", { error })
          })
        }
      }
      setWorkspaceLspDebugStage("initialize-monaco-wrapper")
      await initializeMonacoWrapper(workspaceWorkbenchOptions(currentWorkspaceFileService.root), {
        registerAdditionalExtensions: true,
        waitForDefaultExtensions: false,
      })
      const { ConfigurationTarget, commands, workspace } = await import("vscode")
      await workspace.getConfiguration("explorer").update(
        "compactFolders",
        false,
        ConfigurationTarget.Global,
      )
      // executeCommand returns a VS Code Thenable (no .catch); wrap it so the
      // refresh is best-effort and never fails service init.
      await Promise.resolve(commands.executeCommand("workbench.files.action.refreshFilesExplorer")).catch(
        (error) => {
          console.debug("workspace explorer refresh failed", { error })
        },
      )
      setWorkspaceLspDebugStage("load-workspace-extensions")
      // Load additional extensions not included in the default set
      void Promise.allSettled([
        setiThemeExtensionReady(),
        emmetExtensionReady(),
        dockerExtensionReady(),
        dotenvExtensionReady(),
        cppExtensionReady(),
        goExtensionReady(),
        sqlExtensionReady(),
        xmlExtensionReady(),
      ])
    }

    if (currentWorkspaceFileService.root) {
      setWorkspaceLspDebugStage("sync-workspace-root")
      void syncWorkspaceVscodeRoot(currentWorkspaceFileService.root).catch((error) => {
        console.debug("workspace VS Code root sync failed", {
          error,
          workspaceRoot: currentWorkspaceFileService.root,
        })
      })
    }

    if (!workspaceEditorOpenHandlerRegistered) {
      setWorkspaceLspDebugStage("register-editor-open-handler")
      registerEditorOpenHandler(async (modelRef, options) => {
        const activeWorkspaceRoot = currentWorkspaceFileService.root
        const selection = editorSelection(options)
        const relativePath = activeWorkspaceRoot
          ? workspaceLspRelativePathFromUri(activeWorkspaceRoot, modelRef.object.textEditorModel.uri.toString())
          : null
        if (relativePath === null || !currentOpenFile) {
          return null
        }

        return (
          (await currentOpenFile(relativePath, {
            endColumn: selection?.endColumn,
            endLineNumber: selection?.endLineNumber,
            startColumn: selection?.startColumn,
            startLineNumber: selection?.startLineNumber,
          })) ?? null
        )
      })
      workspaceEditorOpenHandlerRegistered = true
    }
    setWorkspaceLspDebugStage("ready")
  })().catch((error) => {
    monacoVscodeApiReady = null
    throw error
  })

  return monacoVscodeApiReady.then(async () => {
    if (workspaceRoot) {
      void syncWorkspaceVscodeRoot(workspaceRoot).catch((error) => {
        console.debug("workspace VS Code root sync failed", { error, workspaceRoot })
      })
    }
  })
}

export async function openWorkspaceVscodeFile({
  options,
  relativePath,
  workspaceRoot,
}: {
  options?: WorkspaceLspOpenFileOptions
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
  if (!workspaceMonacoIsInitialized()) {
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

function workspaceWorkbenchOptions(workspaceRoot: string | null) {
  if (!workspaceRoot) {
    return undefined
  }
  const rootUri = workspaceRootUri(workspaceRoot)

  return {
    configurationDefaults: {
      "explorer.compactFolders": false,
    },
    workspaceProvider: {
      open: async () => false,
      trusted: true,
      workspace: {
        folderUri: rootUri,
        id: workspaceRoot,
      },
    },
  }
}

async function syncWorkspaceVscodeRoot(workspaceRoot: string) {
  if (workspaceVscodeRoot === workspaceRoot) {
    return
  }

  if (!workspaceMonacoIsInitialized()) {
    return
  }

  const { getService } = await import("@codingame/monaco-vscode-api")
  const contextService = await getService(IWorkspaceContextService)
  const folders = contextService.getWorkspace().folders
  const rootUri = workspaceRootUri(workspaceRoot)
  setWorkspaceLspDebugContext({
    actual: folders.map((folder) => folder.uri.toString()),
    expected: rootUri.toString(),
  })
  if (folders.length === 1 && folders[0]?.uri.toString() === rootUri.toString()) {
    workspaceVscodeRoot = workspaceRoot
    return
  }

  console.debug("workspace VS Code context root mismatch", {
    actual: folders.map((folder) => folder.uri.toString()),
    expected: rootUri.toString(),
  })
  await reinitializeWorkspace({
    id: workspaceRoot,
    uri: rootUri,
  })
  workspaceVscodeRoot = workspaceRoot
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

function registerWorkspaceVscodeServiceOverrides() {
  if (workspaceVscodeServiceOverridesRegistered) {
    return
  }

  if (!workspaceExtensionHostWorkerRegistered) {
    registerWorker(
      "extensionHostWorkerMain",
      new MonacoWorker(new URL(extensionHostWorkerUrl, import.meta.url), {
        type: "module",
      }),
    )
    workspaceExtensionHostWorkerRegistered = true
  }

  registerServices({
    ...getStandaloneMonacoEditorOverrides(),
    ...getAccessibilityServiceOverride(),
    ...getConfigurationServiceOverride(),
    ...getDialogsServiceOverride(),
    ...getExplorerServiceOverride(),
    ...getExtensionServiceOverride({
      enableWorkerExtensionHost: true,
    }),
    ...getKeybindingsServiceOverride(),
    ...getLanguageDetectionWorkerServiceOverride(),
    ...getLanguagesServiceOverride(),
    ...getLifecycleServiceOverride(),
    ...getMarkersServiceOverride(),
    ...getModelServiceOverride(),
    ...getNotificationsServiceOverride(),
    ...getOutputServiceOverride(),
    ...getSearchServiceOverride(),
    ...getStorageServiceOverride(),
    ...getTerminalServiceOverride(slabTerminalBackend),
    ...getTextmateServiceOverride(),
    ...getThemeServiceOverride(),
    ...getWorkingCopyServiceOverride(),
    ...getViewsServiceOverride(undefined, undefined, (state) => ({
      ...state,
      editor: {
        ...state.editor,
        restoreEditors: false,
      },
      views: {
        ...state.views,
        defaults: ["workbench.explorer.fileView"],
      },
    })),
  })
  workspaceVscodeServiceOverridesRegistered = true
}

export async function startWorkspaceLspSession({
  language,
  monaco,
  model,
  workspaceRoot,
}: {
  language: string
  monaco: typeof Monaco
  model: Monaco.editor.ITextModel
  workspaceRoot: string
}): Promise<WorkspaceLspSession | null> {
  if (!supportsWorkspaceLsp(language)) {
    return null
  }

  let socket: WebSocket | null = null
  let languageClient: WorkspaceLanguageClient | null = null
  let disposed = false
  let reconnectTimer: number | null = null
  let reconnectDelayMs = WORKSPACE_LSP_RECONNECT_INITIAL_DELAY_MS

  try {
    await ensureWorkspaceLspServices()

    // Register the model as a VSCode text document before starting the language client.
    // The Monaco model is created when the editor mounts, which happens before the
    // @codingame/monaco-vscode-api bridge finishes initializing. Because of this timing,
    // the bridge never intercepts the model-creation event and the model is absent from
    // vscode.workspace.textDocuments. Without this registration, MonacoLanguageClient
    // never sends textDocument/didOpen and the server returns nothing for hover/definition.
    const { workspace: vscodeWorkspace, Uri: VscodeUri } = await import("vscode")
    const registeredModelUris = new Set<string>()
    const registeredModels = new Map<string, Monaco.editor.ITextModel>()
    const registerModel = async (modelToRegister: Monaco.editor.ITextModel) => {
      const uri = modelToRegister.uri.toString()
      registeredModels.set(uri, modelToRegister)
      if (registeredModelUris.has(uri)) {
        return
      }

      await vscodeWorkspace.openTextDocument(VscodeUri.parse(uri))
      registeredModelUris.add(uri)
    }
    await registerModel(model)

    const jsonrpc = await import("vscode-ws-jsonrpc")

    const clearReconnectTimer = () => {
      if (reconnectTimer !== null) {
        window.clearTimeout(reconnectTimer)
        reconnectTimer = null
      }
    }

    const connect = async () => {
      clearReconnectTimer()
      const nextSocket = new WebSocket(workspaceLspUrl(language))
      const rpcSocket = jsonrpc.toSocket(nextSocket)
      const nextLanguageClient = new WorkspaceLanguageClient({
        id: `workspace-${language}`,
        name: `Workspace ${language} Language Server`,
        clientOptions: {
          documentSelector: [{ scheme: "file", language }],
          workspaceFolder: {
            index: 0,
            name: "workspace",
            uri: monaco.Uri.parse(workspaceLspFileUri(workspaceRoot)),
          },
          initializationOptions: {
            workspaceRoot,
          },
          errorHandler: {
            error: () => ({ action: ErrorAction.Continue }),
            closed: () => ({ action: CloseAction.DoNotRestart }),
          },
        },
        messageTransports: {
          reader: new jsonrpc.WebSocketMessageReader(rpcSocket),
          writer: new jsonrpc.WebSocketMessageWriter(rpcSocket),
        },
      })

      socket = nextSocket
      languageClient = nextLanguageClient

      nextSocket.addEventListener("close", () => {
        if (disposed || socket !== nextSocket) {
          return
        }
        socket = null
        if (languageClient === nextLanguageClient) {
          languageClient = null
        }
        void nextLanguageClient.stop().catch(() => {})
        scheduleReconnect()
      })

      try {
        await waitForSocketOpen(nextSocket)
        if (disposed || socket !== nextSocket) {
          nextSocket.close(1000, "workspace LSP session replaced")
          await nextLanguageClient.stop().catch(() => {})
          return
        }

        await nextLanguageClient.start()
        registeredModelUris.clear()
        await Promise.all([...registeredModels.values()].map(registerModel))
        reconnectDelayMs = WORKSPACE_LSP_RECONNECT_INITIAL_DELAY_MS
      } catch (error) {
        if (socket === nextSocket) {
          socket = null
        }
        if (languageClient === nextLanguageClient) {
          languageClient = null
        }
        nextSocket.close(1000, "workspace LSP reconnect failed")
        await nextLanguageClient.stop().catch(() => {})
        throw error
      }
    }

    const scheduleReconnect = () => {
      if (disposed || reconnectTimer !== null) {
        return
      }
      const delayMs = reconnectDelayMs
      reconnectDelayMs = Math.min(
        reconnectDelayMs * 2,
        WORKSPACE_LSP_RECONNECT_MAX_DELAY_MS,
      )
      reconnectTimer = window.setTimeout(() => {
        reconnectTimer = null
        void connect().catch((error) => {
          console.debug("workspace LSP reconnect failed", {
            language,
            uri: model.uri.toString(),
            error,
          })
          scheduleReconnect()
        })
      }, delayMs)
    }

    await connect()
    return {
      definitionTarget: async (definitionModel, position) => {
        const currentRelativePath = workspaceLspRelativePathFromUri(workspaceRoot, definitionModel.uri.toString())
        const definitions = await languageClient?.sendRequest<unknown>(
          "textDocument/definition",
          textDocumentPositionParams(definitionModel, position),
        )
        const target = workspaceLspDefinitionTargetFromResult(workspaceRoot, definitions)
        if (!target || target.relativePath !== currentRelativePath || !target.startLineNumber) {
          return target
        }

        const importSpecifierPosition = workspaceLspImportSpecifierPositionForTarget(
          definitionModel.getLineContent(target.startLineNumber),
          target,
        )
        if (!importSpecifierPosition) {
          return target
        }

        const moduleDefinitions = await languageClient?.sendRequest<unknown>(
          "textDocument/definition",
          textDocumentPositionParams(definitionModel, importSpecifierPosition),
        )
        return workspaceLspDefinitionTargetFromResult(workspaceRoot, moduleDefinitions) ?? target
      },
      registerModel,
      dispose: async () => {
        disposed = true
        clearReconnectTimer()
        await languageClient?.stop()
        socket?.close(1000, "workspace LSP session disposed")
      },
    }
  } catch (error) {
    console.debug("workspace LSP unavailable", { language, uri: model.uri.toString(), error })
    closeWorkspaceLspSocket(socket, 1000, "workspace LSP session unavailable")
    await stopWorkspaceLspClient(languageClient)
    return null
  }
}

function closeWorkspaceLspSocket(socket: WebSocket | null, code: number, reason: string) {
  socket?.close(code, reason)
}

function stopWorkspaceLspClient(client: WorkspaceLanguageClient | null) {
  return client?.stop().catch(() => {}) ?? Promise.resolve()
}

function textDocumentPositionParams(
  model: Monaco.editor.ITextModel,
  position: Monaco.IPosition,
) {
  return {
    position: {
      character: position.column - 1,
      line: position.lineNumber - 1,
    },
    textDocument: {
      uri: model.uri.toString(),
    },
  }
}

function editorSelection(options: unknown): WorkspaceLspOpenFileOptions | undefined {
  if (!options || typeof options !== "object" || !("selection" in options)) {
    return undefined
  }

  return options.selection as WorkspaceLspOpenFileOptions | undefined
}

function workspaceLspUrl(language: string) {
  const endpoint = new URL(SERVER_BASE_URL)
  endpoint.protocol = endpoint.protocol === "https:" ? "wss:" : "ws:"
  endpoint.pathname = `/v1/workspace/lsp/${encodeURIComponent(language)}`
  endpoint.search = ""
  endpoint.hash = ""
  return endpoint.toString()
}

function waitForSocketOpen(socket: WebSocket) {
  return new Promise<void>((resolve, reject) => {
    if (socket.readyState === WebSocket.OPEN) {
      resolve()
      return
    }

    socket.addEventListener("open", () => resolve(), { once: true })
    socket.addEventListener("error", () => reject(new Error("workspace LSP websocket failed")), {
      once: true,
    })
  })
}

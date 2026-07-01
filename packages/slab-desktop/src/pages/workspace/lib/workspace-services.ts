import * as monaco from "monaco-editor"
import { URI } from "@codingame/monaco-vscode-api/vscode/vs/base/common/uri"
import { IWorkspaceContextService } from "@codingame/monaco-vscode-api/vscode/vs/platform/workspace/common/workspace.service"
import { initialize as initializeServices } from "@codingame/monaco-vscode-api"
import {
  ExtensionHostKind,
  registerExtension,
  type RegisterLocalProcessExtensionResult,
} from "@codingame/monaco-vscode-api/extensions"
import type * as Vscode from "vscode"
import "vscode/localExtensionHost"
import getAccessibilityServiceOverride from "@codingame/monaco-vscode-accessibility-service-override"
import getConfigurationServiceOverride, {
  reinitializeWorkspace,
} from "@codingame/monaco-vscode-configuration-service-override"
import getDialogsServiceOverride from "@codingame/monaco-vscode-dialogs-service-override"
import type { OpenEditor } from "@codingame/monaco-vscode-editor-service-override"
import getEmmetServiceOverride from "@codingame/monaco-vscode-emmet-service-override"
import getExplorerServiceOverride from "@codingame/monaco-vscode-explorer-service-override"
import getExtensionServiceOverride from "@codingame/monaco-vscode-extensions-service-override"
import getKeybindingsServiceOverride from "@codingame/monaco-vscode-keybindings-service-override"
import getLanguageDetectionWorkerServiceOverride from "@codingame/monaco-vscode-language-detection-worker-service-override"
import getLanguagesServiceOverride from "@codingame/monaco-vscode-languages-service-override"
import getLifecycleServiceOverride from "@codingame/monaco-vscode-lifecycle-service-override"
import getLogServiceOverride from "@codingame/monaco-vscode-log-service-override"
import getMarkersServiceOverride from "@codingame/monaco-vscode-markers-service-override"
import getModelServiceOverride from "@codingame/monaco-vscode-model-service-override"
import getNotificationsServiceOverride from "@codingame/monaco-vscode-notifications-service-override"
import getOutputServiceOverride from "@codingame/monaco-vscode-output-service-override"
import getPreferencesServiceOverride from "@codingame/monaco-vscode-preferences-service-override"
import getQuickAccessServiceOverride from "@codingame/monaco-vscode-quickaccess-service-override"
import getSearchServiceOverride from "@codingame/monaco-vscode-search-service-override"
import getSnippetServiceOverride from "@codingame/monaco-vscode-snippets-service-override"
import getStorageServiceOverride from "@codingame/monaco-vscode-storage-service-override"
import getTerminalServiceOverride from "@codingame/monaco-vscode-terminal-service-override"
import getTextmateServiceOverride from "@codingame/monaco-vscode-textmate-service-override"
import getThemeServiceOverride from "@codingame/monaco-vscode-theme-service-override"
import getViewsServiceOverride from "@codingame/monaco-vscode-views-service-override"
import getWorkingCopyServiceOverride from "@codingame/monaco-vscode-working-copy-service-override"
import type { IUntypedEditorInput } from "@codingame/monaco-vscode-api/vscode/vs/workbench/common/editor"
import "@codingame/monaco-vscode-api/vscode/vs/workbench/contrib/files/browser/files.contribution._editorPane"
import { slabTerminalBackend } from "./workspace-terminal-service"
import { getStandaloneMonacoEditorOverrides } from "./workspace-standalone-monaco"
import { whenWorkspaceExtensionsReady } from "./workspace-extensions"
import {
  clearSlabWorkspaceFileSystemCache,
  ensureSlabWorkspaceFileSystem,
  preloadSlabWorkspaceFileSystem,
  setSlabWorkspaceFileSystemRoot,
} from "./workspace-file-system-provider"
import {
  workspaceLspRelativePathFromUri,
  workspaceLspModelPath,
  workspaceRootUri,
  type WorkspaceLspOpenFile,
  type WorkspaceLspOpenFileOptions,
} from "./workspace-uri"

/**
 * VS Code service bootstrap for the embedded workspace editor.
 *
 * This module replaces `@codingame/monaco-editor-wrapper`: instead of the
 * wrapper's browser-only `initialize`/`registerServices`/`registerWorker`/`registerEditorOpenHandler`,
 * we talk to `@codingame/monaco-vscode-api` directly. File I/O, LSP, and search are
 * already fused to `slab-server` (see {@link "./workspace-file-system-provider.ts"}
 * and {@link "./workspace-language-client.ts"}); this file only owns the VS Code
 * service-override wiring, worker URL resolution, and the editor-open routing.
 *
 * We deliberately do NOT register `getWorkbenchServiceOverride` — there is no full
 * VS Code shell. Individual service overrides + `getViewsServiceOverride` render the
 * editor/explorer parts directly.
 */

type WorkspaceFileService = {
  root: string | null
}

let monacoVscodeApiReady: Promise<void> | null = null
let currentOpenFile: WorkspaceLspOpenFile | null = null
let currentWorkspaceFileService: WorkspaceFileService = { root: null }
let workspaceFileSystemOverlayRegistered = false
let workspaceServicesPrepared = false
let workspaceServicesInitialized = false
let workspaceVscodeRoot: string | null = null
let workspaceServices: monaco.editor.IEditorOverrideServices | null = null
let workspaceLocalExtension: RegisterLocalProcessExtensionResult | null = null
let workspaceVscodeApi: Promise<typeof Vscode> | null = null

function setWorkspaceLspDebugStage(stage: string) {
  if (typeof window === "undefined") {
    return
  }

  ;(window as typeof window & { __SLAB_WORKSPACE_LSP_STAGE__?: string })["__SLAB_WORKSPACE_LSP_STAGE__"] = stage
}

function setWorkspaceLspDebugContext(context: unknown) {
  if (typeof window === "undefined") {
    return
  }

  ;(window as typeof window & { __SLAB_WORKSPACE_LSP_CONTEXT__?: unknown })[
    "__SLAB_WORKSPACE_LSP_CONTEXT__"
  ] = context
}

function pushWorkspaceLspDebugDirectory(entry: unknown) {
  if (typeof window === "undefined") {
    return
  }

  const target = window as typeof window & { __SLAB_WORKSPACE_LSP_DIRECTORIES__?: unknown[] }
  target["__SLAB_WORKSPACE_LSP_DIRECTORIES__"] = [
    ...(target["__SLAB_WORKSPACE_LSP_DIRECTORIES__"] ?? []).slice(-20),
    entry,
  ]
}

function pushWorkspaceLspDebugStat(entry: unknown) {
  if (typeof window === "undefined") {
    return
  }

  const target = window as typeof window & { __SLAB_WORKSPACE_LSP_STATS__?: unknown[] }
  target["__SLAB_WORKSPACE_LSP_STATS__"] = [
    ...(target["__SLAB_WORKSPACE_LSP_STATS__"] ?? []).slice(-30),
    entry,
  ]
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
  if (workspaceRoot && workspaceServicesInitialized) {
    void syncWorkspaceVscodeRoot(workspaceRoot).catch((error) => {
      console.debug("workspace VS Code root sync failed", { workspaceRoot, error })
    })
  }
}

export function workspaceLspServicesReady() {
  return workspaceServicesInitialized
}

export function isWorkspaceServicesInitialized() {
  return workspaceServicesInitialized
}

export async function getWorkspaceVscodeApi(workspaceRoot?: string | null) {
  await ensureWorkspaceLspServices(workspaceRoot)
  return getWorkspaceVscodeApiAfterServicesReady()
}

/**
 * Resolves once the VS Code service container for the workspace editor is ready.
 * Idempotent: concurrent callers share the same init promise. Re-entrancy on a
 * later workspace root re-syncs the context service before resolving.
 */
export function ensureWorkspaceLspServices(workspaceRoot?: string | null) {
  if (workspaceRoot !== undefined) {
    setWorkspaceLspFileServiceRoot(workspaceRoot)
  }

  monacoVscodeApiReady ??= (async () => {
    if (!workspaceServicesInitialized) {
      setWorkspaceLspDebugStage("register-service-overrides")
      prepareWorkspaceServices()
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
      setWorkspaceLspDebugStage("initialize-monaco-services")
      await initializeWorkspaceServices(currentWorkspaceFileService.root)
      setWorkspaceLspDebugStage("load-workspace-extensions")
      // Load additional extensions not included in the default set
      void whenWorkspaceExtensionsReady()
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

function workspaceWorkbenchOptions(workspaceRoot: string | null) {
  if (!workspaceRoot) {
    return undefined
  }
  const rootUri = workspaceRootUri(workspaceRoot)

  return {
    configurationDefaults: {
      "explorer.compactFolders": false,
      "workbench.editor.enablePreview": false,
      "workbench.editor.enablePreviewFromCodeNavigation": false,
      "workbench.editor.enablePreviewFromQuickOpen": false,
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

async function initializeWorkspaceServices(workspaceRoot: string | null) {
  prepareWorkspaceServices()
  ensureWorkspaceLocalExtension()
  await initializeServices(
    workspaceServices!,
    undefined,
    workspaceWorkbenchOptions(workspaceRoot) ?? {},
    { userHome: URI.file("/") },
  )
  workspaceServicesInitialized = true

  setWorkspaceLspDebugStage("activate-local-extension")
  const { ConfigurationTarget, commands, workspace } = await getWorkspaceVscodeApiAfterServicesReady()
  await workspace.getConfiguration("explorer").update("compactFolders", false, ConfigurationTarget.Global)
  const workbenchEditorConfig = workspace.getConfiguration("workbench.editor")
  await Promise.all([
    workbenchEditorConfig.update("enablePreview", false, ConfigurationTarget.Global),
    workbenchEditorConfig.update("enablePreviewFromCodeNavigation", false, ConfigurationTarget.Global),
    workbenchEditorConfig.update("enablePreviewFromQuickOpen", false, ConfigurationTarget.Global),
  ])
  // executeCommand returns a VS Code Thenable (no .catch); wrap it so the
  // refresh is best-effort and never fails service init.
  await Promise.resolve(commands.executeCommand("workbench.files.action.refreshFilesExplorer")).catch((error) => {
    console.debug("workspace explorer refresh failed", { error })
  })
}

/**
 * Build the full service-override map once, register the Monaco worker URLs once,
 * and wire the editor-open routing. Replaces the wrapper's `registerServices` +
 * `registerWorker` + `registerEditorOpenHandler`.
 */
function prepareWorkspaceServices() {
  if (workspaceServicesPrepared) {
    return
  }

  registerWorkspaceMonacoWorkers()
  ensureWorkspaceLocalExtension()

  workspaceServices = {
    ...getStandaloneMonacoEditorOverrides(),
    ...getLogServiceOverride(),
    ...getAccessibilityServiceOverride(),
    ...getConfigurationServiceOverride(),
    ...getPreferencesServiceOverride(),
    ...getSnippetServiceOverride(),
    ...getDialogsServiceOverride(),
    ...getExplorerServiceOverride(),
    // Custom-UI setup: we render VSCode *parts*, not the workbench, and run LSP
    // over our own WebSocket. The default theme/grammar extensions run on the
    // local (main-thread) extension host (see `import "vscode/localExtensionHost"`).
    // The worker extension host spawns an iframe + worker whose handshake hangs
    // here (60s timeout) and blocks service startup — it is not needed, so leave
    // it disabled.
    ...getExtensionServiceOverride({
      enableWorkerExtensionHost: false,
    }),
    ...getKeybindingsServiceOverride(),
    ...getQuickAccessServiceOverride(),
    ...getLanguageDetectionWorkerServiceOverride(),
    ...getLanguagesServiceOverride(),
    ...getLifecycleServiceOverride(),
    ...getMarkersServiceOverride(),
    ...getModelServiceOverride(),
    ...getNotificationsServiceOverride(),
    ...getOutputServiceOverride(),
    ...getSearchServiceOverride(),
    ...getEmmetServiceOverride(),
    ...getStorageServiceOverride(),
    ...getTerminalServiceOverride(slabTerminalBackend),
    ...getTextmateServiceOverride(),
    ...getThemeServiceOverride(),
    ...getWorkingCopyServiceOverride(),
    ...getViewsServiceOverride(slabOpenCodeEditor, undefined, (state) => ({
      ...state,
      editor: {
        ...state.editor,
        restoreEditors: state.editor.restoreEditors,
      },
      views: {
        ...state.views,
        defaults: ["workbench.explorer.fileView"],
      },
    })),
  }
  workspaceServicesPrepared = true
}

function ensureWorkspaceLocalExtension() {
  workspaceLocalExtension ??= registerExtension(
    {
      name: "workspace-api",
      publisher: "slab",
      version: "1.0.0",
      engines: {
        vscode: "*",
      },
    },
    ExtensionHostKind.LocalProcess,
    { system: true },
  )
  return workspaceLocalExtension
}

function getWorkspaceVscodeApiAfterServicesReady() {
  workspaceVscodeApi ??= ensureWorkspaceLocalExtension().getApi()
  return workspaceVscodeApi
}

/**
 * Worker URL resolution for Monaco/VS Code. Replaces the wrapper's `registerWorker` +
 * `FakeWorker`: instead of spawning workers eagerly, we record their module URL and
 * `{ type: "module" }` options and hand them back through `window.MonacoEnvironment`,
 * which `@codingame/monaco-vscode-api` reads when it actually spawns each worker.
 */
type SlabMonacoWorkerEntry = { url: string | URL; options?: WorkerOptions }

const workspaceMonacoWorkers: Record<string, SlabMonacoWorkerEntry> = {
  editorWorkerService: {
    url: new URL("monaco-editor/esm/vs/editor/editor.worker.js", import.meta.url),
    options: { type: "module" },
  },
  TextMateWorker: {
    url: new URL("@codingame/monaco-vscode-textmate-service-override/worker", import.meta.url),
    options: { type: "module" },
  },
  LanguageDetectionWorker: {
    url: new URL("@codingame/monaco-vscode-language-detection-worker-service-override/worker", import.meta.url),
    options: { type: "module" },
  },
  OutputLinkDetectionWorker: {
    url: new URL("@codingame/monaco-vscode-output-service-override/worker", import.meta.url),
    options: { type: "module" },
  },
}

function registerWorkspaceMonacoWorkers() {
  if (typeof window === "undefined") {
    return
  }

  const environment: monaco.Environment & {
    getWorkerOptions?: (moduleId: string, label: string) => WorkerOptions | undefined
  } = {
    getWorkerUrl: (_moduleId, label) => workspaceMonacoWorkers[label]?.url?.toString(),
    getWorkerOptions: (_moduleId, label) => workspaceMonacoWorkers[label]?.options,
  }
  window.MonacoEnvironment = environment
}

/**
 * Editor-open routing, wired into the editor service override. Definition navigation
 * and other "open this model" requests route workspace files back through the VS Code
 * editor service so the active tab, editor history, and React workspace state stay in sync.
 * The legacy read-only peek popup is reserved for files outside the active workspace.
 */
const slabOpenCodeEditor: OpenEditor = async (modelRef, options) => {
  let modelEditor: monaco.editor.ICodeEditor | undefined
  const activeWorkspaceRoot = currentWorkspaceFileService.root
  const selection = editorSelection(options)
  const relativePath = activeWorkspaceRoot
    ? workspaceLspRelativePathFromUri(activeWorkspaceRoot, modelRef.object.textEditorModel.uri.toString())
    : null

  if (activeWorkspaceRoot && relativePath !== null) {
    if (currentOpenFile) {
      const handlerEditor = await currentOpenFile(relativePath, selection)
      if (handlerEditor) {
        modelEditor = handlerEditor
      }
    }

    if (!modelEditor) {
      const { getService, IEditorGroupsService, IEditorService } = await import("@codingame/monaco-vscode-api")
      const editorService = await getService(IEditorService)
      const editorGroupsService = await getService(IEditorGroupsService)
      const targetGroup = editorGroupsService.groups.find((group: { id: number; label?: unknown }) =>
        !String(group.label ?? "").startsWith("standalone editor"),
      )?.id
      const textSelection = selection?.startLineNumber && selection.startColumn
        ? {
          endColumn: selection.endColumn ?? selection.startColumn,
          endLineNumber: selection.endLineNumber ?? selection.startLineNumber,
          startColumn: selection.startColumn,
          startLineNumber: selection.startLineNumber,
        }
        : undefined
      const editorPane = await editorService.openEditor(
        {
          resource: URI.parse(workspaceLspModelPath(activeWorkspaceRoot, relativePath)),
          options: {
            active: true,
            pinned: true,
            revealIfOpened: true,
            selection: textSelection,
          },
        } as IUntypedEditorInput,
        targetGroup,
      )
      modelEditor = editorPane?.getControl() as monaco.editor.ICodeEditor | undefined
    }
  }

  if (!modelEditor) {
    modelEditor = openWorkspacePeekEditor(modelRef)
  }

  modelEditor.focus()
  modelEditor.getDomNode()?.scrollIntoView({
    block: "nearest",
    inline: "nearest",
  })
  return modelEditor
}

let workspacePeekEditor: ({
  model: monaco.editor.ITextModel
  editor: monaco.editor.ICodeEditor
} & monaco.IDisposable) | null = null

/**
 * Read-only overlay editor shown when an open request can't be routed to slab's
 * React surface. Ports the wrapper's `EditorOpenHandlerRegistry.openNewCodeEditor`
 * fallback so peek-on-definition keeps working in edge cases.
 */
function openWorkspacePeekEditor(modelRef: Parameters<OpenEditor>[0]): monaco.editor.ICodeEditor {
  if (workspacePeekEditor != null && modelRef.object.textEditorModel === workspacePeekEditor.model) {
    return workspacePeekEditor.editor
  }
  if (workspacePeekEditor != null) {
    workspacePeekEditor.dispose()
    workspacePeekEditor = null
  }

  const container = document.createElement("div")
  container.style.position = "fixed"
  container.style.zIndex = "10000"
  container.style.backgroundColor = "rgba(0, 0, 0, 0.5)"
  container.style.top = container.style.bottom = container.style.left = container.style.right = "0"
  container.style.cursor = "pointer"

  const editorElem = document.createElement("div")
  editorElem.style.position = "absolute"
  editorElem.style.top = editorElem.style.bottom = editorElem.style.left = editorElem.style.right = "0"
  editorElem.style.margin = "auto"
  editorElem.style.width = "80%"
  editorElem.style.height = "80%"
  container.appendChild(editorElem)
  document.body.appendChild(container)

  const editor = monaco.editor.create(editorElem, {
    model: modelRef.object.textEditorModel,
    readOnly: true,
    automaticLayout: true,
  })
  workspacePeekEditor = {
    dispose: () => {
      modelRef.dispose()
      editor.dispose()
      document.body.removeChild(container)
      workspacePeekEditor = null
    },
    model: modelRef.object.textEditorModel,
    editor,
  }

  container.addEventListener("mousedown", (event) => {
    if (event.target !== container) {
      return
    }
    workspacePeekEditor?.dispose()
  })

  return editor
}

export async function syncWorkspaceVscodeRoot(workspaceRoot: string) {
  if (workspaceVscodeRoot === workspaceRoot) {
    return
  }

  if (!workspaceServicesInitialized) {
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

function editorSelection(options: unknown): WorkspaceLspOpenFileOptions | undefined {
  if (!options || typeof options !== "object" || !("selection" in options)) {
    return undefined
  }

  return options.selection as WorkspaceLspOpenFileOptions | undefined
}

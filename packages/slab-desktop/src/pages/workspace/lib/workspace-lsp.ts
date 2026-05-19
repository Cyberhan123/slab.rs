import type * as Monaco from "monaco-editor"
import type { IFileChange } from "@codingame/monaco-vscode-files-service-override"
import { URI } from "@codingame/monaco-vscode-api/vscode/vs/base/common/uri"
import {
  initialize as initializeMonacoWrapper,
  isInitialized as workspaceMonacoIsInitialized,
  registerEditorOpenHandler,
  registerServices,
} from "@codingame/monaco-editor-wrapper"
import "@codingame/monaco-editor-wrapper/features/extensionHostWorker"
import "@codingame/monaco-editor-wrapper/features/search"
import getAccessibilityServiceOverride from "@codingame/monaco-vscode-accessibility-service-override"
import getConfigurationServiceOverride from "@codingame/monaco-vscode-configuration-service-override"
import { whenReady as cppExtensionReady } from "@codingame/monaco-vscode-cpp-default-extension"
import getDialogsServiceOverride from "@codingame/monaco-vscode-dialogs-service-override"
import getExplorerServiceOverride from "@codingame/monaco-vscode-explorer-service-override"
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
  workspaceCreateDirectory,
  workspaceDeletePath,
  workspaceRenamePath,
  workspaceReadDirectory,
  workspaceReadFile,
  workspaceStatPath,
  workspaceWriteFile,
} from "@/lib/workspace-bridge"
import type {
  WorkspaceDirectoryResponse,
  WorkspaceFileContent,
  WorkspacePathMetadata,
} from "@/lib/workspace-bridge"
import {
  workspaceLspDefinitionTargetFromResult,
  workspaceLspImportSpecifierPositionForTarget,
  supportsWorkspaceLsp,
  workspaceLspModelPath,
  workspaceLspRelativePathFromUri,
  type WorkspaceLspDefinitionTarget,
  type WorkspaceLspOpenFileOptions,
} from "./workspace-lsp-utils"
import { slabTerminalBackend } from "./workspace-terminal-service"

export {
  supportsWorkspaceLsp,
  workspaceLspDefinitionTargetFromResult,
  workspaceLspImportSpecifierPositionForTarget,
  workspaceLspModelPath,
  workspaceLspRelativePathFromUri,
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
let workspaceVscodeServiceOverridesRegistered = false
let workspaceVscodeRoot: string | null = null

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
  currentWorkspaceFileService = { root: workspaceRoot }
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
      registerWorkspaceVscodeServiceOverrides()
      if (!workspaceFileSystemOverlayRegistered) {
        await registerWorkspaceFileSystemOverlay()
        workspaceFileSystemOverlayRegistered = true
      }
      await initializeMonacoWrapper(workspaceWorkbenchOptions(currentWorkspaceFileService.root), {
        registerAdditionalExtensions: true,
        waitForDefaultExtensions: true,
      })
      // Load additional extensions not included in the default set
      await Promise.allSettled([
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
      await syncWorkspaceVscodeRoot(currentWorkspaceFileService.root)
    }

    if (!workspaceEditorOpenHandlerRegistered) {
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
  })().catch((error) => {
    monacoVscodeApiReady = null
    throw error
  })

  return monacoVscodeApiReady.then(async () => {
    if (workspaceRoot) {
      await syncWorkspaceVscodeRoot(workspaceRoot)
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

function workspaceWorkbenchOptions(workspaceRoot: string | null) {
  if (!workspaceRoot) {
    return undefined
  }

  return {
    workspaceProvider: {
      open: async () => false,
      trusted: true,
      workspace: {
        folderUri: URI.file(workspaceRoot),
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

  const { getService, IWorkspaceContextService, IWorkspaceEditingService } = await import("@codingame/monaco-vscode-api")
  const contextService = await getService(IWorkspaceContextService)
  const folders = contextService.getWorkspace().folders
  const rootUri = URI.file(workspaceRoot)
  if (folders.length === 1 && folders[0]?.uri.toString() === rootUri.toString()) {
    workspaceVscodeRoot = workspaceRoot
    return
  }

  const workspaceEditingService = await getService(IWorkspaceEditingService)
  await workspaceEditingService.updateFolders(0, folders.length, [{
    name: workspaceRoot.split(/[\\/]/).findLast(Boolean) ?? "Workspace",
    uri: rootUri,
  }], true)
  workspaceVscodeRoot = workspaceRoot
}

function relativePathFromEditorInput(workspaceRoot: string, input: unknown) {
  const resource = resourceFromEditorInput(input)
  if (!resource) {
    return null
  }

  return workspaceLspRelativePathFromUri(workspaceRoot, resource.toString())
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

function resourceFromEditorInput(input: unknown) {
  if (!input || typeof input !== "object") {
    return null
  }

  const record = input as Record<string, unknown>
  const resource = record.resource
  if (resource && typeof resource === "object" && "toString" in resource) {
    return resource as { toString(): string }
  }

  const toUntyped = record.toUntyped
  if (typeof toUntyped !== "function") {
    return null
  }

  const untyped = toUntyped.call(input)
  if (!untyped || typeof untyped !== "object") {
    return null
  }

  const untypedResource = (untyped as Record<string, unknown>).resource
  if (untypedResource && typeof untypedResource === "object" && "toString" in untypedResource) {
    return untypedResource as { toString(): string }
  }

  return null
}

function registerWorkspaceVscodeServiceOverrides() {
  if (workspaceVscodeServiceOverridesRegistered) {
    return
  }

  registerServices({
    ...getAccessibilityServiceOverride(),
    ...getConfigurationServiceOverride(),
    ...getDialogsServiceOverride(),
    ...getExplorerServiceOverride(),
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
    const registerModel = async (modelToRegister: Monaco.editor.ITextModel) => {
      const uri = modelToRegister.uri.toString()
      if (registeredModelUris.has(uri)) {
        return
      }

      await vscodeWorkspace.openTextDocument(VscodeUri.parse(uri))
      registeredModelUris.add(uri)
    }
    await registerModel(model)

    socket = new WebSocket(workspaceLspUrl(language))
    const jsonrpc = await import("vscode-ws-jsonrpc")
    const rpcSocket = jsonrpc.toSocket(socket)
    languageClient = new WorkspaceLanguageClient({
      id: `workspace-${language}`,
      name: `Workspace ${language} Language Server`,
      clientOptions: {
        documentSelector: [{ scheme: "file", language }],
        workspaceFolder: {
          index: 0,
          name: "workspace",
          uri: monaco.Uri.file(workspaceRoot),
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

    await waitForSocketOpen(socket)
    await languageClient.start()
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
        await languageClient?.stop()
        socket?.close(1000, "workspace LSP session disposed")
      },
    }
  } catch (error) {
    console.debug("workspace LSP unavailable", { language, uri: model.uri.toString(), error })
    socket?.close(1000, "workspace LSP session unavailable")
    await languageClient?.stop().catch(() => {})
    return null
  }
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

async function registerWorkspaceFileSystemOverlay() {
  const {
    FileChangeType,
    FileSystemProviderCapabilities,
    FileSystemProviderError,
    FileSystemProviderErrorCode,
    FileType,
    registerFileSystemOverlay,
  } = await import("@codingame/monaco-vscode-files-service-override")
  const { Emitter } = await import("@codingame/monaco-vscode-api/vscode/vs/base/common/event")
  const textEncoder = new TextEncoder()
  const textDecoder = new TextDecoder()
  const noopDisposable = { dispose() {} }
  const changesEmitter = new Emitter<readonly IFileChange[]>()
  const pendingDirectoryReads = new Map<string, Promise<WorkspaceDirectoryResponse>>()
  const pendingFileReads = new Map<string, Promise<WorkspaceFileContent>>()
  const pendingPathStats = new Map<string, Promise<WorkspacePathMetadata>>()
  const clearWorkspaceFileSystemCache = () => {
    pendingDirectoryReads.clear()
    pendingFileReads.clear()
    pendingPathStats.clear()
  }
  const relativePathForResource = (resource: string) => {
    const workspaceRoot = currentWorkspaceFileService.root
    const relativePath = workspaceRoot
      ? workspaceLspRelativePathFromUri(workspaceRoot, resource)
      : null
    if (relativePath === null) {
      throw FileSystemProviderError.create(
        "workspace LSP file is outside the active workspace",
        FileSystemProviderErrorCode.NoPermissions,
      )
    }

    return relativePath
  }

  registerFileSystemOverlay(100, {
    capabilities: FileSystemProviderCapabilities.FileReadWrite,
    onDidChangeCapabilities: () => noopDisposable,
    onDidChangeFile: changesEmitter.event,
    async delete(resource, options) {
      await workspaceDeletePath({
        recursive: Boolean(options.recursive),
        relativePath: relativePathForResource(resource.toString()),
      })
      clearWorkspaceFileSystemCache()
      changesEmitter.fire([{ resource, type: FileChangeType.DELETED }])
    },
    async mkdir(resource) {
      await workspaceCreateDirectory({
        relativePath: relativePathForResource(resource.toString()),
      })
      clearWorkspaceFileSystemCache()
      changesEmitter.fire([{ resource, type: FileChangeType.ADDED }])
    },
    async readdir(resource) {
      const relativePath = relativePathForResource(resource.toString())
      const pendingDirectory = pendingDirectoryReads.get(relativePath)
      const directoryPromise =
        pendingDirectory ??
        (async () => {
          const nextDirectory = await workspaceReadDirectory(relativePath)
          return nextDirectory
        })().finally(() => {
          pendingDirectoryReads.delete(relativePath)
        })

      if (!pendingDirectory) {
        pendingDirectoryReads.set(relativePath, directoryPromise)
      }
      const directory = await directoryPromise

      return directory.entries.map((entry) => [
        entry.name,
        entry.kind === "directory" ? FileType.Directory : FileType.File,
      ])
    },
    async readFile(resource) {
      const relativePath = relativePathForResource(resource.toString())
      const pendingFile = pendingFileReads.get(relativePath)
      const filePromise =
        pendingFile ??
        (async () => {
          const nextFile = await workspaceReadFile(relativePath)
          return nextFile
        })().finally(() => {
          pendingFileReads.delete(relativePath)
        })

      if (!pendingFile) {
        pendingFileReads.set(relativePath, filePromise)
      }
      const file = await filePromise

      return textEncoder.encode(file.content)
    },
    async rename(from, to) {
      await workspaceRenamePath({
        fromRelativePath: relativePathForResource(from.toString()),
        toRelativePath: relativePathForResource(to.toString()),
      })
      clearWorkspaceFileSystemCache()
      changesEmitter.fire([
        { resource: from, type: FileChangeType.DELETED },
        { resource: to, type: FileChangeType.ADDED },
      ])
    },
    async stat(resource) {
      const relativePath = relativePathForResource(resource.toString())
      if (!relativePath) {
        return {
          ctime: Date.now(),
          mtime: Date.now(),
          size: 0,
          type: FileType.Directory,
        }
      }

      try {
        const pendingStat = pendingPathStats.get(relativePath)
        const metadataPromise =
          pendingStat ??
          (async () => {
            const nextMetadata = await workspaceStatPath(relativePath)
            return nextMetadata
          })().finally(() => {
            pendingPathStats.delete(relativePath)
          })

        if (!pendingStat) {
          pendingPathStats.set(relativePath, metadataPromise)
        }
        const metadata = await metadataPromise

        return {
          ctime: metadata.createdAt || Date.now(),
          mtime: metadata.modifiedAt || Date.now(),
          size: metadata.sizeBytes,
          type: metadata.kind === "directory" ? FileType.Directory : FileType.File,
        }
      } catch {
        throw FileSystemProviderError.create(
          "workspace LSP file was not found",
          FileSystemProviderErrorCode.FileNotFound,
        )
      }
    },
    watch() {
      return noopDisposable
    },
    async writeFile(resource, content) {
      await workspaceWriteFile({
        content: textDecoder.decode(content),
        relativePath: relativePathForResource(resource.toString()),
      })
      clearWorkspaceFileSystemCache()
      changesEmitter.fire([{ resource, type: FileChangeType.UPDATED }])
    },
  })
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

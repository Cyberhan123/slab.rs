import type * as Monaco from "monaco-editor"
import {
  initialize as initializeMonacoWrapper,
  isInitialized as workspaceMonacoIsInitialized,
  registerEditorOpenHandler,
} from "@codingame/monaco-editor-wrapper"
import "@codingame/monaco-editor-wrapper/features/extensionHostWorker"
import type { MonacoLanguageClient } from "monaco-languageclient"
import { SERVER_BASE_URL } from "@slab/api/config"
import {
  workspaceReadDirectory,
  workspaceReadFile,
  workspaceWriteFile,
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
  dispose: () => Promise<void>
}

export type WorkspaceLspOpenFile = (
  relativePath: string,
  options?: WorkspaceLspOpenFileOptions,
) => Promise<Monaco.editor.IStandaloneCodeEditor | undefined>

type WorkspaceFileService = {
  root: string | null
}

let monacoVscodeApiReady: Promise<void> | null = null
let currentOpenFile: WorkspaceLspOpenFile | null = null
let currentWorkspaceFileService: WorkspaceFileService = { root: null }
let workspaceFileSystemOverlayRegistered = false
let workspaceEditorOpenHandlerRegistered = false

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
}

export function workspaceLspServicesReady() {
  return workspaceMonacoIsInitialized()
}

export function ensureWorkspaceLspServices() {
  monacoVscodeApiReady ??= (async () => {
    if (!workspaceMonacoIsInitialized()) {
      await initializeMonacoWrapper(undefined, {
        registerAdditionalExtensions: false,
        waitForDefaultExtensions: false,
      })
    }

    if (!workspaceFileSystemOverlayRegistered) {
      await registerWorkspaceFileSystemOverlay()
      workspaceFileSystemOverlayRegistered = true
    }

    if (!workspaceEditorOpenHandlerRegistered) {
      registerEditorOpenHandler(async (modelRef, options) => {
        const workspaceRoot = currentWorkspaceFileService.root
        const selection = editorSelection(options)
        const relativePath = workspaceRoot
          ? workspaceLspRelativePathFromUri(workspaceRoot, modelRef.object.textEditorModel.uri.toString())
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

  return monacoVscodeApiReady
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
  let languageClient: MonacoLanguageClient | null = null

  try {
    await ensureWorkspaceLspServices()

    // Register the model as a VSCode text document before starting the language client.
    // The Monaco model is created when the editor mounts, which happens before the
    // @codingame/monaco-vscode-api bridge finishes initializing. Because of this timing,
    // the bridge never intercepts the model-creation event and the model is absent from
    // vscode.workspace.textDocuments. Without this registration, MonacoLanguageClient
    // never sends textDocument/didOpen and the server returns nothing for hover/definition.
    const { workspace: vscodeWorkspace, Uri: VscodeUri } = await import("vscode")
    await vscodeWorkspace.openTextDocument(VscodeUri.parse(model.uri.toString()))

    socket = new WebSocket(workspaceLspUrl(language))
    const [languageClientModule, { CloseAction, ErrorAction }, jsonrpc] = await Promise.all([
      import("monaco-languageclient"),
      import("vscode-languageclient/browser.js"),
      import("vscode-ws-jsonrpc"),
    ])
    const rpcSocket = jsonrpc.toSocket(socket)
    languageClient = new languageClientModule.MonacoLanguageClient({
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
      dispose: async () => {
        await languageClient?.stop()
        socket?.close()
      },
    }
  } catch (error) {
    console.debug("workspace LSP unavailable", { language, uri: model.uri.toString(), error })
    socket?.close()
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
    FileSystemProviderCapabilities,
    FileSystemProviderError,
    FileSystemProviderErrorCode,
    FileType,
    registerFileSystemOverlay,
  } = await import("@codingame/monaco-vscode-files-service-override")
  const textEncoder = new TextEncoder()
  const textDecoder = new TextDecoder()
  const noopDisposable = { dispose() {} }
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
    onDidChangeFile: () => noopDisposable,
    async delete() {
      throw FileSystemProviderError.create("Not allowed", FileSystemProviderErrorCode.NoPermissions)
    },
    async mkdir() {
      throw FileSystemProviderError.create("Not allowed", FileSystemProviderErrorCode.NoPermissions)
    },
    async readdir(resource) {
      const relativePath = relativePathForResource(resource.toString())
      const directory = await workspaceReadDirectory(relativePath, { includeIgnored: true })

      return directory.entries.map((entry) => [
        entry.name,
        entry.kind === "directory" ? FileType.Directory : FileType.File,
      ])
    },
    async readFile(resource) {
      const file = await workspaceReadFile(relativePathForResource(resource.toString()))
      return textEncoder.encode(file.content)
    },
    async rename() {
      throw FileSystemProviderError.create("Not allowed", FileSystemProviderErrorCode.NoPermissions)
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
        const file = await workspaceReadFile(relativePath)
        return {
          ctime: Date.now(),
          mtime: Date.now(),
          size: file.sizeBytes,
          type: FileType.File,
        }
      } catch {
        try {
          await workspaceReadDirectory(relativePath, { includeIgnored: true })
          return {
            ctime: Date.now(),
            mtime: Date.now(),
            size: 0,
            type: FileType.Directory,
          }
        } catch {
          throw FileSystemProviderError.create(
            "workspace LSP file was not found",
            FileSystemProviderErrorCode.FileNotFound,
          )
        }
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

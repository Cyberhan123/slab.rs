import type * as Monaco from "monaco-editor"
import type { MonacoLanguageClient } from "monaco-languageclient"
import { SERVER_BASE_URL } from "@slab/api/config"
import {
  workspaceReadDirectory,
  workspaceReadFile,
  workspaceWriteFile,
} from "@/lib/workspace-bridge"

const SUPPORTED_WORKSPACE_LSP_LANGUAGES = new Set([
  "typescript",
  "javascript",
  "json",
  "css",
  "less",
  "scss",
  "html",
  "python",
  "c",
  "cpp",
  "go",
  "rust",
])

export type WorkspaceLspSession = {
  dispose: () => Promise<void>
}

export type WorkspaceLspOpenFileOptions = {
  endColumn?: number
  endLineNumber?: number
  startColumn?: number
  startLineNumber?: number
}

export type WorkspaceLspOpenFile = (
  relativePath: string,
  options?: WorkspaceLspOpenFileOptions,
) => Promise<Monaco.editor.IStandaloneCodeEditor | undefined>

type WorkspaceFileService = {
  root: string | null
}

let monacoVscodeApiReady: Promise<void> | null = null
let monacoVscodeApiStarted = false
let currentOpenFile: WorkspaceLspOpenFile | null = null
let currentWorkspaceFileService: WorkspaceFileService = { root: null }

export function supportsWorkspaceLsp(language: string) {
  return SUPPORTED_WORKSPACE_LSP_LANGUAGES.has(language)
}

export function workspaceLspModelUri(
  monaco: typeof Monaco,
  workspaceRoot: string,
  relativePath: string,
) {
  return monaco.Uri.parse(workspaceLspModelPath(workspaceRoot, relativePath))
}

export function workspaceLspModelPath(workspaceRoot: string, relativePath: string) {
  const path = relativePath.replace(/\\/g, "/").replace(/^\/+/, "")
  const root = workspaceRoot.replace(/\\/g, "/").replace(/\/+$/, "")
  const absolutePath = `${root}/${path}`
  const prefixedPath = absolutePath.startsWith("/") ? absolutePath : `/${absolutePath}`

  return `file://${encodeURI(prefixedPath)}`
}

export function workspaceLspRelativePathFromUri(
  workspaceRoot: string,
  uriString: string,
) {
  let pathname = uriString
  try {
    const url = new URL(uriString)
    if (url.protocol !== "file:") {
      return null
    }
    pathname = url.hostname ? `/${url.hostname}${url.pathname}` : url.pathname
  } catch {
    // Monaco can also hand back path-like strings in tests and internal flows.
  }

  const rootPath = normalizeWorkspacePath(workspaceRoot)
  const absolutePath = normalizeWorkspacePath(decodeURI(pathname))
  const normalizedRoot = rootPath.endsWith("/") ? rootPath : `${rootPath}/`

  if (absolutePath === rootPath) {
    return ""
  }

  if (!absolutePath.startsWith(normalizedRoot)) {
    return null
  }

  return absolutePath.slice(normalizedRoot.length)
}

export function setWorkspaceLspOpenFile(openFile: WorkspaceLspOpenFile | null) {
  currentOpenFile = openFile
}

export function setWorkspaceLspFileServiceRoot(workspaceRoot: string | null) {
  currentWorkspaceFileService = { root: workspaceRoot }
}

export function workspaceLspServicesReady() {
  return monacoVscodeApiStarted
}

export function ensureWorkspaceLspServices() {
  monacoVscodeApiReady ??= (async () => {
    const [{ MonacoVscodeApiWrapper }, { configureDefaultWorkerFactory }] = await Promise.all([
      import("monaco-languageclient/vscodeApiWrapper"),
      import("monaco-languageclient/workerFactory"),
    ])
    const apiWrapper = new MonacoVscodeApiWrapper({
      $type: "extended",
      viewsConfig: {
        $type: "EditorService",
        openEditorFunc: async (modelRef, options) => {
          const workspaceRoot = currentWorkspaceFileService.root
          const selection = editorSelection(options)
          const relativePath = workspaceRoot
            ? workspaceLspRelativePathFromUri(workspaceRoot, modelRef.object.textEditorModel.uri.toString())
            : null
          if (relativePath === null || !currentOpenFile) {
            return undefined
          }

          return currentOpenFile(relativePath, {
            endColumn: selection?.endColumn,
            endLineNumber: selection?.endLineNumber,
            startColumn: selection?.startColumn,
            startLineNumber: selection?.startLineNumber,
          })
        },
      },
      monacoWorkerFactory: configureDefaultWorkerFactory,
    })

    await registerWorkspaceFileSystemOverlay()
    await apiWrapper.start({ caller: "slab-workspace-lsp" })
    monacoVscodeApiStarted = true
  })().catch((error) => {
    monacoVscodeApiReady = null
    monacoVscodeApiStarted = false
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
      const directory = await workspaceReadDirectory(relativePath)

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
          await workspaceReadDirectory(relativePath)
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

function normalizeWorkspacePath(path: string) {
  let normalized = path.replace(/\\/g, "/")
  if (/^\/[A-Za-z]:/.test(normalized)) {
    normalized = normalized.slice(1)
  }
  return normalized.replace(/\/+$/, "")
}

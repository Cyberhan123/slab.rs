import type * as Monaco from "monaco-editor"
import type { MonacoLanguageClient } from "monaco-languageclient"
import { SERVER_BASE_URL } from "@slab/api/config"

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

let monacoVscodeApiReady: Promise<void> | null = null

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
    await ensureMonacoVscodeApi()

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

function ensureMonacoVscodeApi() {
  monacoVscodeApiReady ??= (async () => {
    const [{ MonacoVscodeApiWrapper }, { configureDefaultWorkerFactory }] = await Promise.all([
      import("monaco-languageclient/vscodeApiWrapper"),
      import("monaco-languageclient/workerFactory"),
    ])
    const apiWrapper = new MonacoVscodeApiWrapper({
      $type: "extended",
      viewsConfig: {
        $type: "EditorService",
      },
      monacoWorkerFactory: configureDefaultWorkerFactory,
    })

    await apiWrapper.start({ caller: "slab-workspace-lsp" })
  })().catch((error) => {
    monacoVscodeApiReady = null
    throw error
  })

  return monacoVscodeApiReady
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

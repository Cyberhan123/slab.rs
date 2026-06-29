import type * as Monaco from "monaco-editor"
import {
  BaseLanguageClient,
  CloseAction,
  ErrorAction,
  type LanguageClientOptions,
  type MessageTransports,
} from "vscode-languageclient/browser.js"
import { SERVER_BASE_URL } from "@slab/api/config"
import {
  ensureWorkspaceLspServices,
  getWorkspaceVscodeApi,
} from "./workspace-services"
import {
  supportsWorkspaceLsp,
  workspaceLspDefinitionTargetFromResult,
  workspaceLspFileUri,
  workspaceLspImportSpecifierPositionForTarget,
  workspaceLspRelativePathFromUri,
  type WorkspaceLspDefinitionTarget,
} from "./workspace-uri"

/**
 * WebSocket language client for the workspace editor.
 *
 * LSP traffic is fused to `slab-server`: each session dials `ws://<server>/v1/workspace/lsp/<language>`,
 * which the backend proxies to the real language server (web or stdio provider — see
 * `bin/slab-server` + `crates/slab-app-core`). The client owns the JSON-RPC transport,
 * reconnect with exponential backoff, model registration, and definition-target resolution.
 */

const WORKSPACE_LSP_RECONNECT_INITIAL_DELAY_MS = 250
const WORKSPACE_LSP_RECONNECT_MAX_DELAY_MS = 5_000

export type WorkspaceLspSession = {
  definitionTarget: (
    model: Monaco.editor.ITextModel,
    position: Monaco.IPosition,
  ) => Promise<WorkspaceLspDefinitionTarget | null>
  registerModel: (model: Monaco.editor.ITextModel) => Promise<void>
  dispose: () => Promise<void>
}

type WorkspaceLanguageClientOptions = {
  clientOptions: LanguageClientOptions
  id: string
  messageTransports: MessageTransports
  name: string
}

type WorkspaceLspDebugState = {
  connections?: unknown[]
  definitionRequests?: unknown[]
  didOpen?: unknown[]
  initializeResult?: unknown
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

export async function startWorkspaceLspSession({
  language,
  model,
  workspaceRoot,
}: {
  language: string
  model: Monaco.editor.ITextModel
  workspaceRoot: string
}): Promise<WorkspaceLspSession | null> {
  if (!supportsWorkspaceLsp(language)) {
    return null
  }

  let socket: WebSocket | null = null
  let languageClient: WorkspaceLanguageClient | null = null
  let disposed = false
  let connectPromise: Promise<void> | null = null
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
    const { workspace: vscodeWorkspace, Uri: VscodeUri } = await getWorkspaceVscodeApi()
    const registeredModelUris = new Set<string>()
    const registeredModels = new Map<string, Monaco.editor.ITextModel>()
    const registerModel = async (modelToRegister: Monaco.editor.ITextModel) => {
      const uri = modelToRegister.uri.toString()
      registeredModels.set(uri, modelToRegister)
      if (registeredModelUris.has(uri)) {
        return
      }

      await vscodeWorkspace.openTextDocument(VscodeUri.parse(uri))
      pushWorkspaceLspClientDebug("didOpen", {
        languageId: modelToRegister.getLanguageId(),
        textLength: modelToRegister.getValueLength(),
        uri,
        version: modelToRegister.getVersionId(),
      })
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
      pushWorkspaceLspClientDebug("connections", { event: "connecting", language })
      const nextSocket = new WebSocket(workspaceLspUrl(language))
      const rpcSocket = jsonrpc.toSocket(nextSocket)
      const nextLanguageClient = new WorkspaceLanguageClient({
        id: `workspace-${language}`,
        name: `Workspace ${language} Language Server`,
        clientOptions: {
          documentSelector: [{ scheme: "file", language }],
          uriConverters: {
            code2Protocol: (uri) => workspaceLspProtocolUri(uri.toString()),
            protocol2Code: (uri) => VscodeUri.parse(uri),
          },
          workspaceFolder: {
            index: 0,
            name: "workspace",
            uri: VscodeUri.parse(workspaceLspFileUri(workspaceRoot)),
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

      nextSocket.addEventListener("close", (event) => {
        pushWorkspaceLspClientDebug("connections", {
          code: event.code,
          event: "closed",
          language,
          reason: event.reason,
          wasClean: event.wasClean,
        })
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
        if (disposed || socket !== nextSocket || languageClient !== nextLanguageClient) {
          await nextLanguageClient.stop().catch(() => {})
          return
        }
        setWorkspaceLspClientInitializeDebug(nextLanguageClient.initializeResult)
        registeredModelUris.clear()
        await Promise.all([...registeredModels.values()].map(registerModel))
        reconnectDelayMs = WORKSPACE_LSP_RECONNECT_INITIAL_DELAY_MS
        pushWorkspaceLspClientDebug("connections", { event: "ready", language })
      } catch (error) {
        pushWorkspaceLspClientDebug("connections", {
          error: error instanceof Error ? error.message : String(error),
          event: "failed",
          language,
        })
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

    const connectOnce = () => {
      if (connectPromise) {
        return connectPromise
      }

      const nextConnectPromise = connect().finally(() => {
        if (connectPromise === nextConnectPromise) {
          connectPromise = null
        }
      })
      connectPromise = nextConnectPromise
      return nextConnectPromise
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
        void connectOnce().catch((error) => {
          console.debug("workspace LSP reconnect failed", {
            language,
            uri: model.uri.toString(),
            error,
          })
          scheduleReconnect()
        })
      }, delayMs)
    }

    const languageClientForRequest = async () => {
      if (languageClient) {
        return languageClient
      }

      clearReconnectTimer()
      await connectOnce().catch((error) => {
        console.debug("workspace LSP request reconnect failed", {
          language,
          uri: model.uri.toString(),
          error,
        })
      })
      return languageClient
    }

    const requestDefinition = async (
      definitionModel: Monaco.editor.ITextModel,
      position: Monaco.IPosition,
    ) => {
      const protocolUri = workspaceLspProtocolUri(definitionModel.uri.toString())
      const client = await languageClientForRequest()
      let definitions: unknown = null
      let errorMessage: string | null = null
      try {
        definitions = client
          ? await client.sendRequest<unknown>(
            "textDocument/definition",
            textDocumentPositionParams(definitionModel, position),
          )
          : null
      } catch (error) {
        errorMessage = error instanceof Error ? error.message : String(error)
      }
      pushWorkspaceLspClientDebug("definitionRequests", {
        clientReady: Boolean(client),
        definitions: definitions ?? null,
        error: errorMessage,
        position,
        protocolUri,
        rawUri: definitionModel.uri.toString(),
      })
      return definitions
    }

    await connectOnce()
    return {
      definitionTarget: async (definitionModel, position) => {
        const currentRelativePath = workspaceLspRelativePathFromUri(workspaceRoot, definitionModel.uri.toString())
        const definitions = await requestDefinition(definitionModel, position)
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

        const moduleDefinitions = await requestDefinition(definitionModel, importSpecifierPosition)
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

function pushWorkspaceLspClientDebug(key: keyof WorkspaceLspDebugState, value: unknown) {
  if (typeof window === "undefined") {
    return
  }

  const target = window as typeof window & { __SLAB_WORKSPACE_LSP_CLIENT__?: WorkspaceLspDebugState }
  const state = target["__SLAB_WORKSPACE_LSP_CLIENT__"] ?? {}
  const current = state[key]
  target["__SLAB_WORKSPACE_LSP_CLIENT__"] = {
    ...state,
    [key]: [...(Array.isArray(current) ? current : []), value].slice(-20),
  }
}

function setWorkspaceLspClientInitializeDebug(value: unknown) {
  if (typeof window === "undefined") {
    return
  }

  const target = window as typeof window & { __SLAB_WORKSPACE_LSP_CLIENT__?: WorkspaceLspDebugState }
  target["__SLAB_WORKSPACE_LSP_CLIENT__"] = {
    ...target["__SLAB_WORKSPACE_LSP_CLIENT__"],
    initializeResult: value,
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
      uri: workspaceLspProtocolUri(model.uri.toString()),
    },
  }
}

function workspaceLspUrl(language: string) {
  const endpoint = new URL(SERVER_BASE_URL)
  endpoint.protocol = endpoint.protocol === "https:" ? "wss:" : "ws:"
  endpoint.pathname = `/v1/workspace/lsp/${encodeURIComponent(language)}`
  endpoint.search = ""
  endpoint.hash = ""
  return endpoint.toString()
}

function workspaceLspProtocolUri(uri: string) {
  return uri.replace(/^file:\/\/\/([A-Za-z])%3A\//i, "file:///$1:/")
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

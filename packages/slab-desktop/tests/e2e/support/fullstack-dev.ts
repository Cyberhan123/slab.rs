import {
  execFileSync,
  spawn,
  type ChildProcessWithoutNullStreams,
} from "node:child_process"
import { createServer } from "node:net"
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import { delimiter, dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { setTimeout as delay } from "node:timers/promises"

import type { components } from "@slab/api/v1"

type Schema = components["schemas"]
export type AgentResponsesServerMessage = Schema["AgentResponsesServerMessage"]
export type AgentSessionRestored = Extract<
  AgentResponsesServerMessage,
  { type: "agent.session.restored" }
>
export type AgentThreadMessageResponse = Schema["AgentThreadMessageResponse"]
export type ChatToolCall = Schema["ChatToolCall"]
export type SessionResponse = Schema["SessionResponse"]
type TaskResponse = Schema["TaskResponse"]
type UiStateValueResponse = Schema["UiStateValueResponse"]
type UnifiedModelResponse = Schema["UnifiedModelResponse"]

type JsonRequestInit = Omit<RequestInit, "body"> & {
  json?: unknown
}

export type ManagedDevProcess = {
  logs: string[]
  stop: () => Promise<void>
}

type ManagedChild = {
  child: ChildProcessWithoutNullStreams
  label: string
}

export type FullstackDevEnvironment = {
  databasePath: string
  databaseUrl: string
  e2eRootDir: string
  modelConfigDir: string
  pluginsDir: string
  repoRoot: string
  rootDir: string
  serverBaseUrl: string
  serverBind: string
  serverPort: number
  sessionStateDir: string
  settingsOverlayPath: string
  settingsPath: string
  uiBaseUrl: string
  uiPort: number
  workspaceRoot: string
}

export type LocalModelBootstrapOptions = {
  modelId: "Qwen2.5-0.5B-Instruct" | "Qwen3.5-9B"
  selectedPresetId?: string
  selectedVariantId: "Q4_K_M" | "Q8_0"
  load?: boolean
  selectForAssistant?: boolean
}

const setupSchemaUrl = "https://slab.reorgix.com/manifests/v1/settings-document.schema.json"
const supportDir = dirname(fileURLToPath(import.meta.url))
const packageRoot = resolve(supportDir, "../../..")
const repoRoot = resolve(packageRoot, "../..")
const persistentE2eRootDir = join(repoRoot, ".slab", "e2e")
const modelPackDir = join(repoRoot, "models", "dist")
const processStartSkewMs = 1_000
const serverReadinessTimeoutMs = 600_000
const uiReadinessTimeoutMs = 180_000
const modelDownloadTaskTimeoutMs = 1_800_000
const defaultTaskTimeoutMs = 900_000
const taskPollIntervalMs = 1_000
const defaultEventuallyTimeoutMs = 30_000
const defaultEventuallyIntervalMs = 250
const managedProcessStopTimeoutMs = 12_000
const devLogRingLimit = 500

export async function createFullstackDevEnvironment(): Promise<FullstackDevEnvironment> {
  const serverPort = await reserveTcpPort()
  const uiPort = await reserveTcpPort()
  const serverBind = `127.0.0.1:${serverPort}`
  const serverBaseUrl = `http://${serverBind}`
  const uiBaseUrl = `http://127.0.0.1:${uiPort}`

  mkdirSync(persistentE2eRootDir, { recursive: true })
  const rootDir = mkdtempSync(join(persistentE2eRootDir, "run-"))
  const settingsDir = join(rootDir, "config")
  const modelConfigDir = join(settingsDir, "models")
  const pluginsDir = join(rootDir, "plugins")
  const sessionStateDir = join(rootDir, "sessions")
  const workspaceRoot = join(rootDir, "workspace")
  const workspaceSlabDir = join(workspaceRoot, ".slab")
  const settingsPath = join(settingsDir, "settings.json")
  const settingsOverlayPath = join(workspaceSlabDir, "settings.json")
  const databasePath = join(rootDir, "slab.db")

  for (const dir of [
    settingsDir,
    modelConfigDir,
    pluginsDir,
    sessionStateDir,
    workspaceSlabDir,
    workspaceRoot,
  ]) {
    mkdirSync(dir, { recursive: true })
  }

  writeFileSync(join(workspaceRoot, "README.md"), "# Slab E2E Workspace\n", "utf8")
  writeSettingsDocument(settingsPath, {
    databaseUrl: sqliteUrlForPath(databasePath),
    modelConfigDir,
    pluginsDir,
    serverBind,
    sessionStateDir,
  })
  writeSettingsDocument(settingsOverlayPath, {
    databaseUrl: sqliteUrlForPath(databasePath),
    modelConfigDir,
    pluginsDir,
    serverBind,
    sessionStateDir,
  })

  return {
    databasePath,
    databaseUrl: sqliteUrlForPath(databasePath),
    e2eRootDir: persistentE2eRootDir,
    modelConfigDir,
    pluginsDir,
    repoRoot,
    rootDir,
    serverBaseUrl,
    serverBind,
    serverPort,
    sessionStateDir,
    settingsOverlayPath,
    settingsPath,
    uiBaseUrl,
    uiPort,
    workspaceRoot,
  }
}

export async function startFullstackDev(
  testEnv: FullstackDevEnvironment
): Promise<ManagedDevProcess> {
  await assertTcpPortAvailable(testEnv.serverPort, "slab-server")
  await assertTcpPortAvailable(testEnv.uiPort, "desktop Vite")

  const logs: string[] = []
  const startedAt = new Date(Date.now() - processStartSkewMs)
  const cargoShimPath = installCargoShim(testEnv.rootDir)
  const commonEnv: NodeJS.ProcessEnv = {
    ...process.env,
    BROWSER: "none",
    CARGO: cargoShimPath,
    NO_COLOR: "1",
    SLAB_BIND: testEnv.serverBind,
    SLAB_CORS_ORIGINS: `${testEnv.uiBaseUrl},http://localhost:${testEnv.uiPort}`,
    SLAB_DATABASE_URL: testEnv.databaseUrl,
    SLAB_ENABLE_SWAGGER: "true",
    SLAB_LOG: "info",
    SLAB_MODEL_CONFIG_DIR: testEnv.modelConfigDir,
    SLAB_PLUGINS_DIR: testEnv.pluginsDir,
    SLAB_SESSION_STATE_DIR: testEnv.sessionStateDir,
    SLAB_SETTINGS_OVERLAY_PATH: testEnv.settingsOverlayPath,
    SLAB_SETTINGS_PATH: testEnv.settingsPath,
    SLAB_WORKSPACE_ROOT: testEnv.workspaceRoot,
    VITE_API_BASE_URL: testEnv.uiBaseUrl,
    VITE_API_PROXY_TARGET: testEnv.serverBaseUrl,
  }
  prependToPath(commonEnv, dirname(cargoShimPath))
  delete commonEnv.RUSTC_WRAPPER
  delete commonEnv.SLAB_E2E_MODE

  const children: ManagedChild[] = []
  const server = spawnSlabServer(testEnv, commonEnv)
  children.push({ child: server, label: "slab-server" })
  rememberOutput("slab-server", server, logs)

  const processHandle = {
    logs,
    stop: async () => {
      await stopManagedChildren(children, logs, testEnv.repoRoot, startedAt)
    },
  }

  try {
    await waitForHttpOk(
      `${testEnv.serverBaseUrl}/health`,
      "slab-server health",
      server,
      logs,
      serverReadinessTimeoutMs
    )

    const vite = spawn("bun", [
      "run",
      "--cwd",
      "packages/slab-desktop",
      "dev",
      "--host",
      "127.0.0.1",
      "--port",
      String(testEnv.uiPort),
      "--strictPort",
      "true",
    ], {
      cwd: testEnv.repoRoot,
      env: commonEnv,
      stdio: "pipe",
    })
    children.push({ child: vite, label: "desktop-vite" })
    rememberOutput("desktop-vite", vite, logs)

    await waitForHttpOk(testEnv.uiBaseUrl, "desktop dev UI", vite, logs, uiReadinessTimeoutMs)
    return processHandle
  } catch (error) {
    await processHandle.stop().catch(() => {})
    throw error
  }
}

export async function completeSetup(baseUrl: string): Promise<void> {
  await requestJson<Schema["SetupStatusResponse"]>(baseUrl, "/v1/setup/complete", {
    json: { initialized: true } satisfies Schema["CompleteSetupRequest"],
    method: "POST",
  })
}

export async function bootstrapLocalModel(
  baseUrl: string,
  options: LocalModelBootstrapOptions
): Promise<UnifiedModelResponse> {
  await completeSetup(baseUrl)
  await importLocalModelPack(baseUrl, options.modelId)

  let model = await selectModelConfigVariant(
    baseUrl,
    options.modelId,
    options.selectedVariantId,
    options.selectedPresetId
  )

  model = await ensureModelDownloaded(baseUrl, model.id)

  if (options.load ?? true) {
    await requestJson<Schema["ModelStatusResponse"]>(baseUrl, "/v1/models/load", {
      json: { model_id: model.id } satisfies Schema["LoadModelRequest"],
      method: "POST",
    })
  }

  if (options.selectForAssistant ?? true) {
    await selectAssistantModel(baseUrl, model.id)
  }

  return model
}

export async function importLocalModelPack(
  baseUrl: string,
  modelId: LocalModelBootstrapOptions["modelId"]
): Promise<UnifiedModelResponse> {
  return importModelPack(baseUrl, modelId)
}

export async function selectModelConfigVariant(
  baseUrl: string,
  modelId: string,
  selectedVariantId: LocalModelBootstrapOptions["selectedVariantId"],
  selectedPresetId = "default"
): Promise<UnifiedModelResponse> {
  return requestJson<UnifiedModelResponse>(
    baseUrl,
    `/v1/models/${encodeURIComponent(modelId)}/config-selection`,
    {
      json: {
        selected_preset_id: selectedPresetId,
        selected_variant_id: selectedVariantId,
      } satisfies Schema["UpdateModelConfigSelectionRequest"],
      method: "PUT",
    }
  )
}

export async function getModel(baseUrl: string, modelId: string): Promise<UnifiedModelResponse> {
  return requestJson<UnifiedModelResponse>(baseUrl, `/v1/models/${encodeURIComponent(modelId)}`)
}

export async function ensureModelDownloaded(
  baseUrl: string,
  modelId: string
): Promise<UnifiedModelResponse> {
  const current = await getModel(baseUrl, modelId)
  if (modelHasLocalPath(current)) {
    return current
  }

  const accepted = await requestJson<Schema["OperationAcceptedResponse"]>(
    baseUrl,
    "/v1/models/download",
    {
      json: { model_id: modelId } satisfies Schema["DownloadModelRequest"],
      method: "POST",
    }
  )
  await waitForTaskSucceeded(baseUrl, accepted.operation_id, modelDownloadTaskTimeoutMs)

  const downloaded = await getModel(baseUrl, modelId)
  if (!modelHasLocalPath(downloaded)) {
    throw new Error(`Model ${modelId} download succeeded, but local_path is still empty.`)
  }
  return downloaded
}

export async function waitForTaskSucceeded(
  baseUrl: string,
  taskId: string,
  timeoutMs = defaultTaskTimeoutMs
): Promise<TaskResponse> {
  return eventually(
    `task ${taskId} succeeded`,
    async () => {
      const task = await requestJson<TaskResponse>(baseUrl, `/v1/tasks/${encodeURIComponent(taskId)}`)
      if (task.status === "succeeded") {
        return task
      }
      if (isFailedTaskStatus(task.status)) {
        throw new Error(
          `Task ${taskId} ended with ${task.status}: ${task.error_msg ?? "no error message"}`
        )
      }
      return null
    },
    timeoutMs,
    taskPollIntervalMs
  )
}

export async function selectAssistantModel(baseUrl: string, modelId: string): Promise<void> {
  const state = (await getPersistedUiState<{ selections?: Record<string, string> }>(
    baseUrl,
    "zustand:header-ui"
  )) ?? { selections: {} }

  await putPersistedUiState(baseUrl, "zustand:header-ui", {
    ...state,
    selections: {
      ...state.selections,
      "assistant:model": modelId,
    },
  })
}

export async function selectAssistantSession(
  baseUrl: string,
  sessionId: string,
  sessionName?: string
): Promise<void> {
  const state = (await getPersistedUiState<{
    currentSessionId?: string
    deepThink?: boolean
    sessionLabels?: Record<string, string>
  }>(baseUrl, "zustand:assistant-ui")) ?? {}

  await putPersistedUiState(baseUrl, "zustand:assistant-ui", {
    ...state,
    currentSessionId: sessionId,
    deepThink: false,
    sessionLabels: sessionName
      ? {
          ...state.sessionLabels,
          [sessionId]: sessionName,
        }
      : state.sessionLabels,
  })
}

export async function restoreSession(
  baseUrl: string,
  sessionId: string
): Promise<AgentSessionRestored> {
  const response = await requestJson<AgentResponsesServerMessage>(baseUrl, "/v1/agents/responses", {
    json: {
      request_id: `restore-${Date.now()}`,
      session_id: sessionId,
      type: "agent.session.restore",
    } satisfies Schema["AgentResponsesClientMessage"],
    method: "POST",
  })

  if (response.type !== "agent.session.restored") {
    throw new Error(`Expected agent.session.restored, received ${response.type}`)
  }

  return response
}

export async function listSessions(baseUrl: string): Promise<SessionResponse[]> {
  return requestJson<SessionResponse[]>(baseUrl, "/v1/sessions")
}

export async function createSession(baseUrl: string, name?: string): Promise<SessionResponse> {
  const session = await requestJson<SessionResponse>(baseUrl, "/v1/sessions", {
    json: {} satisfies Schema["CreateSessionRequest"],
    method: "POST",
  })
  if (!name) {
    return session
  }
  return requestJson<SessionResponse>(baseUrl, `/v1/sessions/${encodeURIComponent(session.id)}`, {
    json: { name } satisfies Schema["UpdateSessionRequest"],
    method: "PUT",
  })
}

export async function getPersistedUiState<T>(baseUrl: string, key: string): Promise<T | null> {
  const response = await fetch(`${baseUrl}/v1/ui-state/${encodeURIComponent(key)}`)
  if (response.status === 404) {
    return null
  }
  if (!response.ok) {
    throw new Error(`GET /v1/ui-state/${key} failed with ${response.status}: ${await response.text()}`)
  }
  const body = (await response.json()) as UiStateValueResponse
  const persisted = JSON.parse(body.value) as { state?: T }
  return persisted.state ?? null
}

export async function putPersistedUiState<T>(
  baseUrl: string,
  key: string,
  state: T
): Promise<void> {
  await requestJson<UiStateValueResponse>(baseUrl, `/v1/ui-state/${encodeURIComponent(key)}`, {
    json: {
      value: JSON.stringify({
        state,
        version: 0,
      }),
    } satisfies Schema["UpdateUiStateRequest"],
    method: "PUT",
  })
}

export async function requestJson<T>(
  baseUrl: string,
  path: string,
  init: JsonRequestInit = {}
): Promise<T> {
  const headers = new Headers(init.headers)
  if (init.json !== undefined && !headers.has("content-type")) {
    headers.set("content-type", "application/json")
  }

  const response = await fetch(`${baseUrl}${path}`, {
    ...init,
    body: init.json === undefined ? undefined : JSON.stringify(init.json),
    headers,
  })
  const text = await response.text()
  const body = text ? (JSON.parse(text) as T) : (undefined as T)

  if (!response.ok) {
    throw new Error(`${init.method ?? "GET"} ${path} failed with ${response.status}: ${text}`)
  }

  return body
}

export async function eventually<T>(
  label: string,
  assertion: () => Promise<T | false | null | undefined> | T | false | null | undefined,
  timeoutMs = defaultEventuallyTimeoutMs,
  intervalMs = defaultEventuallyIntervalMs
): Promise<T> {
  const deadline = Date.now() + timeoutMs
  let lastError: unknown

  while (Date.now() < deadline) {
    try {
      // eslint-disable-next-line no-await-in-loop
      const result = await assertion()
      if (result) {
        return result
      }
    } catch (error) {
      lastError = error
    }

    // eslint-disable-next-line no-await-in-loop
    await delay(intervalMs)
  }

  const suffix = lastError instanceof Error ? ` Last error: ${lastError.message}` : ""
  throw new Error(`${label} timed out after ${timeoutMs}ms.${suffix}`)
}

export function cleanupFullstackDevEnvironment(testEnv: FullstackDevEnvironment | undefined): void {
  if (testEnv?.rootDir) {
    rmSync(testEnv.rootDir, { force: true, recursive: true })
  }
}

export function formatDevLogs(logs: string[]): string {
  return logs.length > 0 ? `\nRecent output:\n${logs.join("\n")}` : ""
}

async function importModelPack(baseUrl: string, modelId: string): Promise<UnifiedModelResponse> {
  const packPath = join(modelPackDir, `${modelId}.slab`)
  if (!existsSync(packPath)) {
    throw new Error(`Missing model pack ${packPath}. Run bun run gen:model-packs.`)
  }

  const bytes = readFileSync(packPath)
  const body = new FormData()
  body.set("file", new File([bytes], `${modelId}.slab`))

  const response = await fetch(`${baseUrl}/v1/models/import-pack`, {
    body,
    method: "POST",
  })
  const text = await response.text()
  if (!response.ok) {
    throw new Error(`POST /v1/models/import-pack failed with ${response.status}: ${text}`)
  }
  return JSON.parse(text) as UnifiedModelResponse
}

function modelHasLocalPath(model: UnifiedModelResponse): boolean {
  return model.status === "ready" && typeof model.spec.local_path === "string" && model.spec.local_path.length > 0
}

function isFailedTaskStatus(status: string): boolean {
  return status === "failed" || status === "cancelled" || status === "interrupted"
}

function writeSettingsDocument(
  path: string,
  options: {
    databaseUrl: string
    modelConfigDir: string
    pluginsDir: string
    serverBind: string
    sessionStateDir: string
  }
): void {
  writeFileSync(
    path,
    `${JSON.stringify(
      {
        $schema: setupSchemaUrl,
        schema_version: 2,
        agent: {
          debug: false,
          hooks: {
            enabled: false,
            scripts: [],
          },
          memories: {
            enabled: false,
          },
          tools: {
            mcp: {
              enabled: false,
              servers: [],
            },
            websearch: {
              default_provider: "duckduckgo",
            },
          },
        },
        database: {
          url: options.databaseUrl,
        },
        general: {
          language: "en-US",
        },
        logging: {
          json: false,
          level: "info",
        },
        models: {
          config_dir: options.modelConfigDir,
        },
        plugin: {
          install_dir: options.pluginsDir,
        },
        runtime: {
          mode: "managed_children",
          transport: "ipc",
          sessions: {
            state_dir: options.sessionStateDir,
          },
          ggml: {
            backends: {
              diffusion: { enabled: false },
              llama: { enabled: true, context_length: 2048 },
              whisper: { enabled: false },
            },
          },
          candle: { enabled: false },
          onnx: { enabled: false },
        },
        server: {
          address: options.serverBind,
          swagger: {
            enabled: true,
          },
        },
        telemetry: {
          enabled: false,
        },
        tools: {
          ffmpeg: {
            auto_download: false,
            enabled: false,
          },
        },
      },
      null,
      2
    )}\n`,
    "utf8"
  )
}

async function reserveTcpPort(): Promise<number> {
  return new Promise((resolvePort, reject) => {
    const server = createServer()
    server.once("error", reject)
    server.listen(0, "127.0.0.1", () => {
      const address = server.address()
      if (!address || typeof address === "string") {
        server.close(() => reject(new Error("Unable to reserve a TCP port.")))
        return
      }
      const { port } = address
      server.close((error) => {
        if (error) {
          reject(error)
          return
        }
        resolvePort(port)
      })
    })
  })
}

async function assertTcpPortAvailable(port: number, label: string): Promise<void> {
  await assertTcpPortAvailableOnHost(port, label, "127.0.0.1")
}

async function assertTcpPortAvailableOnHost(port: number, label: string, host: string): Promise<void> {
  await new Promise<void>((resolveAvailable, reject) => {
    const probe = createServer()
    probe.once("error", () => {
      reject(
        new Error(
          `${label} port ${host}:${port} is already in use. Stop the existing dev process before running bun run test:e2e.`
        )
      )
    })
    probe.listen(port, host, () => {
      probe.close((error) => {
        if (error) {
          reject(error)
          return
        }
        resolveAvailable()
      })
    })
  })
}

function sqliteUrlForPath(path: string): string {
  const normalized = path.replaceAll("\\", "/")
  return normalized.startsWith("/") ? `sqlite://${normalized}?mode=rwc` : `sqlite:///${normalized}?mode=rwc`
}

function spawnSlabServer(
  testEnv: FullstackDevEnvironment,
  env: NodeJS.ProcessEnv
): ChildProcessWithoutNullStreams {
  const args = [
    "--database-url",
    testEnv.databaseUrl,
    "--settings-path",
    testEnv.settingsPath,
    "--settings-overlay-path",
    testEnv.settingsOverlayPath,
    "--workspace-root",
    testEnv.workspaceRoot,
    "--model-config-dir",
    testEnv.modelConfigDir,
    "--session-state-dir",
    testEnv.sessionStateDir,
    "--plugins-dir",
    testEnv.pluginsDir,
    "--log",
    "info",
    "--shutdown-on-stdin-close",
  ]
  const exe = join(
    testEnv.repoRoot,
    "target",
    "debug",
    process.platform === "win32" ? "slab-server.exe" : "slab-server"
  )

  if (existsSync(exe)) {
    return spawn(exe, args, {
      cwd: testEnv.repoRoot,
      env,
      stdio: "pipe",
    })
  }

  return spawn("bun", [
    "./scripts/cargo/run.ts",
    "run",
    "-p",
    "slab-server",
    "--bin",
    "slab-server",
    "--",
    ...args,
  ], {
    cwd: testEnv.repoRoot,
    env,
    stdio: "pipe",
  })
}

function rememberOutput(label: string, child: ChildProcessWithoutNullStreams, logs: string[]): void {
  const remember = (chunk: Buffer | string) => {
    for (const line of String(chunk).split(/\r?\n/)) {
      if (!line.trim()) {
        continue
      }
      logs.push(`[${label}] ${line}`)
      if (logs.length > devLogRingLimit) {
        logs.shift()
      }
    }
  }

  child.stdout.on("data", remember)
  child.stderr.on("data", remember)
}

function installCargoShim(rootDir: string): string {
  const shimDir = join(rootDir, "bin")
  mkdirSync(shimDir, { recursive: true })

  const realCargo = findCargoExecutable()
  if (process.platform === "win32") {
    const shimPath = join(shimDir, "cargo.cmd")
    writeFileSync(
      shimPath,
      `@echo off\r\n"${realCargo}" --config "build.rustc-wrapper=''" %*\r\n`,
      "utf8"
    )
    return shimPath
  }

  const shimPath = join(shimDir, "cargo")
  writeFileSync(
    shimPath,
    `#!/bin/sh\nexec "${realCargo}" --config "build.rustc-wrapper=''" "$@"\n`,
    "utf8"
  )
  chmodSync(shimPath, 0o755)
  return shimPath
}

function findCargoExecutable(): string {
  const command = process.platform === "win32" ? "where.exe" : "which"
  const output = execFileSync(command, ["cargo"], { encoding: "utf8" })
  const cargoPath = output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean)

  if (!cargoPath) {
    throw new Error("Unable to resolve cargo executable for E2E dev startup.")
  }

  return cargoPath
}

function prependToPath(env: NodeJS.ProcessEnv, entry: string): void {
  const pathKey = Object.keys(env).find((key) => key.toLowerCase() === "path") ?? "PATH"
  env[pathKey] = `${entry}${delimiter}${env[pathKey] ?? ""}`
}

async function waitForHttpOk(
  url: string,
  label: string,
  child: ChildProcessWithoutNullStreams,
  logs: string[],
  timeoutMs: number
): Promise<void> {
  await eventually(
    label,
    async () => {
      if (child.exitCode !== null) {
        throw new Error(`${label} process exited with ${child.exitCode}.${formatDevLogs(logs)}`)
      }

      try {
        const response = await fetch(url)
        return response.ok
      } catch {
        return false
      }
    },
    timeoutMs
  )
}

async function stopManagedChildren(
  children: ManagedChild[],
  logs: string[],
  rootPath: string,
  startedAt: Date
): Promise<void> {
  for (const child of children.toReversed()) {
    if (child.child.exitCode !== null) {
      continue
    }
    killProcessTree(child.child)
    // eslint-disable-next-line no-await-in-loop
    await waitForExit(child.child, managedProcessStopTimeoutMs).catch(() => {
      logs.push(`[${child.label}] timed out while stopping process ${child.child.pid}`)
    })
  }

  if (process.platform === "win32") {
    stopWindowsDevProcesses(rootPath, startedAt, [process.pid, process.ppid].filter(Boolean))
  }
}

function waitForExit(child: ChildProcessWithoutNullStreams, timeoutMs: number): Promise<void> {
  return new Promise((resolveExit, reject) => {
    if (child.exitCode !== null) {
      resolveExit()
      return
    }

    const timeout = setTimeout(() => {
      child.off("exit", onExit)
      reject(new Error(`Timed out waiting for process ${child.pid} to exit.`))
    }, timeoutMs)

    const onExit = () => {
      clearTimeout(timeout)
      resolveExit()
    }

    child.once("exit", onExit)
  })
}

function killProcessTree(child: ChildProcessWithoutNullStreams): void {
  if (child.pid === undefined || child.exitCode !== null) {
    return
  }

  if (process.platform === "win32") {
    try {
      execFileSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], { stdio: "ignore" })
    } catch {
      return
    }
    return
  }

  child.kill("SIGKILL")
}

function stopWindowsDevProcesses(rootPath: string, startedAt: Date, protectedPids: number[]): void {
  const script = `
$started = [DateTimeOffset]::Parse(${JSON.stringify(startedAt.toISOString())}).LocalDateTime
$repo = ${JSON.stringify(rootPath)}
$protected = @(${protectedPids.join(",")})
Get-Process -ErrorAction SilentlyContinue |
  Where-Object {
    ($protected -notcontains $_.Id) -and
    $_.StartTime -ge $started -and
    (
      ($_.ProcessName -in @('slab-server', 'slab-runtime', 'vite')) -and $_.Path -like "$repo*"
    )
  } |
  Stop-Process -Force -ErrorAction SilentlyContinue
`

  execFileSync("powershell.exe", ["-NoProfile", "-Command", script], { stdio: "ignore" })
}

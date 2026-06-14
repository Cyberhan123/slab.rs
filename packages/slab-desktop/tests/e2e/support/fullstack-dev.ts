import { spawn, execFileSync, type ChildProcessWithoutNullStreams } from "node:child_process"
import { createServer } from "node:net"
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
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
export type SessionResponse = Schema["SessionResponse"]
type UiStateValueResponse = Schema["UiStateValueResponse"]
type UnifiedModelResponse = Schema["UnifiedModelResponse"]

type JsonRequestInit = Omit<RequestInit, "body"> & {
  json?: unknown
}

export type ManagedDevProcess = {
  child: ChildProcessWithoutNullStreams
  logs: string[]
  stop: () => Promise<void>
}

export type FullstackDevEnvironment = {
  databasePath: string
  databaseUrl: string
  modelConfigDir: string
  pluginsDir: string
  repoRoot: string
  rootDir: string
  serverBaseUrl: string
  serverBind: string
  sessionStateDir: string
  settingsPath: string
  uiBaseUrl: string
}

const setupSchemaUrl = "https://slab.reorgix.com/manifests/v1/settings-document.schema.json"
const serverBind = "127.0.0.1:3000"
const serverBaseUrl = `http://${serverBind}`
const uiBaseUrl = "http://localhost:1420"

const supportDir = dirname(fileURLToPath(import.meta.url))
const packageRoot = resolve(supportDir, "../../..")
const repoRoot = resolve(packageRoot, "../..")

export async function createFullstackDevEnvironment(): Promise<FullstackDevEnvironment> {
  await assertTcpPortAvailable(3000, "slab-server")
  await assertTcpPortAvailable(1420, "desktop Vite")

  const rootDir = mkdtempSync(join(tmpdir(), "slab-desktop-e2e-"))
  const settingsDir = join(rootDir, "config")
  const modelConfigDir = join(settingsDir, "models")
  const pluginsDir = join(rootDir, "plugins")
  const sessionStateDir = join(rootDir, "sessions")
  const settingsPath = join(settingsDir, "settings.json")
  const databasePath = join(rootDir, "slab.db")

  for (const dir of [settingsDir, modelConfigDir, pluginsDir, sessionStateDir]) {
    mkdirSync(dir, { recursive: true })
  }

  writeFileSync(
    settingsPath,
    `${JSON.stringify(
      {
        $schema: setupSchemaUrl,
        schema_version: 2,
        general: {
          language: "en-US",
        },
        logging: {
          json: false,
          level: "warn",
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
          },
        },
        runtime: {
          mode: "managed_children",
          transport: "ipc",
          ggml: {
            backends: {
              diffusion: { enabled: false },
              llama: { enabled: false },
              whisper: { enabled: false },
            },
          },
          candle: { enabled: false },
          onnx: { enabled: false },
        },
        plugin: {
          install_dir: pluginsDir,
        },
        server: {
          address: serverBind,
          swagger: {
            enabled: true,
          },
        },
      },
      null,
      2
    )}\n`,
    "utf8"
  )

  return {
    databasePath,
    databaseUrl: sqliteUrlForPath(databasePath),
    modelConfigDir,
    pluginsDir,
    repoRoot,
    rootDir,
    serverBaseUrl,
    serverBind,
    sessionStateDir,
    settingsPath,
    uiBaseUrl,
  }
}

export async function startFullstackDev(
  testEnv: FullstackDevEnvironment
): Promise<ManagedDevProcess> {
  const logs: string[] = []
  const startedAt = new Date(Date.now() - 1_000)
  const cargoShimPath = installCargoShim(testEnv.rootDir)
  const childEnv: NodeJS.ProcessEnv = {
    ...process.env,
    BROWSER: "none",
    CARGO: cargoShimPath,
    NO_COLOR: "1",
    SLAB_BIND: testEnv.serverBind,
    SLAB_CORS_ORIGINS: `${testEnv.uiBaseUrl},http://127.0.0.1:1420`,
    SLAB_DATABASE_URL: testEnv.databaseUrl,
    SLAB_E2E_MODE: "1",
    SLAB_ENABLE_SWAGGER: "true",
    SLAB_LOG: "warn",
    SLAB_MODEL_CONFIG_DIR: testEnv.modelConfigDir,
    SLAB_SESSION_STATE_DIR: testEnv.sessionStateDir,
    SLAB_SETTINGS_PATH: testEnv.settingsPath,
    VITE_API_BASE_URL: testEnv.serverBaseUrl,
  }
  prependToPath(childEnv, dirname(cargoShimPath))
  delete childEnv.RUSTC_WRAPPER

  const child = spawn("bun", ["run", "dev"], {
    cwd: testEnv.repoRoot,
    env: childEnv,
    stdio: "pipe",
  })
  rememberOutput(child, logs)

  const processHandle = {
    child,
    logs,
    stop: async () => {
      await stopProcess(child, logs, testEnv.repoRoot, startedAt)
    },
  }

  try {
    await waitForHttpOk(`${testEnv.serverBaseUrl}/health`, "slab-server health", child, logs, 600_000)
    await waitForHttpOk(testEnv.uiBaseUrl, "desktop dev UI", child, logs, 180_000)
    return processHandle
  } catch (error) {
    await processHandle.stop().catch(() => {})
    throw error
  }
}

export async function seedBackend(baseUrl: string): Promise<UnifiedModelResponse> {
  await requestJson<Schema["SetupStatusResponse"]>(baseUrl, "/v1/setup/complete", {
    json: { initialized: true } satisfies Schema["CompleteSetupRequest"],
    method: "POST",
  })

  const model = await requestJson<UnifiedModelResponse>(baseUrl, "/v1/models", {
    json: {
      capabilities: ["chat_generation", "text_generation"],
      display_name: "E2E Assistant",
      kind: "cloud",
      runtime_presets: {
        max_tokens: 256,
      },
      spec: {
        context_window: 8192,
        provider_id: "e2e",
        remote_model_id: "e2e-assistant",
      },
    } satisfies Schema["CreateModelRequest"],
    method: "POST",
  })

  await putPersistedUiState(baseUrl, "zustand:header-ui", {
    selections: {
      "assistant:model": model.id,
    },
  })

  return model
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
  timeoutMs = 30_000,
  intervalMs = 250
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

async function putPersistedUiState<T>(baseUrl: string, key: string, state: T): Promise<void> {
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

async function assertTcpPortAvailable(port: number, label: string): Promise<void> {
  await assertTcpPortAvailableOnHost(port, label, "127.0.0.1")
  await assertTcpPortAvailableOnHost(port, label, "::1")
}

async function assertTcpPortAvailableOnHost(port: number, label: string, host: string): Promise<void> {
  await new Promise<void>((resolveAvailable, reject) => {
    const probe = createServer()
    probe.once("error", (error: NodeJS.ErrnoException) => {
      if (host === "::1" && error.code === "EAFNOSUPPORT") {
        resolveAvailable()
        return
      }
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

function rememberOutput(child: ChildProcessWithoutNullStreams, logs: string[]): void {
  const remember = (chunk: Buffer | string) => {
    for (const line of String(chunk).split(/\r?\n/)) {
      if (!line.trim()) {
        continue
      }
      logs.push(line)
      if (logs.length > 300) {
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
        throw new Error(`${label} process exited with ${child.exitCode}.${formatLogs(logs)}`)
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

async function stopProcess(
  child: ChildProcessWithoutNullStreams,
  logs: string[],
  rootPath: string,
  startedAt: Date
): Promise<void> {
  if (process.platform === "win32") {
    if (child.exitCode === null) {
      killProcessTree(child)
      await waitForExit(child, 12_000).catch(() => {})
    }
    stopWindowsDevProcesses(rootPath, startedAt, [process.pid, process.ppid].filter(Boolean))
    return
  }

  if (child.exitCode !== null) {
    return
  }

  child.kill("SIGTERM")

  try {
    await waitForExit(child, 12_000)
  } catch {
    child.kill("SIGKILL")
    await waitForExit(child, 12_000).catch(() => {
      throw new Error(`Failed to stop dev process ${child.pid}.${formatLogs(logs)}`)
    })
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
      ($_.ProcessName -in @('slab-app', 'slab-server', 'vite')) -and $_.Path -like "$repo*"
    )
  } |
  Stop-Process -Force -ErrorAction SilentlyContinue
`

  execFileSync("powershell.exe", ["-NoProfile", "-Command", script], { stdio: "ignore" })
}

function formatLogs(logs: string[]): string {
  return logs.length > 0 ? `\nRecent output:\n${logs.join("\n")}` : ""
}

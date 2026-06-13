import { spawn, execFileSync, type ChildProcessWithoutNullStreams } from "node:child_process"
import { createServer } from "node:net"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { setTimeout as delay } from "node:timers/promises"

import { afterAll, beforeAll, describe, expect, it } from "vitest"
import { chromium, type Browser, type BrowserContext, type Locator, type Page } from "playwright"

import type { components } from "@slab/api/v1"

type Schema = components["schemas"]
type AgentResponsesServerMessage = Schema["AgentResponsesServerMessage"]
type AgentSessionRestored = Extract<
  AgentResponsesServerMessage,
  { type: "agent.session.restored" }
>
type SessionResponse = Schema["SessionResponse"]
type UiStateValueResponse = Schema["UiStateValueResponse"]
type UnifiedModelResponse = Schema["UnifiedModelResponse"]

type JsonRequestInit = Omit<RequestInit, "body"> & {
  json?: unknown
}

type ManagedProcess = {
  child: ChildProcessWithoutNullStreams
  logs: string[]
  stop: () => Promise<void>
}

type FullstackEnvironment = {
  databasePath: string
  databaseUrl: string
  modelConfigDir: string
  packageRoot: string
  pluginsDir: string
  repoRoot: string
  rootDir: string
  serverBaseUrl: string
  serverBind: string
  serverPort: number
  sessionStateDir: string
  settingsPath: string
  uiBaseUrl: string
  uiPort: number
}

const composerPlaceholder = "Type a message or drop files..."
const sendButtonName = "Send message"
const setupSchemaUrl = "https://slab.reorgix.com/manifests/v1/settings-document.schema.json"

const testDir = dirname(fileURLToPath(import.meta.url))
const packageRoot = resolve(testDir, "../..")
const repoRoot = resolve(packageRoot, "../..")

let env: FullstackEnvironment

describe.sequential("assistant fullstack e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let server: ManagedProcess | undefined
  let vite: ManagedProcess | undefined
  let page: Page

  beforeAll(async () => {
    env = await createFullstackEnvironment()
    server = await startSlabServer(env)
    await seedBackend(env.serverBaseUrl)
    vite = await startVite(env)

    browser = await chromium.launch({ headless: true })
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
    })
    await context.addInitScript(() => {
      window.localStorage.setItem("slab.ui.language", "en-US")
    })
    page = await context.newPage()
    await page.goto(env.uiBaseUrl, { waitUntil: "domcontentloaded", timeout: 60_000 })
    await waitForComposerReady(page)
  })

  afterAll(async () => {
    await context?.close().catch(() => {})
    await browser?.close().catch(() => {})
    await vite?.stop().catch(() => {})
    await server?.stop().catch(() => {})
    if (env?.rootDir) {
      rmSync(env.rootDir, { force: true, recursive: true })
    }
  })

  it("drives real assistant execution, tool loops, persistence, and session management", async () => {
    const runId = `assistant-fullstack-${Date.now()}`
    const firstPrompt = `Persist this assistant e2e message ${runId}`
    const firstReply = `E2E assistant persisted reply: ${firstPrompt}`
    const firstSessionLabel =
      firstPrompt.length > 42 ? `${firstPrompt.slice(0, 42)}...` : firstPrompt

    await sendAssistantMessage(page, firstPrompt)
    await expectVisibleText(page, firstPrompt)
    await expectVisibleText(page, firstReply)

    const firstSession = await waitForSessionNamed(firstSessionLabel)
    const firstRestore = await restoreSession(env.serverBaseUrl, firstSession.id)
    expect(firstRestore.thread?.status).toBe("completed")
    expect(firstRestore.messages.some((message) => message.role === "user" && message.content === firstPrompt)).toBe(true)
    expect(firstRestore.messages.some((message) => message.role === "assistant" && message.content === firstReply)).toBe(true)

    const loopPrompt = `Run the tool loop ${runId}`
    await sendAssistantMessage(page, loopPrompt)
    await expectVisibleText(page, loopPrompt)
    await expectVisibleText(page, "e2e assistant loop", { exact: false })
    await expectVisibleText(page, "in_progress: record plan", { exact: false })
    await expectVisibleText(page, "E2E loop complete after plan_update tool output.")

    const loopRestore = await restoreSession(env.serverBaseUrl, firstSession.id)
    expect(loopRestore.thread?.status).toBe("completed")
    expect(
      loopRestore.messages.some(
        (message) =>
          message.role === "assistant" &&
          (message.tool_calls ?? []).some((toolCall) => toolCall.function.name === "plan_update")
      )
    ).toBe(true)
    expect(
      loopRestore.messages.some(
        (message) =>
          message.role === "tool" &&
          message.content.includes("\"summary\":\"e2e assistant loop\"")
      )
    ).toBe(true)
    expect(
      loopRestore.messages.some(
        (message) =>
          message.role === "assistant" &&
          message.content === "E2E loop complete after plan_update tool output."
      )
    ).toBe(true)

    await page.getByRole("button", { name: "Create session" }).click()
    const secondSessionId = await waitForCurrentAssistantSession((sessionId) => sessionId !== firstSession.id)
    await expectVisibleText(page, "Start a new thread and keep the stage focused.")

    if (!server) {
      throw new Error("slab-server process was not started.")
    }

    await server.stop()
    server = await startSlabServer(env)

    const restoredAfterRestart = await restoreSession(env.serverBaseUrl, firstSession.id)
    expect(restoredAfterRestart.thread?.status).toBe("completed")
    expect(restoredAfterRestart.messages.some((message) => message.content === firstReply)).toBe(true)
    expect(
      restoredAfterRestart.messages.some(
        (message) => message.role === "tool" && message.content.includes("e2e assistant loop")
      )
    ).toBe(true)

    await page.reload({ waitUntil: "domcontentloaded", timeout: 60_000 })
    await waitForComposerReady(page)
    await waitForCurrentAssistantSession((sessionId) => sessionId === secondSessionId)
    await expectVisibleText(page, "Start a new thread and keep the stage focused.")
    expect(await page.getByText(firstPrompt).isVisible().catch(() => false)).toBe(false)

    await deleteSessionFromSheet(page, firstSession)

    await eventually("deleted session disappears from backend list", async () => {
      const sessions = await listSessions(env.serverBaseUrl)
      return sessions.every((session) => session.id !== firstSession.id)
    })
  })
})

async function createFullstackEnvironment(): Promise<FullstackEnvironment> {
  const [serverPort, uiPort] = await Promise.all([findFreePort(), findFreePort()])
  const rootDir = mkdtempSync(join(tmpdir(), "slab-assistant-fullstack-"))
  const settingsDir = join(rootDir, "config")
  const modelConfigDir = join(settingsDir, "models")
  const pluginsDir = join(rootDir, "plugins")
  const sessionStateDir = join(rootDir, "sessions")
  const settingsPath = join(settingsDir, "settings.json")
  const databasePath = join(rootDir, "slab.db")
  const serverBind = `127.0.0.1:${serverPort}`
  const serverBaseUrl = `http://${serverBind}`
  const uiBaseUrl = `http://127.0.0.1:${uiPort}`

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
    packageRoot,
    pluginsDir,
    repoRoot,
    rootDir,
    serverBaseUrl,
    serverBind,
    serverPort,
    sessionStateDir,
    settingsPath,
    uiBaseUrl,
    uiPort,
  }
}

async function startSlabServer(testEnv: FullstackEnvironment): Promise<ManagedProcess> {
  const logs: string[] = []
  const serverEnv = {
    ...process.env,
    NO_COLOR: "1",
    SLAB_BIND: testEnv.serverBind,
    SLAB_CORS_ORIGINS: `${testEnv.uiBaseUrl},http://localhost:${testEnv.uiPort}`,
    SLAB_DATABASE_URL: testEnv.databaseUrl,
    SLAB_E2E_MODE: "1",
    SLAB_ENABLE_SWAGGER: "true",
    SLAB_LOG: "warn",
    SLAB_MODEL_CONFIG_DIR: testEnv.modelConfigDir,
    SLAB_SESSION_STATE_DIR: testEnv.sessionStateDir,
    SLAB_SETTINGS_PATH: testEnv.settingsPath,
  }
  delete serverEnv.RUSTC_WRAPPER

  const child = spawn(
    "cargo",
    [
      "--config",
      'build.rustc-wrapper=""',
      "run",
      "--bin",
      "slab-server",
      "--",
      "--shutdown-on-stdin-close",
      "--settings-path",
      testEnv.settingsPath,
      "--database-url",
      testEnv.databaseUrl,
      "--model-config-dir",
      testEnv.modelConfigDir,
      "--session-state-dir",
      testEnv.sessionStateDir,
      "--plugins-dir",
      testEnv.pluginsDir,
    ],
    {
      cwd: testEnv.repoRoot,
      env: serverEnv,
      stdio: "pipe",
    }
  )
  rememberOutput(child, logs)

  const processHandle = {
    child,
    logs,
    stop: async () => {
      await stopProcess(child, logs, "shutdown")
    },
  }

  try {
    await waitForHttpOk(`${testEnv.serverBaseUrl}/health`, "slab-server health", child, logs, 150_000)
    return processHandle
  } catch (error) {
    await processHandle.stop().catch(() => {})
    throw error
  }
}

async function startVite(testEnv: FullstackEnvironment): Promise<ManagedProcess> {
  const logs: string[] = []
  const child = spawn(
    "bun",
    ["x", "vite", "--host", "127.0.0.1", "--port", String(testEnv.uiPort), "--strictPort"],
    {
      cwd: testEnv.packageRoot,
      env: {
        ...process.env,
        BROWSER: "none",
        VITE_API_BASE_URL: testEnv.serverBaseUrl,
      },
      stdio: "pipe",
    }
  )
  rememberOutput(child, logs)

  const processHandle = {
    child,
    logs,
    stop: async () => {
      await stopProcess(child, logs)
    },
  }

  try {
    await waitForHttpOk(testEnv.uiBaseUrl, "Vite dev server", child, logs, 90_000)
    return processHandle
  } catch (error) {
    await processHandle.stop().catch(() => {})
    throw error
  }
}

async function seedBackend(baseUrl: string): Promise<UnifiedModelResponse> {
  await requestJson<Schema["SetupStatusResponse"]>(baseUrl, "/v1/setup/complete", {
    json: { initialized: true } satisfies Schema["CompleteSetupRequest"],
    method: "POST",
  })

  return requestJson<UnifiedModelResponse>(baseUrl, "/v1/models", {
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
}

async function sendAssistantMessage(page: Page, message: string): Promise<void> {
  await waitForComposerReady(page)
  const composer = page.getByPlaceholder(composerPlaceholder)
  await composer.fill(message)
  await page.getByRole("button", { name: sendButtonName }).click()
}

async function waitForComposerReady(page: Page): Promise<Locator> {
  const composer = page.getByPlaceholder(composerPlaceholder)
  await composer.waitFor({ state: "visible", timeout: 60_000 })
  await eventually("assistant composer is editable", async () => composer.isEditable())
  return composer
}

async function expectVisibleText(
  page: Page,
  text: string,
  options: { exact?: boolean } = {}
): Promise<void> {
  await page
    .getByText(text, { exact: options.exact ?? true })
    .first()
    .waitFor({ state: "visible", timeout: 60_000 })
}

async function waitForSessionNamed(name: string): Promise<SessionResponse> {
  return eventually("assistant session label persisted", async () => {
    const sessions = await listSessions(env.serverBaseUrl)
    return sessions.find((session) => session.name === name)
  })
}

async function waitForCurrentAssistantSession(
  predicate: (sessionId: string) => boolean
): Promise<string> {
  return eventually("assistant current session persisted", async () => {
    const state = await getPersistedUiState<{ currentSessionId?: string }>(
      env.serverBaseUrl,
      "zustand:assistant-ui"
    )
    const currentSessionId = state?.currentSessionId
    if (!currentSessionId || !predicate(currentSessionId)) {
      return null
    }
    const sessions = await listSessions(env.serverBaseUrl)
    return sessions.some((session) => session.id === currentSessionId) ? currentSessionId : null
  })
}

async function deleteSessionFromSheet(page: Page, session: SessionResponse): Promise<void> {
  await page.getByRole("button", { name: "Manage sessions" }).click()
  const dialog = page.getByRole("dialog", { name: "Manage sessions" })
  await dialog.waitFor({ state: "visible", timeout: 30_000 })

  const row = dialog.locator(".workspace-soft-panel", { hasText: session.name }).first()
  await row.waitFor({ state: "visible", timeout: 30_000 })
  await row.locator("button").last().click()
  await page.getByRole("menuitem", { name: "Delete" }).click()
}

async function restoreSession(baseUrl: string, sessionId: string): Promise<AgentSessionRestored> {
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

async function listSessions(baseUrl: string): Promise<SessionResponse[]> {
  return requestJson<SessionResponse[]>(baseUrl, "/v1/sessions")
}

async function getPersistedUiState<T>(baseUrl: string, key: string): Promise<T | null> {
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

async function requestJson<T>(
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

async function findFreePort(): Promise<number> {
  return new Promise((resolvePort, reject) => {
    const probe = createServer()
    probe.once("error", reject)
    probe.listen(0, "127.0.0.1", () => {
      const address = probe.address()
      if (!address || typeof address === "string") {
        probe.close(() => reject(new Error("Failed to allocate a TCP port.")))
        return
      }
      probe.close((error) => {
        if (error) {
          reject(error)
          return
        }
        resolvePort(address.port)
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
      if (logs.length > 240) {
        logs.shift()
      }
    }
  }

  child.stdout.on("data", remember)
  child.stderr.on("data", remember)
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
  stdinCommand?: string
): Promise<void> {
  if (child.exitCode !== null) {
    return
  }

  if (stdinCommand && child.stdin.writable) {
    child.stdin.write(`${stdinCommand}\n`)
  } else {
    child.kill("SIGTERM")
  }

  try {
    await waitForExit(child, 10_000)
  } catch {
    killProcessTree(child)
    await waitForExit(child, 10_000).catch(() => {
      throw new Error(`Failed to stop process ${child.pid}.${formatLogs(logs)}`)
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
    execFileSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], { stdio: "ignore" })
    return
  }

  child.kill("SIGKILL")
}

async function eventually<T>(
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

function formatLogs(logs: string[]): string {
  return logs.length > 0 ? `\nRecent output:\n${logs.join("\n")}` : ""
}

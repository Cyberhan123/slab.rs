import {
  execFileSync,
  spawn,
  spawnSync,
  type ChildProcessWithoutNullStreams,
} from "node:child_process"
import { createServer, Socket } from "node:net"
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { setTimeout as delay } from "node:timers/promises"

import { remote } from "webdriverio"

const setupSchemaUrl = "https://slab.reorgix.com/manifests/v1/settings-document.schema.json"
const supportDir = dirname(fileURLToPath(import.meta.url))
const packageRoot = resolve(supportDir, "../../..")
const repoRoot = resolve(packageRoot, "../..")
const persistentE2eRootDir = join(repoRoot, ".slab", "e2e-desktop")
const desktopApiPort = 3000
const desktopApiBaseUrl = `http://127.0.0.1:${desktopApiPort}`
const tauriDriverPort = 4444
const readinessTimeoutMs = 180_000
const processStopTimeoutMs = 12_000
const logRingLimit = 500

type JsonRequestInit = Omit<RequestInit, "body"> & {
  json?: unknown
}

export type DesktopWebDriverEnvironment = {
  appPath: string
  browser?: WebdriverIO.Browser
  driver?: ChildProcessWithoutNullStreams
  logs: string[]
  modelConfigDir: string
  pluginsDir: string
  rootDir: string
  serverBaseUrl: string
  sessionStateDir: string
  settingsPath: string
  workspaceRoot: string
}

export async function createDesktopWebDriverEnvironment(): Promise<DesktopWebDriverEnvironment> {
  mkdirSync(persistentE2eRootDir, { recursive: true })
  const rootDir = mkdtempSync(join(persistentE2eRootDir, "run-"))
  const settingsDir = join(rootDir, "config")
  const modelConfigDir = join(settingsDir, "models")
  const pluginsDir = join(rootDir, "plugins")
  const sessionStateDir = join(rootDir, "sessions")
  const workspaceRoot = join(rootDir, "workspace")
  const settingsPath = join(settingsDir, "settings.json")
  const databasePath = join(rootDir, "slab.db")

  for (const dir of [settingsDir, modelConfigDir, pluginsDir, sessionStateDir, workspaceRoot]) {
    mkdirSync(dir, { recursive: true })
  }

  mkdirSync(join(workspaceRoot, "src"), { recursive: true })
  writeFileSync(join(workspaceRoot, "README.md"), "# Desktop Workspace\n", "utf8")
  writeFileSync(join(workspaceRoot, "src", "main.rs"), "fn main() {}\n", "utf8")
  writeSettingsDocument(settingsPath, {
    databaseUrl: sqliteUrlForPath(databasePath),
    modelConfigDir,
    pluginsDir,
    serverBind: "127.0.0.1:3000",
    sessionStateDir,
  })

  return {
    appPath: desktopAppPath(),
    logs: [],
    modelConfigDir,
    pluginsDir,
    rootDir,
    serverBaseUrl: desktopApiBaseUrl,
    sessionStateDir,
    settingsPath,
    workspaceRoot,
  }
}

export async function startDesktopWebDriver(
  testEnv: DesktopWebDriverEnvironment,
): Promise<WebdriverIO.Browser> {
  const tauriDriverPath = requireCommand("tauri-driver")
  requirePlatformWebDriver()
  await assertTcpPortAvailable(desktopApiPort, "desktop slab-server")
  await assertTcpPortAvailable(tauriDriverPort, "tauri-driver")
  ensureDesktopAppBuilt(testEnv.appPath)

  const env: NodeJS.ProcessEnv = {
    ...process.env,
    BROWSER: "none",
    NO_COLOR: "1",
    SLAB_DATABASE_URL: sqliteUrlForPath(join(testEnv.rootDir, "slab.db")),
    SLAB_LOG: "info",
    SLAB_MODEL_CONFIG_DIR: testEnv.modelConfigDir,
    SLAB_PLUGINS_DIR: testEnv.pluginsDir,
    SLAB_SESSION_STATE_DIR: testEnv.sessionStateDir,
    SLAB_SETTINGS_PATH: testEnv.settingsPath,
    SLAB_WORKSPACE_ROOT: testEnv.workspaceRoot,
  }
  delete env.RUSTC_WRAPPER

  const driver = spawn(tauriDriverPath, [], {
    cwd: repoRoot,
    env,
    stdio: "pipe",
  })
  testEnv.driver = driver
  rememberOutput("tauri-driver", driver, testEnv.logs)
  await waitForTcpPort(tauriDriverPort, "tauri-driver", driver, testEnv.logs)

  const browser = await remote({
    hostname: "127.0.0.1",
    logLevel: "error",
    path: "/",
    port: tauriDriverPort,
    capabilities: {
      "tauri:options": {
        application: testEnv.appPath,
      },
    } as WebdriverIO.Capabilities,
  })
  testEnv.browser = browser

  await waitForHttpOk(`${desktopApiBaseUrl}/health`, "desktop slab-server", driver, testEnv.logs)
  await requestJson(desktopApiBaseUrl, "/v1/setup/complete", {
    json: { initialized: true },
    method: "POST",
  })
  await requestJson(desktopApiBaseUrl, "/v1/workspace/open", {
    json: { rootPath: testEnv.workspaceRoot },
    method: "POST",
  })
  await installBrowserErrorCapture(browser)

  return browser
}

export async function stopDesktopWebDriver(
  testEnv: DesktopWebDriverEnvironment | undefined,
): Promise<void> {
  await testEnv?.browser?.deleteSession().catch(() => {})
  await stopProcess(testEnv?.driver)
  if (testEnv?.rootDir) {
    rmSync(testEnv.rootDir, { force: true, recursive: true })
  }
}

export async function collectBrowserFailureMessages(browser: WebdriverIO.Browser): Promise<string[]> {
  const captured = await browser.execute(() => {
    const global = window as typeof window & {
      __SLAB_DESKTOP_E2E_ERRORS__?: string[]
    }
    // eslint-disable-next-line no-underscore-dangle -- shared browser-global capture contract
    return global.__SLAB_DESKTOP_E2E_ERRORS__ ?? []
  })

  const driverLogs = await browser
    .getLogs("browser")
    .then((logs) => logs.map((entry) => {
      if (entry && typeof entry === "object" && "message" in entry) {
        return String(entry.message)
      }
      return String(entry)
    }))
    .catch(() => [])

  return [...captured, ...driverLogs]
}

export function isWorkspaceDesktopFailure(message: string): boolean {
  return /ipc\.localhost|request_script_injection|failed to start workspace terminal|FileOperationError|Unable to resolve nonexistent file/i.test(message)
}

export async function eventually<T>(
  label: string,
  assertion: () => Promise<T | false | null | undefined> | T | false | null | undefined,
  timeoutMs = 60_000,
  intervalMs = 500,
): Promise<T> {
  const deadline = Date.now() + timeoutMs
  let lastError: unknown

  /* eslint-disable no-await-in-loop -- intentional sequential polling with
     backoff: each attempt must observe the previous result before retrying. */
  while (Date.now() < deadline) {
    try {
      const result = await assertion()
      if (result) {
        return result
      }
    } catch (error) {
      lastError = error
    }

    await delay(intervalMs)
  }
  /* eslint-enable no-await-in-loop */

  const suffix = lastError instanceof Error ? ` Last error: ${lastError.message}` : ""
  throw new Error(`${label} timed out after ${timeoutMs}ms.${suffix}`)
}

function ensureDesktopAppBuilt(appPath: string): void {
  execFileSync("bun", ["run", "build:app"], {
    cwd: repoRoot,
    env: { ...process.env, NO_COLOR: "1" },
    stdio: "inherit",
  })

  if (!existsSync(appPath)) {
    throw new Error(`Desktop app executable was not produced at ${appPath}.`)
  }
}

function desktopAppPath(): string {
  const executable = process.platform === "win32" ? "slab-app.exe" : "slab-app"
  return join(repoRoot, "target", "debug", executable)
}

function requirePlatformWebDriver(): void {
  if (process.platform === "win32") {
    requireCommand("msedgedriver")
    return
  }
  if (process.platform === "linux") {
    requireCommand("WebKitWebDriver")
    return
  }
  throw new Error("Desktop WebDriver E2E requires Windows msedgedriver or Linux WebKitWebDriver.")
}

function requireCommand(command: string): string {
  const lookupCommand = process.platform === "win32" ? "where.exe" : "which"
  const result = spawnSync(lookupCommand, [command], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  })
  const path = result.stdout.split(/\r?\n/).find(Boolean)
  if (result.status !== 0 || !path) {
    throw new Error(
      `Missing required desktop E2E dependency \`${command}\` on PATH. Install it before running bun run test:e2e:desktop.`,
    )
  }
  return path.trim()
}

async function assertTcpPortAvailable(port: number, label: string): Promise<void> {
  await new Promise<void>((resolveAvailable, reject) => {
    const probe = createServer()
    probe.once("error", () => {
      reject(new Error(`${label} port ${port} is already in use. Stop stale slab-server/slab-app/tauri-driver processes before running desktop E2E.`))
    })
    probe.listen(port, "127.0.0.1", () => {
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

async function waitForTcpPort(
  port: number,
  label: string,
  child: ChildProcessWithoutNullStreams,
  logs: string[],
): Promise<void> {
  await eventually(
    `${label} TCP listener`,
    async () => {
      if (child.exitCode !== null) {
        throw new Error(`${label} exited early with ${child.exitCode}.${formatLogs(logs)}`)
      }
      return canConnect(port)
    },
    readinessTimeoutMs,
  )
}

function canConnect(port: number): Promise<boolean> {
  return new Promise((resolveConnect) => {
    const socket = new Socket()
    socket.setTimeout(500)
    socket.once("connect", () => {
      socket.destroy()
      resolveConnect(true)
    })
    socket.once("timeout", () => {
      socket.destroy()
      resolveConnect(false)
    })
    socket.once("error", () => {
      socket.destroy()
      resolveConnect(false)
    })
    socket.connect(port, "127.0.0.1")
  })
}

async function waitForHttpOk(
  url: string,
  label: string,
  child: ChildProcessWithoutNullStreams,
  logs: string[],
): Promise<void> {
  await eventually(
    label,
    async () => {
      if (child.exitCode !== null) {
        throw new Error(`${label} exited early with ${child.exitCode}.${formatLogs(logs)}`)
      }
      const response = await fetch(url).catch(() => null)
      return response?.ok ? true : null
    },
    readinessTimeoutMs,
  )
}

async function requestJson<T>(
  baseUrl: string,
  path: string,
  init: JsonRequestInit = {},
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
  if (!response.ok) {
    throw new Error(`${init.method ?? "GET"} ${path} failed with ${response.status}: ${text}`)
  }
  return text ? (JSON.parse(text) as T) : (undefined as T)
}

async function installBrowserErrorCapture(browser: WebdriverIO.Browser): Promise<void> {
  await browser.execute(() => {
    const global = window as typeof window & {
      __SLAB_DESKTOP_E2E_ERRORS__?: string[]
      __SLAB_DESKTOP_E2E_CAPTURE_INSTALLED__?: boolean
    }
    /* eslint-disable no-underscore-dangle -- these __delimited__ names are a
       documented browser-global contract shared with the page under test. */
    if (global.__SLAB_DESKTOP_E2E_CAPTURE_INSTALLED__) {
      return
    }
    global.__SLAB_DESKTOP_E2E_CAPTURE_INSTALLED__ = true
    global.__SLAB_DESKTOP_E2E_ERRORS__ = []
    const push = (message: unknown) => {
      global.__SLAB_DESKTOP_E2E_ERRORS__?.push(String(message))
    }
    /* eslint-enable no-underscore-dangle */
    window.addEventListener("error", (event) => push(event.message))
    window.addEventListener("unhandledrejection", (event) => push(event.reason))
    document.addEventListener("securitypolicyviolation", (event) => {
      push(`${event.violatedDirective} ${event.blockedURI}`)
    })
    const originalError = console.error.bind(console)
    console.error = (...args: unknown[]) => {
      push(args.map(String).join(" "))
      originalError(...args)
    }
    localStorage.setItem("slab.ui.language", "en-US")
  })
}

async function stopProcess(child: ChildProcessWithoutNullStreams | undefined): Promise<void> {
  if (!child || child.exitCode !== null) {
    return
  }
  child.kill()
  const deadline = Date.now() + processStopTimeoutMs
  while (child.exitCode === null && Date.now() < deadline) {
    // eslint-disable-next-line no-await-in-loop -- poll until the child process exits
    await delay(100)
  }
  if (child.exitCode === null) {
    child.kill("SIGKILL")
  }
}

function rememberOutput(
  label: string,
  child: ChildProcessWithoutNullStreams,
  logs: string[],
): void {
  const remember = (chunk: Buffer) => {
    const text = chunk.toString("utf8")
    for (const line of text.split(/\r?\n/)) {
      if (!line.trim()) {
        continue
      }
      logs.push(`[${label}] ${line}`)
      if (logs.length > logRingLimit) {
        logs.splice(0, logs.length - logRingLimit)
      }
    }
  }
  child.stdout.on("data", remember)
  child.stderr.on("data", remember)
}

function formatLogs(logs: string[]): string {
  return logs.length ? `\nRecent output:\n${logs.join("\n")}` : ""
}

function sqliteUrlForPath(path: string): string {
  const normalized = path.replaceAll("\\", "/")
  return normalized.startsWith("/") ? `sqlite://${normalized}?mode=rwc` : `sqlite:///${normalized}?mode=rwc`
}

function writeSettingsDocument(
  path: string,
  options: {
    databaseUrl: string
    modelConfigDir: string
    pluginsDir: string
    serverBind: string
    sessionStateDir: string
  },
): void {
  writeFileSync(
    path,
    `${JSON.stringify(
      {
        $schema: setupSchemaUrl,
        schema_version: 2,
        agent: {
          debug: false,
          hooks: { enabled: false, scripts: [] },
          memories: { enabled: false },
          tools: {
            mcp: { enabled: false, servers: [] },
            websearch: { default_provider: "duckduckgo" },
          },
        },
        database: { url: options.databaseUrl },
        general: { language: "en-US" },
        logging: { json: false, level: "info" },
        models: { config_dir: options.modelConfigDir },
        plugin: { install_dir: options.pluginsDir },
        runtime: {
          mode: "managed_children",
          transport: "ipc",
          sessions: { state_dir: options.sessionStateDir },
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
          swagger: { enabled: true },
        },
        telemetry: { enabled: false },
        tools: {
          ffmpeg: { auto_download: false, enabled: false },
        },
      },
      null,
      2,
    )}\n`,
    "utf8",
  )
}

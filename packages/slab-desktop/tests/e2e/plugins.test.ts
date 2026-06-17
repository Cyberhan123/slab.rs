import { createHash } from "node:crypto"
import { writeFileSync } from "node:fs"
import { join } from "node:path"

import { chromium, type Browser, type BrowserContext, type Frame, type Page } from "playwright"
import { afterAll, beforeAll, describe, expect, it } from "vitest"

import {
  cleanupFullstackDevEnvironment,
  completeSetup,
  createFullstackDevEnvironment,
  eventually,
  startFullstackDev,
  type FullstackDevEnvironment,
  type ManagedDevProcess,
} from "./support/fullstack-dev"

const pluginId = "e2e-models-read"

let env: FullstackDevEnvironment | undefined

describe.sequential("plugins e2e", () => {
  let browser: Browser | undefined
  let context: BrowserContext | undefined
  let dev: ManagedDevProcess | undefined
  let page: Page
  let pluginPackPath: string

  beforeAll(async () => {
    env = await createFullstackDevEnvironment()
    pluginPackPath = writeE2ePluginPack(env.rootDir)

    dev = await startFullstackDev(env)
    await completeSetup(env.serverBaseUrl)

    browser = await chromium.launch({ headless: true })
    context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
    })
    await context.addInitScript(() => {
      window.localStorage.setItem("slab.ui.language", "en-US")
    })
    page = await context.newPage()
  })

  afterAll(async () => {
    await context?.close().catch(() => {})
    await browser?.close().catch(() => {})
    await dev?.stop().catch(() => {})
    cleanupFullstackDevEnvironment(env)
  })

  it("imports a browser plugin and enforces the plugin API bridge permissions", async () => {
    const testEnv = requireEnv()
    const browserEvents: string[] = []
    page.on("console", (message) => {
      browserEvents.push(`[console:${message.type()}] ${message.text()}`)
    })
    page.on("requestfailed", (request) => {
      if (request.url().includes(pluginId)) {
        browserEvents.push(`[requestfailed] ${request.url()} ${request.failure()?.errorText ?? ""}`)
      }
    })
    page.on("response", (response) => {
      if (response.url().includes(pluginId)) {
        browserEvents.push(`[response] ${response.status()} ${response.url()}`)
      }
    })

    await page.goto(`${testEnv.uiBaseUrl}/plugins`, {
      waitUntil: "domcontentloaded",
      timeout: 60_000,
    })
    await page.getByTestId("plugin-import-open-button").waitFor({ state: "visible", timeout: 60_000 })
    await page.getByTestId("plugin-import-open-button").click()
    await page.getByTestId("plugin-import-file-input").setInputFiles(pluginPackPath)
    await page.getByTestId("plugin-import-submit-button").click()

    await page.getByTestId(`plugin-card-${pluginId}`).waitFor({ state: "visible", timeout: 60_000 })
    await page.getByTestId(`sidebar-link-plugins-${pluginId}`).waitFor({
      state: "visible",
      timeout: 60_000,
    })
    await page.getByTestId(`sidebar-link-plugins-${pluginId}`).click()

    await page.getByTestId(`plugin-view-${pluginId}`).waitFor({ state: "visible", timeout: 60_000 })
    const pluginFrame = await waitForPluginFrame(page, browserEvents)

    await eventually("plugin bridge returns models", async () => {
      const text = await pluginFrame.getByTestId("plugin-models-status").textContent()
      return text?.startsWith("models ok") ? text : null
    })
    const modelsStatus = await pluginFrame.getByTestId("plugin-models-status").textContent()
    expect(modelsStatus).toMatch(/^models ok \d+$/)

    const deniedStatus = await eventually("plugin bridge rejects unauthorized API", async () => {
      const text = await pluginFrame.getByTestId("plugin-denied-status").textContent()
      return text?.includes("audio:transcribe") ? text : null
    })
    expect(deniedStatus).toContain("audio:transcribe")
  })
})

async function waitForPluginFrame(page: Page, browserEvents: string[]): Promise<Frame> {
  const iframe = page.getByTestId(`plugin-frame-${pluginId}`)
  await iframe.waitFor({ state: "attached", timeout: 60_000 })
  const iframeElement = await iframe.elementHandle()
  const pluginFrame = await iframeElement?.contentFrame()
  if (!pluginFrame) {
    throw new Error(`Plugin iframe did not expose a content frame. Frames: ${page.frames().map((frame) => frame.url()).join(", ")}`)
  }

  try {
    await pluginFrame.getByTestId("plugin-static").waitFor({ state: "visible", timeout: 60_000 })
  } catch (error) {
    const src = await iframe.getAttribute("src")
    let assetDebug = "iframe src is missing"
    if (src) {
      const response = await fetch(src)
      assetDebug = [
        `iframe src: ${src}`,
        `asset status: ${response.status}`,
        `asset csp: ${response.headers.get("content-security-policy") ?? "missing"}`,
        `asset body: ${(await response.text()).slice(0, 240)}`,
        `frame url: ${pluginFrame.url()}`,
        `frame body: ${(await pluginFrame.content()).slice(0, 240)}`,
        `all frames: ${page.frames().map((frame) => frame.url()).join(", ")}`,
        `browser events: ${browserEvents.join("\n")}`,
      ].join("\n")
    }
    throw new Error(`${error instanceof Error ? error.message : String(error)}\n${assetDebug}`)
  }

  return pluginFrame
}

function writeE2ePluginPack(rootDir: string): string {
  const htmlPath = "ui/index.html"
  const scriptPath = "ui/app.js"
  const html = `<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <title>E2E Models Read</title>
  </head>
  <body>
    <main>
      <h1 data-testid="plugin-static">E2E Models Read</h1>
      <p data-testid="plugin-models-status">models pending</p>
      <p data-testid="plugin-denied-status">denied pending</p>
    </main>
    <script src="./app.js" defer></script>
  </body>
</html>
`
  const script = `"use strict";

const HOST_SOURCE = "slab-plugin-host";
const SDK_SOURCE = "slab-plugin-sdk";
let nextRequestId = 0;
const pending = new Map();

window.addEventListener("message", (event) => {
  const message = event.data;
  if (!message || message.source !== HOST_SOURCE || message.type !== "api.response") {
    return;
  }

  const handlers = pending.get(message.id);
  if (!handlers) {
    return;
  }
  pending.delete(message.id);

  if (message.ok) {
    handlers.resolve(message.response);
    return;
  }
  handlers.reject(new Error(message.error || "Plugin API request failed"));
});

function requestSlabApi(method, path, body = null) {
  const id = String(++nextRequestId);
  const request = { method, path, headers: {}, body };
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    window.parent.postMessage({ source: SDK_SOURCE, type: "api.request", id, request }, "*");
    window.setTimeout(() => {
      if (!pending.has(id)) {
        return;
      }
      pending.delete(id);
      reject(new Error("Timed out waiting for plugin API response"));
    }, 15000);
  });
}

async function main() {
  const modelsStatus = document.querySelector("[data-testid='plugin-models-status']");
  const deniedStatus = document.querySelector("[data-testid='plugin-denied-status']");

  try {
    const response = await requestSlabApi("GET", "/v1/models");
    if (response.status !== 200) {
      throw new Error("GET /v1/models returned " + response.status);
    }
    const models = JSON.parse(response.body || "[]");
    modelsStatus.textContent = "models ok " + models.length;
  } catch (error) {
    modelsStatus.textContent = "models error " + (error && error.message ? error.message : String(error));
  }

  try {
    await requestSlabApi("POST", "/v1/audio/transcriptions");
    deniedStatus.textContent = "denied missing";
  } catch (error) {
    deniedStatus.textContent = "denied ok " + (error && error.message ? error.message : String(error));
  }
}

void main();
`
  const manifest = {
    manifestVersion: 1,
    id: pluginId,
    name: "E2E Models Read",
    version: "0.1.0",
    runtime: {
      ui: {
        entry: htmlPath,
      },
    },
    permissions: {
      network: {
        mode: "blocked",
        allowHosts: [],
      },
      ui: ["route:create", "sidebar:item:create"],
      slabApi: ["models:read"],
    },
    contributes: {
      routes: [
        {
          id: "main",
          path: "/plugins/e2e-models-read",
          title: "E2E Models Read",
        },
      ],
      sidebar: [
        {
          id: "main",
          label: "E2E Models",
          route: "main",
        },
      ],
    },
    integrity: {
      filesSha256: {
        [htmlPath]: sha256Hex(html),
        [scriptPath]: sha256Hex(script),
      },
    },
  }
  const packBytes = buildStoredZip({
    [`${pluginId}/plugin.json`]: `${JSON.stringify(manifest, null, 2)}\n`,
    [`${pluginId}/${htmlPath}`]: html,
    [`${pluginId}/${scriptPath}`]: script,
  })
  const packPath = join(rootDir, `${pluginId}.plugin.slab`)
  writeFileSync(packPath, packBytes)
  return packPath
}

function sha256Hex(content: string): string {
  return createHash("sha256").update(content).digest("hex")
}

function buildStoredZip(entries: Record<string, string>): Uint8Array {
  const localRecords: Buffer[] = []
  const centralRecords: Buffer[] = []
  let offset = 0

  for (const [path, content] of Object.entries(entries)) {
    const name = Buffer.from(path, "utf8")
    const data = Buffer.from(content, "utf8")
    const crc = crc32(data)
    const localHeader = Buffer.alloc(30)
    localHeader.writeUInt32LE(0x04034b50, 0)
    localHeader.writeUInt16LE(20, 4)
    localHeader.writeUInt16LE(0, 6)
    localHeader.writeUInt16LE(0, 8)
    localHeader.writeUInt16LE(0, 10)
    localHeader.writeUInt16LE(0, 12)
    localHeader.writeUInt32LE(crc, 14)
    localHeader.writeUInt32LE(data.byteLength, 18)
    localHeader.writeUInt32LE(data.byteLength, 22)
    localHeader.writeUInt16LE(name.byteLength, 26)
    localHeader.writeUInt16LE(0, 28)
    localRecords.push(localHeader, name, data)

    const centralHeader = Buffer.alloc(46)
    centralHeader.writeUInt32LE(0x02014b50, 0)
    centralHeader.writeUInt16LE(20, 4)
    centralHeader.writeUInt16LE(20, 6)
    centralHeader.writeUInt16LE(0, 8)
    centralHeader.writeUInt16LE(0, 10)
    centralHeader.writeUInt16LE(0, 12)
    centralHeader.writeUInt16LE(0, 14)
    centralHeader.writeUInt32LE(crc, 16)
    centralHeader.writeUInt32LE(data.byteLength, 20)
    centralHeader.writeUInt32LE(data.byteLength, 24)
    centralHeader.writeUInt16LE(name.byteLength, 28)
    centralHeader.writeUInt16LE(0, 30)
    centralHeader.writeUInt16LE(0, 32)
    centralHeader.writeUInt16LE(0, 34)
    centralHeader.writeUInt16LE(0, 36)
    centralHeader.writeUInt32LE(0, 38)
    centralHeader.writeUInt32LE(offset, 42)
    centralRecords.push(centralHeader, name)

    offset += localHeader.byteLength + name.byteLength + data.byteLength
  }

  const localBytes = Buffer.concat(localRecords)
  const centralBytes = Buffer.concat(centralRecords)
  const end = Buffer.alloc(22)
  end.writeUInt32LE(0x06054b50, 0)
  end.writeUInt16LE(0, 4)
  end.writeUInt16LE(0, 6)
  end.writeUInt16LE(Object.keys(entries).length, 8)
  end.writeUInt16LE(Object.keys(entries).length, 10)
  end.writeUInt32LE(centralBytes.byteLength, 12)
  end.writeUInt32LE(localBytes.byteLength, 16)
  end.writeUInt16LE(0, 20)

  return Buffer.concat([localBytes, centralBytes, end])
}

function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff
  for (const byte of bytes) {
    crc = CRC32_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8)
  }
  return (crc ^ 0xffffffff) >>> 0
}

const CRC32_TABLE = new Uint32Array(
  Array.from({ length: 256 }, (_, index) => {
    let value = index
    for (let bit = 0; bit < 8; bit += 1) {
      value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1
    }
    return value >>> 0
  })
)

function requireEnv(): FullstackDevEnvironment {
  if (!env) {
    throw new Error("Fullstack dev environment was not initialized.")
  }

  return env
}

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { defineConfig } from "@playwright/test";

const uiPort = 4173;
const apiPort = 3300;
const apiBaseUrl = `http://127.0.0.1:${apiPort}`;
const uiBaseUrl = `http://127.0.0.1:${uiPort}`;

const packageRoot = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(packageRoot, "../..");
const stateRoot = path.join(packageRoot, "node_modules", ".playwright-state");
const sessionStateDir = path.join(stateRoot, "sessions");
const modelConfigDir = path.join(stateRoot, "models");
const settingsPath = path.join(stateRoot, "settings.json");
const databasePath = path.join(stateRoot, "slab-e2e.db");
const libDir = path.join(repoRoot, "bin", "slab-app", "src-tauri", "resources", "libs");

for (const dir of [stateRoot, sessionStateDir, modelConfigDir]) {
  fs.mkdirSync(dir, { recursive: true });
}

process.env.PLAYWRIGHT_API_BASE_URL = apiBaseUrl;

function sqliteUrlForPath(filePath: string) {
  const normalized = filePath.replace(/\\/g, "/");
  return `sqlite:///${normalized}?mode=rwc`;
}

export default defineConfig({
  testDir: "./tests/e2e",
  timeout: 60_000,
  fullyParallel: false,
  workers: 1,
  expect: {
    timeout: 10_000,
  },
  reporter: [
    ["list"],
    [
      "html",
      {
        open: "never",
        outputFolder: "./node_modules/.playwright-report",
      },
    ],
  ],
  outputDir: "./node_modules/.playwright-artifacts",
  use: {
    baseURL: uiBaseUrl,
    trace: "on-first-retry",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  webServer: [
    {
      command: "bun run ./tests/e2e/start-slab-server.ts",
      cwd: packageRoot,
      url: `${apiBaseUrl}/v1/setup/status`,
      reuseExistingServer: false,
      timeout: 240_000,
      env: {
        ...process.env,
        SLAB_BIND: `127.0.0.1:${apiPort}`,
        SLAB_DATABASE_URL: sqliteUrlForPath(databasePath),
        SLAB_SETTINGS_PATH: settingsPath,
        SLAB_MODEL_CONFIG_DIR: modelConfigDir,
        SLAB_SESSION_STATE_DIR: sessionStateDir,
        SLAB_LIB_DIR: libDir,
        SLAB_LOG: "warn",
      },
    },
    {
      command: "bun run ./tests/e2e/start-vite.ts",
      cwd: packageRoot,
      url: uiBaseUrl,
      reuseExistingServer: false,
      timeout: 120_000,
      env: {
        ...process.env,
        SLAB_E2E_UI_PORT: String(uiPort),
        VITE_API_BASE_URL: apiBaseUrl,
      },
    },
  ],
});

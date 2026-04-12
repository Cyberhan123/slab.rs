import fs from "node:fs";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";

const scriptDir = path.dirname(import.meta.filename);
const packageRoot = path.resolve(scriptDir, "..", "..");
const repoRoot = path.resolve(packageRoot, "..", "..");
const targetDir = path.join(repoRoot, "target", "debug");
const isWindows = process.platform === "win32";
const serverExe = path.join(targetDir, isWindows ? "slab-server.exe" : "slab-server");
const runtimeExe = path.join(targetDir, isWindows ? "slab-runtime.exe" : "slab-runtime");

const bind = process.env.SLAB_BIND ?? "127.0.0.1:3300";
const settingsPath =
  process.env.SLAB_SETTINGS_PATH ??
  path.join(packageRoot, "node_modules", ".playwright-state", "settings.json");
const databaseUrl = process.env.SLAB_DATABASE_URL;
const libDir = process.env.SLAB_LIB_DIR;

if (!databaseUrl) {
  throw new Error("SLAB_DATABASE_URL is required for Playwright server startup.");
}

if (!libDir) {
  throw new Error("SLAB_LIB_DIR is required for Playwright server startup.");
}

for (const dir of [path.dirname(settingsPath), libDir]) {
  if (!fs.existsSync(dir)) {
    throw new Error(`Required path is missing: ${dir}`);
  }
}

fs.writeFileSync(
  settingsPath,
  JSON.stringify(
    {
      $schema: "https://slab.reorgix.com/manifests/v1/settings-document.schema.json",
      schema_version: 2,
      logging: {
        level: "warn",
        json: false,
      },
      runtime: {
        mode: "managed_children",
        transport: "ipc",
        ggml: {
          backends: {
            llama: {
              enabled: true,
            },
            whisper: {
              enabled: false,
            },
            diffusion: {
              enabled: false,
            },
          },
        },
        candle: {
          enabled: false,
        },
        onnx: {
          enabled: false,
        },
      },
      server: {
        address: bind,
        swagger: {
          enabled: true,
        },
      },
    },
    null,
    2,
  ),
  "utf8",
);

function runMakeTask(task: string) {
  const result = spawnSync("cargo", ["make", task], {
    cwd: repoRoot,
    stdio: "inherit",
    env: process.env,
  });

  if (result.status !== 0) {
    throw new Error(`cargo make ${task} failed with exit code ${result.status ?? "unknown"}`);
  }
}

if (!fs.existsSync(serverExe) || !fs.existsSync(runtimeExe)) {
  runMakeTask("build-runtime-dev");
  runMakeTask("build-server-dev");
}

if (!fs.existsSync(serverExe)) {
  throw new Error(`slab-server executable was not found at ${serverExe}`);
}

if (!fs.existsSync(runtimeExe)) {
  throw new Error(`slab-runtime executable was not found at ${runtimeExe}`);
}

const serverArgs = [
  "--shutdown-on-stdin-close",
  "--settings-path",
  settingsPath,
  "--database-url",
  databaseUrl,
  "--lib-dir",
  libDir,
];

const child = spawn(serverExe, serverArgs, {
  cwd: repoRoot,
  env: {
    ...process.env,
    SLAB_BIND: bind,
  },
  stdio: ["pipe", "inherit", "inherit"],
});

let shutdownRequested = false;

function requestShutdown() {
  if (shutdownRequested) {
    return;
  }

  shutdownRequested = true;
  child.stdin?.write("shutdown\n");

  setTimeout(() => {
    if (!child.killed) {
      child.kill();
    }
  }, 8_000).unref();
}

process.on("SIGINT", requestShutdown);
process.on("SIGTERM", requestShutdown);

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exit(code ?? 0);
});

import { execFileSync, spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { createServer } from "node:net";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const runtimeLibDir = resolve(repoRoot, "bin/slab-app/src-tauri/resources/libs");
const startupTimeoutMs = 120_000;
const serverBinaryName = globalThis.process.platform === "win32" ? "slab-server.exe" : "slab-server";
const serverBinaryPath = resolve(repoRoot, "target", "debug", serverBinaryName);

function sqliteUrlForPath(path: string): string {
  const normalized = path.replaceAll("\\", "/");
  return normalized.startsWith("/")
    ? `sqlite://${normalized}?mode=rwc`
    : `sqlite:///${normalized}?mode=rwc`;
}

async function findFreePort(): Promise<number> {
  return await new Promise((resolvePort, reject) => {
    const probe = createServer();
    probe.once("error", reject);
    probe.listen(0, "127.0.0.1", () => {
      const address = probe.address();
      if (!address || typeof address === "string") {
        probe.close(() => reject(new Error("Failed to allocate a TCP port for slab-server tests.")));
        return;
      }

      const { port } = address;
      probe.close((closeError) => {
        if (closeError) {
          reject(closeError);
          return;
        }
        resolvePort(port);
      });
    });
  });
}

function splitLines(chunk: string): string[] {
  return chunk.split(/\r?\n/).filter((line) => line.trim().length > 0);
}

function canUsePrebuiltBinary(): boolean {
  try {
    execFileSync(serverBinaryPath, ["--version"], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

function writeTestSettings(path: string, bindAddress: string): void {
  const settings = {
    $schema: "https://slab.reorgix.com/manifests/v1/settings-document.schema.json",
    schema_version: 2,
    runtime: {
      mode: "managed_children",
      ggml: {
        backends: {
          llama: { enabled: false },
          whisper: { enabled: false },
          diffusion: { enabled: false },
        },
      },
    },
    server: {
      address: bindAddress,
    },
  };

  writeFileSync(path, `${JSON.stringify(settings, null, 2)}\n`, "utf8");
}

async function killProcessTree(child: ChildProcessWithoutNullStreams): Promise<void> {
  if (child.exitCode !== null) {
    return;
  }

  if (globalThis.process.platform === "win32") {
    execFileSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], { stdio: "ignore" });
    return;
  }

  child.kill("SIGTERM");
}

export interface SlabServerTestHarnessOptions {
  adminToken?: string;
}

export interface JsonResponse<T> {
  response: Response;
  body: T;
}

export interface SlabServerTestHarness {
  readonly baseUrl: string;
  request(path: string, init?: RequestInit): Promise<Response>;
  requestJson<T>(path: string, init?: RequestInit): Promise<JsonResponse<T>>;
  stop(): Promise<void>;
}

export async function startSlabServerHarness(
  options: SlabServerTestHarnessOptions = {}
): Promise<SlabServerTestHarness> {
  const port = await findFreePort();
  const rootDir = mkdtempSync(join(tmpdir(), "slab-server-vitest-"));
  const settingsDir = join(rootDir, "config");
  const modelConfigDir = join(settingsDir, "models");
  const settingsPath = join(settingsDir, "settings.json");
  const databasePath = join(rootDir, "slab.db");
  const databaseUrl = sqliteUrlForPath(databasePath);
  const baseUrl = `http://127.0.0.1:${port}`;
  const bindAddress = `127.0.0.1:${port}`;
  const logLines: string[] = [];

  mkdirSync(modelConfigDir, { recursive: true });
  writeTestSettings(settingsPath, bindAddress);

  const prebuiltBinary = canUsePrebuiltBinary();
  const command = prebuiltBinary ? serverBinaryPath : "cargo";
  const args = prebuiltBinary
    ? [
        "--settings-path",
        settingsPath,
        "--database-url",
        databaseUrl,
        "--model-config-dir",
        modelConfigDir
      ]
    : [
        "run",
        "--bin",
        "slab-server",
        "--",
        "--settings-path",
        settingsPath,
        "--database-url",
        databaseUrl,
        "--model-config-dir",
        modelConfigDir
      ];

  const child = spawn(command, args, {
    cwd: repoRoot,
    env: {
      ...process.env,
      SLAB_BIND: bindAddress,
      SLAB_ADMIN_TOKEN: options.adminToken,
      SLAB_LIB_DIR: runtimeLibDir,
      SLAB_LOG: process.env.SLAB_LOG ?? "warn",
      SLAB_ENABLE_SWAGGER: "true",
      NO_COLOR: "1"
    },
    stdio: "pipe"
  });

  const rememberOutput = (chunk: Buffer) => {
    for (const line of splitLines(chunk.toString("utf8"))) {
      logLines.push(line);
      if (logLines.length > 200) {
        logLines.shift();
      }
    }
  };

  child.stdout.on("data", rememberOutput);
  child.stderr.on("data", rememberOutput);

  const describeLogs = () =>
    logLines.length > 0 ? `\nRecent slab-server output:\n${logLines.join("\n")}` : "";

  const stop = async () => {
    try {
      await killProcessTree(child);
    } catch {
      // Best effort cleanup. The temp directory removal below should not be
      // blocked if the process already exited between checks.
    } finally {
      rmSync(rootDir, { force: true, recursive: true });
    }
  };

  try {
    const deadline = Date.now() + startupTimeoutMs;

    while (Date.now() < deadline) {
      if (child.exitCode !== null) {
        throw new Error(
          `slab-server exited before becoming healthy (exit code ${child.exitCode}).${describeLogs()}`
        );
      }

      try {
        const response = await fetch(`${baseUrl}/health`);
        if (response.ok) {
          return {
            baseUrl,
            request: (path, init) => fetch(`${baseUrl}${path}`, init),
            async requestJson(path, init) {
              const response = await fetch(`${baseUrl}${path}`, init);
              const body = (await response.json()) as T;
              return { response, body };
            },
            stop
          };
        }
      } catch {
        // Keep polling until the server comes up or the process exits.
      }

      await new Promise((resolveDelay) => setTimeout(resolveDelay, 500));
    }

    throw new Error(
      `Timed out waiting ${startupTimeoutMs}ms for slab-server at ${baseUrl}.${describeLogs()}`
    );
  } catch (error) {
    await stop();
    throw error;
  }
}

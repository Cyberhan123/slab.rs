import { execFileSync, spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { randomUUID } from "node:crypto";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { createServer } from "node:net";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath } from "node:url";

import { cargoEnv } from "../../../../scripts/cargo/env";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../../..");
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
  externalBaseUrl?: string;
}

export interface JsonResponse<T> {
  response: Response;
  body: T;
}

export interface SeededModelDownloadTask {
  modelId: string;
  sourceKey: string;
  taskId: string;
}

export interface RecordedRequest {
  method: string;
  path: string;
}

export interface SlabServerTestHarness {
  readonly baseUrl: string;
  readonly databasePath?: string;
  recordRequest(path: string, init?: RequestInit | string): void;
  recordedRequests(): readonly RecordedRequest[];
  request(path: string, init?: RequestInit): Promise<Response>;
  requestFormData(path: string, body: FormData, init?: Omit<RequestInit, "body">): Promise<Response>;
  requestJson<T>(path: string, init?: RequestInit): Promise<JsonResponse<T>>;
  seedFailedModelDownloadTask(): Promise<SeededModelDownloadTask>;
  stop(): Promise<void>;
}

function requestMethod(init?: RequestInit | string): string {
  if (typeof init === "string") {
    return init.toUpperCase();
  }

  return (init?.method ?? "GET").toUpperCase();
}

function createRequestRecorder() {
  const requests: RecordedRequest[] = [];

  return {
    record(path: string, init?: RequestInit | string): void {
      requests.push({ method: requestMethod(init), path });
    },
    snapshot(): readonly RecordedRequest[] {
      return [...requests];
    }
  };
}

export async function startSlabServerHarness(
  options: SlabServerTestHarnessOptions = {}
): Promise<SlabServerTestHarness> {
  const externalBaseUrl = options.externalBaseUrl?.trim().replace(/\/+$/, "");
  const recorder = createRequestRecorder();
  if (externalBaseUrl) {
    return {
      baseUrl: externalBaseUrl,
      recordRequest: recorder.record,
      recordedRequests: recorder.snapshot,
      request(path, init) {
        recorder.record(path, init);
        return fetch(`${externalBaseUrl}${path}`, init);
      },
      requestFormData(path, body, init) {
        recorder.record(path, init);
        return fetch(`${externalBaseUrl}${path}`, { ...init, body });
      },
      async requestJson<T>(path: string, init?: RequestInit) {
        recorder.record(path, init);
        const response = await fetch(`${externalBaseUrl}${path}`, init);
        const body = (await response.json()) as T;
        return { response, body };
      },
      seedFailedModelDownloadTask: async () => {
        throw new Error("Model download task seeding is only available for the local test harness.");
      },
      stop: async () => {}
    };
  }

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
        "--config",
        "build.rustc-wrapper=\"\"",
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
  const childEnv: NodeJS.ProcessEnv = {
    ...cargoEnv(),
    SLAB_BIND: bindAddress,
    SLAB_ADMIN_TOKEN: options.adminToken,
    SLAB_LIB_DIR: runtimeLibDir,
    SLAB_LOG: process.env.SLAB_LOG ?? "warn",
    SLAB_ENABLE_SWAGGER: "true",
    NO_COLOR: "1"
  };
  childEnv.CARGO_BUILD_RUSTC_WRAPPER = "";
  delete childEnv.RUSTC_WRAPPER;

  const child = spawn(command, args, {
    cwd: repoRoot,
    env: childEnv,
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
        const healthResponse = await fetch(`${baseUrl}/health`);
        if (healthResponse.ok) {
          return {
            baseUrl,
            databasePath,
            recordRequest: recorder.record,
            recordedRequests: recorder.snapshot,
            request(path, init) {
              recorder.record(path, init);
              return fetch(`${baseUrl}${path}`, init);
            },
            requestFormData(path, body, init) {
              recorder.record(path, init);
              return fetch(`${baseUrl}${path}`, { ...init, body });
            },
            async requestJson<T>(path: string, init?: RequestInit) {
              recorder.record(path, init);
              const response = await fetch(`${baseUrl}${path}`, init);
              const body = (await response.json()) as T;
              return { response, body };
            },
            seedFailedModelDownloadTask: async () => seedFailedModelDownloadTask(databasePath),
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

function seedFailedModelDownloadTask(databasePath: string): SeededModelDownloadTask {
  const modelId = `smoke-download-model-${randomUUID()}`;
  const taskId = `smoke-download-task-${randomUUID()}`;
  const repoId = "slab/smoke-download";
  const filename = "smoke-model.gguf";
  const hubProvider = "hf_hub";
  const sourceKey = `hugging_face::${repoId}::${filename}`;
  const now = new Date().toISOString();
  const inputData = JSON.stringify({
    backend_id: "ggml.llama",
    candidates: [
      {
        artifacts: {
          model: filename
        },
        filename,
        hub_provider: hubProvider,
        primary_artifact_id: "model",
        repo_id: repoId,
        source_key: sourceKey
      }
    ],
    model_id: modelId
  });

  const db = new DatabaseSync(databasePath);
  try {
    db.exec("PRAGMA foreign_keys = ON");
    db.prepare(
      `INSERT INTO models (
        id, display_name, status, spec, runtime_presets, created_at, updated_at, kind, backend_id,
        config_schema_version, config_policy_version, capabilities, materialized_artifacts,
        selected_download_source
      ) VALUES (?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`
    ).run(
      modelId,
      "Smoke restart model",
      "not_downloaded",
      JSON.stringify({
        filename,
        hub_provider: hubProvider,
        repo_id: repoId
      }),
      now,
      now,
      "local",
      "ggml.llama",
      2,
      3,
      JSON.stringify(["text_generation"]),
      "{}"
    );
    db.prepare(
      `INSERT INTO tasks (
        id, core_task_id, model_id, task_type, status, input_data, result_data, error_msg,
        created_at, updated_at
      ) VALUES (?, NULL, ?, ?, ?, ?, NULL, ?, ?, ?)`
    ).run(taskId, modelId, "model_download", "failed", inputData, "seeded failed download", now, now);
    db.prepare(
      `INSERT INTO model_downloads (
        task_id, model_id, source_key, repo_id, filename, hub_provider, status, error_msg,
        created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
    ).run(
      taskId,
      modelId,
      sourceKey,
      repoId,
      filename,
      hubProvider,
      "failed",
      "seeded failed download",
      now,
      now
    );
  } finally {
    db.close();
  }

  return { modelId, sourceKey, taskId };
}

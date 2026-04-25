#!/usr/bin/env bun

import { spawn, spawnSync } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import openapiTS, { astToString } from "openapi-typescript";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const docsUrl = "http://127.0.0.1:3000/api-docs/openapi.json";
const outputPath = path.join(repoRoot, "packages", "api", "src", "v1.d.ts");
const settingsPath = path.join(repoRoot, "testdata", "api-settings.json");
const serverBinaryPath = path.join(
  repoRoot,
  "target",
  "debug",
  process.platform === "win32" ? "slab-server.exe" : "slab-server",
);

type ServerHandle = {
  logs: string[];
  process: ReturnType<typeof spawn>;
};

async function main() {
  let startedServer: ServerHandle | undefined;

  registerCleanup(() => stopServer(startedServer));

  if (!(await probeDocs())) {
    await runCommand("cargo", ["build", "-p", "slab-server"]);
    startedServer = startServer();
    await waitForDocs(startedServer);
  }

  const ast = await openapiTS(new URL(docsUrl));
  const rendered = astToString(ast).trimEnd();

  await mkdir(path.dirname(outputPath), { recursive: true });
  await writeFile(outputPath, `${rendered}\n`, "utf8");

  console.log(
    `Generated ${path.relative(repoRoot, outputPath).replace(/\\/g, "/")} from ${docsUrl}.`,
  );

  await stopServer(startedServer);
}

async function probeDocs() {
  try {
    const response = await fetch(docsUrl);
    return response.ok;
  } catch {
    return false;
  }
}

async function runCommand(command: string, args: string[]) {
  await new Promise<void>((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: repoRoot,
      env: process.env,
      shell: false,
      stdio: "inherit",
      windowsHide: true,
    });

    child.once("error", reject);
    child.once("exit", (code) => {
      if (code === 0) {
        resolve();
        return;
      }

      reject(new Error(`${command} ${args.join(" ")} exited with code ${code ?? "unknown"}.`));
    });
  });
}

function startServer(): ServerHandle {
  const logs: string[] = [];
  const child = spawn(serverBinaryPath, ["--settings-path", settingsPath], {
    cwd: repoRoot,
    env: process.env,
    shell: false,
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });

  child.stdout?.on("data", (chunk) => captureLogs(logs, chunk));
  child.stderr?.on("data", (chunk) => captureLogs(logs, chunk));

  return { process: child, logs };
}

function captureLogs(logs: string[], chunk: string | Buffer) {
  const text = chunk.toString().trim();
  if (!text) {
    return;
  }

  logs.push(text);
  if (logs.length > 40) {
    logs.splice(0, logs.length - 40);
  }
}

async function waitForDocs(server: ServerHandle, timeoutMs = 10_000) {
  await pollForDocs(server, Date.now(), timeoutMs);
}

async function pollForDocs(server: ServerHandle, startedAt: number, timeoutMs: number): Promise<void> {
  if (await probeDocs()) {
    return;
  }

  if (server.process.exitCode === null && Date.now() - startedAt < timeoutMs) {
    await sleep(200);
    await pollForDocs(server, startedAt, timeoutMs);
    return;
  }

  const recentLogs =
    server.logs.length > 0
      ? `\nRecent slab-server logs:\n${server.logs.map((line) => `  ${line}`).join("\n")}`
      : "";

  throw new Error(`slab-server did not expose /api-docs/openapi.json in time.${recentLogs}`);
}

async function stopServer(server: ServerHandle | undefined) {
  if (!server || server.process.exitCode !== null) {
    return;
  }

  if (process.platform === "win32") {
    spawnSync("taskkill", ["/PID", String(server.process.pid), "/T", "/F"], {
      cwd: repoRoot,
      stdio: "ignore",
      windowsHide: true,
    });
  } else {
    server.process.kill("SIGTERM");
  }

  await waitForExit(server.process, 5_000);
}

async function waitForExit(processHandle: ServerHandle["process"], timeoutMs: number) {
  if (processHandle.exitCode !== null) {
    return;
  }

  await Promise.race([
    new Promise<void>((resolve) => {
      processHandle.once("exit", () => resolve());
    }),
    sleep(timeoutMs),
  ]);
}

function registerCleanup(cleanup: () => Promise<void>) {
  let cleanedUp = false;

  const runCleanup = async () => {
    if (cleanedUp) {
      return;
    }
    cleanedUp = true;
    await cleanup();
  };

  for (const signal of ["SIGINT", "SIGTERM"] as const) {
    process.on(signal, () => {
      void runCleanup().finally(() => {
        process.exit(signal === "SIGINT" ? 130 : 143);
      });
    });
  }

  process.on("exit", () => {
    if (!cleanedUp) {
      void cleanup();
    }
  });
}

function sleep(timeoutMs: number) {
  return new Promise<void>((resolve) => {
    setTimeout(resolve, timeoutMs);
  });
}

try {
  await main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}

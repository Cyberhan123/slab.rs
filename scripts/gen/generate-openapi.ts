#!/usr/bin/env bun

import { spawn, spawnSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import openapiTS, { astToString } from "openapi-typescript";
import ts from "typescript";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const docsPath = "/api-docs/openapi.json";
const outputPath = path.join(repoRoot, "packages", "api", "src", "v1.d.ts");
const settingsTemplatePath = path.join(repoRoot, "testdata", "api-settings.json");
const serverBinaryPath = path.join(
  repoRoot,
  "target",
  "debug",
  process.platform === "win32" ? "slab-server.exe" : "slab-server",
);

type ServerHandle = {
  docsUrl: string;
  logs: string[];
  process: ReturnType<typeof spawn>;
};

type GenerationContext = {
  bindAddress: string;
  databaseUrl: string;
  docsUrl: string;
  settingsPath: string;
  tempDir: string;
};

type ApiSettingsTemplate = {
  database: {
    url: string;
  };
  server: {
    address: string;
  };
};

async function main() {
  let generationContext: GenerationContext | undefined;
  let startedServer: ServerHandle | undefined;

  const cleanup = async () => {
    await stopServer(startedServer);
    startedServer = undefined;
    await removeGenerationContext(generationContext);
    generationContext = undefined;
  };

  registerSignalCleanup(cleanup);

  try {
    await runCommand("cargo", ["build", "-p", "slab-server"]);
    generationContext = await createGenerationContext();
    startedServer = startServer(generationContext);
    await waitForDocs(startedServer);

    const ast = await openapiTS(new URL(generationContext.docsUrl), {
      transform(schemaObject) {
        if (schemaObject.type === "string" && schemaObject.format === "binary") {
          return ts.factory.createTypeReferenceNode("Blob");
        }

        return undefined;
      },
    });
    const rendered = astToString(ast).trimEnd();

    await mkdir(path.dirname(outputPath), { recursive: true });
    await writeFile(outputPath, `${rendered}\n`, "utf8");

    console.log(
      `Generated ${path.relative(repoRoot, outputPath).replace(/\\/g, "/")} from ${
        generationContext.docsUrl
      }.`,
    );
  } finally {
    await cleanup();
  }
}

async function createGenerationContext(): Promise<GenerationContext> {
  const port = await findFreePort();
  const bindAddress = `127.0.0.1:${port}`;
  const docsUrl = `http://${bindAddress}${docsPath}`;
  const tempDir = await mkdtemp(path.join(tmpdir(), "slab-openapi-"));
  const settingsPath = path.join(tempDir, "api-settings.json");
  const databasePath = path.join(tempDir, "slab.db").replaceAll("\\", "/");
  const databaseUrl = databasePath.startsWith("/")
    ? `sqlite://${databasePath}?mode=rwc`
    : `sqlite:///${databasePath}?mode=rwc`;
  const settings = JSON.parse(
    await readFile(settingsTemplatePath, "utf8"),
  ) as ApiSettingsTemplate;

  settings.server.address = bindAddress;
  settings.database.url = databaseUrl;
  await writeFile(settingsPath, `${JSON.stringify(settings, null, 2)}\n`, "utf8");

  return { bindAddress, databaseUrl, docsUrl, settingsPath, tempDir };
}

async function findFreePort(): Promise<number> {
  return await new Promise((resolvePort, reject) => {
    const probe = createServer();
    probe.once("error", reject);
    probe.listen(0, "127.0.0.1", () => {
      const address = probe.address();
      if (!address || typeof address === "string") {
        probe.close(() => reject(new Error("Failed to allocate a TCP port for slab-server.")));
        return;
      }

      probe.close(() => resolvePort(address.port));
    });
  });
}

async function removeGenerationContext(context: GenerationContext | undefined) {
  if (!context) {
    return;
  }

  await rm(context.tempDir, { force: true, recursive: true });
}

async function probeDocs(docsUrl: string) {
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

function startServer(context: GenerationContext): ServerHandle {
  const logs: string[] = [];
  const child = spawn(serverBinaryPath, ["--settings-path", context.settingsPath], {
    cwd: repoRoot,
    env: {
      ...process.env,
      SLAB_BIND: context.bindAddress,
      SLAB_DATABASE_URL: context.databaseUrl,
      SLAB_ENABLE_SWAGGER: "true",
    },
    shell: false,
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });

  child.stdout?.on("data", (chunk) => captureLogs(logs, chunk));
  child.stderr?.on("data", (chunk) => captureLogs(logs, chunk));

  return { docsUrl: context.docsUrl, process: child, logs };
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
  if (await probeDocs(server.docsUrl)) {
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

function registerSignalCleanup(cleanup: () => Promise<void>) {
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

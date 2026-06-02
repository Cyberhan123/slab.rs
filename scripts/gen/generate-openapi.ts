#!/usr/bin/env bun

import { spawn } from "node:child_process";
import { copyFile, mkdir, mkdtemp, readdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import openapiTS, { astToString } from "openapi-typescript";
import ts from "typescript";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const outputPath = path.join(repoRoot, "packages", "api", "src", "v1.d.ts");
const pythonSdkRoot = path.join(repoRoot, "python", "slab-python-sdk");
const pythonClientOutputPath = path.join(pythonSdkRoot, "src", "slab_api_client");
const pythonClientConfigPath = path.join(pythonSdkRoot, "openapi-python-client.yaml");
const OPENAPI_PYTHON_CLIENT_VERSION = "0.28.4";
const PYTHON_STRING_MAP_SCHEMA = "SlabStringMap";
const nativeRuntimeLibDir = path.join(
  repoRoot,
  "bin",
  "slab-app",
  "src-tauri",
  "resources",
  "libs",
);

async function main() {
  const serverBinaryPath = await ensureServerBinary();
  const openapi = JSON.parse(
    await runCommandCapture(serverBinaryPath, ["--print-openapi"], {
      env: withNativeRuntimeLibraryPath(process.env),
    }),
  );

  const ast = await openapiTS(openapi, {
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
  await generatePythonClient(openapi);

  console.log(
    `Generated ${path.relative(repoRoot, outputPath).replace(/\\/g, "/")} from slab-server --print-openapi.`,
  );
  console.log(
    `Generated ${path.relative(repoRoot, pythonClientOutputPath).replace(/\\/g, "/")} from slab-server --print-openapi.`,
  );
}

async function ensureServerBinary() {
  const serverBinaryPath = path.join(
    repoRoot,
    "target",
    "debug",
    process.platform === "win32" ? "slab-server.exe" : "slab-server",
  );
  try {
    await runCommand("bazelisk", ["run", "//tools/cargo:build_sidecars"]);
  } catch {
    await runCommand("cargo", ["build", "-p", "slab-server"]);
  }
  return serverBinaryPath;
}

async function generatePythonClient(openapi: unknown) {
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "slab-openapi-python-"));
  try {
    const openapiPath = path.join(tempRoot, "openapi.json");
    const generatedPath = path.join(tempRoot, "slab_api_client");
    await writeFile(
      openapiPath,
      `${JSON.stringify(normalizeOpenApiForPythonClient(openapi), null, 2)}\n`,
      "utf8",
    );
    await runCommand("uvx", [
      "--from",
      `openapi-python-client==${OPENAPI_PYTHON_CLIENT_VERSION}`,
      "openapi-python-client",
      "generate",
      "--path",
      openapiPath,
      "--output-path",
      generatedPath,
      "--config",
      pythonClientConfigPath,
      "--meta",
      "none",
      "--overwrite",
    ]);
    await replaceGeneratedPythonClient(generatedPath, pythonClientOutputPath);
  } finally {
    await rm(tempRoot, { force: true, recursive: true });
  }
}

function normalizeOpenApiForPythonClient(value: unknown): unknown {
  const normalized = normalizeOpenApiNode(value);
  if (asRecord(normalized)) {
    const root = normalized as Record<string, unknown>;
    const components = asRecord(root.components) ?? {};
    const schemas = asRecord(components.schemas) ?? {};
    schemas[PYTHON_STRING_MAP_SCHEMA] = {
      type: "object",
      additionalProperties: { type: "string" },
    };
    components.schemas = schemas;
    root.components = components;
  }
  return normalized;
}

function normalizeOpenApiNode(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(normalizeOpenApiNode);
  }
  if (!value || typeof value !== "object") {
    return value;
  }

  const input = value as Record<string, unknown>;
  if (isStringMapSchema(input)) {
    return { $ref: `#/components/schemas/${PYTHON_STRING_MAP_SCHEMA}` };
  }

  const output: Record<string, unknown> = {};
  for (const [key, child] of Object.entries(input)) {
    if (key === "propertyNames") {
      continue;
    }
    output[key] = normalizeOpenApiNode(child);
  }
  return output;
}

function isStringMapSchema(value: Record<string, unknown>): boolean {
  if (value.type !== "object") {
    return false;
  }
  if ("properties" in value) {
    return false;
  }
  const additionalProperties = asRecord(value.additionalProperties);
  return additionalProperties?.type === "string";
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return undefined;
  }
  return value as Record<string, unknown>;
}

async function replaceGeneratedPythonClient(sourceDir: string, destDir: string) {
  await mkdir(destDir, { recursive: true });
  await Promise.all(
    ["api", "models", "__init__.py", "client.py", "errors.py", "types.py"].map((entry) =>
      rm(path.join(destDir, entry), { force: true, recursive: true }),
    ),
  );
  await copyDirectory(sourceDir, destDir);
}

async function copyDirectory(sourceDir: string, destDir: string) {
  const rows = await readdir(sourceDir, { withFileTypes: true });
  await Promise.all(
    rows.map(async (row) => {
      if (row.name === ".ruff_cache" || row.name === "__pycache__") {
        return;
      }

      const sourcePath = path.join(sourceDir, row.name);
      const destPath = path.join(destDir, row.name);
      if (row.isDirectory()) {
        await mkdir(destPath, { recursive: true });
        await copyDirectory(sourcePath, destPath);
        return;
      }
      if (row.isFile()) {
        await mkdir(path.dirname(destPath), { recursive: true });
        await copyFile(sourcePath, destPath);
      }
    }),
  );
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

function withNativeRuntimeLibraryPath(env: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
  if (process.platform !== "win32") {
    return env;
  }

  return {
    ...env,
    PATH: [nativeRuntimeLibDir, env.PATH].filter(Boolean).join(path.delimiter),
  };
}

async function runCommandCapture(
  command: string,
  args: string[],
  options: { env?: NodeJS.ProcessEnv } = {},
) {
  return await new Promise<string>((resolve, reject) => {
    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];
    const child = spawn(command, args, {
      cwd: repoRoot,
      env: options.env ?? process.env,
      shell: false,
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });

    child.stdout?.on("data", (chunk) => stdout.push(Buffer.from(chunk)));
    child.stderr?.on("data", (chunk) => stderr.push(Buffer.from(chunk)));
    child.once("error", reject);
    child.once("exit", (code) => {
      if (code === 0) {
        resolve(Buffer.concat(stdout).toString("utf8"));
        return;
      }

      const stderrText = Buffer.concat(stderr).toString("utf8").trim();
      reject(
        new Error(
          `${command} ${args.join(" ")} exited with code ${code ?? "unknown"}${
            stderrText ? `\n${stderrText}` : ""
          }`,
        ),
      );
    });
  });
}


try {
  await main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}

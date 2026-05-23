#!/usr/bin/env bun

import { spawn } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import openapiTS, { astToString } from "openapi-typescript";
import ts from "typescript";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const outputPath = path.join(repoRoot, "packages", "api", "src", "v1.d.ts");
const serverBinaryPath = path.join(
  repoRoot,
  "target",
  "debug",
  process.platform === "win32" ? "slab-server.exe" : "slab-server",
);

async function main() {
  await runCommand("cargo", ["build", "-p", "slab-server"]);
  const openapi = JSON.parse(await runCommandCapture(serverBinaryPath, ["--print-openapi"]));

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

  console.log(
    `Generated ${path.relative(repoRoot, outputPath).replace(/\\/g, "/")} from slab-server --print-openapi.`,
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

async function runCommandCapture(command: string, args: string[]) {
  return await new Promise<string>((resolve, reject) => {
    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];
    const child = spawn(command, args, {
      cwd: repoRoot,
      env: process.env,
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

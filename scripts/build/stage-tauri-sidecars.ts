#!/usr/bin/env bun

import { spawn } from "node:child_process";
import { chmod, copyFile, mkdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const targetDir = path.resolve(repoRoot, process.env.CARGO_TARGET_DIR ?? "target");
const tauriBinariesDir = path.join(repoRoot, "bin", "slab-app", "src-tauri", "binaries");
const sidecars = ["slab-server", "slab-runtime", "slab-js-runtime", "slab-python-runtime"];

async function main() {
  const profile = parseProfile(process.argv.slice(2));
  const explicitTarget = process.env.CARGO_BUILD_TARGET;
  const target = explicitTarget ?? (await hostTarget());
  const profileDir = explicitTarget
    ? path.join(targetDir, explicitTarget, profile)
    : path.join(targetDir, profile);
  const extension = target.includes("windows") ? ".exe" : "";

  await mkdir(tauriBinariesDir, { recursive: true });

  for (const sidecar of sidecars) {
    const sourcePath = path.join(profileDir, `${sidecar}${extension}`);
    const destPath = path.join(tauriBinariesDir, `${sidecar}-${target}${extension}`);
    await copyFile(sourcePath, destPath);
    if (!target.includes("windows")) {
      await chmod(destPath, 0o755);
    }
    console.log(`Staged ${path.relative(repoRoot, destPath).replace(/\\/g, "/")}`);
  }
}

function parseProfile(argv: string[]) {
  let profile = "debug";

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = argv[index + 1];

    if (argument !== "--profile") {
      throw new Error(`Unknown argument: ${argument}`);
    }
    if (value !== "debug" && value !== "release") {
      throw new Error("--profile must be debug or release.");
    }

    profile = value;
    index += 1;
  }

  return profile;
}

async function hostTarget() {
  const output = await runCommandCapture("rustc", ["-vV"]);
  const hostLine = output
    .split(/\r?\n/)
    .find((line) => line.startsWith("host: "));
  if (!hostLine) {
    throw new Error("Unable to determine Rust host target from rustc -vV.");
  }

  return hostLine.slice("host: ".length).trim();
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

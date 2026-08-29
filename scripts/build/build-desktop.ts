#!/usr/bin/env bun

// One-command desktop build.
//
// Windows: produces the full offline installer (NSIS setup + runtime payload
// CABs wrapped by slab-windows-full-installer) at
// `target/release/bundle/nsis/Slab_<version>_x64-offline-setup.exe`.
// The bare `Slab_<version>_x64-setup.exe` in the same directory is the
// resource-less inner NSIS payload — installing it directly yields an app
// without the ggml runtime libraries.
//
// Non-Windows: falls back to the plain unbundled debug binary (the historical
// `build:desktop` behavior), matching `build:desktop:debug`.

import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");

function run(command: string, args: string[]) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    env: process.env,
    shell: false,
    stdio: "inherit",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} exited with code ${result.status ?? "unknown"}`);
  }
}

function main() {
  if (process.platform !== "win32") {
    console.log(`build:desktop: non-Windows platform, building unbundled debug binary
(installer bundling is Windows-only; see bun run build:desktop:debug)`);
    run("bun", ["run", "build:desktop:debug"]);
    return;
  }

  run("bun", ["run", "build:windows-installer"]);
  run("bun", ["./scripts/cargo/run.ts", "build", "--release", "-p", "slab-windows-full-installer"]);

  const bootstrapExe = path.join(repoRoot, "target", "release", "slab-windows-full-installer.exe");
  if (!existsSync(bootstrapExe)) {
    throw new Error(`slab-windows-full-installer was not built at ${bootstrapExe}`);
  }
  run(bootstrapExe, ["pack"]);

  const bundleDir = path.join(repoRoot, "target", "release", "bundle", "nsis");
  const offlineInstaller = readdirSync(bundleDir)
    .filter((name) => /^Slab_.*_x64-offline-setup\.exe$/.test(name))
    .map((name) => ({ name, mtime: readdirStatMtime(path.join(bundleDir, name)) }))
    .sort((left, right) => right.mtime - left.mtime)[0]?.name;
  if (!offlineInstaller) {
    throw new Error(`pack completed but no Slab_*_x64-offline-setup.exe found under ${bundleDir}`);
  }

  console.log(`\nOffline installer ready: ${path.join(bundleDir, offlineInstaller)}`);
  console.log("Install with THIS file (it carries the ggml runtime payloads).");
}

function readdirStatMtime(file: string): number {
  return statSync(file).mtimeMs;
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}

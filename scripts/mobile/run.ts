#!/usr/bin/env bun

/**
 * Thin wrapper that invokes the Flutter CLI inside `flutter/slab-mobile` so
 * the root `package.json` stays the canonical entrypoint for every workflow
 * (same spawn pattern as `scripts/cargo/run.ts` / `gen/generate-harness-bindings.ts`).
 *
 * Usage: bun ./scripts/mobile/run.ts <flutter args...>
 *   bun run dev:mobile   → run
 *   bun run check:mobile → analyze
 *   bun run test:mobile  → test
 */

import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const mobileDir = path.join(repoRoot, "flutter", "slab-mobile");

const args = process.argv.slice(2);
if (args.length === 0) {
  console.error("usage: bun ./scripts/mobile/run.ts <flutter args...>");
  process.exit(1);
}

function run(command: string, argv: string[]): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, argv, {
      cwd: mobileDir,
      stdio: "inherit",
      shell: process.platform === "win32",
    });
    child.on("exit", (code) =>
      code === 0
        ? resolve()
        : reject(new Error(`${command} ${argv.join(" ")} exited with ${code}`)),
    );
    child.on("error", reject);
  });
}

try {
  await run("flutter", args);
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`flutter failed: ${message}`);
  if (/ENOENT|not recognized|not found/i.test(message)) {
    console.error(
      "The Flutter SDK was not found. Install it (https://docs.flutter.dev/get-started/install) " +
        "and ensure `flutter` is on PATH. The pinned SDK version is recorded in flutter/slab-mobile/README.md.",
    );
  }
  process.exit(1);
}

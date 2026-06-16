#!/usr/bin/env bun

import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { cargoEnv } from "./env";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const args = process.argv.slice(2);

if (args.length === 0) {
  console.error("Usage: bun ./scripts/cargo/run.ts <cargo-args...>");
  process.exit(1);
}

const result = spawnSync("cargo", args, {
  cwd: repoRoot,
  env: cargoEnv(),
  shell: false,
  stdio: "inherit",
});

if (result.error) {
  throw result.error;
}
if (result.status !== 0) {
  throw new Error(`cargo ${args.join(" ")} exited with code ${result.status ?? "unknown"}`);
}

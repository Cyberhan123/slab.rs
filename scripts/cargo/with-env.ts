#!/usr/bin/env bun

import { spawnSync } from "node:child_process";

import { cargoEnv } from "./env";

const args = process.argv.slice(2);

if (args.length === 0) {
  console.error("Usage: bun ./scripts/cargo/with-env.ts <command> [args...]");
  process.exit(1);
}

const result = spawnSync(args[0], args.slice(1), {
  env: cargoEnv(),
  shell: false,
  stdio: "inherit",
});

if (result.error) {
  throw result.error;
}
if (result.status !== 0) {
  throw new Error(`${args[0]} ${args.slice(1).join(" ")} exited with code ${result.status ?? "unknown"}`);
}

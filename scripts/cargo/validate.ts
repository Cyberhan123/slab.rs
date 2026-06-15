#!/usr/bin/env bun

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");

type Mode = "check" | "lint" | "lint-rustc" | "clippy" | "test" | "sandbox";

const mode = parseMode(process.argv[2]);

try {
  if (mode === "lint") {
    runCargo("lint-rustc", cargoEnv(true));
    runCargo("clippy", cargoEnv(false));
  } else {
    runCargo(mode, cargoEnv(mode === "lint-rustc"));
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}

function parseMode(value: string | undefined): Mode {
  switch (value) {
    case "check":
    case "lint":
    case "lint-rustc":
    case "clippy":
    case "test":
    case "sandbox":
      return value;
    default:
      throw new Error(
        "Usage: bun ./scripts/cargo/validate.ts <check|lint|lint-rustc|clippy|test|sandbox>",
      );
  }
}

function cargoEnv(rustWarningsAsErrors: boolean) {
  const env = { ...process.env };
  env.TAURI_CONFIG = tauriConfigWithDisabledExternalBins(env.TAURI_CONFIG);
  if (process.platform === "darwin") {
    env.RUSTFLAGS = env.RUSTFLAGS
      ? `${env.RUSTFLAGS} -C link-arg=-lc++`
      : "-C link-arg=-lc++";
    for (const libDir of ["/opt/homebrew/lib", "/usr/local/lib"]) {
      if (existsSync(libDir)) {
        env.RUSTFLAGS = env.RUSTFLAGS
          ? `${env.RUSTFLAGS} -L native=${libDir}`
          : `-L native=${libDir}`;
      }
    }
  }
  if (rustWarningsAsErrors) {
    env.RUSTFLAGS = env.RUSTFLAGS ? `${env.RUSTFLAGS} -D warnings` : "-D warnings";
  }
  return env;
}

function tauriConfigWithDisabledExternalBins(existing: string | undefined) {
  const config = existing ? parseJsonObject(existing, "TAURI_CONFIG") : {};
  const bundle = isPlainObject(config.bundle) ? config.bundle : {};
  config.bundle = { ...bundle, externalBin: null };
  return JSON.stringify(config);
}

function parseJsonObject(value: string, name: string) {
  let parsed: unknown;
  try {
    parsed = JSON.parse(value);
  } catch (error) {
    throw new Error(`${name} must contain valid JSON: ${String(error)}`, { cause: error });
  }
  if (!isPlainObject(parsed)) {
    throw new Error(`${name} must be a JSON object.`);
  }
  return { ...parsed };
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function runCargo(cargoMode: Exclude<Mode, "lint">, env: NodeJS.ProcessEnv) {
  const args = cargoArgs(cargoMode);
  const result = spawnSync("cargo", args, {
    cwd: repoRoot,
    env,
    shell: false,
    stdio: "inherit",
  });

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`cargo ${args.join(" ")} exited with code ${result.status ?? "unknown"}`);
  }
}

function cargoArgs(cargoMode: Exclude<Mode, "lint">) {
  switch (cargoMode) {
    case "check":
      return ["check", "--workspace", "--all-targets", "--color=never"];
    case "lint-rustc":
      return ["check", "--workspace", "--all-targets", "--color=never"];
    case "clippy":
      return ["clippy", "--workspace", "--all-targets", "--color=never", "--", "-D", "warnings"];
    case "test":
      return ["test", "--workspace", "--color=never"];
    case "sandbox":
      return ["test", "-p", "slab-sandboxing", "--color=never"];
  }
}

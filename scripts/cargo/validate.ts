#!/usr/bin/env bun

import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { cargoEnv } from "./env";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");

type Mode = "check" | "lint" | "lint-rustc" | "clippy" | "test" | "sandbox";

const mode = parseMode(process.argv[2]);

try {
  if (mode === "lint") {
    runCargo("lint-rustc", cargoEnv({ disableTauriExternalBins: true, rustWarningsAsErrors: true }));
    runCargo("clippy", cargoEnv({ disableTauriExternalBins: true }));
  } else {
    runCargo(
      mode,
      cargoEnv({
        disableTauriExternalBins: true,
        rustWarningsAsErrors: mode === "lint-rustc",
      }),
    );
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

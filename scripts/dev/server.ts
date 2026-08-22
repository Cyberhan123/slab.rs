#!/usr/bin/env bun

/**
 * Runs the headless `slab-server` gateway for non-desktop clients (mobile,
 * browser, remote tooling). Builds the server plus its sibling sidecars
 * (`slab-runtime`, `slab-js-runtime` — slab-server resolves those next to its
 * own exe, so they must share `target/debug/`) and then execs the binary in
 * the foreground. Cargo incremental makes an up-to-date build a near no-op.
 * Default bind is `127.0.0.1:3000` (settings overridable).
 *
 * Usage: bun run dev:server [-- <slab-server args...>]
 *   e.g. bun run dev:server -- --gateway-bind 0.0.0.0:3000
 */

import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { cargoEnv } from "../cargo/env";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const serverBin = path.join(
  repoRoot,
  "target",
  "debug",
  process.platform === "win32" ? "slab-server.exe" : "slab-server",
);

const serverArgs = process.argv.slice(2);
if (serverArgs[0] === "--") {
  serverArgs.shift();
}

// A bare target/debug run resolves vendor DLLs from ./resources/libs, which
// only exists in the packaged layout. Default to the staged payload the
// Tauri host and test:e2e use, unless the caller picked a lib dir themselves.
if (!serverArgs.includes("--lib-dir")) {
  serverArgs.push("--lib-dir", path.join(repoRoot, "bin/slab-app/src-tauri/resources/libs"));
}

const build = spawnSync(
  "cargo",
  ["build", "-p", "slab-server", "-p", "slab-runtime", "-p", "slab-js-runtime"],
  {
    cwd: repoRoot,
    env: cargoEnv(),
    stdio: "inherit",
  },
);

if (build.error) {
  throw build.error;
}
if (build.status !== 0) {
  console.error(`cargo build exited with code ${build.status ?? "unknown"}`);
  process.exit(1);
}
if (!existsSync(serverBin)) {
  console.error(`slab-server binary not found at ${serverBin}`);
  process.exit(1);
}

const server = spawn(serverBin, serverArgs, { cwd: repoRoot, stdio: "inherit" });

for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"] as const) {
  process.on(signal, () => {
    if (server.exitCode === null) {
      server.kill(signal);
    }
  });
}

server.on("error", (error) => {
  console.error(`slab-server failed to start: ${error.message}`);
  process.exit(1);
});

server.on("exit", (code) => {
  process.exit(code ?? 0);
});

#!/usr/bin/env bun

/**
 * Mobile dev stack in one command: starts the headless slab-server
 * (`scripts/dev/server.ts`, which builds it first) in the background, waits
 * until it is reachable, then runs `flutter run` in the foreground. The server
 * is killed when flutter exits. Assumes an Android emulator/device is already
 * attached (`flutter devices`); from the emulator the default bind
 * `127.0.0.1:3000` is reachable as `http://10.0.2.2:3000`.
 *
 * Usage: bun run dev:mobile [-- <flutter run args...>]
 *   e.g. bun run dev:mobile -- --dart-define=SLAB_API_BASE_URL=http://10.0.2.2:3000
 */

import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const mobileDir = path.join(repoRoot, "flutter", "slab-mobile");

// Best-effort readiness probe against the default bind; if the server is
// configured to bind elsewhere this gives up and flutter launches anyway.
const healthUrl = "http://127.0.0.1:3000/health";
const healthTimeoutMs = 60_000;

const extraArgs = process.argv.slice(2);
if (extraArgs[0] === "--") {
  extraArgs.shift();
}
const flutterArgs = ["run", ...extraArgs];

console.log("[dev:mobile] starting slab-server (background)...");
const server = spawn(process.execPath, [path.join(scriptDir, "..", "dev", "server.ts")], {
  cwd: repoRoot,
  stdio: ["ignore", "inherit", "inherit"],
});
server.on("exit", (code) => {
  console.log(`[dev:mobile] slab-server exited with code ${code ?? "unknown"}`);
});

const serverReady = await waitForServer(server);
if (!serverReady) {
  console.warn(
    `[dev:mobile] slab-server not reachable at ${healthUrl} yet — continuing anyway. ` +
      "If it binds a non-default address, pass --dart-define=SLAB_API_BASE_URL to flutter.",
  );
}

console.log(`[dev:mobile] running flutter ${flutterArgs.join(" ")}...`);
const flutter = spawn("flutter", flutterArgs, {
  cwd: mobileDir,
  stdio: "inherit",
  shell: process.platform === "win32",
});

for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"] as const) {
  process.on(signal, () => {
    killTree(flutter);
    killTree(server);
    process.exit(0);
  });
}

flutter.on("error", (error) => {
  console.error(`[dev:mobile] flutter failed to start: ${error.message}`);
  killTree(server);
  process.exit(1);
});

flutter.on("exit", (code) => {
  console.log("[dev:mobile] flutter exited — stopping slab-server");
  killTree(server);
  process.exit(code ?? 0);
});

async function waitForServer(serverProcess: ChildProcess): Promise<boolean> {
  const deadline = Date.now() + healthTimeoutMs;
  while (Date.now() < deadline) {
    if (serverProcess.exitCode !== null) {
      return false;
    }
    try {
      // eslint-disable-next-line no-await-in-loop
      const response = await fetch(healthUrl, { signal: AbortSignal.timeout(1_500) });
      if (response.ok) {
        return true;
      }
    } catch {
      // Not listening yet — keep polling while the build/server warms up.
    }
    // eslint-disable-next-line no-await-in-loop
    await sleep(1_000);
  }
  return false;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// taskkill /t walks the whole tree: slab-server supervises slab-runtime /
// slab-js-runtime children that must not be orphaned.
function killTree(child: ChildProcess): void {
  if (child.pid === undefined || child.exitCode !== null) {
    return;
  }
  if (process.platform === "win32") {
    try {
      execFileSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], { stdio: "ignore" });
    } catch {
      // Process already gone — nothing to clean up.
    }
    return;
  }
  child.kill("SIGTERM");
}

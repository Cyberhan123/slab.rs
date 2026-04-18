import path from "node:path";

import spawn from "cross-spawn";

const scriptDir = path.dirname(import.meta.filename);
const packageRoot = path.resolve(scriptDir, "..", "..");
const uiPort = process.env.SLAB_E2E_UI_PORT ?? "4173";

const child = spawn("bun", ["x", "vite", "--host", "127.0.0.1", "--port", uiPort], {
  cwd: packageRoot,
  env: process.env,
  stdio: "inherit",
});

let shutdownRequested = false;

function requestShutdown() {
  if (shutdownRequested) {
    return;
  }

  shutdownRequested = true;
  child.kill("SIGTERM");

  setTimeout(() => {
    if (!child.killed) {
      child.kill();
    }
  }, 8_000).unref();
}

process.on("SIGINT", requestShutdown);
process.on("SIGTERM", requestShutdown);

child.on("exit", (code: number | null, signal: NodeJS.Signals | null) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exit(code ?? 0);
});

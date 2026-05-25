import process from "node:process";

const serverArgs = globalThis.__SLAB_LSP_ARGS__ ?? [];
process.argv = ["slab-js-runtime", "typescript-language-server", ...serverArgs];
globalThis.process = process;

await import("typescript-language-server/lib/cli.mjs");

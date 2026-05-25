import process from "node:process";

const serverArgs = globalThis.__SLAB_LSP_ARGS__ ?? [];
process.argv = ["slab-js-runtime", "vscode-json-language-server", ...serverArgs];
globalThis.process = process;

await import("vscode-langservers-extracted/lib/json-language-server/node/jsonServerMain.js");

import process from "node:process";

const serverArgs = globalThis.__SLAB_LSP_ARGS__ ?? [];
process.argv = ["slab-js-runtime", "vscode-html-language-server", ...serverArgs];
globalThis.process = process;

await import("vscode-langservers-extracted/lib/html-language-server/node/htmlServerMain.js");

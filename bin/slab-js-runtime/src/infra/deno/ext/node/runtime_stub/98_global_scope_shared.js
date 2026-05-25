import { core } from "ext:core/mod.js";

const { Console } = core.loadExtScript("ext:deno_web/01_console.js");
const consoleValue = globalThis.console ??
  new Console((msg, level) => core.print(msg, level > 1));

export const windowOrWorkerGlobalScope = {
  console: {
    __proto__: null,
    value: consoleValue,
    enumerable: false,
    writable: true,
    configurable: true,
  },
};

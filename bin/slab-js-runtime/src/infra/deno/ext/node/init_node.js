import { core, internals } from "ext:core/mod.js";
import process from "node:process";

const { initializeDebugEnv } = core.loadExtScript("ext:deno_node/internal/util/debuglog.ts");
initializeDebugEnv("rustyscript");

if (internals.__bootstrapNodeProcess) {
  internals.__bootstrapNodeProcess(undefined, [], {}, "rustyscript", true);
}

globalThis.process ??= process;

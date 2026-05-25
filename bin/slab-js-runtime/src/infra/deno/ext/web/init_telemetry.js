import { core } from "ext:core/mod.js";

const telemetry = core.loadExtScript("ext:deno_telemetry/telemetry.ts");
core.loadExtScript("ext:deno_telemetry/util.ts");

globalThis.Deno.telemetry = telemetry.telemetry;

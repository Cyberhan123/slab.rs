import { core } from "ext:core/mod.js";

const consoleModule = core.loadExtScript("ext:deno_web/01_console.js");
const console = new consoleModule.Console((msg, level) =>
  core.print(msg, level > 1)
);

for (const name of [
  "log",
  "debug",
  "info",
  "warn",
  "error",
  "dir",
  "dirxml",
  "assert",
  "clear",
  "count",
  "countReset",
  "group",
  "groupCollapsed",
  "groupEnd",
  "table",
  "time",
  "timeEnd",
  "timeLog",
  "trace",
]) {
  const value = console[name];
  if (typeof value === "function") {
    console[name] = value.bind(console);
  }
}

const windowOrWorkerGlobalScope = {
  console: core.propNonEnumerable(console),
};
const unstableForWindowOrWorkerGlobalScope = { __proto__: null };

export { unstableForWindowOrWorkerGlobalScope, windowOrWorkerGlobalScope };

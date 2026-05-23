import * as _console from 'ext:deno_web/01_console.js';

import { applyToGlobal, nonEnumerable } from 'ext:rustyscript/rustyscript.js';

const consoleInstance = new _console.Console((msg, level) =>
  globalThis.Deno.core.print(msg, level > 1),
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
  const value = consoleInstance[name];
  if (typeof value === "function") {
    consoleInstance[name] = value.bind(consoleInstance);
  }
}
applyToGlobal({
  console: nonEnumerable(consoleInstance),
});

globalThis.Deno.inspect = _console.inspect;

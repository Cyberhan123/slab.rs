import { core } from "ext:core/mod.js";
const webStorage = core.loadExtScript("ext:deno_webstorage/01_webstorage.js");

import { applyToGlobal, getterOnly, nonEnumerable } from 'ext:rustyscript/rustyscript.js';
applyToGlobal({
    Storage: nonEnumerable(webStorage.Storage),
    sessionStorage: getterOnly(webStorage.sessionStorage),
    localStorage: getterOnly(webStorage.localStorage),
});

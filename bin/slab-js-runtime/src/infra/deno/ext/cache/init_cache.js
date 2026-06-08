import { core } from "ext:core/mod.js";
const caches = core.loadExtScript("ext:deno_cache/01_cache.js");

import { applyToGlobal, nonEnumerable } from 'ext:rustyscript/rustyscript.js';
applyToGlobal({
    caches: {
        enumerable: true,
        configurable: true,
        get: caches.cacheStorage,
    },
    CacheStorage: nonEnumerable(caches.CacheStorage),
    Cache: nonEnumerable(caches.Cache),
});

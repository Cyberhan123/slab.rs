#!/usr/bin/env bun

/**
 * Exports the slab-i18n catalogs (TypeScript modules — bun imports them
 * natively, no build step) into flat JSON catalogs for the Flutter mobile app.
 * Because both outputs derive from the same source in the same commit, mobile
 * strings can never drift from web/desktop.
 *
 * Output (committed, deterministic; regenerate with `bun run gen:mobile`):
 *   flutter/slab-mobile/assets/i18n/{en-US,zh-CN}.json
 *   — deep-flattened dot keys over the `common` / `layouts` / `pages`
 *     namespaces, preserving i18next `{{var}}` placeholders. The `server`
 *     namespace is a server-field translation map, not a UI catalog — skipped.
 *
 * Usage: bun ./scripts/i18n/export-mobile-locales.ts [--check]
 *   --check regenerates in memory and diffs against disk (exit 1 on drift).
 */

import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { enUS } from "../../packages/slab-i18n/src/locales/en-US";
import { zhCN } from "../../packages/slab-i18n/src/locales/zh-CN";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const OUT_DIR = path.join(repoRoot, "flutter/slab-mobile/assets/i18n");
const LOCALES = [
  { tag: "en-US", catalog: enUS },
  { tag: "zh-CN", catalog: zhCN },
] as const;
const NAMESPACES = ["common", "layouts", "pages"] as const;
const CHECK = process.argv.includes("--check");

type Catalog = Record<string, unknown>;

/** Deep-flatten one namespace object to `namespace.a.b` → string entries. */
function flatten(prefix: string, value: unknown, out: Map<string, string>): void {
  if (typeof value === "string") {
    out.set(prefix, value);
    return;
  }
  // Non-string leaves (label arrays, option objects) are not translated strings — skip.
  if (value === null || typeof value !== "object" || Array.isArray(value)) return;
  for (const [key, child] of Object.entries(value as Catalog)) {
    flatten(`${prefix}.${key}`, child, out);
  }
}

const flattened = LOCALES.map(({ tag, catalog }) => {
  const entries = new Map<string, string>();
  for (const namespace of NAMESPACES) {
    flatten(namespace, (catalog as Catalog)[namespace], entries);
  }
  return { tag, entries };
});

// Key parity between locales is already guarded on the TS side
// (locale-parity.test.ts); assert it here too so the JSON cannot diverge.
const [enKeys, zhKeys] = flattened.map(({ entries }) => [...entries.keys()].toSorted());
if (enKeys.length !== zhKeys.length || enKeys.some((key, i) => key !== zhKeys[i])) {
  const enSet = new Set(enKeys);
  const zhSet = new Set(zhKeys);
  const onlyEn = enKeys.filter((key) => !zhSet.has(key));
  const onlyZh = zhKeys.filter((key) => !enSet.has(key));
  throw new Error(
    `locale key parity broken:\n  only en-US: ${onlyEn.join(", ") || "<none>"}\n  only zh-CN: ${onlyZh.join(", ") || "<none>"}`,
  );
}

const artifacts = flattened.map(({ tag, entries }) => {
  const outPath = path.join(OUT_DIR, `${tag}.json`);
  const doc = Object.fromEntries([...entries.keys()].toSorted().map((key) => [key, entries.get(key)]));
  return { tag, outPath, text: `${JSON.stringify(doc, null, 2)}\n`, count: entries.size };
});

if (CHECK) {
  const drift = (
    await Promise.all(
      artifacts.map(async ({ outPath, text }) => {
        const actual = await readFile(outPath, "utf8").catch(() => null);
        return actual === text ? null : path.relative(repoRoot, outPath).replaceAll("\\", "/");
      }),
    )
  ).filter((entry): entry is string => entry !== null);
  if (drift.length > 0) {
    console.error(`locale drift detected — regenerate with \`bun run gen:mobile\`:\n  ${drift.join("\n  ")}`);
    process.exit(1);
  }
} else {
  await mkdir(OUT_DIR, { recursive: true });
  await Promise.all(artifacts.map(({ outPath, text }) => writeFile(outPath, text, "utf8")));
  for (const { tag, count } of artifacts) {
    console.log(`Exported ${count} keys → flutter/slab-mobile/assets/i18n/${tag}.json`);
  }
}

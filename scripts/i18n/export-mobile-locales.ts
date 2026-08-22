#!/usr/bin/env bun

/**
 * Exports the slab-i18n catalogs (TypeScript modules — bun imports them
 * natively, no build step) into the Flutter mobile app. Because both outputs
 * derive from the same source in the same commit, mobile strings can never
 * drift from web/desktop.
 *
 * Output (committed, deterministic; regenerate with `bun run gen:mobile`):
 *   flutter/slab-mobile/lib/core/l10n/arb/app_{en,zh}.arb
 *   — ARB interchange format for translation tooling: deep-flattened dot
 *     keys verbatim (gen-l10n itself cannot consume them — its keys must be
 *     Dart identifiers — so `flutter gen-l10n` is NOT part of the build),
 *     i18next `{{var}}` placeholders preserved, `@@locale` metadata only.
 *   flutter/slab-mobile/lib/core/l10n/catalog_entries.g.dart
 *   — the runtime source: `const Map<String, String>` per locale consumed
 *     by SlabCatalog (keeps the `catalog.t(key)` string-key API, including
 *     the runtime-downloaded `server.*` namespace that error envelopes use).
 *
 * Both carry deep-flattened dot keys over the `common` / `layouts` /
 * `pages` / `server` namespaces.
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
const ARB_DIR = path.join(repoRoot, "flutter/slab-mobile/lib/core/l10n/arb");
const ENTRIES_OUT = path.join(
  repoRoot,
  "flutter/slab-mobile/lib/core/l10n/catalog_entries.g.dart",
);
const LOCALES = [
  { tag: "en-US", dartConst: "slabCatalogEnUs", arbName: "app_en" },
  { tag: "zh-CN", dartConst: "slabCatalogZhCn", arbName: "app_zh" },
] as const;
const NAMESPACES = ["common", "layouts", "pages", "server"] as const;
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

/** Escape a value for a single-quoted Dart string literal. */
function dartString(value: string): string {
  return value
    .replace(/\\/g, "\\\\")
    .replace(/'/g, "\\'")
    .replace(/\$/g, "\\$")
    .replace(/\r/g, "\\r")
    .replace(/\n/g, "\\n");
}

const flattened = LOCALES.map(({ tag }, index) => {
  const catalog = index === 0 ? enUS : zhCN;
  const entries = new Map<string, string>();
  for (const namespace of NAMESPACES) {
    flatten(namespace, (catalog as Catalog)[namespace], entries);
  }
  // Keys must stay single-quoted-Dart-safe — they are emitted verbatim.
  for (const key of entries.keys()) {
    if (/[\\']/.test(key)) {
      throw new Error(`catalog key contains a Dart-literal-unsafe character: ${key}`);
    }
  }
  return { tag, entries };
});

// Key parity between locales is already guarded on the TS side
// (locale-parity.test.ts); assert it here too so the outputs cannot diverge.
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

function arbText(locale: string, entries: Map<string, string>): string {
  // @@locale first (insertion order is preserved); no per-key @-metadata —
  // gen-l10n never consumes these files, metadata would only bloat them.
  const doc: Record<string, string> = { "@@locale": locale };
  for (const key of [...entries.keys()].toSorted()) doc[key] = entries.get(key)!;
  return `${JSON.stringify(doc, null, 2)}\n`;
}

function entriesText(catalogs: { tag: string; entries: Map<string, string> }[]): string {
  const maps = catalogs
    .map((_locale, index) => {
      const { tag, dartConst } = LOCALES[index];
      const { entries } = catalogs[index];
      const lines = [...entries.keys()]
        .toSorted()
        .map((key) => `  '${key}': '${dartString(entries.get(key)!)}',`);
      return `/// Flat ${tag} catalog (dot keys, i18next {{var}} placeholders).\nconst Map<String, String> ${dartConst} = <String, String>{\n${lines.join("\n")}\n};`;
    })
    .join("\n\n");
  return [
    "// GENERATED by `bun run gen:mobile` — do not edit.",
    "// Source: packages/slab-i18n TS catalogs via scripts/i18n/export-mobile-locales.ts.",
    "",
    maps,
    "",
  ].join("\n");
}

const artifacts = [
  ...flattened.map(({ entries }, index) => ({
    label: `flutter/slab-mobile/lib/core/l10n/arb/${LOCALES[index].arbName}.arb`,
    outPath: path.join(ARB_DIR, `${LOCALES[index].arbName}.arb`),
    text: arbText(index === 0 ? "en" : "zh", entries),
    count: entries.size,
  })),
  {
    label: "flutter/slab-mobile/lib/core/l10n/catalog_entries.g.dart",
    outPath: ENTRIES_OUT,
    text: entriesText(flattened),
    count: flattened[0].entries.size,
  },
];

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
  await Promise.all(
    [ARB_DIR, path.dirname(ENTRIES_OUT)].map((dir) => mkdir(dir, { recursive: true })),
  );
  await Promise.all(artifacts.map(({ outPath, text }) => writeFile(outPath, text, "utf8")));
  for (const { label, count } of artifacts) {
    console.log(`Exported ${count} keys → ${label}`);
  }
}

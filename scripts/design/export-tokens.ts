#!/usr/bin/env bun

/**
 * Exports the design tokens from `packages/slab-components/src/styles/globals.css`
 * into the Flutter mobile app — the one-way pipeline that keeps slab-mobile
 * visually unified with web/desktop without sharing UI code.
 *
 * Outputs (both committed, both deterministic; regenerate with `bun run gen:mobile`):
 *   packages/slab-mobile/design/tokens.json          — inspectable intermediate
 *                                                      (keeps the raw oklch strings for a
 *                                                      future wide-gamut upgrade)
 *   packages/slab-mobile/lib/theme/slab_tokens.g.dart — `SlabTokensLight/Dark` + `SlabMetrics`
 *
 * Color conversion mirrors the browser family of algorithms: oklch →
 * `culori.clampChroma` (chroma-reducing gamut clip) → sRGB hex. Compound CSS
 * values (box shadows, color-mix) are emitted as `{"raw": ...}` with
 * `"dart": false`; the Flutter theme supplies native equivalents.
 *
 * Usage: bun ./scripts/design/export-tokens.ts [--check]
 *   --check regenerates in memory and diffs against disk (exit 1 on drift).
 */

import { clampChroma, converter, formatHex8, parse } from "culori";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const GLOBALS_PATH = path.join(
  repoRoot,
  "packages/slab-components/src/styles/globals.css",
);
const JSON_OUT = path.join(repoRoot, "packages/slab-mobile/design/tokens.json");
const DART_OUT = path.join(
  repoRoot,
  "packages/slab-mobile/lib/theme/slab_tokens.g.dart",
);
const REM_TO_PX = 16;
const CHECK = process.argv.includes("--check");

// ── CSS parsing ─────────────────────────────────────────────────────────────

/** Strip block comments first — prose in comments must not fool selector matching. */
function stripComments(css: string): string {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

/**
 * Extract the balanced `{ ... }` body of the block whose selector matches
 * `selectorRe`. The regex must anchor the selector to a following `{` (e.g.
 * `\.dark\s*\{`) so mentions inside comments, `@custom-variant`, or nested
 * selectors cannot match ahead of the real block.
 */
function extractBlock(css: string, selectorRe: RegExp): string {
  const match = selectorRe.exec(css);
  if (match === null) throw new Error(`selector ${selectorRe} not found in globals.css`);
  const open = css.indexOf("{", match.index);
  let depth = 0;
  for (let i = open; i < css.length; i += 1) {
    if (css[i] === "{") depth += 1;
    else if (css[i] === "}") {
      depth -= 1;
      if (depth === 0) return css.slice(open + 1, i);
    }
  }
  throw new Error(`unbalanced block for ${selectorRe}`);
}

type Decl = { name: string; value: string };

function parseDecls(block: string): Decl[] {
  const out: Decl[] = [];
  for (const match of block.matchAll(/(--[\w-]+)\s*:\s*([^;{}]+);/g)) {
    out.push({ name: match[1], value: match[2].trim() });
  }
  return out;
}

/** Resolve `var(--x)` references against `scope` (with :root fallback). */
function resolveVars(value: string, scope: Map<string, string>, root: Map<string, string>): string {
  let out = value;
  for (let i = 0; i < 8; i += 1) {
    const next = out.replace(
      /var\((--[\w-]+)\)/g,
      (_, name: string) => scope.get(name) ?? root.get(name) ?? "",
    );
    if (next === out) break;
    out = next;
  }
  return out.trim();
}

// ── Token classification ────────────────────────────────────────────────────

type ColorToken = { raw: string; argb: string };
type RawToken = { raw: string; dart: false };
type Metric = number | string | string[] | number[];

type ModeTokens = { colors: Map<string, ColorToken>; raws: Map<string, RawToken> };

function toArgb(colorValue: string): string | null {
  const color = parse(colorValue);
  if (color === null) return null;
  const rgb = converter("rgb")(clampChroma(color));
  if (rgb === null || Number.isNaN(rgb.r) || Number.isNaN(rgb.g) || Number.isNaN(rgb.b)) {
    return null;
  }
  // formatHex8 emits #RRGGBBAA; Dart's Color() takes 0xAARRGGBB.
  const hex8 = formatHex8(rgb);
  return `#${hex8.slice(7, 9)}${hex8.slice(1, 7)}`.toUpperCase();
}

/** Kebab-case CSS name → lowerCamelCase Dart field (`--chart-1` → `chart1`). */
function camelName(name: string): string {
  const stripped = name.replace(/^--/, "").replace(/^spacing-/, "");
  const parts = stripped.split("-");
  return parts
    .map((part, index) => (index === 0 ? part : part.charAt(0).toUpperCase() + part.slice(1)))
    .join("");
}

function round(value: number): number {
  return Math.round(value * 100) / 100;
}

// ── Main pipeline ───────────────────────────────────────────────────────────

const css = stripComments(await readFile(GLOBALS_PATH, "utf8"));
const rootDecls = parseDecls(extractBlock(css, /:root\s*\{/));
const darkDecls = parseDecls(extractBlock(css, /\.dark\s*\{/));
const themeDecls = parseDecls(extractBlock(css, /@theme\s*\{/));
const themeInlineDecls = parseDecls(extractBlock(css, /@theme inline\s*\{/));

const rootScope = new Map(rootDecls.map((d) => [d.name, d.value]));
const darkScope = new Map(darkDecls.map((d) => [d.name, d.value]));

/** Compound CSS values that cannot be compiled to a single Dart constant. */
const RAW_ONLY = new Set([
  "--elevation-1",
  "--elevation-2",
  "--elevation-3",
  "--glass-bg",
  "--glass-bg-strong",
  "--glass-border",
  "--glass-highlight",
]);

function isColorValue(resolved: string): boolean {
  return /^oklch\(/i.test(resolved);
}

/**
 * Merge the declaration list for one mode: `.dark` overrides inherit their
 * position from the `:root` entry (CSS cascade — dark only redefines a subset).
 */
function mergedDecls(overrides: Map<string, string>): Map<string, string> {
  const merged = new Map(rootScope);
  for (const [name, value] of overrides) merged.set(name, value);
  return merged;
}

function buildMode(decls: Map<string, string>, scope: Map<string, string>): ModeTokens {
  const colors = new Map<string, ColorToken>();
  const raws = new Map<string, RawToken>();
  for (const [name, value] of decls) {
    const resolved = resolveVars(value, scope, rootScope);
    if (isColorValue(resolved) && !RAW_ONLY.has(name)) {
      const argb = toArgb(resolved);
      if (argb) {
        colors.set(name, { raw: resolved, argb });
        continue;
      }
    }
    if (RAW_ONLY.has(name)) {
      raws.set(name, { raw: resolved, dart: false });
      continue;
    }
    // Non-color scalars (`--radius`, `--glass-blur`, `--ease-out-expo`) are
    // mode-independent — handled by the metrics pass below.
  }
  return { colors, raws };
}

const light = buildMode(mergedDecls(new Map()), rootScope);
const dark = buildMode(mergedDecls(darkScope), darkScope);

// ── Metrics (mode-independent constants from :root + @theme layers) ────────

const metrics = new Map<string, Metric>();

function pushMetric(name: string, value: string, block: "root" | "theme" | "themeInline"): void {
  const key = camelName(name);
  if (key === "animateSoftIn") return; // animation shorthand — supplied natively in Dart
  const radiusCalc = value.match(/^calc\(var\(--radius\) ([+-]) (\d+(?:\.\d+)?)px\)$/);
  if (radiusCalc) {
    const base = typeof metrics.get("radius") === "number" ? (metrics.get("radius") as number) : 0;
    const delta = Number.parseFloat(radiusCalc[2]);
    metrics.set(key, radiusCalc[1] === "-" ? round(base - delta) : round(base + delta));
    return;
  }
  const rem = value.match(/^(-?[\d.]+)rem$/);
  if (rem) {
    metrics.set(key, round(Number.parseFloat(rem[1]) * REM_TO_PX));
    return;
  }
  const px = value.match(/^(-?[\d.]+)px$/);
  if (px) {
    metrics.set(key, round(Number.parseFloat(px[1])));
    return;
  }
  const em = value.match(/^(-?[\d.]+)em$/);
  if (em) {
    metrics.set(key, Number.parseFloat(em[1])); // letter-spacing fraction; theme multiplies by font size
    return;
  }
  const ms = value.match(/^(-?[\d.]+)ms$/);
  if (ms) {
    metrics.set(key, Number.parseInt(ms[1], 10));
    return;
  }
  const bezier = value.match(/^cubic-bezier\(([^)]+)\)$/);
  if (bezier) {
    metrics.set(key, bezier[1].split(",").map((p) => Number.parseFloat(p.trim())));
    return;
  }
  if (block === "themeInline" && name.startsWith("--font-") && value.includes(",")) {
    metrics.set(key, value.split(",").map((entry) => entry.trim().replace(/^['"]|['"]$/g, "")));
    return;
  }
  // Unhandled shapes land in the JSON only (never silently dropped).
  metrics.set(key, value);
}

for (const { name, value } of rootDecls) {
  if (light.colors.has(name) || RAW_ONLY.has(name)) continue;
  pushMetric(name, value, "root");
}
for (const { name, value } of themeDecls) pushMetric(name, value, "theme");
for (const { name, value } of themeInlineDecls) {
  if (name.startsWith("--color-")) continue; // aliases of tokens already exported
  if (/^--radius-(sm|md|lg)$/.test(name)) continue; // derived from the base radius below
  pushMetric(name, value, "themeInline");
}
// @theme inline derives sm/md/lg from the base radius; emit resolved values.
const radiusBase = typeof metrics.get("radius") === "number" ? (metrics.get("radius") as number) : 0;
metrics.set("radiusSm", round(radiusBase - 4));
metrics.set("radiusMd", round(radiusBase - 2));
metrics.set("radiusLg", round(radiusBase));

// ── Emit: tokens.json ───────────────────────────────────────────────────────

function modeToJson(mode: ModeTokens): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [name, token] of mode.colors) out[camelName(name)] = { raw: token.raw, argb: token.argb };
  for (const [name, token] of mode.raws) out[camelName(name)] = token;
  return out;
}

const jsonDoc = {
  generatedBy: "bun run gen:mobile — scripts/design/export-tokens.ts (do not edit)",
  source: "packages/slab-components/src/styles/globals.css",
  light: modeToJson(light),
  dark: modeToJson(dark),
  metrics: Object.fromEntries(metrics),
};
const jsonText = `${JSON.stringify(jsonDoc, null, 2)}\n`;

// ── Emit: slab_tokens.g.dart ────────────────────────────────────────────────

function dartColorClass(className: string, mode: ModeTokens): string {
  const lines: string[] = [`class ${className} {`];
  const names: string[] = [];
  for (const [name, token] of mode.colors) {
    const fieldName = camelName(name);
    names.push(fieldName);
    lines.push(`  static const Color ${fieldName} = Color(${token.argb.replace("#", "0x")});`);
  }
  lines.push(``, `  /// Field names in declaration order (light/dark parity guard).`);
  lines.push(`  static const List<String> tokenNames = [${names.map((n) => `'${n}'`).join(", ")}];`);
  lines.push("}", "");
  return lines.join("\n");
}

function dartMetricsClass(): string {
  const lines: string[] = ["class SlabMetrics {"];
  for (const [key, value] of metrics) {
    if (typeof value === "number") {
      lines.push(`  static const double ${key} = ${value};`);
    } else if (Array.isArray(value) && value.every((entry) => typeof entry === "number")) {
      lines.push(`  static const Cubic ${key} = Cubic(${(value as number[]).join(", ")});`);
    } else if (Array.isArray(value)) {
      const entries = (value as string[]).map((entry) => `'${entry.replaceAll("'", "\\'")}'`);
      lines.push(`  static const List<String> ${key} = [${entries.join(", ")}];`);
    }
    // String values are JSON-only (unhandled CSS shapes) — intentionally no Dart line.
  }
  lines.push("}", "");
  return lines.join("\n");
}

const dartText = [
  "// GENERATED by `bun run gen:mobile` — do not edit.",
  "// Source of truth: packages/slab-components/src/styles/globals.css.",
  "// Raw oklch strings for every token live in design/tokens.json (wide-gamut upgrade path).",
  "",
  "import 'dart:ui' show Color;",
  "import 'package:flutter/animation.dart' show Cubic;",
  "",
  dartColorClass("SlabTokensLight", light),
  dartColorClass("SlabTokensDark", dark),
  dartMetricsClass(),
].join("\n");

// ── Write or drift-check ────────────────────────────────────────────────────

async function writeOrCheck(outPath: string, expected: string): Promise<string[]> {
  const relative = path.relative(repoRoot, outPath).replaceAll("\\", "/");
  if (CHECK) {
    const actual = await readFile(outPath, "utf8").catch(() => null);
    return actual === expected ? [] : [relative];
  }
  await mkdir(path.dirname(outPath), { recursive: true });
  await writeFile(outPath, expected, "utf8");
  return [];
}

const drift = [
  ...(await writeOrCheck(JSON_OUT, jsonText)),
  ...(await writeOrCheck(DART_OUT, dartText)),
];

if (drift.length > 0) {
  console.error(
    `design token drift detected — regenerate with \`bun run gen:mobile\`:\n  ${drift.join("\n  ")}`,
  );
  process.exit(1);
}

if (!CHECK) {
  console.log(
    `Exported ${light.colors.size} light / ${dark.colors.size} dark colors + ${metrics.size} metrics → packages/slab-mobile (tokens.json + slab_tokens.g.dart).`,
  );
}

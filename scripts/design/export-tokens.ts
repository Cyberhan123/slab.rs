#!/usr/bin/env bun

/**
 * Exports the design tokens from `packages/slab-components/src/styles/globals.css`
 * into the Flutter mobile app — the one-way pipeline that keeps slab-mobile
 * visually unified with web/desktop without sharing UI code.
 *
 * Outputs (all committed, all deterministic; regenerate with `bun run gen:mobile`):
 *   flutter/slab-mobile/design/tokens.json          — inspectable intermediate
 *                                                      (keeps the raw oklch strings for a
 *                                                      future wide-gamut upgrade)
 *   flutter/slab-mobile/lib/theme/slab_tokens.g.dart — `SlabTokensLight/Dark` + `SlabMetrics`
 *   flutter/slab-mobile/assets/theme/tdesign-theme.json — tdesign_flutter theme
 *                                                      (`"slab"` light + `"slabDark"` dark;
 *                                                      see the TDesign section below)
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
const JSON_OUT = path.join(repoRoot, "flutter/slab-mobile/design/tokens.json");
const DART_OUT = path.join(
  repoRoot,
  "flutter/slab-mobile/lib/theme/slab_tokens.g.dart",
);
const TD_OUT = path.join(
  repoRoot,
  "flutter/slab-mobile/assets/theme/tdesign-theme.json",
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

// ── Emit: tdesign-theme.json ────────────────────────────────────────────────
//
// The tdesign_flutter component library reads its theme from a TDThemeData
// ThemeExtension loaded from JSON: `{ "slab": { color, radius }, "slabDark": … }`.
// Semantics (verified against tdesign_flutter 0.2.7 source):
//   * Missing keys fall back to the package's built-in LIGHT defaults in BOTH
//     modes (`TDMap` factory fallback) — so both modes must carry the complete
//     palette; partial JSON would leak TDesign blue in dark mode.
//   * Hex strings: 6-digit `#RRGGBB` (opaque) or 8-digit `#AARRGGBB` (alpha in
//     the high byte — matches `Color(int.parse(..., radix: 16))`).
//   * Scale conventions mirrored from the package defaults: functional scales
//     (brand/error/warning/success) run light→dark in light mode and invert in
//     dark mode; the gray ramp runs light→dark in both modes (dark surfaces
//     pick the high indices). Semantic anchors per mode: brand normal = stop 7
//     (light) / 8 (dark), error normal = stop 6, warning/success normal = stop 5.
//
// Every value is derived from slab tokens — no TDesign default is imported.
// slab has no `--warning` token; `--chart-3` (amber) is the sanctioned anchor.
// `--brand-gold` stays out of the scales (bespoke approval chrome via SlabExtras).

type Oklch = { l: number; c: number; h: number };

/** Fallback hue when a neutral token carries none (c=0): the page background's
 * cool tint keeps gray interpolation inside slab's palette family. */
function modeHue(mode: ModeTokens): number {
  const parsed = parse(mode.colors.get("--background")!.raw);
  const hue = parsed?.h;
  return Number.isFinite(hue as number) ? (hue as number) : 247.86;
}

const oklchCache = new Map<string, Oklch>();

/** Parsed oklch anchors for one mode, keyed by CSS token name. */
function oklchOf(mode: ModeTokens, name: string): Oklch {
  const key = `${mode === light ? "L" : "D"}:${name}`;
  const cached = oklchCache.get(key);
  if (cached) return cached;
  const token = mode.colors.get(name);
  if (token === undefined) throw new Error(`token ${name} missing for TDesign theme`);
  const parsed = parse(token.raw);
  if (parsed === null) throw new Error(`cannot parse oklch for ${name}: ${token.raw}`);
  const fallbackHue = modeHue(mode);
  const value: Oklch = {
    l: parsed.l ?? 0,
    c: parsed.c ?? 0,
    h: Number.isFinite(parsed.h as number) ? (parsed.h as number) : fallbackHue,
  };
  oklchCache.set(key, value);
  return value;
}

/** oklch → opaque `#RRGGBB` through the same clampChroma pipeline as Dart. */
function hex6(l: number, c: number, h: number): string {
  const argb = toArgb(`oklch(${(l * 100).toFixed(6)}% ${c.toFixed(6)} ${h.toFixed(6)})`);
  if (argb === null) throw new Error(`oklch→hex conversion failed for ${l} ${c} ${h}`);
  return `#${argb.slice(3)}`;
}

/** Opaque `#RRGGBB` of a slab token in one mode. */
function tokenHex6(mode: ModeTokens, name: string): string {
  const token = mode.colors.get(name);
  if (token === undefined) throw new Error(`token ${name} missing for TDesign theme`);
  return `#${token.argb.slice(3)}`;
}

/** oklch → 8-digit `#AARRGGBB` (alpha 0..1 in the high byte). */
function hex8(l: number, c: number, h: number, alpha: number): string {
  const base = hex6(l, c, h).slice(1);
  const aa = Math.round(alpha * 255)
    .toString(16)
    .padStart(2, "0")
    .toUpperCase();
  return `#${aa}${base}`;
}

function lerp(a: number, b: number, t: number): number {
  return a + (b - a) * t;
}

function mixOklch(a: Oklch, b: Oklch, t: number): Oklch {
  return { l: lerp(a.l, b.l, t), c: lerp(a.c, b.c, t), h: lerp(a.h, b.h, t) };
}

/**
 * Two-segment 10-stop ramp around a slab anchor: exact at the anchor index,
 * interpolating lightness out to the mode's ramp ends. Chroma/hue stay at the
 * anchor's (gamut-clamped per stop) — the scale is slab's hue at slab's reach.
 */
function scale10(anchor: Oklch, anchorIndex: number, isDark: boolean): string[] {
  const [lMin, lMax] = isDark ? [0.2, 0.97] : [0.97, 0.16];
  const stops: string[] = [];
  for (let i = 1; i <= 10; i += 1) {
    const l = i <= anchorIndex
      ? lerp(lMin, anchor.l, (i - 1) / (anchorIndex - 1))
      : lerp(anchor.l, lMax, (i - anchorIndex) / (10 - anchorIndex));
    stops.push(hex6(l, anchor.c, anchor.h));
  }
  return stops;
}

/**
 * 14-stop gray ramp from pinned slab neutrals, piecewise-interpolated in
 * oklch. Light and dark pin sets differ on purpose: slab's dark neutrals are
 * independently cooler than an index-shift of the light ramp, and TDesign's
 * dark surfaces consume the high indices (page=g14, container=g13, stroke=g11).
 */
function grayRamp(mode: ModeTokens, pins: Array<[number, string]>): string[] {
  const anchors = pins.map(([index, name]) => ({ index, ok: oklchOf(mode, name) }));
  const stops: string[] = [];
  for (let i = 1; i <= 14; i += 1) {
    let hi = anchors.findIndex((pin) => pin.index >= i);
    if (hi === -1) hi = anchors.length - 1;
    const lo = Math.max(0, hi - 1);
    const span = anchors[hi].index - anchors[lo].index;
    const t = span === 0 ? 0 : Math.min(1, Math.max(0, (i - anchors[lo].index) / span));
    const mixed = mixOklch(anchors[lo].ok, anchors[hi].ok, t);
    stops.push(hex6(mixed.l, mixed.c, mixed.h));
  }
  return stops;
}

/** Per-mode complete TDesign palette (sorted keys, deterministic). */
function buildTdPalette(mode: ModeTokens, isDark: boolean): Record<string, string | number> {
  const color: Record<string, string> = {};
  const set = (key: string, value: string) => {
    if (color[key] !== undefined) throw new Error(`duplicate TDesign key ${key}`);
    color[key] = value;
  };

  // Functional scales. slab anchors: brand=--primary, error=--destructive,
  // warning=--chart-3 (slab has no warning token — amber chart color),
  // success=--success. Hover/click stops follow TDesign's per-mode defaults.
  const families: Array<{
    prefix: string;
    anchor: string;
    normalStop: number;
    hoverStop: number;
    clickStop: number;
  }> = [
    // brand: light normal@7 hover@6 click@8; dark normal@8 hover@7 click@9
    { prefix: "brand", anchor: "--primary", normalStop: isDark ? 8 : 7, hoverStop: isDark ? 7 : 6, clickStop: isDark ? 9 : 8 },
    // error: normal@6 hover@5 click@7 in both modes
    { prefix: "error", anchor: "--destructive", normalStop: 6, hoverStop: 5, clickStop: 7 },
    // warning/success: normal@5 hover@4 click@6 in both modes
    { prefix: "warning", anchor: "--chart-3", normalStop: 5, hoverStop: 4, clickStop: 6 },
    { prefix: "success", anchor: "--success", normalStop: 5, hoverStop: 4, clickStop: 6 },
  ];
  for (const family of families) {
    const anchor = oklchOf(mode, family.anchor);
    const stops = scale10(anchor, family.normalStop, isDark);
    stops.forEach((hex, i) => set(`${family.prefix}Color${i + 1}`, hex));
    const direct = tokenHex6(mode, family.anchor);
    set(`${family.prefix}NormalColor`, direct);
    set(`${family.prefix}HoverColor`, stops[family.hoverStop - 1]);
    set(`${family.prefix}ActiveColor`, stops[family.clickStop - 1]);
    set(`${family.prefix}ClickColor`, stops[family.clickStop - 1]);
    set(`${family.prefix}LightColor`, stops[0]);
    set(`${family.prefix}FocusColor`, stops[1]);
    set(`${family.prefix}DisabledColor`, stops[2]);
    set(`${family.prefix}ColorLightHover`, stops[1]);
    if (stops[family.normalStop - 1] !== direct) {
      throw new Error(`${family.prefix} anchor stop ${family.normalStop} != direct token`);
    }
  }

  // Gray ramp: pinned per mode (see grayRamp doc). Both ramps run
  // light→dark; dark surfaces pick the high indices per TDesign convention.
  const grayPins: Array<[number, string]> = isDark
    ? [[1, "--foreground"], [5, "--muted-foreground"], [11, "--border"], [12, "--secondary"], [13, "--card"], [14, "--background"]]
    : [[1, "--card"], [2, "--background"], [3, "--secondary"], [4, "--border"], [7, "--muted-foreground"], [14, "--foreground"]];
  grayRamp(mode, grayPins).forEach((hex, i) => set(`grayColor${i + 1}`, hex));

  // Text-on-neutral ladder: foreground → muted-foreground, dimming toward the
  // page background at the disabled end.
  const fg = oklchOf(mode, "--foreground");
  const mf = oklchOf(mode, "--muted-foreground");
  const bg = oklchOf(mode, "--background");
  const gyStops = [fg, mixOklch(fg, mf, 0.4), mf, mixOklch(mf, bg, 0.35)];
  gyStops.forEach((stop, i) => set(`fontGyColor${i + 1}`, hex6(stop.l, stop.c, stop.h)));

  // Text-on-color ladder: white in light mode, slab's (near-white) foreground
  // in dark mode, with a descending alpha ramp.
  const whBase = isDark ? fg : { l: 1, c: 0, h: modeHue(mode) };
  const whAlphas = [1, 0.9, 0.75, 0.65];
  whAlphas.forEach((alpha, i) => set(`fontWhColor${i + 1}`, hex8(whBase.l, whBase.c, whBase.h, alpha)));
  set("whiteColor1", isDark ? tokenHex6(mode, "--foreground") : "#FFFFFF");

  // Surfaces.
  set("bgColorPage", tokenHex6(mode, "--background"));
  set("bgColorContainer", tokenHex6(mode, "--card"));
  set("bgColorContainerHover", tokenHex6(mode, "--secondary"));
  set("bgColorContainerActive", tokenHex6(mode, "--accent"));
  set("bgColorContainerSelect", tokenHex6(mode, "--accent"));
  set("bgColorSecondaryContainer", tokenHex6(mode, "--secondary"));
  set("bgColorSecondaryContainerHover", tokenHex6(mode, "--accent"));
  set("bgColorSecondaryContainerActive", tokenHex6(mode, "--accent"));
  set("bgColorComponent", tokenHex6(mode, "--secondary"));
  set("bgColorComponentHover", tokenHex6(mode, "--accent"));
  set("bgColorComponentActive", tokenHex6(mode, "--accent"));
  set("bgColorComponentDisabled", tokenHex6(mode, "--muted"));
  set("bgColorSecondaryComponent", tokenHex6(mode, "--accent"));
  set("bgColorSecondaryComponentHover", tokenHex6(mode, "--accent"));
  set("bgColorSecondaryComponentActive", tokenHex6(mode, "--border"));
  set("bgColorSpecialComponent", tokenHex6(mode, "--popover"));

  // Text + strokes.
  set("textColorPrimary", tokenHex6(mode, "--foreground"));
  set("textColorSecondary", tokenHex6(mode, "--muted-foreground"));
  set("textColorPlaceholder", tokenHex6(mode, "--muted-foreground"));
  set("textDisabledColor", tokenHex6(mode, "--muted-foreground"));
  set("textColorBrand", tokenHex6(mode, "--primary"));
  set("textColorLink", tokenHex6(mode, "--primary"));
  set("textColorAnti", tokenHex6(mode, "--primary-foreground"));
  set("componentStrokeColor", tokenHex6(mode, "--border"));
  set("componentBorderColor", tokenHex6(mode, "--border"));

  // Radius: slab's rounded scale replaces TDesign's 3/6/9/12 defaults.
  // (Pill/circle radii keep package defaults — non-color, no leak risk.)
  const radius: Record<string, number> = {
    radiusSmall: metrics.get("radiusSm") as number,
    radiusDefault: metrics.get("radiusMd") as number,
    radiusLarge: metrics.get("radiusLg") as number,
    radiusExtraLarge: metrics.get("radiusXl") as number,
  };

  const sortedColor: Record<string, string> = {};
  for (const key of Object.keys(color).sort()) sortedColor[key] = color[key];
  return { color: sortedColor, radius };
}

const tdLight = buildTdPalette(light, false);
const tdDark = buildTdPalette(dark, true);

// ── TDesign assertions (loud failures, mirrors of the locale parity guard) ──

{
  const lightKeys = Object.keys(tdLight.color).sort().join(",");
  const darkKeys = Object.keys(tdDark.color).sort().join(",");
  if (lightKeys !== darkKeys) throw new Error("tdesign theme light/dark key parity broken");

  // Every color key the package getters read must be present (missing keys
  // would silently fall back to built-in TDesign light blue).
  const REQUIRED_KEYS = [
    ...["brand", "error", "warning", "success"].flatMap((p) => [
      ...Array.from({ length: 10 }, (_, i) => `${p}Color${i + 1}`),
      `${p}NormalColor`, `${p}HoverColor`, `${p}ClickColor`, `${p}LightColor`, `${p}FocusColor`, `${p}DisabledColor`,
    ]),
    ...Array.from({ length: 14 }, (_, i) => `grayColor${i + 1}`),
    ...Array.from({ length: 4 }, (_, i) => `fontGyColor${i + 1}`),
    ...Array.from({ length: 4 }, (_, i) => `fontWhColor${i + 1}`),
    "whiteColor1",
    "bgColorPage", "bgColorContainer", "bgColorContainerHover", "bgColorContainerActive", "bgColorContainerSelect",
    "bgColorSecondaryContainer", "bgColorSecondaryContainerHover", "bgColorSecondaryContainerActive",
    "bgColorComponent", "bgColorComponentHover", "bgColorComponentActive", "bgColorComponentDisabled",
    "textColorPrimary", "textColorSecondary", "textColorPlaceholder", "textColorBrand", "textColorLink", "textColorAnti",
    "textDisabledColor", "componentStrokeColor", "componentBorderColor",
  ];
  for (const key of REQUIRED_KEYS) {
    if (!(key in tdLight.color)) throw new Error(`tdesign theme missing required key: ${key}`);
  }

  // Spot anchors: the theme MUST carry slab's exact brand/page values.
  const spotChecks: Array<[Record<string, string>, string, string]> = [
    [tdLight.color as Record<string, string>, "brandNormalColor", tokenHex6(light, "--primary")],
    [tdDark.color as Record<string, string>, "brandNormalColor", tokenHex6(dark, "--primary")],
    [tdLight.color as Record<string, string>, "bgColorPage", tokenHex6(light, "--background")],
    [tdDark.color as Record<string, string>, "bgColorPage", tokenHex6(dark, "--background")],
  ];
  for (const [palette, key, expected] of spotChecks) {
    if (palette[key] !== expected) throw new Error(`tdesign theme spot check failed: ${key} ${palette[key]} != ${expected}`);
  }

  // Scale lightness must be monotonic (ramp direction inverts per mode; the
  // gray ramp runs light→dark in both). Catches token drift that would make
  // e.g. hover lighter than normal in dark mode.
  const toOklchL = (hex: string): number => {
    const converted = converter("oklch")(parse(hex)!);
    return converted.l ?? 0;
  };
  const ramps: Array<[string, string, Record<string, string>, number, "up" | "down"]> = [
    ...(["brand", "error", "warning", "success"] as const).flatMap((p) => [
      [p, "light", tdLight.color as Record<string, string>, 10, "down"] as [string, string, Record<string, string>, number, "up" | "down"],
      [p, "dark", tdDark.color as Record<string, string>, 10, "up"] as [string, string, Record<string, string>, number, "up" | "down"],
    ]),
    ["gray", "light", tdLight.color as Record<string, string>, 14, "down"] as [string, string, Record<string, string>, number, "up" | "down"],
    ["gray", "dark", tdDark.color as Record<string, string>, 14, "down"] as [string, string, Record<string, string>, number, "up" | "down"],
  ];
  for (const [prefix, modeName, palette, len, dir] of ramps) {
    const key = (i: number) => `${prefix}Color${i + 1}`;
    const ls = Array.from({ length: len }, (_, i) => toOklchL(palette[key(i)]));
    const ok = ls.every((v, i) => i === 0 || (dir === "down" ? v <= ls[i - 1] + 1e-6 : v >= ls[i - 1] - 1e-6));
    if (!ok) throw new Error(`tdesign theme ${prefix} (${modeName}) lightness not monotonic ${dir}: ${ls.map((v) => v.toFixed(3)).join(" ")}`);
  }
}

const tdJson = {
  _generatedBy: "bun run gen:mobile — scripts/design/export-tokens.ts (do not edit); TDThemeData.fromJson('slab', …, darkName: 'slabDark')",
  slab: tdLight,
  slabDark: tdDark,
};
const tdText = `${JSON.stringify(tdJson, null, 2)}\n`;

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
  ...(await writeOrCheck(TD_OUT, tdText)),
];

if (drift.length > 0) {
  console.error(
    `design token drift detected — regenerate with \`bun run gen:mobile\`:\n  ${drift.join("\n  ")}`,
  );
  process.exit(1);
}

if (!CHECK) {
  console.log(
    `Exported ${light.colors.size} light / ${dark.colors.size} dark colors + ${metrics.size} metrics + tdesign theme (${Object.keys(tdLight.color).length} keys/mode) → flutter/slab-mobile (tokens.json + slab_tokens.g.dart + assets/theme/tdesign-theme.json).`,
  );
}

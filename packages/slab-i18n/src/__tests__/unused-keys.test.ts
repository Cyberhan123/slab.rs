import { readFileSync, readdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { describe, expect, it } from "vitest";

import { enUS } from "../locales/en-US";

/**
 * Static reference guard for locale keys.
 *
 * Assumptions (verified when this test was introduced):
 * - Consumer code references keys as quoted literals (`t('pages.hub.hero.badge')`)
 *   or as trailing template prefixes (`` t(`pages.plugins.status.${status}`) ``).
 *   There are no mid-key interpolations and no string concatenation.
 * - Variable-key call sites resolve to literal tables defined in source files
 *   (route `labelKey` metas, WINDOW_CONTROL_LABEL_KEYS, ...), so they are
 *   captured by the literal scan.
 * - The `server.*` domain is backend-driven: keys are locked to the Rust
 *   `ServerI18nKey` enum via the compile-time coverage check in
 *   `locales/server.ts`, so unused analysis does not apply to it.
 */

const REPO_ROOT_ANCHOR = "slab-workspace";

/** Directories/files whose source may reference locale keys. */
const SCAN_ROOTS = [
  // Primary business consumer (tests count as references).
  "packages/slab-ui/src",
  // Programmatic references (DEFAULT_ASSISTANT_LABELS) live here; the rest of
  // this package is locale definitions and would self-reference.
  "packages/slab-i18n/src/index.ts",
  // Shell apps currently only import for side effects; kept defensively.
  "packages/slab-web/src",
  "packages/slab-desktop/src",
] as const;

/**
 * Locale keys intentionally kept without a static reference.
 * Map from key -> reason. Remove entries when the key gains a reference.
 */
const ALLOWED_UNUSED: ReadonlyMap<string, string> = new Map([]);

type LocaleTree = Record<string, unknown>;

function resolveRepoRoot(): string {
  let dir = path.dirname(fileURLToPath(import.meta.url));
  for (let i = 0; i < 6; i += 1) {
    const manifestPath = path.join(dir, "package.json");
    try {
      const manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as { name?: string };
      if (manifest.name === REPO_ROOT_ANCHOR) return dir;
    } catch {
      // keep walking up
    }
    dir = path.dirname(dir);
  }
  throw new Error(`unable to locate repo root (package name "${REPO_ROOT_ANCHOR}") from ${import.meta.url}`);
}

function collectSourceFiles(root: string, out: string[] = []): string[] {
  const stat = statSync(root);
  if (stat.isFile()) {
    if (/\.(ts|tsx)$/.test(root)) out.push(root);
    return out;
  }
  for (const entry of readdirSync(root)) {
    if (entry === "node_modules" || entry === "dist" || entry.startsWith(".")) continue;
    collectSourceFiles(path.join(root, entry), out);
  }
  return out;
}

/** Literal keys like `pages.hub.hero.badge` inside quotes. */
const LITERAL_KEY_PATTERN = /(['"`])(pages|layouts|common)\.[A-Za-z0-9_.+-]+\1/g;

/** Trailing template prefixes like `` `pages.plugins.status.${status}` ``. */
const TEMPLATE_KEY_PATTERN = /['"`](pages|layouts|common)\.[A-Za-z0-9_.+-]*\$\{/g;

function collectReferencedKeys(files: string[]): {
  literals: Set<string>;
  templatePrefixes: string[];
} {
  const literals = new Set<string>();
  const templatePrefixes = new Set<string>();
  for (const file of files) {
    const content = readFileSync(file, "utf8");
    for (const match of content.matchAll(LITERAL_KEY_PATTERN)) {
      literals.add(match[0].slice(1, -1));
    }
    for (const match of content.matchAll(TEMPLATE_KEY_PATTERN)) {
      const raw = match[0].slice(1).replace(/\$\{$/, "");
      // `prefix.` must match `prefix.xxx` keys without doubling the dot.
      templatePrefixes.add(raw.replace(/\.$/, ""));
    }
  }
  return { literals, templatePrefixes: [...templatePrefixes] };
}

function collectLocaleLeaves(value: unknown, prefix = "", out: string[] = []): string[] {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    for (const [key, entry] of Object.entries(value as LocaleTree)) {
      collectLocaleLeaves(entry, prefix ? `${prefix}.${key}` : key, out);
    }
    return out;
  }
  out.push(prefix);
  return out;
}

const PLURAL_SUFFIX = /_(one|other|zero|two|few|many)$/;

function describeRepo() {
  const root = resolveRepoRoot();
  const files = SCAN_ROOTS.flatMap((rel) => collectSourceFiles(path.join(root, rel)));
  const { literals, templatePrefixes } = collectReferencedKeys(files);
  const localeKeys = new Set(collectLocaleLeaves(enUS).filter((key) => !key.startsWith("server.")));
  return { root, files, literals, templatePrefixes, localeKeys };
}

describe("unused locale keys", () => {
  it("keeps locale keys referenced by consumer sources", () => {
    const { literals, templatePrefixes, localeKeys } = describeRepo();

    const deadKeys = [...localeKeys]
      .filter((key) => {
        if (literals.has(key)) return false;
        const base = key.replace(PLURAL_SUFFIX, "");
        if (base !== key && literals.has(base)) return false;
        return !templatePrefixes.some(
          (prefix) => key === prefix || key.startsWith(`${prefix}.`),
        );
      })
      .filter((key) => !ALLOWED_UNUSED.has(key))
      .toSorted();

    // Joined on purpose: on failure the diff lists every unreferenced key.
    expect(
      deadKeys.map((key) => `${key} (no reference in ${SCAN_ROOTS.join(", ")})`).join("\n"),
    ).toBe("");
  });

  it("keeps the allowlist free of stale entries", () => {
    const { localeKeys } = describeRepo();
    const stale = [...ALLOWED_UNUSED.keys()].filter((key) => !localeKeys.has(key)).toSorted();
    // Joined on purpose: on failure the diff lists every stale entry.
    expect(stale.join("\n")).toBe("");
  });
});

describe("referenced locale keys", () => {
  it("keeps every statically referenced key defined in the locale", () => {
    const { literals, localeKeys } = describeRepo();

    const missing = [...literals]
      .filter((key) => {
        if (localeKeys.has(key)) return false;
        // Plural bases resolve to `_one`/`_other` variants at runtime.
        if (localeKeys.has(`${key}_one`) || localeKeys.has(`${key}_other`)) return false;
        return true;
      })
      .toSorted();

    // Joined on purpose: on failure the diff lists every missing key.
    expect(missing.join("\n")).toBe("");
  });

  it("keeps template prefix families backed by at least one locale key", () => {
    const { templatePrefixes, localeKeys } = describeRepo();

    const orphanPrefixes = templatePrefixes
      .filter((prefix) => {
        for (const key of localeKeys) {
          if (key === prefix || key.startsWith(`${prefix}.`)) return false;
        }
        return true;
      })
      .toSorted();

    // Joined on purpose: on failure the diff lists every orphan prefix.
    expect(orphanPrefixes.join("\n")).toBe("");
  });
});

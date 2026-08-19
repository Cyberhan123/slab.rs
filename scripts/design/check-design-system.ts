import { readdir, readFile } from "node:fs/promises"
import { extname, join, relative } from "node:path"

const ROOTS = [
  "packages/slab-components/src",
  "packages/slab-desktop/src",
  "packages/slab-ui/src",
  "packages/slab-plugin-ui/src",
  // Flutter app: same philosophy in Dart — colors flow from the generated
  // tokens (slab_tokens.g.dart), never raw literals in app code.
  "packages/slab-mobile/lib",
]

const GLOBALS_PATH = "packages/slab-components/src/styles/globals.css"

const RULES = [
  {
    name: "text px classes",
    pattern: /text-\[[0-9]+px\]/g,
    max: 0,
  },
  {
    name: "target radius classes",
    pattern: /rounded-\[(24|28|30|32|34)px\]/g,
    max: 0,
  },
  {
    name: "raw numeric drop shadows",
    pattern: /shadow-\[0_/g,
    max: 5,
  },
  {
    name: "var opacity backgrounds",
    pattern: /bg-\[var\([^\]]+\)\]\/[0-9]+/g,
    max: 0,
  },
  {
    name: "arbitrary tracking",
    pattern: /tracking-\[[^\]]+\]/g,
    max: 0,
  },
  {
    // Tailwind arbitrary values must reference theme tokens as utilities
    // (bg-card), not var() brackets. Radix/Shiki runtime vars and the
    // explorer's component-local layout var are exempt.
    name: "arbitrary var() utilities",
    pattern: /-\[(?:color:)?var\(--(?!radix-|shiki-|workspace-explorer-width)/g,
    max: 0,
  },
] as const

const RAW_HEX_PATTERN = /#[0-9A-Fa-f]{3,8}\b/g

const RAW_HEX_ALLOWLIST = [
  // Native macOS window stoplight colors.
  /^packages\/slab-ui\/src\/layouts\/window-controls\.tsx$/,
  // Third-party chart SVG selectors target literal generated strokes.
  /^packages\/slab-components\/src\/chart\.tsx$/,
  // Xterm requires literal ANSI palette values.
  /^packages\/slab-ui\/src\/pages\/workspace\/components\/workspace-console-panel\.tsx$/,
  // Monaco needs literal fallback colors when CSS variable resolution is unavailable.
  /^packages\/slab-ui\/src\/pages\/workspace\/lib\/monaco-theme\.tsx?$/,
  // Test fixture asserts the human-facing hash prefix, not a color.
  /^packages\/slab-ui\/src\/pages\/task\/__tests__\/utils\.test\.tsx?$/,
]

// ── Flutter (Dart) leg ──────────────────────────────────────────────────────
// The mobile app consumes the SAME tokens through the generated
// `SlabTokensLight/Dark`/`SlabExtras` classes; raw color literals in Dart app
// code are the mirror of raw hex in TSX.

const DART_ROOT = "packages/slab-mobile/lib"

const DART_RAW_COLOR_RULES = [
  { name: "raw Dart Color literals", pattern: /Color\(0x[0-9A-Fa-f]{8}\)/g },
  { name: "Material Colors.* palette", pattern: /Colors\.[a-zA-Z]+/g },
] as const

const DART_COLOR_ALLOWLIST = [
  // The one legitimate source of literal colors: the generated token file.
  /^packages\/slab-mobile\/lib\/theme\/slab_tokens\.g\.dart$/,
]

async function main() {
  const globals = await readFile(GLOBALS_PATH, "utf8")
  if (!globals.includes("prefers-reduced-motion")) {
    fail("missing prefers-reduced-motion guard in globals.css")
  }

  const files = await collectFiles(ROOTS)
  const failures: string[] = []

  for (const rule of RULES) {
    const matches = collectMatches(files, rule.pattern)
    if (matches.length > rule.max) {
      failures.push(formatFailure(rule.name, matches, rule.max))
    }
  }

  const rawHexMatches = collectMatches(files, RAW_HEX_PATTERN).filter(
    (match) => !RAW_HEX_ALLOWLIST.some((pattern) => pattern.test(match.file)),
  )
  if (rawHexMatches.length > 0) {
    failures.push(formatFailure("raw hex colors", rawHexMatches, 0))
  }

  const dartFiles = await walk(DART_ROOT)
  for (const rule of DART_RAW_COLOR_RULES) {
    const dartMatches = collectMatches(dartFiles, rule.pattern).filter(
      (match) => !DART_COLOR_ALLOWLIST.some((pattern) => pattern.test(match.file)),
    )
    if (dartMatches.length > 0) {
      failures.push(formatFailure(rule.name, dartMatches, 0))
    }
  }

  if (failures.length > 0) {
    fail(failures.join("\n\n"))
  }

  console.log("design-system guard passed")
}

function collectMatches(
  files: Array<{ file: string; source: string }>,
  pattern: RegExp,
) {
  const matches: Array<{ file: string; line: number; text: string }> = []
  for (const { file, source } of files) {
    const stripped = stripBlockComments(source)
    const lines = stripped.split(/\r?\n/)
    for (const [index, line] of lines.entries()) {
      pattern.lastIndex = 0
      if (pattern.test(line)) {
        matches.push({
          file,
          line: index + 1,
          text: line.trim(),
        })
      }
    }
  }
  return matches
}

function formatFailure(
  name: string,
  matches: Array<{ file: string; line: number; text: string }>,
  max: number,
) {
  const preview = matches.slice(0, 20)
    .map((match) => `  ${match.file}:${match.line} ${match.text}`)
    .join("\n")
  const suffix = matches.length > 20 ? `\n  ... ${matches.length - 20} more` : ""
  return `${name}: ${matches.length} matches, max ${max}\n${preview}${suffix}`
}

function fail(message: string): never {
  console.error(message)
  process.exit(1)
}

function stripBlockComments(source: string) {
  return source.replace(/\/\*[\s\S]*?\*\//g, "")
}

async function collectFiles(roots: string[]) {
  const files = await Promise.all(roots.map((root) => walk(root)))
  return files.flat()
}

async function walk(dir: string): Promise<Array<{ file: string; source: string }>> {
  const entries = await readdir(dir, { withFileTypes: true })
  const results = await Promise.all(entries.map(async (entry) => {
    const fullPath = join(dir, entry.name)
    if (entry.isDirectory()) {
      return walk(fullPath)
    }

    if (!isCheckedFile(entry.name)) {
      return []
    }

    return [{
      file: relative(process.cwd(), fullPath).replaceAll("\\", "/"),
      source: await readFile(fullPath, "utf8"),
    }]
  }))

  return results.flat()
}

function isCheckedFile(name: string) {
  const ext = extname(name)
  return (
    ext === ".ts" ||
    ext === ".tsx" ||
    ext === ".css" ||
    ext === ".js" ||
    ext === ".jsx" ||
    ext === ".mjs" ||
    ext === ".dart"
  )
}

await main()

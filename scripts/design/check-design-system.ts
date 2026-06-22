import { readdir, readFile } from "node:fs/promises"
import { extname, join, relative } from "node:path"

const ROOTS = [
  "packages/slab-components/src",
  "packages/slab-desktop/src",
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
] as const

const RAW_HEX_PATTERN = /#[0-9A-Fa-f]{3,8}\b/g

const RAW_HEX_ALLOWLIST = [
  // Native macOS window stoplight colors.
  /^packages\/slab-desktop\/src\/layouts\/window-controls\.tsx$/,
  // Third-party chart SVG selectors target literal generated strokes.
  /^packages\/slab-components\/src\/chart\.tsx$/,
  // Xterm requires literal ANSI palette values.
  /^packages\/slab-desktop\/src\/pages\/workspace\/components\/workspace-console-panel\.tsx$/,
  // Monaco needs literal fallback colors when CSS variable resolution is unavailable.
  /^packages\/slab-desktop\/src\/pages\/workspace\/lib\/monaco-theme\.tsx?$/,
  // Test fixture asserts the human-facing hash prefix, not a color.
  /^packages\/slab-desktop\/src\/pages\/task\/__tests__\/utils\.test\.tsx?$/,
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
  return ext === ".ts" || ext === ".tsx" || ext === ".css" || ext === ".js" || ext === ".jsx" || ext === ".mjs"
}

await main()

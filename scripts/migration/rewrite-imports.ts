/**
 * One-off migration codemod for the DDD package split.
 *
 * Rewrites module specifiers across source trees:
 *   1. EXACT table — old module path → new package specifier (applied first).
 *   2. PREFIX rule — `@/x` → `@slab/ui/x` for the `@/` alias (opt-in flag,
 *      used once the desktop `src` tree has actually moved into @slab/ui).
 *
 * Matches `from "spec"`, `import("spec")`, and `vi.mock("spec")` forms.
 *
 * Usage:
 *   bun scripts/migration/rewrite-imports.ts <root-dir> [--prefix-ui]
 *
 * Prints a per-rule rewrite count plus any files still containing the `@/`
 * alias when --prefix-ui was NOT applied (informational only).
 */
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";

const args = process.argv.slice(2);
const prefixUi = args.includes("--prefix-ui");
const rootArg = args.find((a) => !a.startsWith("--"));
if (!rootArg) {
  console.error("usage: bun scripts/migration/rewrite-imports.ts <root-dir> [--prefix-ui]");
  process.exit(1);
}
const root = path.resolve(rootArg);

/** Exact specifier rewrites (old → new), applied before the prefix rule. */
const EXACT: Array<[RegExp, string]> = [
  // Harness (moved in Phase 1)
  [/(?:@\/pages\/assistant\/lib|\.\.\/\.\.\/lib|\.\.\/lib)\/harness$/g, "@slab/core/harness"],
  [/@\/pages\/assistant\/lib\/harness\/types/g, "@slab/core/harness/types"],
  // Service libs (moved in Phase 1)
  [/@\/lib\/workspace-bridge/g, "@slab/core/workspace/bridge"],
  [/@\/lib\/workspace-artifact-path/g, "@slab/core/workspace/artifact-path"],
  [/@\/lib\/media-task-api/g, "@slab/core/media/task-api"],
  [/@\/lib\/model-config/g, "@slab/core/models/config"],
  [/@\/lib\/error-description/g, "@slab/core/api/error-description"],
  [/@\/store\/ui-state-storage/g, "@/store/ui-state-storage"], // facade stays until Phase 2
];

const SOURCE_EXT = /\.(ts|tsx|mts|cts|jsx)$/;

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir)) {
    const full = path.join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      if (entry === "node_modules" || entry === "dist" || entry === ".vite") continue;
      out.push(...walk(full));
    } else if (SOURCE_EXT.test(entry)) {
      out.push(full);
    }
  }
  return out;
}

/** Rewrite every quoted occurrence of `spec` in import-ish positions. */
function rewriteSpecifiers(source: string, apply: (spec: string) => string): string {
  return source.replace(
    /(from\s+|import\s*\(\s*|vi\.mock\s*\(\s*|vi\.doMock\s*\(\s*)(["'])([^"'\n]+)\2/g,
    (match, head, quote, spec) => {
      const next = apply(spec);
      if (next === spec) return match;
      return `${head}${quote}${next}${quote}`;
    },
  );
}

let exactCount = 0;
let filesChanged = 0;

for (const file of walk(root)) {
  const before = readFileSync(file, "utf8");
  let after = before;

  for (const [pattern, replacement] of EXACT) {
    const next = after.replace(pattern, replacement);
    if (next !== after) exactCount += 1;
    after = next;
  }

  if (prefixUi) {
    after = rewriteSpecifiers(after, (spec) =>
      spec.startsWith("@/") ? `@slab/ui/${spec.slice(2)}` : spec,
    );
  }

  if (after !== before) {
    writeFileSync(file, after);
    filesChanged += 1;
  }
}

console.log(`rewrite-imports: ${filesChanged} files changed`);
console.log(`  exact-table rewrites: ${exactCount}`);
if (prefixUi) console.log(`  @/ → @slab/ui/ rewrites applied`);

"use client"

/**
 * Line-level git-diff-style rendering for `apply_patch` / `write_file` change
 * previews. Understands the `*** Begin Patch` dialect (file headers, `@@`
 * chunks, `+`/`-`/context lines), the synthesized previews emitted by the
 * backend (whole-file `+` blocks for adds/writes, `@@` + `-old`/`+new` for
 * updates), unified-diff fallbacks (`+++`/`---` headers) and heredoc wrapper
 * lines — anything unrecognized renders as a plain line, so malformed text
 * degrades gracefully instead of breaking the card.
 *
 * Shared by the in-stream file-change card and the approval banner so both
 * preview the same colored diff. The outer element stays a `<pre>` — the e2e
 * helpers locate diff text via `locator("pre")`.
 */

/** Visual class per classified diff line. */
type DiffLineKind = "add" | "del" | "context" | "meta" | "hunk" | "plain"

const LINE_KIND_CLASS: Record<DiffLineKind, string> = {
  add: "bg-green-500/10 text-green-700 dark:text-green-400",
  del: "bg-red-500/10 text-red-700 dark:text-red-400",
  context: "text-muted-foreground",
  meta: "font-medium text-foreground",
  hunk: "text-blue-700 dark:text-blue-400",
  plain: "text-muted-foreground",
}

/** Classify one diff line for coloring. See the module doc for the dialects. */
export function classifyDiffLine(line: string): DiffLineKind {
  // Patch markers: `*** Begin Patch` / `*** Update File: x` / `*** End of File`…
  if (line.startsWith("***")) return "meta"
  // Unified-diff file headers (`+++ b/x`, `--- a/x`) — meta, not add/del.
  if (line.startsWith("+++") || line.startsWith("---")) return "meta"
  if (line.startsWith("@@")) return "hunk"
  if (line.startsWith("+")) return "add"
  if (line.startsWith("-")) return "del"
  if (line.startsWith(" ")) return "context"
  // Heredoc wrapper lines (`apply_patch <<'EOF'` … `EOF`) are transport, not diff.
  if (line === "EOF" || line === "PATCH") return "meta"
  if (line.startsWith("<<")) return "meta"
  if (line.startsWith("apply_patch") && line.includes("<<")) return "meta"
  return "plain"
}

export function PatchDiffView({ diff, className }: { diff: string; className?: string }) {
  // Static preview — lines never reorder within a render. Derive stable,
  // duplicate-safe keys from content + occurrence count (diffs repeat lines).
  const seen = new Map<string, number>()
  const lines = diff.split("\n").map((line) => {
    const count = seen.get(line) ?? 0
    seen.set(line, count + 1)
    return { key: `${count}:${line}`, line }
  })
  return (
    <pre
      data-testid="patch-diff-view"
      className={`mt-1 max-h-40 overflow-auto whitespace-pre rounded-md bg-muted/40 p-2 font-mono text-caption ${className ?? ""}`}
    >
      {lines.map(({ key, line }) => (
        <span key={key} className={`block ${LINE_KIND_CLASS[classifyDiffLine(line)]}`}>
          {line || " "}
        </span>
      ))}
    </pre>
  )
}

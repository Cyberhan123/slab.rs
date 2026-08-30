/**
 * One-line summaries for tool calls, in the style users scan in a terminal:
 *
 * ```
 * Read: src/main.rs
 * Write: packages/ui/sender.tsx
 * Bash: ls -a
 * Grep: TODO
 * ```
 *
 * The collapsed tool row always shows `label: detail` so the basic information
 * stays visible even when the row is folded (the compact "thinking-style"
 * layout). Labels are tool identities and are deliberately NOT translated —
 * only the surrounding UI chrome is localized.
 */

/** The visible pieces of a collapsed tool row: `label: detail`. */
export type ToolSummary = {
  label: string
  detail: string
}

/** Max characters of the detail before middle-truncating with an ellipsis. */
const DETAIL_LIMIT = 80

/** Read a non-empty string field off an arguments object. */
function field(input: unknown, key: string): string | undefined {
  if (typeof input !== "object" || input === null) return undefined
  const value = (input as Record<string, unknown>)[key]
  return typeof value === "string" && value.length > 0 ? value : undefined
}

/** The first string-valued field of an arguments object, for the fallback. */
function firstStringField(input: unknown): string | undefined {
  if (typeof input !== "object" || input === null) return undefined
  for (const value of Object.values(input as Record<string, unknown>)) {
    if (typeof value === "string" && value.length > 0) return value
  }
  return undefined
}

/** Middle-truncate long details (paths/commands) keeping the tail visible. */
function truncateDetail(detail: string): string {
  if (detail.length <= DETAIL_LIMIT) return detail
  const head = Math.ceil((DETAIL_LIMIT - 1) / 2)
  const tail = Math.floor((DETAIL_LIMIT - 1) / 2)
  return `${detail.slice(0, head)}…${detail.slice(detail.length - tail)}`
}

interface FileChangeLike {
  path?: string
  type?: string
}

function fileChangeSummary(input: unknown): ToolSummary {
  const changes =
    (typeof input === "object" && input !== null
      ? (input as { changes?: unknown }).changes
      : undefined) ?? []
  const entries = Array.isArray(changes) ? (changes as FileChangeLike[]) : []
  if (entries.length !== 1) {
    return { label: "Patch", detail: entries.length > 1 ? `${entries.length} files` : "" }
  }
  const entry = entries[0] ?? {}
  const label = entry.type === "delete" ? "Delete" : "Write"
  return { label, detail: entry.path ?? "" }
}

/**
 * Summarize a tool call for its collapsed row. `toolName` is the harness tool
 * name (`commandExecution`, `read_file`, `git_status`, an MCP tool name, …);
 * `input` is the part's parsed arguments (or a tool-specific payload such as a
 * `fileChange` changes list). Unknown tools fall back to
 * `{toolName}: {first string argument}`.
 */
export function summarizeToolCall(toolName: string, input: unknown): ToolSummary {
  switch (toolName) {
    case "commandExecution":
      return { label: "Bash", detail: truncateDetail(field(input, "command") ?? "") }
    case "fileChange":
      return fileChangeSummary(input)
    case "webSearch":
    case "web_search":
    case "tool_search":
      return { label: "Search", detail: truncateDetail(field(input, "query") ?? "") }
    case "read_file":
      return { label: "Read", detail: truncateDetail(field(input, "path") ?? "") }
    case "write_file":
      return { label: "Write", detail: truncateDetail(field(input, "path") ?? "") }
    case "list_dir":
    case "fs_watch":
      return { label: toolName === "list_dir" ? "ListDir" : "Watch", detail: truncateDetail(field(input, "path") ?? "") }
    case "file_glob":
      return { label: "Glob", detail: truncateDetail(field(input, "pattern") ?? "") }
    case "grep":
      return { label: "Grep", detail: truncateDetail(field(input, "pattern") ?? "") }
    case "verify":
      return { label: "Verify", detail: truncateDetail(field(input, "command") ?? "") }
    case "delegate_subagent":
    case "task.complete":
      return {
        label: "Agent",
        detail: truncateDetail(
          field(input, "description") ?? field(input, "task") ?? field(input, "prompt") ?? "",
        ),
      }
    default:
      break
  }
  // `git_status` / `git_diff` / `git_commit` → `Git: status` / …
  if (toolName.startsWith("git_")) {
    return { label: "Git", detail: toolName.slice("git_".length) }
  }
  return { label: toolName, detail: truncateDetail(firstStringField(input) ?? "") }
}

import { describe, expect, it } from "vitest"

import { summarizeToolCall } from "../tool-summaries"

describe("summarizeToolCall", () => {
  it("summarizes the built-in tool calls with their primary argument", () => {
    expect(summarizeToolCall("commandExecution", { command: "ls -a", cwd: "/repo" })).toEqual({
      label: "Bash",
      detail: "ls -a",
    })
    expect(summarizeToolCall("read_file", { path: "src/main.rs" })).toEqual({
      label: "Read",
      detail: "src/main.rs",
    })
    expect(summarizeToolCall("write_file", { path: "a/b.txt", content: "x" })).toEqual({
      label: "Write",
      detail: "a/b.txt",
    })
    expect(summarizeToolCall("grep", { pattern: "TODO", path: "src" })).toEqual({
      label: "Grep",
      detail: "TODO",
    })
    expect(summarizeToolCall("file_glob", { pattern: "**/*.rs" })).toEqual({
      label: "Glob",
      detail: "**/*.rs",
    })
    expect(summarizeToolCall("list_dir", { path: "packages/ui" })).toEqual({
      label: "ListDir",
      detail: "packages/ui",
    })
    expect(summarizeToolCall("webSearch", { query: "rust async" })).toEqual({
      label: "Search",
      detail: "rust async",
    })
    expect(summarizeToolCall("verify", { command: "cargo test" })).toEqual({
      label: "Verify",
      detail: "cargo test",
    })
  })

  it("summarizes file changes as Write/Delete/Patch", () => {
    expect(
      summarizeToolCall("fileChange", { changes: [{ path: "a.txt", type: "edit", diff: "+x" }] }),
    ).toEqual({ label: "Write", detail: "a.txt" })
    expect(
      summarizeToolCall("fileChange", { changes: [{ path: "b.txt", type: "delete" }] }),
    ).toEqual({ label: "Delete", detail: "b.txt" })
    expect(
      summarizeToolCall("fileChange", {
        changes: [
          { path: "a.txt", type: "edit" },
          { path: "b.txt", type: "add" },
        ],
      }),
    ).toEqual({ label: "Patch", detail: "2 files" })
  })

  it("maps git tools to a Git label with the subcommand", () => {
    expect(summarizeToolCall("git_status", {})).toEqual({ label: "Git", detail: "status" })
    expect(summarizeToolCall("git_commit", { message: "x" })).toEqual({
      label: "Git",
      detail: "commit",
    })
  })

  it("falls back to the tool name plus the first string argument", () => {
    expect(summarizeToolCall("some_plugin_tool", { target: "zzz", n: 1 })).toEqual({
      label: "some_plugin_tool",
      detail: "zzz",
    })
    expect(summarizeToolCall("mystery", {})).toEqual({ label: "mystery", detail: "" })
    expect(summarizeToolCall("mystery", undefined)).toEqual({ label: "mystery", detail: "" })
  })

  it("middle-truncates long details while keeping the tail visible", () => {
    const long = `packages/slab-ui/src/pages/assistant/components/${"x".repeat(100)}/deeply/nested/file.tsx`
    const summary = summarizeToolCall("read_file", { path: long })
    expect(summary.detail.length).toBeLessThanOrEqual(81)
    expect(summary.detail.startsWith("packages/")).toBe(true)
    expect(summary.detail.endsWith("nested/file.tsx")).toBe(true)
  })
})

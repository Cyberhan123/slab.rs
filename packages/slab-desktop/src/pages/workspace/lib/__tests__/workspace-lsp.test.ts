import { describe, expect, it } from "vitest"

import { supportsWorkspaceLsp, workspaceLspModelPath } from "../workspace-lsp"

describe("workspace LSP helpers", () => {
  it("matches supported language providers", () => {
    expect(supportsWorkspaceLsp("typescript")).toBe(true)
    expect(supportsWorkspaceLsp("python")).toBe(true)
    expect(supportsWorkspaceLsp("markdown")).toBe(false)
    expect(supportsWorkspaceLsp("plaintext")).toBe(false)
  })

  it("builds file uri model paths for Monaco", () => {
    expect(workspaceLspModelPath("C:\\Users\\demo\\repo", "src/index.ts")).toBe(
      "file:///C:/Users/demo/repo/src/index.ts",
    )
  })
})

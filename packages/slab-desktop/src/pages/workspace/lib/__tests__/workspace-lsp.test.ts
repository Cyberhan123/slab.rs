import { describe, expect, it } from "vitest"

import {
  supportsWorkspaceLsp,
  workspaceLspModelPath,
  workspaceLspRelativePathFromUri,
} from "../workspace-lsp"

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

  it("maps workspace file uris back to relative paths", () => {
    expect(
      workspaceLspRelativePathFromUri(
        "C:\\Users\\demo\\repo",
        "file:///C:/Users/demo/repo/src/index.ts",
      ),
    ).toBe("src/index.ts")
  })

  it("decodes escaped file uri paths", () => {
    expect(
      workspaceLspRelativePathFromUri(
        "C:\\Users\\demo\\repo",
        "file:///C:/Users/demo/repo/src/my%20file.ts",
      ),
    ).toBe("src/my file.ts")
  })

  it("rejects file uris outside the workspace", () => {
    expect(
      workspaceLspRelativePathFromUri(
        "C:\\Users\\demo\\repo",
        "file:///C:/Users/demo/other/src/index.ts",
      ),
    ).toBeNull()
  })
})
